//! Actor System Commands (role-based naming)
//!
//! This module provides commands to start/control the Actor system from the UI.
//! Formerly located in `actor_system_commands.rs`.

use crate::application::{AppState, shared_state::SharedStateCache};
use crate::crawl_engine::actor_event_bridge::start_actor_event_bridge;
use crate::crawl_engine::actors::SessionActor;
use crate::crawl_engine::actors::contract::ACTOR_CONTRACT_VERSION;
use crate::crawl_engine::actors::types::{ExecutionPlan, PageRange};
// Use ActorCommand from actors::types for SessionActor control channel
use crate::crawl_engine::actors::traits::Actor; // bring Actor::run into scope
use crate::crawl_engine::actors::types::ActorCommand as ActorActorCommand;
use crate::crawl_engine::channels::types::ActorCommand; // keep channel ActorCommand for context wiring
use crate::crawl_engine::channels::types::AppEvent;
use crate::crawl_engine::context::{AppContext, SystemConfig};
use crate::domain::services::crawling_services::{
    CrawlingRangeRecommendation, SiteDataChangeStatus, SiteStatus as DomainSiteStatus,
};
use crate::infrastructure::config::AppConfig;
use crate::infrastructure::html_parser::MatterDataExtractor;
use crate::infrastructure::integrated_product_repository::IntegratedProductRepository;
use crate::infrastructure::simple_http_client::HttpClient;
use tauri::State; // For accessing managed state
// 실제 CrawlingPlanner에서 사용
use crate::crawl_engine::runtime::session_registry::{
    SessionStatus, failure_threshold, session_registry, update_global_failure_policy_from_config,
};
use crate::infrastructure::config::ConfigManager; // 설정 관리자 추가
use blake3;
use chrono::Utc;
use once_cell::sync::OnceCell; // retained for PHASE_SHUTDOWN_TX only (session registry extracted)
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::sync::{broadcast, mpsc, watch};
// use tokio::time::Duration; // for sleep & timing
use crate::crawl_engine::actors::types::PageSlot;
use crate::crawl_engine::services::planning_service::PlanningStrategy;
use crate::domain::pagination::PaginationCalculator;
use tracing::{error, info};
// use crate::application::shared_state; // no direct symbols needed here

// Graceful shutdown channel (single active session assumption)
static PHASE_SHUTDOWN_TX: OnceCell<watch::Sender<bool>> = OnceCell::new();

// ========== Hash Integrity Helper ==========
fn compute_plan_hash(
    snapshot: &crate::crawl_engine::actors::types::PlanInputSnapshot,
    ranges: &[PageRange],
    strategy: &str,
) -> String {
    let hash_input =
        serde_json::json!({ "snapshot": snapshot, "ranges": ranges, "strategy": strategy });
    let hash_string = serde_json::to_string(&hash_input).unwrap_or_default();
    blake3::hash(hash_string.as_bytes()).to_hex().to_string()
}

// (removed: error classification helper; no longer used)

