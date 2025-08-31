//! Actor System Commands for Tauri Integration
//!
// Commands to test and use the Actor system from the UI

use crate::application::{AppState, shared_state::SharedStateCache};
use crate::crawl_engine::actor_event_bridge::start_actor_event_bridge;
use crate::crawl_engine::actors::SessionActor;
use crate::crawl_engine::actors::contract::ACTOR_CONTRACT_VERSION;
use crate::crawl_engine::actors::types::{
    CrawlingConfig, ExecutionPlan, PageRange,
};
// Use ActorCommand from actors::types for SessionActor control channel
use crate::crawl_engine::actors::types::ActorCommand as ActorActorCommand;
use crate::crawl_engine::channels::types::ActorCommand; // keep channel ActorCommand for context wiring
use crate::crawl_engine::channels::types::AppEvent;
use crate::crawl_engine::context::{AppContext, SystemConfig};
use crate::crawl_engine::actors::traits::Actor; // bring Actor::run into scope
use crate::domain::services::SiteStatus;
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
    SessionStatus, failure_threshold, session_registry,
    update_global_failure_policy_from_config,
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
use tracing::{error, info, warn};
use crate::crawl_engine::services::planning_service::PlanningStrategy;

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

/// Bootstrap common wiring and spawn SessionActor to execute a pre-planned plan
async fn bootstrap_and_spawn_session(
    app: &AppHandle,
    execution_plan: ExecutionPlan,
    app_config: AppConfig,
    site_status: SiteStatus,
    resume_token: Option<String>,
    retries_per_page: Option<HashMap<u32, u32>>,
    failed_pages: Option<Vec<u32>>,
    retrying_pages: Option<Vec<u32>>,
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
    let use_manual = overrides.start_page.is_some() || overrides.end_page.is_some() || overrides.page_count.is_some();
    let (execution_plan, app_config, _site_status) = if use_manual {
        let strategist = crate::crawl_engine::services::planning_service::ManualPlanningStrategy;
        strategist.plan(&app, Some(&overrides)).await?
    } else {
        let strategist = crate::crawl_engine::services::planning_service::IntelligentPlanningStrategy;
        strategist.plan(&app, Some(&overrides)).await?
    };

    // 2) Mode log (informational only)
    if let Some(mode) = &request.mode { info!("[start_actor_system_crawling] mode={:?}", mode); }

    // 3) ProductDetails feature flag
    let details_enabled = std::env::var("BOOTSTRAP_PRODUCT_DETAILS").ok().is_none_or(|v| v != "0");
    if !details_enabled { info!("🔧 ProductDetails phase disabled via BOOTSTRAP_PRODUCT_DETAILS=0"); }

    let site_status = execution_plan.input_snapshot_to_site_status();
    let (sid, exec_clone) = bootstrap_and_spawn_session(
        &app,
        execution_plan.clone(),
        app_config.clone(),
        site_status,
        None,
        None,
        None,
        None,
    )
    .await?;
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

/// 요청: 현재 실행 중인 세션에 Graceful Shutdown 신호 전송
#[tauri::command]
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
    // We don't hold a broadcast handle here; Session loop will emit shutdown/session completion events
        info!("🛑 Graceful shutdown requested at {}", now);
        // 레지스트리 상태 ShuttingDown 으로 변경
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

/// 실행 중인 세션을 일시정지 (상태: Running -> Paused)
#[tauri::command]
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

/// 일시정지된 세션 재개 (상태: Paused -> Running)
#[tauri::command]
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

/// 현재 레지스트리에 존재하는 세션 ID 목록 (신규 -> 오래된 순 정렬)
#[tauri::command]
pub async fn list_actor_sessions(_app: AppHandle) -> Result<ActorSystemResponse, String> {
    let registry = session_registry();
    let g = registry.read().await;
    let mut sessions: Vec<(String, chrono::DateTime<chrono::Utc>)> =
        g.iter().map(|(k, v)| (k.clone(), v.started_at)).collect();
    sessions.sort_by(|a, b| b.1.cmp(&a.1));
    let ids: Vec<String> = sessions.into_iter().map(|(id, _s)| id).collect();
    Ok(ActorSystemResponse {
        success: true,
        message: "sessions".into(),
        session_id: None,
        data: Some(serde_json::json!({"sessions": ids})),
    })
}

/// 세션 상태 조회 (Running / Paused / Completed 등)
#[tauri::command]
pub async fn get_session_status(
    _app: AppHandle,
    session_id: String,
) -> Result<ActorSystemResponse, String> {
    let registry = session_registry();
    let g = registry.read().await;
    if let Some(entry) = g.get(&session_id) {
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
        let elapsed_ms = now
            .signed_duration_since(entry.started_at)
            .num_milliseconds()
            .max(0) as u64;
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
                "failed_rate": if entry.processed_pages>0 { entry.failed_pages.len() as f64 / entry.processed_pages as f64 } else { 0.0 },
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
            session_id: Some(payload["session_id"].as_str().unwrap().to_string()),
            data: Some(payload),
        })
    } else {
        Err(format!("Unknown session_id={}", session_id))
    }
}

// Helper (primarily for tests) to obtain status payload without needing a real AppHandle.
pub async fn test_build_session_status_payload(session_id: &str) -> Option<serde_json::Value> {
    let registry = session_registry();
    let g = registry.read().await;
    if let Some(entry) = g.get(session_id) {
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
        let elapsed_ms = now
            .signed_duration_since(entry.started_at)
            .num_milliseconds()
            .max(0) as u64;
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
                "failed_rate": if entry.processed_pages>0 { entry.failed_pages.len() as f64 / entry.processed_pages as f64 } else { 0.0 },
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
        Some(payload)
    } else {
        None
    }
}

/// 재시작 토큰을 이용해 새로운 세션을 생성 (v1 최소 구현)
/// 정책:
/// - 기존 session_id 와 다른 새로운 session_id 부여 (UUID 기반)
/// - resume_token 은 JSON: { plan_hash, remaining_pages[], generated_at, processed_pages, total_pages }
/// - plan_hash 무결성: 신규 ExecutionPlan 생성 후 해시 일치 여부 검사 (현재는 입력 토큰의 plan_hash 를 그대로 복제하여 Skip, Phase3에서 실제 재계산)
#[tauri::command]
pub async fn resume_from_token(
    app: AppHandle,
    resume_token: String,
) -> Result<ActorSystemResponse, String> {
    // 1. 토큰 파싱
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
        .filter_map(|x| x.as_u64().map(|n| n as u32))
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
                    map.insert(id.to_string(), count as u32);
                }
            }
            map
        })
        .unwrap_or_default();
    let detail_retries_total: u64 = token_v
        .get("detail_retries_total")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    // 2. 간단한 ExecutionPlan 재구성 (Phase3에서 CrawlingPlanner 부분 재사용으로 대체 예정)
    use crate::crawl_engine::actors::types::{ExecutionPlan, PageRange, PageSlot};
    let new_session_id = format!("resume_{}", uuid::Uuid::new_v4());
    // 단순화: remaining_pages 를 연속 구간으로 그룹핑 (현재는 페이지 정렬 후 하나의 range 로 묶음)
    let mut pages_sorted = remaining_pages.clone();
    pages_sorted.sort_unstable();
    let first = *pages_sorted.first().unwrap();
    let last = *pages_sorted.last().unwrap();
    // 연속 구간 그룹핑
    let mut ranges: Vec<PageRange> = Vec::new();
    let mut seg_start = pages_sorted[0];
    let mut prev = pages_sorted[0];
    for &p in pages_sorted.iter().skip(1) {
        if p == prev + 1 {
            prev = p;
            continue;
        }
        // 구간 종료
        ranges.push(PageRange {
            start_page: seg_start,
            end_page: prev,
            estimated_products: (prev - seg_start + 1) * 12,
            reverse_order: false,
        });
        seg_start = p;
        prev = p;
    }
    // 마지막 구간 push
    ranges.push(PageRange {
        start_page: seg_start,
        end_page: prev,
        estimated_products: (prev - seg_start + 1) * 12,
        reverse_order: false,
    });
    // page_slots 재구성 (논리 page_id/index_in_page canonical 계산)
    // 가정: remaining_pages 는 물리 페이지 번호 (사이트 기준, 1 = 최신, total_pages = 가장 오래된) 혹은 역순 포함 혼재 가능.
    // 1) total_pages 추정: 토큰 내 first/last 로는 불충분하므로 입력 remaining_pages 중 최대값을 total_pages 후보로 사용.
    //    (Phase4: 토큰에 total_pages 명시 필드 추가 고려)
    let inferred_total_pages = *pages_sorted.iter().max().unwrap_or(&last);
    // 2) 각 물리 페이지의 제품 슬롯을 보수적으로 0..(products_per_page-1) 로 가정하되 실제 마지막(오래된) 페이지 용량은 알 수 없어 full 로 가정.
    //    재개 시 정확성보다 안정적 page_id 재현성이 더 중요: old mapping 과의 drift 는 이후 PersistenceAnomaly 로 감지.
    use crate::domain::pagination::PaginationCalculator;
    let calc = PaginationCalculator::default();
    let mut page_slots: Vec<PageSlot> = Vec::new();
    for &physical_page in &pages_sorted {
        // 기본 12개 슬롯 가정 (Phase3: 토큰 전달 값 또는 site status snapshot 연동)
        for index_in_physical in 0..12u32 {
            // PRODUCTS_PER_PAGE 상수와 동기화 필요
            let pos = calc.calculate(physical_page, index_in_physical, inferred_total_pages);
            page_slots.push(PageSlot {
                physical_page,
                page_id: i64::from(pos.page_id),
                index_in_page: pos.index_in_page as i16,
            });
        }
    }
    // 3) page_slots 안정화: (page_id, index_in_page) 기준 정렬 후 중복 제거 (같은 물리 페이지에서 full 가정으로 생긴 초과 슬롯 필터링은 아직 수행하지 않음)
    page_slots.sort_by(|a, b| match a.page_id.cmp(&b.page_id) {
        core::cmp::Ordering::Equal => a.index_in_page.cmp(&b.index_in_page),
        other => other,
    });
    // Deduplicate identical logical slots keeping earliest physical_page (min)
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
        "♻️ Reconstructed logical page_slots (resume): physical_pages={}, total_slots={}, inferred_total_pages={}",
        pages_sorted.len(),
        page_slots.len(),
        inferred_total_pages
    );
    // 샘플 감사 로그 (앞/뒤 5개)
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
        "🔍 page_slot audit head={:?} tail={:?}",
        head_samples, tail_samples
    );
    let batch_size_from_token = token_v
        .get("batch_size")
        .and_then(serde_json::Value::as_u64)
        .map_or(20, |v| v as u32);
    let concurrency_from_token = token_v
        .get("concurrency_limit")
        .and_then(serde_json::Value::as_u64)
        .map_or(5, |v| v as u32);
    // Retry state parsing (v1 token extensions)
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
                    map.insert(page as u32, count as u32);
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
                .filter_map(|x| x.as_u64().map(|n| n as u32))
                .collect()
        })
        .unwrap_or_default();
    let retrying_pages: Vec<u32> = token_v
        .get("retrying_pages")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_u64().map(|n| n as u32))
                .collect()
        })
        .unwrap_or_default();

    // Config load (to refresh failure policy cache)
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
    // 3. 공통 부트스트랩 경로를 사용해 SessionActor로 실행
    let site_status = execution_plan.input_snapshot_to_site_status();
    let cfg_manager =
        ConfigManager::new().map_err(|e| format!("config manager init failed: {}", e))?;
    let app_config = cfg_manager
        .load_config()
        .await
        .map_err(|e| format!("config load failed: {}", e))?;
    let (sid, exec_clone) = bootstrap_and_spawn_session(
        &app,
        execution_plan.clone(),
        app_config,
        site_status,
        Some(resume_token.clone()),
        Some(retries_per_page),
        Some(failed_pages),
        Some(retrying_pages),
    )
    .await?;
    Ok(ActorSystemResponse {
        success: true,
        message: "resume session started".into(),
        session_id: Some(sid),
        data: Some(serde_json::to_value(&exec_clone).unwrap()),
    })
}