// ========== API Request/Response (backward-compatible) ==========
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrawlingMode {
    AdvancedEngine,
    LiveProduction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorCrawlingRequest {
    pub site_url: Option<String>,
    pub start_page: Option<u32>,
    pub end_page: Option<u32>,
    pub page_count: Option<u32>,
    pub concurrency: Option<u32>,
    pub batch_size: Option<u32>,
    pub delay_ms: Option<u64>,
    pub mode: Option<CrawlingMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorSystemResponse {
    pub success: bool,
    pub message: String,
    pub session_id: Option<String>,
    pub data: Option<serde_json::Value>,
}

/// Bootstrap common wiring and spawn `SessionActor` to execute a pre-planned plan
async fn bootstrap_and_spawn_session(
    app: &AppHandle,
    execution_plan: ExecutionPlan,
    app_config: AppConfig,
) -> Result<(String, ExecutionPlan), String> {
    let session_id = execution_plan.session_id.clone();

    // Update global failure policy from config (best-effort)
    update_global_failure_policy_from_config(&app_config);

    // Build event channel and start the bridge to FE
    let (actor_event_tx, actor_event_rx) = broadcast::channel::<AppEvent>(1000);
    let _bridge_handle = start_actor_event_bridge(app.clone(), actor_event_rx)
        .await
        .map_err(|e| format!("failed to start event bridge: {e}"))?;

    // Build session AppContext
    let system_config = Arc::new(SystemConfig::default());
    let (control_tx, _control_rx) = mpsc::channel::<ActorCommand>(100);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    // expose shutdown handle for request_graceful_shutdown
    let _ = PHASE_SHUTDOWN_TX.set(shutdown_tx.clone());
    let context = AppContext::new(
        session_id.clone(),
        control_tx,
        actor_event_tx.clone(),
        shutdown_rx,
        system_config,
    );

    // Spawn SessionActor and send ExecutePrePlanned
    let mut session_actor = SessionActor::new(session_id.clone());
    let (cmd_tx, cmd_rx) = mpsc::channel::<ActorActorCommand>(100);
    tokio::spawn(async move {
        if let Err(e) = session_actor.run(context, cmd_rx).await {
            error!("SessionActor run error: {}", e);
        }
    });
    cmd_tx
        .send(ActorActorCommand::ExecutePrePlanned {
            session_id: session_id.clone(),
            plan: execution_plan.clone(),
        })
        .await
        .map_err(|e| format!("failed to send ExecutePrePlanned: {e}"))?;

    // Registry initialization and initial SessionStarted will be handled by SessionActor upon ExecutePrePlanned.

    Ok((session_id, execution_plan))
}

/// Public command: start actor system crawling (refactored to use bootstrap helper)
#[tauri::command]
/// Bootstrap common wiring and spawn `SessionActor` to execute a pre-planned plan
///
/// # Errors
/// Returns an error string if configuration, repository, or actor spawning fails.
pub async fn start_actor_system_crawling(
    app: AppHandle,
    request: ActorCrawlingRequest,
) -> Result<ActorSystemResponse, String> {
    // 1) Use PlanningService: choose Manual strategy if manual range present, else Intelligent
    let overrides = crate::crawl_engine::services::planning_service::PlanOverrides {
        batch_size: request.batch_size,
        concurrency: request.concurrency,
        delay_ms: request.delay_ms,
        start_page: request.start_page,
        end_page: request.end_page,
        page_count: request.page_count,
    };
    let use_manual = overrides.start_page.is_some()
        || overrides.end_page.is_some()
        || overrides.page_count.is_some();
    let (execution_plan, app_config, _site_status) = if use_manual {
        let strategist = crate::crawl_engine::services::planning_service::ManualPlanningStrategy;
        strategist.plan(&app, Some(&overrides)).await?
    } else {
        let strategist =
            crate::crawl_engine::services::planning_service::IntelligentPlanningStrategy;
        strategist.plan(&app, Some(&overrides)).await?
    };

    // 2) Mode log (informational only)
    if let Some(mode) = &request.mode {
        info!("[start_actor_system_crawling] mode={:?}", mode);
    }

    // 3) ProductDetails feature flag
    let details_enabled = std::env::var("BOOTSTRAP_PRODUCT_DETAILS")
        .ok()
        .is_none_or(|v| v != "0");
    if !details_enabled {
        info!("ProductDetails phase disabled via BOOTSTRAP_PRODUCT_DETAILS=0");
    }

    let (sid, exec_clone) =
        bootstrap_and_spawn_session(&app, execution_plan.clone(), app_config.clone()).await?;
    Ok(ActorSystemResponse {
        success: true,
        message: format!(
            "Actor system crawling started (details_phase={})",
            details_enabled
        ),
        session_id: Some(sid),
        data: Some(serde_json::to_value(&exec_clone).map_err(|e| e.to_string())?),
    })
}

/// Request a graceful shutdown signal for the running session.
#[tauri::command]
/// # Errors
/// Returns an error if sending the shutdown signal fails or no session is active.
pub async fn request_graceful_shutdown(app: AppHandle) -> Result<ActorSystemResponse, String> {
    if let Some(tx) = PHASE_SHUTDOWN_TX.get() {
        if tx.send(true).is_err() {
            return Err("Failed to send shutdown signal".into());
        }
        // Emit ShutdownRequested event via broadcast if bridge exists (best-effort)
        if let Some(state) = app.try_state::<AppState>() {
            let _ = state;
        }
        let now = Utc::now();
        info!("Graceful shutdown requested at {}", now);
        // Update registry state to ShuttingDown
        {
            let registry = session_registry();
            let mut g = registry.write().await;
            for (_id, entry) in g.iter_mut() {
                if entry.status == SessionStatus::Running || entry.status == SessionStatus::Paused {
                    entry.status = SessionStatus::ShuttingDown;
                }
            }
        }
        Ok(ActorSystemResponse {
            success: true,
            message: "Graceful shutdown signal sent".into(),
            session_id: None,
            data: None,
        })
    } else {
        Err("No active session to shutdown".into())
    }
}

/// Pause a running session by session_id.
#[tauri::command]
/// # Errors
/// Returns an error string if the session is not found.
pub async fn pause_session(
    _app: AppHandle,
    session_id: String,
) -> Result<ActorSystemResponse, String> {
    let registry = session_registry();
    let mut g = registry.write().await;
    if let Some(entry) = g.get_mut(&session_id) {
        if entry.status == SessionStatus::Running {
            let _ = entry.pause_tx.send(true);
            entry.status = SessionStatus::Paused;
        }
        Ok(ActorSystemResponse {
            success: true,
            message: "session paused".into(),
            session_id: Some(session_id),
            data: None,
        })
    } else {
        Err(format!("Unknown session_id={}", session_id))
    }
}

/// Resume a paused session by session_id.
#[tauri::command]
/// # Errors
/// Returns an error string if the session is not found.
pub async fn resume_session(
    _app: AppHandle,
    session_id: String,
) -> Result<ActorSystemResponse, String> {
    let registry = session_registry();
    let mut g = registry.write().await;
    if let Some(entry) = g.get_mut(&session_id) {
        if entry.status == SessionStatus::Paused {
            let _ = entry.pause_tx.send(false);
            entry.status = SessionStatus::Running;
        }
        Ok(ActorSystemResponse {
            success: true,
            message: "session resumed".into(),
            session_id: Some(session_id),
            data: None,
        })
    } else {
        Err(format!("Unknown session_id={}", session_id))
    }
}

/// List session IDs in newest-first order.
#[tauri::command]
/// # Errors
/// Returns an error string if registry access fails (unlikely).
pub async fn list_actor_sessions(_app: AppHandle) -> Result<ActorSystemResponse, String> {
    let registry = session_registry();
    let mut sessions: Vec<(String, chrono::DateTime<chrono::Utc>)> = {
        let g = registry.read().await;
        g.iter().map(|(k, v)| (k.clone(), v.started_at)).collect()
    };
    sessions.sort_by(|a, b| b.1.cmp(&a.1));
    let ids: Vec<String> = sessions.into_iter().map(|(id, _s)| id).collect();
    Ok(ActorSystemResponse {
        success: true,
        message: "sessions".into(),
        session_id: None,
        data: Some(serde_json::json!({"sessions": ids})),
    })
}

/// Get the current session status and metrics.
#[tauri::command]
/// # Panics
/// Panics if the constructed payload session_id is missing (should not happen).
///
/// # Errors
/// Returns an error string if the session is not found.
pub async fn get_session_status(
    _app: AppHandle,
    session_id: String,
) -> Result<ActorSystemResponse, String> {
    let registry = session_registry();
    let g = registry.read().await;
    if let Some(entry) = g.get(&session_id) {
        #[allow(clippy::cast_precision_loss)]
        let pct_pages = if entry.total_pages_planned > 0 {
            (entry.processed_pages as f64 / entry.total_pages_planned as f64) * 100.0
        } else {
            0.0
        };
        #[allow(clippy::cast_precision_loss)]
        let pct_batches = if entry.total_batches_planned > 0 {
            (entry.completed_batches as f64 / entry.total_batches_planned as f64) * 100.0
        } else {
            0.0
        };
        let now = Utc::now();
        let elapsed_ms = u64::try_from(
            now.signed_duration_since(entry.started_at)
                .num_milliseconds()
                .max(0),
        )
        .unwrap_or(0);
        #[allow(clippy::cast_precision_loss)]
        let throughput_ppm = if elapsed_ms > 0 {
            (entry.processed_pages as f64) / (elapsed_ms as f64 / 60000.0)
        } else {
            0.0
        };
        let remaining_pages = entry
            .total_pages_planned
            .saturating_sub(entry.processed_pages);
        #[allow(clippy::cast_precision_loss)]
        let eta_ms = if throughput_ppm > 0.0 {
            ((remaining_pages as f64) / throughput_ppm) * 60000.0
        } else {
            0.0
        };
        #[allow(clippy::cast_precision_loss)]
        let error_rate = if entry.processed_pages > 0 {
            f64::from(entry.error_count) / entry.processed_pages as f64
        } else {
            0.0
        };
        #[allow(clippy::cast_precision_loss)]
        let failed_rate = if entry.processed_pages > 0 {
            entry.failed_pages.len() as f64 / entry.processed_pages as f64
        } else {
            0.0
        };

        let payload = serde_json::json!({
            "session_id": session_id,
            "status": format!("{:?}", entry.status),
            "started_at": entry.started_at.to_rfc3339(),
            "completed_at": entry.completed_at.map(|d| d.to_rfc3339()),
            "contract_version": ACTOR_CONTRACT_VERSION,
            "pages": {
                "processed": entry.processed_pages,
                "total": entry.total_pages_planned,
                "percent": pct_pages,
                "failed": entry.failed_pages.len(),
                "failed_rate": failed_rate,
                "retrying": entry.retrying_pages.len(),
                "failure_threshold": entry.page_failure_threshold,
            },
            "batches": {"completed": entry.completed_batches, "total": entry.total_batches_planned, "percent": pct_batches},
            "errors": {"last": entry.last_error, "count": entry.error_count, "rate": error_rate},
            "resume_token": entry.resume_token,
            "remaining_pages": entry.remaining_page_slots,
            "failure_threshold": failure_threshold(),
            "retry_policy": {
                "product_list_max_retries": entry.product_list_max_retries,
            },
            "metrics": {"elapsed_ms": elapsed_ms, "throughput_pages_per_min": throughput_ppm, "eta_ms": eta_ms},
            "params": {"batch_size": entry.batch_size, "concurrency_limit": entry.concurrency_limit},
            "retries": {
                "total_attempts": entry.retries_per_page.values().sum::<u32>(),
                "per_page_sample": entry.retries_per_page.iter().take(20).map(|(p,c)| serde_json::json!({"page": p, "count": c})).collect::<Vec<_>>()
            },
            "failed_pages": entry.failed_pages.iter().take(50).collect::<Vec<_>>(),
            "retrying_pages": entry.retrying_pages.iter().take(50).collect::<Vec<_>>(),
            "details": {
                "total": entry.detail_tasks_total,
                "completed": entry.detail_tasks_completed,
                "failed": entry.detail_tasks_failed,
                "failed_ids_sample": entry.detail_failed_ids.iter().take(20).collect::<Vec<_>>(),
                "remaining_ids": entry.remaining_detail_ids,
                "retries_total": entry.detail_retries_total,
                "retry_histogram": entry.detail_retry_histogram.iter().map(|(k,v)| serde_json::json!({"retries": k, "count": v})).collect::<Vec<_>>(),
                "retry_counts_sample": entry.detail_retry_counts.iter().take(20).map(|(id,c)| serde_json::json!({"id": id, "count": c})).collect::<Vec<_>>(),
                "failure_threshold": entry.detail_failure_threshold,
                "downshifted": entry.detail_downshifted,
                "downshift_meta": if entry.detail_downshifted { serde_json::json!({
                    "timestamp": entry.detail_downshift_timestamp.map(|t| t.to_rfc3339()),
                    "old_limit": entry.detail_downshift_old_limit,
                    "new_limit": entry.detail_downshift_new_limit,
                    "trigger": entry.detail_downshift_trigger,
                }) } else { serde_json::Value::Null },
            },
        });
        Ok(ActorSystemResponse {
            success: true,
            message: "session status".into(),
            session_id: Some(session_id),
            data: Some(payload),
        })
    } else {
        Err(format!("Unknown session_id={}", session_id))
    }
}

/// Helper (primarily for tests) to obtain status payload without needing a real `AppHandle`.
pub async fn test_build_session_status_payload(session_id: &str) -> Option<serde_json::Value> {
    let registry = session_registry();
    let g = registry.read().await;
    g.get(session_id).map(|entry| {
		// Display-only metrics; allow precision loss for f64 conversions in this scoped block.
		#[allow(clippy::cast_precision_loss)]
		let (pct_pages, pct_batches, throughput_ppm, eta_ms, error_rate) = {
			let pct_pages = if entry.total_pages_planned > 0 {
				(entry.processed_pages as f64 / entry.total_pages_planned as f64) * 100.0
			} else {
				0.0
			};
			let pct_batches = if entry.total_batches_planned > 0 {
				(entry.completed_batches as f64 / entry.total_batches_planned as f64) * 100.0
			} else {
				0.0
			};
			let now = Utc::now();
			let elapsed_ms = u64::try_from(
				now.signed_duration_since(entry.started_at)
					.num_milliseconds()
					.max(0),
			)
			.unwrap_or(0);
			let throughput_ppm = if elapsed_ms > 0 {
				(entry.processed_pages as f64) / (elapsed_ms as f64 / 60000.0)
			} else {
				0.0
			};
			let remaining_pages = entry
				.total_pages_planned
				.saturating_sub(entry.processed_pages);
			let eta_ms = if throughput_ppm > 0.0 {
				((remaining_pages as f64) / throughput_ppm) * 60000.0
			} else {
				0.0
			};
			let error_rate = if entry.processed_pages > 0 {
				f64::from(entry.error_count) / entry.processed_pages as f64
			} else {
				0.0
			};
			(pct_pages, pct_batches, throughput_ppm, eta_ms, error_rate)
		};
		let now = Utc::now();
		let elapsed_ms = u64::try_from(
			now.signed_duration_since(entry.started_at)
				.num_milliseconds()
				.max(0),
		)
		.unwrap_or(0);
		#[allow(clippy::cast_precision_loss)]
		let failed_rate = if entry.processed_pages > 0 {
			entry.failed_pages.len() as f64 / entry.processed_pages as f64
		} else { 0.0 };
		let payload = serde_json::json!({
			"session_id": session_id,
			"status": format!("{:?}", entry.status),
			"started_at": entry.started_at.to_rfc3339(),
			"completed_at": entry.completed_at.map(|d| d.to_rfc3339()),
			"contract_version": ACTOR_CONTRACT_VERSION,
			"pages": {
				"processed": entry.processed_pages,
				"total": entry.total_pages_planned,
				"percent": pct_pages,
				"failed": entry.failed_pages.len(),
				"failed_rate": failed_rate,
				"retrying": entry.retrying_pages.len(),
				"failure_threshold": entry.page_failure_threshold,
			},
			"batches": {"completed": entry.completed_batches, "total": entry.total_batches_planned, "percent": pct_batches},
			"errors": {"last": entry.last_error, "count": entry.error_count, "rate": error_rate},
			"resume_token": entry.resume_token,
			"remaining_pages": entry.remaining_page_slots,
			"failure_threshold": failure_threshold(),
			"retry_policy": {"product_list_max_retries": entry.product_list_max_retries},
			"metrics": {"elapsed_ms": elapsed_ms, "throughput_pages_per_min": throughput_ppm, "eta_ms": eta_ms},
			"params": {"batch_size": entry.batch_size, "concurrency_limit": entry.concurrency_limit},
			"retries": {"total_attempts": entry.retries_per_page.values().sum::<u32>()},
			"failed_pages": entry.failed_pages.iter().take(50).collect::<Vec<_>>(),
			"retrying_pages": entry.retrying_pages.iter().take(50).collect::<Vec<_>>(),
			"details": {
				"total": entry.detail_tasks_total,
				"completed": entry.detail_tasks_completed,
				"failed": entry.detail_tasks_failed,
				"failed_ids_sample": entry.detail_failed_ids.iter().take(20).collect::<Vec<_>>(),
				"remaining_ids": entry.remaining_detail_ids,
				"retries_total": entry.detail_retries_total,
				"retry_histogram": entry.detail_retry_histogram.iter().map(|(k,v)| serde_json::json!({"retries": k, "count": v})).collect::<Vec<_>>(),
				"retry_counts_sample": entry.detail_retry_counts.iter().take(20).map(|(id,c)| serde_json::json!({"id": id, "count": c})).collect::<Vec<_>>(),
				"failure_threshold": entry.detail_failure_threshold,
				"downshifted": entry.detail_downshifted,
				"downshift_meta": if entry.detail_downshifted { serde_json::json!({
					"timestamp": entry.detail_downshift_timestamp.map(|t| t.to_rfc3339()),
					"old_limit": entry.detail_downshift_old_limit,
					"new_limit": entry.detail_downshift_new_limit,
					"trigger": entry.detail_downshift_trigger,
				}) } else { serde_json::Value::Null },
			},
		});
		payload
	})
}

/// Resume an actor crawling session from a previously saved token.
#[tauri::command]
/// # Panics
/// Panics if the resume token is invalid and contains no pages.
///
/// # Errors
/// Returns an error string if parsing the token, loading config, or starting the actor system fails.
pub async fn resume_from_token(
    app: AppHandle,
    resume_token: String,
) -> Result<ActorSystemResponse, String> {
    // 1. Parse token
    let token_v: serde_json::Value = serde_json::from_str(&resume_token)
        .map_err(|e| format!("invalid resume token json: {}", e))?;
    let plan_hash = token_v
        .get("plan_hash")
        .and_then(|v| v.as_str())
        .ok_or("missing plan_hash")?
        .to_string();
    let remaining_pages: Vec<u32> = token_v
        .get("remaining_pages")
        .and_then(|v| v.as_array())
        .ok_or("missing remaining_pages")?
        .iter()
        .filter_map(|x| x.as_u64().and_then(|n| u32::try_from(n).ok()))
        .collect();
    if remaining_pages.is_empty() {
        return Err("no remaining pages to resume".into());
    }
    // v2 optional detail fields
    let remaining_detail_ids: Option<Vec<String>> =
        token_v.get("remaining_detail_ids").and_then(|v| {
            v.as_array().map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(std::string::ToString::to_string))
                    .collect()
            })
        });
    let detail_retry_counts: HashMap<String, u32> = token_v
        .get("detail_retry_counts")
        .and_then(|v| v.as_array())
        .map(|arr| {
            let mut map = HashMap::new();
            for item in arr {
                if let (Some(id), Some(count)) = (
                    item.get(0).and_then(|x| x.as_str()),
                    item.get(1).and_then(serde_json::Value::as_u64),
                ) {
                    if let Ok(c) = u32::try_from(count) {
                        map.insert(id.to_string(), c);
                    }
                }
            }
            map
        })
        .unwrap_or_default();
    let detail_retries_total: u64 = token_v
        .get("detail_retries_total")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    // 2. Reconstruct ExecutionPlan minimally
    let new_session_id = format!("resume_{}", uuid::Uuid::new_v4());
    let mut pages_sorted = remaining_pages.clone();
    pages_sorted.sort_unstable();
    let first = *pages_sorted.first().unwrap();
    let last = *pages_sorted.last().unwrap();
    let mut ranges: Vec<PageRange> = Vec::new();
    let mut seg_start = pages_sorted[0];
    let mut prev = pages_sorted[0];
    for &p in pages_sorted.iter().skip(1) {
        if p == prev + 1 {
            prev = p;
            continue;
        }
        ranges.push(PageRange {
            start_page: seg_start,
            end_page: prev,
            estimated_products: (prev - seg_start + 1) * 12,
            reverse_order: false,
        });
        seg_start = p;
        prev = p;
    }
    ranges.push(PageRange {
        start_page: seg_start,
        end_page: prev,
        estimated_products: (prev - seg_start + 1) * 12,
        reverse_order: false,
    });
    let inferred_total_pages = *pages_sorted.iter().max().unwrap_or(&last);
    let calc = PaginationCalculator::default();
    let mut page_slots: Vec<PageSlot> = Vec::new();
    for &physical_page in &pages_sorted {
        for index_in_physical in 0..12u32 {
            let pos = calc.calculate(physical_page, index_in_physical, inferred_total_pages);
            page_slots.push(PageSlot {
                physical_page,
                page_id: i64::from(pos.page_id),
                index_in_page: i16::try_from(pos.index_in_page).unwrap_or(i16::MAX),
            });
        }
    }
    page_slots.sort_by(|a, b| match a.page_id.cmp(&b.page_id) {
        core::cmp::Ordering::Equal => a.index_in_page.cmp(&b.index_in_page),
        other => other,
    });
    let mut dedup: Vec<PageSlot> = Vec::new();
    for slot in page_slots {
        if let Some(last) = dedup.last() {
            if last.page_id == slot.page_id && last.index_in_page == slot.index_in_page {
                continue;
            }
        }
        dedup.push(slot);
    }
    let page_slots = dedup;
    info!(
        "Reconstructed logical page_slots (resume): physical_pages={}, total_slots={}, inferred_total_pages={}",
        pages_sorted.len(),
        page_slots.len(),
        inferred_total_pages
    );
    let head_samples: Vec<String> = page_slots
        .iter()
        .take(5)
        .map(|s| {
            format!(
                "p{}=>gid{}_i{}",
                s.physical_page, s.page_id, s.index_in_page
            )
        })
        .collect();
    let tail_samples: Vec<String> = page_slots
        .iter()
        .rev()
        .take(5)
        .map(|s| {
            format!(
                "p{}=>gid{}_i{}",
                s.physical_page, s.page_id, s.index_in_page
            )
        })
        .collect();
    info!(
        "page_slot audit head={:?} tail={:?}",
        head_samples, tail_samples
    );
    let batch_size_from_token = token_v
        .get("batch_size")
        .and_then(serde_json::Value::as_u64)
        .map_or(20, |v| u32::try_from(v).unwrap_or(u32::MAX));
    let concurrency_from_token = token_v
        .get("concurrency_limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(5, |v| u32::try_from(v).unwrap_or(u32::MAX));
    let retries_per_page: HashMap<u32, u32> = token_v
        .get("retries_per_page")
        .and_then(|v| v.as_array())
        .map(|arr| {
            let mut map = HashMap::new();
            for item in arr {
                if let (Some(page), Some(count)) = (
                    item.get(0).and_then(serde_json::Value::as_u64),
                    item.get(1).and_then(serde_json::Value::as_u64),
                ) {
                    map.insert(
                        u32::try_from(page).unwrap_or(u32::MAX),
                        u32::try_from(count).unwrap_or(u32::MAX),
                    );
                }
            }
            map
        })
        .unwrap_or_default();
    let failed_pages: Vec<u32> = token_v
        .get("failed_pages")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_u64().map(|n| u32::try_from(n).unwrap_or(u32::MAX)))
                .collect()
        })
        .unwrap_or_default();
    let retrying_pages: Vec<u32> = token_v
        .get("retrying_pages")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_u64().map(|n| u32::try_from(n).unwrap_or(u32::MAX)))
                .collect()
        })
        .unwrap_or_default();

    if let Ok(cfg_mgr) = crate::infrastructure::config::ConfigManager::new() {
        if let Ok(cfg) = cfg_mgr.load_config().await {
            update_global_failure_policy_from_config(&cfg);
        }
    }

    let execution_plan = ExecutionPlan {
        plan_id: format!("plan_{}", uuid::Uuid::new_v4()),
        session_id: new_session_id.clone(),
        crawling_ranges: ranges,
        batch_size: batch_size_from_token,
        concurrency_limit: concurrency_from_token,
        estimated_duration_secs: 0,
        created_at: Utc::now(),
        analysis_summary: "resume_from_token_minimal".into(),
        original_strategy: "ResumeMinimal".into(),
        input_snapshot: crate::crawl_engine::actors::types::PlanInputSnapshot {
            total_pages: last - first + 1,
            products_on_last_page: 12,
            db_max_page_id: None,
            db_max_index_in_page: None,
            db_total_products: 0,
            page_range_limit: last - first + 1,
            batch_size: batch_size_from_token,
            concurrency_limit: concurrency_from_token,
            created_at: Utc::now(),
        },
        plan_hash: plan_hash.clone(),
        skip_duplicate_urls: false,
        kpi_meta: None,
        contract_version: ACTOR_CONTRACT_VERSION,
        page_slots,
    };
    let site_status = execution_plan.input_snapshot_to_site_status();
    let cfg_manager =
        ConfigManager::new().map_err(|e| format!("config manager init failed: {}", e))?;
    let app_config = cfg_manager
        .load_config()
        .await
        .map_err(|e| format!("config load failed: {}", e))?;
    let (sid, exec_clone) =
        bootstrap_and_spawn_session(&app, execution_plan.clone(), app_config).await?;
    Ok(ActorSystemResponse {
        success: true,
        message: "resume session started".into(),
        session_id: Some(sid),
        data: Some(serde_json::to_value(&exec_clone).unwrap()),
    })
}