// (Duplicate placeholder block removed)

// (Removed deprecated ServiceBasedBatchCrawlingEngine command block)

/// Test SessionActor functionality
#[tauri::command]
pub async fn test_session_actor_basic(_app: AppHandle) -> Result<ActorSystemResponse, String> {
    info!("🧪 Testing SessionActor...");

    let _system_config = Arc::new(SystemConfig::default());
    let (_control_tx, _control_rx) = mpsc::channel::<ActorCommand>(100);
    let (_event_tx, _event_rx) = mpsc::channel::<AppEvent>(500);

    let _session_actor = SessionActor::new(format!("session_{}", chrono::Utc::now().timestamp()));

    info!("✅ SessionActor created successfully");

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
        "🧠 Calculating intelligent crawling range for Actor system session: {}",
        session_id
    );

    // 앱 상태에서 데이터베이스 풀 가져오기
    let app_state = app_handle.state::<AppState>();
    let db_pool = {
        let pool_guard = app_state.database_pool.read().await;
        pool_guard
            .as_ref()
            .ok_or("Database pool not initialized")?
            .clone()
    };

    // IntegratedProductRepository 생성
    let product_repo = Arc::new(IntegratedProductRepository::new(db_pool));

    // HTTP 클라이언트 생성
    let http_client = HttpClient::create_from_global_config()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // 데이터 추출기 생성
    let data_extractor = MatterDataExtractor::new()
        .map_err(|e| format!("Failed to create data extractor: {}", e))?;

    // 🧠 실제 설정 파일 로드 및 CrawlingPlanner 사용
    info!("🧠 [ACTOR] Loading configuration and using CrawlingPlanner for intelligent analysis...");

    // 실제 앱 설정 로드 (기본값 대신)
    let config_manager = crate::infrastructure::config::ConfigManager::new()
        .map_err(|e| format!("Failed to initialize config manager: {}", e))?;
    let app_config = config_manager
        .load_config()
        .await
        .map_err(|e| format!("Failed to load config: {}", e))?;

    info!(
        "📋 [ACTOR] Configuration loaded: page_range_limit={}, batch_size={}, max_concurrent={}",
        app_config.user.crawling.page_range_limit,
        app_config.user.batch.batch_size,
        app_config.user.max_concurrent_requests
    );

    // StatusChecker 생성 (실제 설정 사용)
    let status_checker_impl = crate::infrastructure::crawling_service_impls::StatusCheckerImpl::new(
        http_client.clone(),
        data_extractor.clone(),
        app_config.clone(),
    );
    let status_checker = Arc::new(status_checker_impl);

    // DatabaseAnalyzer 생성 (실제 DB 분석)
    let db_analyzer = Arc::new(
        crate::infrastructure::crawling_service_impls::DatabaseAnalyzerImpl::new(
            product_repo.clone(),
        ),
    );

    // SystemConfig로 변환 (CrawlingPlanner용)
    let system_config = Arc::new(crate::crawl_engine::context::SystemConfig::default());

    // 🚀 실제 CrawlingPlanner 사용!
    let crawling_planner = crate::crawl_engine::services::crawling_planner::CrawlingPlanner::new(
        status_checker.clone(),
        db_analyzer.clone(),
        system_config.clone(),
    )
    .with_repository(product_repo.clone());

    // 시스템 상태 분석 (캐시 재사용 시도)
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
        "🌐 [ACTOR] Real site analysis: {} pages, {} products on last page",
        site_status.total_pages, site_status.products_on_last_page
    );
    info!(
        "💾 [ACTOR] Real DB analysis: {} total products, {} unique products",
        db_analysis.total_products, db_analysis.unique_products
    );

    // 🎯 실제 CrawlingPlanner로 지능형 전략 결정
    let (range_recommendation, processing_strategy) = crawling_planner
        .determine_crawling_strategy(&site_status, &db_analysis)
        .map_err(|e| format!("Failed to determine crawling strategy: {}", e))?;

    info!(
        "📋 [ACTOR] CrawlingPlanner recommendation: {:?}",
        range_recommendation
    );
    info!(
        "⚙️ [ACTOR] Processing strategy: batch_size={}, concurrency={}",
        processing_strategy.recommended_batch_size, processing_strategy.recommended_concurrency
    );

    // 지능형 범위 권장사항을 실제 페이지 범위로 변환
    let (calculated_start_page, calculated_end_page) =
        if let Some((start, end)) = range_recommendation.to_page_range(site_status.total_pages) {
            // 🔄 역순 크롤링으로 변환 (start > end)
            let reverse_start = if start > end { start } else { end };
            let reverse_end = if start > end { end } else { start };
            info!(
                "🎯 [ACTOR] CrawlingPlanner range: {} to {} (reverse crawling)",
                reverse_start, reverse_end
            );
            (reverse_start, reverse_end)
        } else {
            info!("🔍 [ACTOR] No crawling needed, using verification range");
            let verification_pages = app_config.user.crawling.page_range_limit.min(5);
            let start = site_status.total_pages;
            let end = if start >= verification_pages {
                start - verification_pages + 1
            } else {
                1
            };
            (start, end)
        };

    // 🚨 설정 기반 범위 제한 적용 (user.crawling.page_range_limit)
    let max_allowed_pages = app_config.user.crawling.page_range_limit;
    let requested_pages = if calculated_start_page >= calculated_end_page {
        calculated_start_page - calculated_end_page + 1
    } else {
        calculated_end_page - calculated_start_page + 1
    };

    let (final_start_page, final_end_page) = if requested_pages > max_allowed_pages {
        info!(
            "⚠️ [ACTOR] CrawlingPlanner requested {} pages, but config limits to {} pages",
            requested_pages, max_allowed_pages
        );
        // 설정 제한에 맞춰 범위 조정
        let limited_start = site_status.total_pages;
        let limited_end = if limited_start >= max_allowed_pages {
            limited_start - max_allowed_pages + 1
        } else {
            1
        };
        info!(
            "🔒 [ACTOR] Range limited by config: {} to {} ({} pages)",
            limited_start, limited_end, max_allowed_pages
        );
        (limited_start, limited_end)
    } else {
        // 🚨 프론트엔드에서는 By Design으로 페이지 범위를 지정하지 않음
        // 따라서 항상 CrawlingPlanner 권장사항을 사용
        info!(
            "🧠 [ACTOR] Frontend does not specify page ranges by design - using CrawlingPlanner recommendation"
        );
        info!(
            "🤖 [ACTOR] CrawlingPlanner recommendation: {} to {}",
            calculated_start_page, calculated_end_page
        );

        // ⚠️ request.start_page와 request.end_page는 프론트엔드 테스트 코드에서 설정한 임시값이므로 무시
        if request.start_page.unwrap_or(0) != 0 && request.end_page.unwrap_or(0) != 0 {
            info!(
                "⚠️ [ACTOR] Ignoring frontend test values (start_page: {:?}, end_page: {:?}) - using intelligent planning",
                request.start_page, request.end_page
            );
        }

        // CrawlingPlanner 권장사항 사용
        info!("🎯 [ACTOR] Using CrawlingPlanner intelligent recommendation for optimal crawling");
        (calculated_start_page, calculated_end_page)
    };

    info!("🧠 [ACTOR] Final range calculated:");
    info!(
        "   📊 Range: {} to {} ({} pages, config limit: {})",
        final_start_page,
        final_end_page,
        if final_start_page >= final_end_page {
            final_start_page - final_end_page + 1
        } else {
            final_end_page - final_start_page + 1
        },
        app_config.user.crawling.page_range_limit
    );

    // 분석 정보를 JSON으로 구성
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

    info!("✅ Intelligent range calculation completed for Actor system");
    Ok((final_start_page, final_end_page, analysis_info))
}

// (removed legacy execute_real_batch_actor)

// (run_single_batch_real removed)

/// `CrawlingPlanner` 기반 `ExecutionPlan` 생성 (단일 호출)
///
/// 시스템 상태를 종합 분석하여 최적의 실행 계획을 생성합니다.
/// 이 함수가 호출된 후에는 더 이상 분석/계획 단계가 없습니다.
async fn create_execution_plan(
    app: &AppHandle,
) -> Result<(ExecutionPlan, AppConfig, DomainSiteStatus), Box<dyn std::error::Error + Send + Sync>>
{
    info!("🧠 Creating ExecutionPlan with CrawlingPlanner (cache-aware)...");

    // 1. 설정 로드
    let config_manager = ConfigManager::new()?;
    let app_config = config_manager.load_config().await?;

    // Failure policy cache 업데이트 (config 기반)
    update_global_failure_policy_from_config(&app_config);

    // 2. 이미 초기화된 데이터베이스 풀 사용 (새로 연결하지 않음)
    let app_state = app.state::<AppState>();
    let db_pool = {
        let pool_guard = app_state.database_pool.read().await;
        pool_guard
            .as_ref()
            .ok_or("Database pool not initialized")?
            .clone()
    };

    info!("📊 Using existing database pool from AppState");

    // 3. 서비스 생성 (기존 데이터베이스 풀 재사용)
    let product_repo = Arc::new(IntegratedProductRepository::new(db_pool.clone()));

    // 🔍 데이터베이스 연결 테스트
    info!("🔍 Testing database connection before creating CrawlingPlanner...");
    match product_repo.get_product_count().await {
        Ok(count) => {
            info!(
                "✅ Database connection successful: {} products found",
                count
            );
        }
        Err(e) => {
            error!(
                "❌ Database connection failed in create_execution_plan: {}",
                e
            );
            return Err(format!("Database connection test failed: {}", e).into());
        }
    }

    let http_client = HttpClient::create_from_global_config()?.with_context_label("Planner");
    let data_extractor = MatterDataExtractor::new()?;

    let status_checker = Arc::new(
        crate::infrastructure::crawling_service_impls::StatusCheckerImpl::with_product_repo(
            http_client.clone(),
            data_extractor.clone(),
            app_config.clone(),
            product_repo.clone(),
        ),
    );

    let database_analyzer = Arc::new(
        crate::infrastructure::crawling_service_impls::DatabaseAnalyzerImpl::new(
            product_repo.clone(),
        ),
    );

    // 4. CrawlingPlanner 생성 및 분석
    let crawling_planner = crate::crawl_engine::services::crawling_planner::CrawlingPlanner::new(
        status_checker,
        database_analyzer,
        Arc::new(SystemConfig::default()),
    )
    .with_repository(product_repo.clone());

    info!("🎯 Analyzing system state with CrawlingPlanner (attempting cache reuse)...");

    // === Cache: attempt to reuse previously computed site analysis ===
    let shared_cache: Option<State<SharedStateCache>> = app.try_state::<SharedStateCache>();
    let cached_site_status: Option<DomainSiteStatus> = if let Some(cache_state) =
        shared_cache.as_ref()
    {
        // TTL 5분 기본
        if let Some(cached) = cache_state.get_valid_site_analysis_async(Some(5)).await {
            info!(
                "♻️ Reusing cached SiteStatus: total_pages={}, last_page_products={} (age<=TTL)",
                cached.total_pages, cached.products_on_last_page
            );
            Some(DomainSiteStatus {
                is_accessible: true,
                response_time_ms: 0, // Unknown from cache snapshot
                total_pages: cached.total_pages,
                estimated_products: cached.estimated_products,
                products_on_last_page: cached.products_on_last_page,
                last_check_time: cached.analyzed_at,
                health_score: cached.health_score,
                data_change_status: SiteDataChangeStatus::Stable {
                    count: cached.estimated_products,
                },
                decrease_recommendation: None,
                crawling_range_recommendation: CrawlingRangeRecommendation::Full, // Conservative default
            })
        } else {
            info!("🔄 No valid cached SiteStatus (or expired) – performing fresh check");
            None
        }
    } else {
        info!("📭 SharedStateCache not available in Tauri state – proceeding without cache");
        None
    };

    // ──────────────────────────────────────────────
    // (1) 사전 데이터베이스 상태로 전략 결정 힌트 계산
    let existing_product_count = match product_repo.get_product_count().await {
        Ok(c) => c,
        Err(e) => {
            warn!(
                "⚠️ Failed to get product count for strategy decision: {} -> default NewestFirst",
                e
            );
            0
        }
    };

    // 기본 전략은 NewestFirst. DB에 데이터가 있으면 ContinueFromDb 시도
    let mut chosen_strategy = crate::crawl_engine::actors::types::CrawlingStrategy::NewestFirst;
    if existing_product_count > 0 {
        chosen_strategy = crate::crawl_engine::actors::types::CrawlingStrategy::ContinueFromDb;
        info!(
            "🧭 Choosing ContinueFromDb strategy (existing products={})",
            existing_product_count
        );
    } else {
        info!("🧭 Choosing NewestFirst strategy (empty DB)");
    }

    // (2) CrawlingConfig 생성 (start_page/end_page는 '개수' 표현: start_page - end_page + 1 = 요청 수)
    let crawling_config = CrawlingConfig {
        site_url: "https://csa-iot.org/csa-iot_products/".to_string(),
        start_page: app_config.user.crawling.page_range_limit.max(1), // 요청 개수 표현
        end_page: 1,
        concurrency_limit: app_config.user.max_concurrent_requests,
        batch_size: app_config.user.batch.batch_size,
        request_delay_ms: 1000,
        timeout_secs: 300,
        max_retries: app_config.user.crawling.workers.max_retries,
        strategy: chosen_strategy.clone(),
    };

    // (3) 사이트 상태 및 계획 생성 (사이트 상태 1회 조회 + DB 분석)
    let cache_was_none = cached_site_status.is_none();
    // Attempt DB analysis cache reuse (TTL 3m)
    let cached_db_analysis: Option<crate::domain::services::crawling_services::DatabaseAnalysis> =
        if let Some(cache_state) = shared_cache.as_ref() {
            cache_state
                .get_valid_db_analysis_async(Some(3))
                .await
                .map(
                    |d| crate::domain::services::crawling_services::DatabaseAnalysis {
                        total_products: d.total_products,
                        unique_products: d.total_products, // approximation (no uniqueness snapshot in cached struct)
                        duplicate_count: 0,
                        missing_products_count: 0,
                        last_update: Some(d.analyzed_at),
                        missing_fields_analysis:
                            crate::domain::services::crawling_services::FieldAnalysis {
                                missing_company: 0,
                                missing_model: 0,
                                missing_matter_version: 0,
                                missing_connectivity: 0,
                                missing_certification_date: 0,
                            },
                        data_quality_score: d.quality_score,
                    },
                )
        } else {
            None
        };
    let db_cache_hit = cached_db_analysis.is_some();
    if db_cache_hit {
        if let Some(cache_state) = shared_cache.as_ref() {
            if let Some(db_cached_raw) = cache_state.get_valid_db_analysis_async(Some(3)).await {
                let enriched = db_cached_raw.max_page_id.is_some()
                    && db_cached_raw.max_index_in_page.is_some();
                let max_pid = db_cached_raw
                    .max_page_id.map_or_else(|| "null".into(), |v| v.to_string());
                let max_idx = db_cached_raw
                    .max_index_in_page.map_or_else(|| "null".into(), |v| v.to_string());
                info!(target: "kpi.execution_plan", "{{\"event\":\"db_analysis_cache_hit\",\"total_products\":{},\"has_position\":{},\"max_page_id\":{},\"max_index_in_page\":{}}}", db_cached_raw.total_products, enriched, max_pid, max_idx);
            } else {
                info!(target: "kpi.execution_plan", "{{\"event\":\"db_analysis_cache_hit\",\"warn\":\"cache_disappeared\"}}" );
            }
        } else {
            info!(target: "kpi.execution_plan", "{{\"event\":\"db_analysis_cache_hit\",\"warn\":\"no_shared_cache\"}}" );
        }
    }
    let (crawling_plan, site_status, db_analysis_used) = crawling_planner
        .create_crawling_plan_with_caches(&crawling_config, cached_site_status, cached_db_analysis)
        .await?;
    if !db_cache_hit {
        info!(target: "kpi.execution_plan", "{{\"event\":\"db_analysis_cache_miss\"}}" );
    }
    // Persist fresh site status & db analysis if newly fetched
    if let Some(cache_state_ref) = shared_cache.as_ref() {
        if cache_was_none {
            use crate::application::shared_state::SiteAnalysisResult;
            let site_analysis = SiteAnalysisResult::new(
                site_status.total_pages,
                site_status.products_on_last_page,
                site_status.estimated_products,
                crawling_config.site_url.clone(),
                site_status.health_score,
            );
            cache_state_ref.set_site_analysis(site_analysis).await;
        }
        if !db_cache_hit {
            use crate::application::shared_state::DbAnalysisResult;
            // First attempt to read precise page/index BEFORE caching to avoid placeholder None persistence
            let (precise_page_id, precise_index_in_page) =
                match product_repo.get_max_page_id_and_index().await {
                    Ok(v) => v,
                    Err(_) => (None, None),
                };
            let db_cached = DbAnalysisResult::new(
                db_analysis_used.total_products,
                precise_page_id,
                precise_index_in_page,
                db_analysis_used.data_quality_score,
            );
            cache_state_ref.set_db_analysis(db_cached).await;
        }
    }
    info!(
        "🧪 CrawlingPlanner produced plan with {:?} (requested strategy {:?})",
        crawling_plan.optimization_strategy, chosen_strategy
    );
    if db_cache_hit {
        info!(target: "kpi.execution_plan", "{{\"event\":\"db_analysis_used\",\"source\":\"cache\",\"total_products\":{}}}", db_analysis_used.total_products);
    } else {
        info!(target: "kpi.execution_plan", "{{\"event\":\"db_analysis_used\",\"source\":\"fresh\",\"total_products\":{}}}", db_analysis_used.total_products);
    }

    info!("📋 CrawlingPlan created: {:?}", crawling_plan);

    // === DB Analysis cache advisory (pre-plan) ===
    if let Some(cache_state) = shared_cache.as_ref() {
        if let Some(db_cached) = cache_state.get_valid_db_analysis_async(Some(3)).await {
            info!(
                "♻️ Using cached DB analysis advisory: total_products={} (age TTL<=3m)",
                db_cached.total_products
            );
        }
    }

    // 5. ExecutionPlan 생성 전 hash 산출 및 PlanCache 검사
    let session_id = format!("actor_session_{}", Utc::now().timestamp());
    let plan_id = format!("plan_{}", Utc::now().timestamp());
    // Structured PLAN KPI (after plan_id known)
    let list_phase_count = crawling_plan
        .phases
        .iter()
        .filter(|p| {
            matches!(
                p.phase_type,
                crate::crawl_engine::services::crawling_planner::PhaseType::ListPageCrawling
            )
        })
        .count();
    let total_list_pages: u32 = crawling_plan
        .phases
        .iter()
        .filter(|p| {
            matches!(
                p.phase_type,
                crate::crawl_engine::services::crawling_planner::PhaseType::ListPageCrawling
            )
        })
        .map(|p| p.pages.len() as u32)
        .sum();
    // plan_created KPI는 ExecutionPlan hash 확정 후 한 번만 출력하도록 변경 (중복 제거)

    // CrawlingPlan phases(ListPageCrawling) -> crawling_ranges (중복 재분할 제거)
    let mut crawling_ranges: Vec<PageRange> = crawling_plan
        .phases
        .iter()
        .filter(|p| {
            matches!(
                p.phase_type,
                crate::crawl_engine::services::crawling_planner::PhaseType::ListPageCrawling
            )
        })
        .filter(|p| !p.pages.is_empty())
        .map(|p| {
            let first = *p.pages.first().unwrap(); // newest
            let last = *p.pages.last().unwrap(); // oldest in this batch
            let pages_count = first.saturating_sub(last) + 1; // contiguous descending
            PageRange {
                start_page: first,
                end_page: last,
                estimated_products: pages_count * 12, // heuristic
                reverse_order: true,
            }
        })
        .collect();

    // page_range_limit 적용 (최신 배치를 우선 유지하면서 초과 페이지 잘라내기)
    // page_range_limit already u32; remove redundant cast
    let page_limit = app_config.user.crawling.page_range_limit.max(1);
    if page_limit > 0 {
        let mut accumulated: u32 = 0;
        let mut trim_index: Option<usize> = None;
        for (idx, r) in crawling_ranges.iter_mut().enumerate() {
            let pages_in_range = r.start_page.saturating_sub(r.end_page) + 1;
            if accumulated + pages_in_range > page_limit {
                // 이 range를 부분 절단
                let remaining = page_limit - accumulated;
                if remaining == 0 {
                    trim_index = Some(idx);
                } else if remaining < pages_in_range {
                    // 일부만 필요
                    // start_page는 최신, end_page를 조정하여 remaining 개수를 유지
                    let new_end = r.start_page.saturating_sub(remaining - 1);
                    if new_end > r.end_page {
                        // 안전 가드 (정상적으로는 항상 true)
                        r.end_page = new_end;
                        let new_pages = r.start_page.saturating_sub(r.end_page) + 1;
                        r.estimated_products = new_pages * 12;
                        trim_index = Some(idx + 1); // 이후 range 제거
                    } else {
                        // fallback: 유지 (이상 상황)
                        trim_index = Some(idx + 1);
                    }
                }
                break;
            }
            accumulated += pages_in_range;
        }
        if let Some(cut) = trim_index {
            crawling_ranges.truncate(cut);
        }
    }

    if crawling_ranges.is_empty() {
        // 안전 폴백 (최신 1페이지) - Planner가 비었을 때
        crawling_ranges.push(PageRange {
            start_page: 1,
            end_page: 1,
            estimated_products: 12,
            reverse_order: true,
        });
    }

    let total_pages: u32 = crawling_ranges
        .iter()
        .map(|r| {
            if r.reverse_order {
                r.start_page - r.end_page + 1
            } else {
                r.end_page - r.start_page + 1
            }
        })
        .sum();

    // DB page/index 상태 읽기 (실패 시 None 유지)
    let (db_max_page_id, db_max_index_in_page) =
        match product_repo.get_max_page_id_and_index().await {
            Ok(v) => v,
            Err(e) => {
                warn!("⚠️ Failed to read max page/index: {}", e);
                (None, None)
            }
        };
    // Enrich existing DB analysis cache with position info if it was cached earlier without it
    if let Some(cache_state) = shared_cache.as_ref() {
        cache_state
            .enrich_db_analysis_position(db_max_page_id, db_max_index_in_page)
            .await;
    }
    info!(
        "🧾 DB snapshot: max_page_id={:?} max_index_in_page={:?} total_products_dbMetric={:?}",
        db_max_page_id, db_max_index_in_page, crawling_plan.db_total_products
    );

    // 입력 스냅샷 구성 (사이트/DB 상태 + 핵심 제한값)
    let snapshot = crate::crawl_engine::actors::types::PlanInputSnapshot {
        total_pages: site_status.total_pages,
        products_on_last_page: site_status.products_on_last_page,
        db_max_page_id,
        db_max_index_in_page,
        db_total_products: u64::from(crawling_plan.db_total_products.unwrap_or(0)),
        page_range_limit: app_config.user.crawling.page_range_limit,
        batch_size: app_config.user.batch.batch_size,
        concurrency_limit: app_config.user.max_concurrent_requests,
        created_at: Utc::now(),
    };

    // 해시 계산 (공통 헬퍼 사용)
    let plan_hash = compute_plan_hash(
        &snapshot,
        &crawling_ranges,
        &format!("{:?}", crawling_plan.optimization_strategy),
    );
    info!(target: "kpi.plan", "{{\"event\":\"plan_hash_assigned\",\"plan_id\":\"{}\",\"plan_hash\":\"{}\",\"range_count\":{},\"list_pages_total\":{}}}", plan_id, plan_hash, crawling_ranges.len(), crawling_ranges.iter().map(|r| if r.reverse_order { r.start_page - r.end_page + 1 } else { r.end_page - r.start_page + 1 }).sum::<u32>());

    if let Some(cache_state) = shared_cache.as_ref() {
        if let Some(hit) = cache_state.get_cached_execution_plan(&plan_hash).await {
            return Ok((hit, app_config, site_status));
        }
        info!(
            "🆕 PlanCache miss (hash={}) — creating new ExecutionPlan",
            plan_hash
        );
    }

    // PlanCache hit 확인 (hash 계산 후 조회) - hash 는 아래에서 이미 계산됨
    if let Some(cache_state) = app.try_state::<SharedStateCache>() {
        if let Some(cached_plan) = futures::executor::block_on(async {
            cache_state.get_cached_execution_plan(&plan_hash).await
        }) {
            info!("♻️ PlanCache hit: reuse ExecutionPlan hash={}", plan_hash);
            let json_line = format!(
                "{{\"event\":\"plan_cache_hit\",\"hash\":\"{}\"}}",
                plan_hash
            );
            info!(target: "kpi.execution_plan", "{}", json_line);
            return Ok((cached_plan, app_config, site_status));
        }
    }

    let ranges_len = crawling_ranges.len();
    let strategy_string = format!("{:?}", crawling_plan.optimization_strategy);
    // Precompute page_slots using canonical PaginationCalculator to avoid drift.
    use crate::domain::pagination::PaginationCalculator;
    let calc = PaginationCalculator::default();
    let mut page_slots: Vec<crate::crawl_engine::actors::types::PageSlot> = Vec::new();
    for range in &crawling_ranges {
        let pages_iter: Box<dyn Iterator<Item = u32>> = if range.reverse_order {
            Box::new(range.end_page..=range.start_page)
        } else {
            Box::new(range.start_page..=range.end_page)
        };
        for physical_page in pages_iter {
            if physical_page == 0 {
                continue;
            }
            // Conservative: assume up to PRODUCTS_PER_PAGE slots; last page actual size handled by index filtering or later pruning if needed.
            let assumed_capacity = crate::domain::constants::site::PRODUCTS_PER_PAGE as u32;
            for idx in 0..assumed_capacity {
                // idx = from top (newest) within physical page
                let pos = calc.calculate(physical_page, idx, site_status.total_pages);
                page_slots.push(crate::crawl_engine::actors::types::PageSlot {
                    physical_page,
                    page_id: i64::from(pos.page_id),
                    index_in_page: pos.index_in_page as i16,
                });
            }
        }
    }
    // Sort & dedup logical slots to ensure stable ordering
    page_slots.sort_by(|a, b| match a.page_id.cmp(&b.page_id) {
        core::cmp::Ordering::Equal => a.index_in_page.cmp(&b.index_in_page),
        other => other,
    });
    page_slots.dedup_by(|a, b| a.page_id == b.page_id && a.index_in_page == b.index_in_page);
    // Audit sample
    let head: Vec<String> = page_slots
        .iter()
        .take(5)
        .map(|s| {
            format!(
                "p{}=>gid{}_i{}",
                s.physical_page, s.page_id, s.index_in_page
            )
        })
        .collect();
    let tail: Vec<String> = page_slots
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
        "📐 canonical page_slots audit head={:?} tail={:?} total_slots={}",
        head,
        tail,
        page_slots.len()
    );
    let execution_plan = ExecutionPlan {
        plan_id,
        session_id,
        crawling_ranges,
        batch_size: app_config.user.batch.batch_size,
        concurrency_limit: app_config.user.max_concurrent_requests,
        estimated_duration_secs: crawling_plan.total_estimated_duration_secs,
        created_at: Utc::now(),
        analysis_summary: format!(
            "Strategy: {:?}, Total pages: {}",
            strategy_string, total_pages
        ),
        original_strategy: strategy_string.clone(),
        input_snapshot: snapshot,
        plan_hash,
        skip_duplicate_urls: true,
        kpi_meta: Some(crate::crawl_engine::actors::types::ExecutionPlanKpi {
            total_ranges: ranges_len,
            total_pages,
            batches: ranges_len,
            strategy: strategy_string,
            created_at: Utc::now(),
        }),
        contract_version: ACTOR_CONTRACT_VERSION,
        page_slots,
    };

    info!(
        "✅ ExecutionPlan created successfully: {} pages across {} batches (hash={})",
        total_pages,
        execution_plan.crawling_ranges.len(),
        execution_plan.plan_hash
    );
    if let Some(kpi) = &execution_plan.kpi_meta {
        info!(target: "kpi.plan", "{{\"event\":\"plan_created\",\"plan_id\":\"{}\",\"session_id\":\"{}\",\"list_phase_count\":{},\"total_list_pages\":{},\"strategy\":\"{}\",\"plan_hash\":\"{}\",\"ranges\":{},\"batches\":{},\"ts\":\"{}\"}}",
            execution_plan.plan_id, execution_plan.session_id, list_phase_count, total_list_pages, kpi.strategy, execution_plan.plan_hash, kpi.total_ranges, kpi.batches, kpi.created_at);
    }
    if let Some(cache_state) = app.try_state::<SharedStateCache>() {
        cache_state
            .cache_execution_plan(execution_plan.clone())
            .await;
    }

    Ok((execution_plan, app_config, site_status))
}