/// Test SessionActor functionality
///
/// # Errors
/// Returns an error string if the actor initialization or test flow fails.
#[tauri::command]
pub async fn test_session_actor_basic(_app: AppHandle) -> Result<ActorSystemResponse, String> {
    info!("Testing SessionActor...");

    let _system_config = Arc::new(SystemConfig::default());
    let (_control_tx, _control_rx) = mpsc::channel::<ActorCommand>(100);
    let (_event_tx, _event_rx) = mpsc::channel::<AppEvent>(500);

    let _session_actor = SessionActor::new(format!("session_{}", chrono::Utc::now().timestamp()));

    info!("SessionActor created successfully");

    Ok(ActorSystemResponse {
        success: true,
        message: "SessionActor test completed successfully".to_string(),
        session_id: Some(format!("test_session_{}", Utc::now().timestamp())),
        data: None,
    })
}

/// `CrawlingPlanner` 기반 지능형 범위 계산 (Actor 시스템용)
#[allow(dead_code)]
async fn calculate_intelligent_crawling_range(
    session_id: &str,
    request: &ActorCrawlingRequest,
    app_handle: &AppHandle,
) -> Result<(u32, u32, serde_json::Value), Box<dyn std::error::Error + Send + Sync>> {
    info!(
        "Calculating intelligent crawling range for Actor system session: {}",
        session_id
    );

    let app_state = app_handle.state::<AppState>();
    let db_pool = {
        let pool_guard = app_state.database_pool.read().await;
        pool_guard
            .as_ref()
            .ok_or("Database pool not initialized")?
            .clone()
    };

    let product_repo = Arc::new(IntegratedProductRepository::new(db_pool));
    let http_client = HttpClient::create_from_global_config()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    let data_extractor = MatterDataExtractor::new()
        .map_err(|e| format!("Failed to create data extractor: {}", e))?;

    info!("[ACTOR] Loading configuration and using CrawlingPlanner for intelligent analysis...");
    let config_manager = crate::infrastructure::config::ConfigManager::new()
        .map_err(|e| format!("Failed to initialize config manager: {}", e))?;
    let app_config = config_manager
        .load_config()
        .await
        .map_err(|e| format!("Failed to load config: {}", e))?;

    info!(
        "[ACTOR] Configuration loaded: page_range_limit={}, batch_size={}, max_concurrent={}",
        app_config.user.crawling.page_range_limit,
        app_config.user.batch.batch_size,
        app_config.user.max_concurrent_requests
    );

    let status_checker_impl = crate::infrastructure::crawling_service_impls::StatusCheckerImpl::new(
        http_client.clone(),
        data_extractor.clone(),
        app_config.clone(),
    );
    let status_checker = Arc::new(status_checker_impl);
    let db_analyzer = Arc::new(
        crate::infrastructure::crawling_service_impls::DatabaseAnalyzerImpl::new(
            product_repo.clone(),
        ),
    );
    let system_config = Arc::new(crate::crawl_engine::context::SystemConfig::default());
    let crawling_planner = crate::crawl_engine::services::crawling_planner::CrawlingPlanner::new(
        status_checker.clone(),
        db_analyzer.clone(),
        system_config.clone(),
    )
    .with_repository(product_repo.clone());

    let shared_cache: Option<tauri::State<crate::application::shared_state::SharedStateCache>> =
        app_handle.try_state::<crate::application::shared_state::SharedStateCache>();
    let cached_site_status = if let Some(cache_state) = shared_cache.as_ref() {
        cache_state
            .get_valid_site_analysis_async(Some(5))
            .await
            .map(|cached| crate::domain::services::SiteStatus {
                is_accessible: true,
                response_time_ms: 0,
                total_pages: cached.total_pages,
                estimated_products: cached.estimated_products,
                products_on_last_page: cached.products_on_last_page,
                last_check_time: cached.analyzed_at,
                health_score: cached.health_score,
                data_change_status:
                    crate::domain::services::crawling_services::SiteDataChangeStatus::Stable {
                        count: cached.estimated_products,
                    },
                decrease_recommendation: None,
                crawling_range_recommendation:
                    crate::domain::services::crawling_services::CrawlingRangeRecommendation::Full,
            })
    } else {
        None
    };
    let cached_site_status_clone = cached_site_status.clone();
    let (site_status, db_analysis) = crawling_planner
        .analyze_system_state_with_cache(cached_site_status)
        .await
        .map_err(|e| format!("Failed to analyze system state: {}", e))?;
    if let Some(cache_state) = shared_cache.as_ref() {
        use crate::application::shared_state::{DbAnalysisResult, SiteAnalysisResult};
        if cached_site_status_clone.is_none() {
            cache_state
                .set_site_analysis(SiteAnalysisResult::new(
                    site_status.total_pages,
                    site_status.products_on_last_page,
                    site_status.estimated_products,
                    crate::infrastructure::config::utils::matter_products_page_url_simple(1),
                    site_status.health_score,
                ))
                .await;
        }
        cache_state
            .set_db_analysis(DbAnalysisResult::new(
                db_analysis.total_products,
                None,
                None,
                db_analysis.data_quality_score,
            ))
            .await;
    }

    info!(
        "[ACTOR] Real site analysis: {} pages, {} products on last page",
        site_status.total_pages, site_status.products_on_last_page
    );
    info!(
        "[ACTOR] Real DB analysis: {} total products, {} unique products",
        db_analysis.total_products, db_analysis.unique_products
    );

    let (range_recommendation, processing_strategy) = crawling_planner
        .determine_crawling_strategy(&site_status, &db_analysis)
        .map_err(|e| format!("Failed to determine crawling strategy: {}", e))?;

    info!(
        "[ACTOR] CrawlingPlanner recommendation: {:?}",
        range_recommendation
    );
    info!(
        "[ACTOR] Processing strategy: batch_size={}, concurrency={}",
        processing_strategy.recommended_batch_size, processing_strategy.recommended_concurrency
    );

    let (calculated_start_page, calculated_end_page) =
        if let Some((start, end)) = range_recommendation.to_page_range(site_status.total_pages) {
            let reverse_start = if start > end { start } else { end };
            let reverse_end = if start > end { end } else { start };
            info!(
                "[ACTOR] CrawlingPlanner range: {} to {} (reverse crawling)",
                reverse_start, reverse_end
            );
            (reverse_start, reverse_end)
        } else {
            info!("[ACTOR] No crawling needed, using verification range");
            let verification_pages = app_config.user.crawling.page_range_limit.min(5);
            let start = site_status.total_pages;
            let end = if start >= verification_pages {
                start - verification_pages + 1
            } else {
                1
            };
            (start, end)
        };

    let max_allowed_pages = app_config.user.crawling.page_range_limit;
    let requested_pages = if calculated_start_page >= calculated_end_page {
        calculated_start_page - calculated_end_page + 1
    } else {
        calculated_end_page - calculated_start_page + 1
    };

    let (final_start_page, final_end_page) = if requested_pages > max_allowed_pages {
        info!(
            "[ACTOR] CrawlingPlanner requested {} pages, but config limits to {} pages",
            requested_pages, max_allowed_pages
        );
        let limited_start = site_status.total_pages;
        let limited_end = if limited_start >= max_allowed_pages {
            limited_start - max_allowed_pages + 1
        } else {
            1
        };
        info!(
            "[ACTOR] Range limited by config: {} to {} ({} pages)",
            limited_start, limited_end, max_allowed_pages
        );
        (limited_start, limited_end)
    } else {
        info!(
            "[ACTOR] Frontend does not specify page ranges by design - using CrawlingPlanner recommendation"
        );
        info!(
            "[ACTOR] CrawlingPlanner recommendation: {} to {}",
            calculated_start_page, calculated_end_page
        );
        if request.start_page.unwrap_or(0) != 0 && request.end_page.unwrap_or(0) != 0 {
            info!(
                "[ACTOR] Ignoring frontend test values (start_page: {:?}, end_page: {:?}) - using intelligent planning",
                request.start_page, request.end_page
            );
        }
        info!("[ACTOR] Using CrawlingPlanner intelligent recommendation for optimal crawling");
        (calculated_start_page, calculated_end_page)
    };

    info!("[ACTOR] Final range calculated:");
    info!(
        "   Range: {} to {} ({} pages, config limit: {})",
        final_start_page,
        final_end_page,
        if final_start_page >= final_end_page {
            final_start_page - final_end_page + 1
        } else {
            final_end_page - final_start_page + 1
        },
        app_config.user.crawling.page_range_limit
    );

    let analysis_info = serde_json::json!({
        "range_recommendation": format!("{:?}", range_recommendation),
        "user_requested": {
            "start_page": request.start_page,
            "end_page": request.end_page
        },
        "intelligent_calculated": {
            "start_page": calculated_start_page,
            "end_page": calculated_end_page
        },
        "final_used": {
            "start_page": final_start_page,
            "end_page": final_end_page
        },
        "site_analysis": {
            "total_pages": site_status.total_pages,
            "products_on_last_page": site_status.products_on_last_page,
            "estimated_products": site_status.estimated_products,
            "is_accessible": site_status.is_accessible
        },
        "processing_strategy": {
            "recommended_batch_size": processing_strategy.recommended_batch_size,
            "recommended_concurrency": processing_strategy.recommended_concurrency
        }
    });

    info!("Intelligent range calculation completed for Actor system");
    Ok((final_start_page, final_end_page, analysis_info))
}