// (Removed unused adjust_execution_plan_with_page_overrides; logic lives in planning_service)

// Build an ExecutionPlan from explicit pages (newest -> oldest) with contiguous range compression.
async fn build_execution_plan_from_explicit_pages(
    app: &AppHandle,
    mut pages: Vec<u32>,
) -> Result<(ExecutionPlan, AppConfig, DomainSiteStatus), String> {
    use crate::domain::pagination::PaginationCalculator;

    // Load config
    let config_manager = ConfigManager::new().map_err(|e| e.to_string())?;
    let app_config = config_manager
        .load_config()
        .await
        .map_err(|e| e.to_string())?;
    update_global_failure_policy_from_config(&app_config);

    // Site status: prefer cache, else reuse planner path
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

    // Normalize pages
    pages.retain(|p| *p >= 1 && *p <= site_status.total_pages);
    pages.sort_by(|a, b| b.cmp(a)); // newest -> oldest
    pages.dedup();
    if pages.is_empty() {
        return Err("No valid pages after normalization".into());
    }

    // Compress to contiguous reverse ranges
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

    // Precompute page_slots
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
                    index_in_page: pos.index_in_page as i16,
                });
            }
        }
    }
    page_slots.sort_by(|a, b| match a.page_id.cmp(&b.page_id) {
        core::cmp::Ordering::Equal => a.index_in_page.cmp(&b.index_in_page),
        other => other,
    });
    page_slots.dedup_by(|a, b| a.page_id == b.page_id && a.index_in_page == b.index_in_page);

    // Snapshot + hash
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

    // Plan
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
        // Manual runs should update existing records' fields (page_id/index_in_page),
        // so do not skip duplicates in this mode.
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

/// Start manual crawl via Actor pipeline from explicit pages (async, full parity; no validation phase).
#[tauri::command(async)]
pub async fn start_manual_crawl_pages_actor(
    app: AppHandle,
    pages: Vec<u32>,
    _skip_validation: Option<bool>,
) -> Result<ActorSystemResponse, String> {
    if pages.is_empty() {
        return Err("No pages provided".into());
    }
    let (execution_plan, app_config, site_status) =
        build_execution_plan_from_explicit_pages(&app, pages).await?;

    info!(target: "kpi.plan", "{{\"event\":\"manual_actor_started\",\"session_id\":\"{}\",\"plan_id\":\"{}\",\"ranges\":{},\"hash\":\"{}\"}}",
        execution_plan.session_id,
        execution_plan.plan_id,
        execution_plan.crawling_ranges.len(),
        execution_plan.plan_hash
    );

    let (sid, exec_clone) = bootstrap_and_spawn_session(
        &app,
        execution_plan.clone(),
        app_config.clone(),
        site_status,
        None,
        None,
        None,
        None,
    )
    .await?;

    Ok(ActorSystemResponse {
        success: true,
        message: "Manual actor crawl started".into(),
        session_id: Some(sid),
        data: Some(serde_json::to_value(&exec_clone).map_err(|e| e.to_string())?),
    })
}

// (removed: legacy execute_session_actor_with_execution_plan; SessionActor handles execution)

// (Removed unused simulation helpers: execute_batch_actor_simulation, run_simulation_crawling)

// =====================================================
//  Data Consistency Check Command (page_id / index_in_page)
// =====================================================
#[tauri::command]
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

// ===================== Tests (Phase C: resume token integrity) =====================
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