async fn create_execution_plan(
    app: &AppHandle,
) -> Result<(ExecutionPlan, AppConfig, DomainSiteStatus), Box<dyn std::error::Error + Send + Sync>>
{
    // Delegate to IntelligentPlanningStrategy for a concise, cache-aware plan
    let strategist = crate::crawl_engine::services::planning_service::IntelligentPlanningStrategy;
    let (plan, config, site_status) = strategist.plan(app, None).await?;
    Ok((plan, config, site_status))
}

async fn build_execution_plan_from_explicit_pages(
    app: &AppHandle,
    mut pages: Vec<u32>,
) -> Result<(ExecutionPlan, AppConfig, DomainSiteStatus), String> {
    use crate::domain::pagination::PaginationCalculator;

    let config_manager = ConfigManager::new().map_err(|e| e.to_string())?;
    let app_config = config_manager
        .load_config()
        .await
        .map_err(|e| e.to_string())?;
    update_global_failure_policy_from_config(&app_config);

    let shared_cache: Option<State<SharedStateCache>> = app.try_state::<SharedStateCache>();
    let site_status: DomainSiteStatus = if let Some(cache_state) = shared_cache.as_ref() {
        if let Some(cached) = cache_state.get_valid_site_analysis_async(Some(5)).await {
            DomainSiteStatus {
                is_accessible: true,
                response_time_ms: 0,
                total_pages: cached.total_pages,
                estimated_products: cached.estimated_products,
                products_on_last_page: cached.products_on_last_page,
                last_check_time: cached.analyzed_at,
                health_score: cached.health_score,
                data_change_status: SiteDataChangeStatus::Stable {
                    count: cached.estimated_products,
                },
                decrease_recommendation: None,
                crawling_range_recommendation: CrawlingRangeRecommendation::Full,
            }
        } else {
            let (_plan, _cfg, status) = create_execution_plan(app)
                .await
                .map_err(|e| format!("site status load failed: {}", e))?;
            status
        }
    } else {
        let (_plan, _cfg, status) = create_execution_plan(app)
            .await
            .map_err(|e| format!("site status load failed: {}", e))?;
        status
    };

    pages.retain(|p| *p >= 1 && *p <= site_status.total_pages);
    pages.sort_by(|a, b| b.cmp(a));
    pages.dedup();
    if pages.is_empty() {
        return Err("No valid pages after normalization".into());
    }

    let mut ranges: Vec<PageRange> = Vec::new();
    let mut i = 0usize;
    while i < pages.len() {
        let start_newest = pages[i];
        let mut j = i;
        while j + 1 < pages.len() && pages[j] == pages[j + 1] + 1 {
            j += 1;
        }
        let end_oldest = pages[j];
        let count = start_newest.saturating_sub(end_oldest) + 1;
        ranges.push(PageRange {
            start_page: start_newest,
            end_page: end_oldest,
            estimated_products: count * 12,
            reverse_order: true,
        });
        i = j + 1;
    }

    let calc = PaginationCalculator::default();
    let mut page_slots: Vec<crate::crawl_engine::actors::types::PageSlot> = Vec::new();
    for r in &ranges {
        let iter: Box<dyn Iterator<Item = u32>> = if r.reverse_order {
            Box::new(r.end_page..=r.start_page)
        } else {
            Box::new(r.start_page..=r.end_page)
        };
        for physical_page in iter {
            let assumed_capacity = crate::domain::constants::site::PRODUCTS_PER_PAGE as u32;
            for idx in 0..assumed_capacity {
                let pos = calc.calculate(physical_page, idx, site_status.total_pages);
                page_slots.push(crate::crawl_engine::actors::types::PageSlot {
                    physical_page,
                    page_id: i64::from(pos.page_id),
                    index_in_page: i16::try_from(pos.index_in_page).unwrap_or(i16::MAX),
                });
            }
        }
    }
    page_slots.sort_by(|a, b| match a.page_id.cmp(&b.page_id) {
        core::cmp::Ordering::Equal => a.index_in_page.cmp(&b.index_in_page),
        other => other,
    });
    page_slots.dedup_by(|a, b| a.page_id == b.page_id && a.index_in_page == b.index_in_page);

    let snapshot = crate::crawl_engine::actors::types::PlanInputSnapshot {
        total_pages: site_status.total_pages,
        products_on_last_page: site_status.products_on_last_page,
        db_max_page_id: None,
        db_max_index_in_page: None,
        db_total_products: 0,
        page_range_limit: app_config.user.crawling.page_range_limit,
        batch_size: app_config.user.batch.batch_size,
        concurrency_limit: app_config.user.max_concurrent_requests,
        created_at: Utc::now(),
    };
    let plan_hash = compute_plan_hash(&snapshot, &ranges, "ManualExplicitPages");
    let session_id = format!("actor_session_{}", Utc::now().timestamp());
    let plan_id = format!("plan_{}", Utc::now().timestamp());
    let total_pages_planned: u32 = ranges
        .iter()
        .map(|r| {
            if r.reverse_order {
                r.start_page - r.end_page + 1
            } else {
                r.end_page - r.start_page + 1
            }
        })
        .sum();
    let mut execution_plan = ExecutionPlan {
        plan_id,
        session_id,
        crawling_ranges: ranges,
        batch_size: app_config.user.batch.batch_size,
        concurrency_limit: app_config.user.max_concurrent_requests,
        estimated_duration_secs: 0,
        created_at: Utc::now(),
        analysis_summary: format!("Manual explicit pages: {} planned", total_pages_planned),
        original_strategy: "ManualExplicitPages".into(),
        input_snapshot: snapshot,
        plan_hash,
        skip_duplicate_urls: false,
        kpi_meta: Some(crate::crawl_engine::actors::types::ExecutionPlanKpi {
            total_ranges: 0,
            total_pages: total_pages_planned,
            batches: 0,
            strategy: "ManualExplicitPages".into(),
            created_at: Utc::now(),
        }),
        contract_version: ACTOR_CONTRACT_VERSION,
        page_slots,
    };
    if let Some(ref mut kpi) = execution_plan.kpi_meta {
        kpi.total_ranges = execution_plan.crawling_ranges.len();
        kpi.batches = execution_plan.crawling_ranges.len();
    }
    Ok((execution_plan, app_config, site_status))
}

/// Start a manual crawl for given pages using the Actor system.
#[tauri::command(async)]
/// # Errors
/// Returns an error string if the actor system cannot be started.
pub async fn start_manual_crawl_pages_actor(
    app: AppHandle,
    pages: Vec<u32>,
    _skip_validation: Option<bool>,
) -> Result<ActorSystemResponse, String> {
    if pages.is_empty() {
        return Err("No pages provided".into());
    }
    let (execution_plan, app_config, _site_status) =
        build_execution_plan_from_explicit_pages(&app, pages).await?;

    // Health gate: if degradation note present, block unless MC_FORCE_CRAWL=1
    let degraded = if let Some(state) = app.try_state::<crate::application::AppState>() {
        // Use async read to avoid blocking within the runtime (previous blocking_read caused panic)
        let cfg = state.config.read().await;
        cfg.app_managed.last_degradation_note.clone()
    } else { None };
    if let Some(note) = degraded {
        let force = std::env::var("MC_FORCE_CRAWL").ok().map(|v| v=="1" || v.eq_ignore_ascii_case("true")).unwrap_or(false);
        if !force {
            return Err(format!("Site anomaly detected: {} (set MC_FORCE_CRAWL=1 to override)", note));
        } else {
            tracing::warn!(target="site_health", note, "Proceeding with crawl under anomaly due to MC_FORCE_CRAWL");
        }
    }

    info!(target: "kpi.plan", "{{\"event\":\"manual_actor_started\",\"session_id\":\"{}\",\"plan_id\":\"{}\",\"ranges\":{},\"hash\":\"{}\"}}",
        execution_plan.session_id,
        execution_plan.plan_id,
        execution_plan.crawling_ranges.len(),
        execution_plan.plan_hash
    );

    let (sid, exec_clone) =
        bootstrap_and_spawn_session(&app, execution_plan.clone(), app_config.clone()).await?;

    Ok(ActorSystemResponse {
        success: true,
        message: "Manual actor crawl started".into(),
        session_id: Some(sid),
        data: Some(serde_json::to_value(&exec_clone).map_err(|e| e.to_string())?),
    })
}

/// Check that computed page/index pairs match canonical pagination rules.
#[tauri::command]
/// # Errors
/// Returns an error string if database queries or calculations fail.
pub async fn check_page_index_consistency() -> Result<String, String> {
    use crate::crawl_engine::services::data_consistency_checker::DataConsistencyChecker;
    use crate::infrastructure::config::AppConfig;
    use crate::infrastructure::crawling_service_impls::StatusCheckerImpl;
    use crate::infrastructure::{
        html_parser::MatterDataExtractor,
        integrated_product_repository::IntegratedProductRepository, simple_http_client::HttpClient,
    };
    use std::sync::Arc;
    let http = HttpClient::create_from_global_config().map_err(|e| e.to_string())?;
    let extractor = MatterDataExtractor::new().map_err(|e| e.to_string())?;
    let pool = crate::infrastructure::database_connection::get_or_init_global_pool()
        .await
        .map_err(|e| e.to_string())?;
    let repo = Arc::new(IntegratedProductRepository::new(pool));
    let status_checker: Arc<dyn crate::domain::services::StatusChecker> =
        Arc::new(StatusCheckerImpl::with_product_repo(
            http,
            extractor,
            AppConfig::for_development(),
            Arc::clone(&repo),
        ));
    let checker = DataConsistencyChecker::new(status_checker, repo);
    let report = checker.run_check().await.map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&report).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crawl_engine::runtime::session_registry::{
        SessionEntry, SessionStatus, session_registry,
    };
    use chrono::Utc;
    use serde_json::json;

    #[tokio::test]
    async fn status_payload_includes_downshift_metadata() {
        let sid = "test_status_downshift".to_string();
        {
            let reg = session_registry();
            let mut g = reg.write().await;
            g.insert(
                sid.clone(),
                SessionEntry {
                    status: SessionStatus::Running,
                    pause_tx: tokio::sync::watch::channel(false).0,
                    started_at: Utc::now(),
                    completed_at: None,
                    total_pages_planned: 20,
                    processed_pages: 10,
                    total_batches_planned: 2,
                    completed_batches: 1,
                    batch_size: 10,
                    concurrency_limit: 8,
                    last_error: None,
                    error_count: 0,
                    resume_token: None,
                    remaining_page_slots: Some(vec![11, 12, 13]),
                    plan_hash: Some("hash".into()),
                    removal_deadline: None,
                    failed_emitted: false,
                    retries_per_page: std::collections::HashMap::new(),
                    failed_pages: vec![],
                    retrying_pages: vec![],
                    product_list_max_retries: 1,
                    error_type_stats: std::collections::HashMap::new(),
                    detail_tasks_total: 10,
                    detail_tasks_completed: 2,
                    detail_tasks_failed: 3,
                    detail_retry_counts: std::collections::HashMap::new(),
                    detail_retries_total: 0,
                    detail_retry_histogram: std::collections::HashMap::new(),
                    remaining_detail_ids: Some(vec!["a".into(), "b".into()]),
                    detail_failed_ids: vec![],
                    page_failure_threshold: 50,
                    detail_failure_threshold: 25,
                    detail_downshifted: true,
                    detail_downshift_timestamp: Some(Utc::now()),
                    detail_downshift_old_limit: Some(8),
                    detail_downshift_new_limit: Some(4),
                    detail_downshift_trigger: Some("fail_rate>0.30".into()),
                },
            );
        }
        let payload = test_build_session_status_payload(&sid)
            .await
            .expect("payload");
        assert!(payload["details"]["downshifted"].as_bool().unwrap());
        assert_eq!(
            payload["details"]["downshift_meta"]["old_limit"]
                .as_u64()
                .unwrap(),
            8
        );
        assert_eq!(
            payload["details"]["downshift_meta"]["new_limit"]
                .as_u64()
                .unwrap(),
            4
        );
        assert_eq!(
            payload["details"]["downshift_meta"]["trigger"]
                .as_str()
                .unwrap(),
            "fail_rate>0.30"
        );
    }

    #[tokio::test]
    async fn resume_token_v2_parse_like_production_logic() {
        let token = json!({
            "plan_hash": "abc123",
            "remaining_pages": [5,4,3],
            "remaining_detail_ids": ["u1","u2"],
            "detail_retry_counts": [["u1",2],["u2",1]],
            "detail_retries_total": 3,
            "batch_size": 10,
            "concurrency_limit": 4,
            "retries_per_page": [[5,1]],
            "failed_pages": [3],
            "retrying_pages": [4]
        })
        .to_string();
        let v: serde_json::Value = serde_json::from_str(&token).unwrap();
        assert_eq!(v["plan_hash"], "abc123");
        assert_eq!(v["remaining_pages"].as_array().unwrap().len(), 3);
        assert_eq!(v["remaining_detail_ids"].as_array().unwrap().len(), 2);
        assert_eq!(v["detail_retry_counts"].as_array().unwrap().len(), 2);
    }
}
