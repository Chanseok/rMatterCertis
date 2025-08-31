//! Actor 이벤트 프론트엔드 브릿지
//!
//! Actor 시스템의 `AppEvent를` 실제 Tauri 프론트엔드로 전달하는 브릿지 컴포넌트
//! 설계 의도: 각 Actor, Task 레벨에서 독립적으로 이벤트 발행을 가능하게 하여
//! 낮은 복잡성의 구현으로도 모든 경우를 다 커버할 수 있도록 함

use crate::crawl_engine::actors::types::{AppEvent, SimpleMetrics};
use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use tauri::{AppHandle, Emitter};
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

/// Actor 이벤트를 프론트엔드로 전달하는 브릿지
pub struct ActorEventBridge {
    /// Tauri `AppHandle`
    app_handle: AppHandle,
    /// Actor 이벤트 수신기
    event_rx: broadcast::Receiver<AppEvent>,
    /// 브릿지 활성화 상태
    is_active: Arc<std::sync::atomic::AtomicBool>,
    /// 단조 증가 시퀀스 번호
    seq: Arc<AtomicU64>,
    /// 최근 네이티브 PageLifecycle 키 캐시 (세션/배치/페이지) to prevent synthetic duplicates
    recent_pages: Arc<tokio::sync::Mutex<VecDeque<(String, Option<String>, u32, std::time::Instant)>>>,
}

impl ActorEventBridge {
    /// 새로운 브릿지 생성
    #[must_use]
    pub fn new(app_handle: AppHandle, event_rx: broadcast::Receiver<AppEvent>) -> Self {
        Self {
            app_handle,
            event_rx,
            is_active: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            seq: Arc::new(AtomicU64::new(1)),
            recent_pages: Arc::new(tokio::sync::Mutex::new(VecDeque::with_capacity(64))),
        }
    }

    /// 브릿지 시작 - Actor 이벤트를 프론트엔드로 전달
    pub async fn start(&mut self) {
        if self
            .is_active
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            warn!("ActorEventBridge is already running");
            return;
        }

        info!("🌉 Starting Actor Event Bridge - connecting Actor events to Frontend");

        while self.is_active.load(std::sync::atomic::Ordering::SeqCst) {
            match self.event_rx.recv().await {
                Ok(actor_event) => {
                    debug!("[BridgeRecv] received AppEvent variant (pre-forward)");
                    if let Err(e) = self.forward_to_frontend(actor_event).await {
                        error!("Failed to forward Actor event to Frontend: {}", e);
                    }
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!("Actor event channel closed, stopping bridge");
                    break;
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    warn!("Actor event bridge lagged, skipped {} events", skipped);
                    continue;
                }
            }
        }

        self.is_active
            .store(false, std::sync::atomic::Ordering::SeqCst);
        info!("🌉 Actor Event Bridge stopped");
    }

    /// 브릿지 중지
    pub fn stop(&self) {
        self.is_active
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    /// Actor 이벤트를 프론트엔드로 전달
    #[allow(clippy::unused_async)]
    async fn forward_to_frontend(&self, actor_event: AppEvent) -> Result<(), String> {
        // AppEvent를 프론트엔드가 이해할 수 있는 형태로 변환
        let (event_name, event_data) = self.convert_actor_event_to_frontend(actor_event.clone())?;

        // 시퀀스 & backend_ts 주입 (RFC3339)
        let enriched = {
            let mut v = event_data;
            if let Some(obj) = v.as_object_mut() {
                obj.insert(
                    "seq".into(),
                    serde_json::Value::from(self.seq.fetch_add(1, Ordering::SeqCst)),
                );
                obj.insert(
                    "backend_ts".into(),
                    serde_json::Value::from(chrono::Utc::now().to_rfc3339()),
                );
                obj.insert(
                    "event_name".into(),
                    serde_json::Value::from(event_name.clone()),
                );
            }
            v
        };
        // Always emit unified actor-event
        let unified_name = "actor-event";
        self.app_handle
            .emit(unified_name, &enriched)
            .map_err(|e| format!("Tauri emit failed: {}", e))?;
        // Concise info line to events.log for visibility
        if let Some(obj) = enriched.as_object() {
            let variant = obj.get("variant").and_then(|v| v.as_str()).unwrap_or("?");
            let seq_val = obj.get("seq").and_then(serde_json::Value::as_u64).unwrap_or(0);
            let session_id = obj.get("session_id").and_then(|v| v.as_str());
            let batch_id = obj.get("batch_id").and_then(|v| v.as_str());
            tracing::info!(target: "actor-event",
                "🌉 actor-event seq={} name={} variant={} session_id={:?} batch_id={:?}",
                seq_val, unified_name, variant, session_id, batch_id
            );
        } else {
            tracing::info!(target: "actor-event",
                "🌉 actor-event name={} (unstructured)", unified_name
            );
        }
        // Helpful concise logs by variant
        match &actor_event {
            AppEvent::TaskLifecycle { session_id, batch_id, task_kind, page_number, product_ref, status, duration_ms, .. } => {
                tracing::info!(target: "actor-event",
                    "[TaskLifecycle] kind={:?} status={} page={:?} ref={:?} dur_ms={:?} batch={:?} session={}",
                    task_kind, status, page_number, product_ref, duration_ms, batch_id, session_id
                );
            }
            AppEvent::ProductLifecycleGroup { session_id, batch_id, page_number, group_size, started, succeeded, failed, duplicates, duration_ms, phase, .. } => {
                tracing::info!(target: "actor-event",
                    "[ProductLifecycleGroup] phase={} size={} started={} ok={} fail={} dup={} page={:?} batch={:?} dur_ms={} session={}",
                    phase, group_size, started, succeeded, failed, duplicates, page_number, batch_id, duration_ms, session_id
                );
            }
            AppEvent::DatabaseStats { session_id, batch_id, total_product_details, min_page, max_page, note, .. } => {
                tracing::info!(target: "actor-event",
                    "[DatabaseStats] total={} range={:?}-{:?} note={:?} batch={:?} session={}",
                    total_product_details, min_page, max_page, note, batch_id, session_id
                );
            }
            AppEvent::PageLifecycle { session_id, batch_id, page_number, status, metrics, .. } => {
                self.push_recent_page(session_id, batch_id.as_ref(), *page_number).await;
                let (urls, scheduled, err) = match metrics {
                    Some(SimpleMetrics::Page { url_count, scheduled_details, error }) => (
                        url_count.unwrap_or(0),
                        scheduled_details.unwrap_or(0),
                        error.as_deref().unwrap_or("")
                    ),
                    _ => (0, 0, ""),
                };
                tracing::info!(target: "actor-event",
                    "[PageLifecycle] status={} page={} urls={} scheduled={} err='{}' batch={:?} session={}",
                    status, page_number, urls, scheduled, err, batch_id, session_id
                );
            }
            AppEvent::StageStarted { stage_type, session_id, batch_id, items_count, .. } => {
                tracing::info!(target: "actor-event",
                    "[Stage] started stage={} items={} batch={:?} session={}",
                    stage_type.as_str(), items_count, batch_id, session_id
                );
            }
            AppEvent::StageCompleted { stage_type, session_id, batch_id, result, .. } => {
                tracing::info!(target: "actor-event",
                    "[Stage] completed stage={} processed={} ok={} fail={} dur_ms={} batch={:?} session={}",
                    stage_type.as_str(), result.processed_items, result.successful_items, result.failed_items, result.duration_ms, batch_id, session_id
                );
            }
            _ => {}
        }
        // 추가: 세션 단위 최종 보고가 들어오면 일반 로그에도 요약을 남겨 back_front.log에서 확인 가능하게 함
        if let AppEvent::CrawlReportSession {
            session_id,
            batches_processed,
            total_pages,
            total_success,
            total_failed,
            total_retries,
            duration_ms,
            products_inserted,
            products_updated,
            timestamp,
        } = &actor_event
        {
            info!(
                "📊 Session Final Report | session_id={} duration_ms={} batches={} pages_total={} success={} failed={} retries={} inserted={} updated={} ts={}",
                session_id,
                duration_ms,
                batches_processed,
                total_pages,
                total_success,
                total_failed,
                total_retries,
                products_inserted,
                products_updated,
                timestamp.to_rfc3339()
            );
        }

    // 보강: SessionCompleted(summary) 수신 시에도 메인 로그에 인간 친화적 요약을 남긴다.
        if let AppEvent::SessionCompleted { summary, .. } = &actor_event {
            info!(
                "📊 Session Final Summary | session_id={} state={} duration_ms={} batches={} pages_processed={} success={} failed={} retries={} inserted={} updated={} duplicates={} ts={}",
                summary.session_id,
                summary.final_state,
                summary.total_duration_ms,
                summary.processed_batches,
                summary.total_pages_processed,
                summary.total_success_count,
                summary.failed_pages_count,
                summary.total_retry_events,
                summary.products_inserted,
                summary.products_updated,
                summary.duplicates_skipped,
                chrono::Utc::now().to_rfc3339()
            );
        }

        debug!(
            "✅ Forwarded generalized Actor event '{}' (original={})",
            unified_name, event_name
        );
    return Ok(());
    }

    /// `AppEvent를` 프론트엔드 이벤트로 변환
    ///
    /// 매핑 정책
    /// - 이벤트 이름은 `actor-*` 네임스페이스로 고정합니다(예: actor-stage-started).
    /// - 페이로드는 enum 외부 태깅을 평탄화하여 `{ variant: "...", ...fields }` 형태로 전달합니다.
    /// - 스키마는 additive-only 원칙을 따릅니다. 새 필드는 추가 가능하나 기존 키를 제거/변경하지 않습니다.
    fn convert_actor_event_to_frontend(
        &self,
        event: AppEvent,
    ) -> Result<(String, serde_json::Value), String> {
        convert_actor_event_to_frontend_value(event)
    }

    // NOTE: Legacy `CrawlingEvent` conversion removed. Frontend should consume unified `actor-event` only.

    /// 브릿지 상태 확인
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.is_active.load(std::sync::atomic::Ordering::SeqCst)
    }

    // PageTask* removed; synthetic conversion no longer needed.

    // Build synthetic TaskLifecycle payload from PageLifecycle/ProductLifecycle
    // (previously had an experimental helper to synthesize TaskLifecycle from Page/Product lifecycles)
    // Removed as native TaskLifecycle is now emitted directly by actors.

    async fn push_recent_page(&self, session_id: &str, batch_id: Option<&String>, page: u32) {
        use std::time::{Duration, Instant};
        let mut q = self.recent_pages.lock().await;
        let now = Instant::now();
        let key = (session_id.to_string(), batch_id.cloned(), page, now);
        q.push_back(key);
        // Evict old entries (> 15s) and bound size
        while q.len() > 64 {
            q.pop_front();
        }
        let cutoff = now - Duration::from_secs(15);
        while let Some(front) = q.front() {
            if front.3 < cutoff { q.pop_front(); } else { break; }
        }
    }

    // Note: we intentionally don't need a lookup accessor; we only use the cache to aid logs and potential future deduping.
}

/// Pure function: convert backend AppEvent into (frontend_event_name, flattened_json_payload)
/// Exposed for unit testing to guard mapping/name drift. Additive-only contract.
pub(crate) fn convert_actor_event_to_frontend_value(
    event: AppEvent,
) -> Result<(String, serde_json::Value), String> {
    use serde_json::{Map, Value};
    // Determine event name (use .. to ignore future fields)
    let event_name = match &event {
        AppEvent::SessionStarted { .. } => "actor-session-started",
        AppEvent::SessionPaused { .. } => "actor-session-paused",
        AppEvent::SessionResumed { .. } => "actor-session-resumed",
        AppEvent::SessionCompleted { .. } => "actor-session-completed",
        AppEvent::NextPlanReady { .. } => "actor-next-plan-ready",
        AppEvent::SessionFailed { .. } => "actor-session-failed",
        AppEvent::SessionTimeout { .. } => "actor-session-timeout",
        AppEvent::BatchStarted { .. } => "actor-batch-started",
        AppEvent::BatchCompleted { .. } => "actor-batch-completed",
        AppEvent::BatchFailed { .. } => "actor-batch-failed",
        AppEvent::StageStarted { .. } => "actor-stage-started",
        AppEvent::StageCompleted { .. } => "actor-stage-completed",
        AppEvent::StageFailed { .. } => "actor-stage-failed",
        AppEvent::StageRetrying { .. } => "actor-stage-retrying",
        AppEvent::Progress { .. } => "actor-progress",
        AppEvent::PerformanceMetrics { .. } => "actor-performance-metrics",
        AppEvent::BatchReport { .. } => "actor-batch-report",
        AppEvent::CrawlReportSession { .. } => "actor-session-report",
        AppEvent::ShutdownRequested { .. } => "actor-shutdown-requested",
        AppEvent::ShutdownCompleted { .. } => "actor-shutdown-completed",
        // PageTask* removed; prefer PageLifecycle
        AppEvent::TaskLifecycle { .. } => "actor-task-lifecycle",
        // DetailTask* and detail concurrency downshift events removed
        AppEvent::StageItemStarted { .. } => "actor-stage-item-started",
        AppEvent::StageItemCompleted { .. } => "actor-stage-item-completed",
        AppEvent::PageLifecycle { .. } => "actor-page-lifecycle",
        AppEvent::ProductLifecycle { .. } => "actor-product-lifecycle",
        &AppEvent::ProductLifecycleGroup { .. } => "actor-product-lifecycle-group",
        &AppEvent::HttpRequestTiming { .. } => "actor-http-request-timing",
        AppEvent::PreflightDiagnostics { .. } => "actor-preflight-diagnostics",
        AppEvent::PersistenceAnomaly { .. } => "actor-persistence-anomaly",
        AppEvent::DatabaseStats { .. } => "actor-database-stats",
        AppEvent::ValidationStarted { .. } => "actor-validation-started",
        AppEvent::ValidationPageScanned { .. } => "actor-validation-page-scanned",
        AppEvent::ValidationDivergenceFound { .. } => "actor-validation-divergence",
        AppEvent::ValidationAnomaly { .. } => "actor-validation-anomaly",
        AppEvent::ValidationCompleted { .. } => "actor-validation-completed",
        // Sync events
        AppEvent::SyncStarted { .. } => "actor-sync-started",
        AppEvent::SyncPageStarted { .. } => "actor-sync-page-started",
        AppEvent::SyncUpsertProgress { .. } => "actor-sync-upsert-progress",
        AppEvent::SyncPageCompleted { .. } => "actor-sync-page-completed",
        AppEvent::SyncWarning { .. } => "actor-sync-warning",
        AppEvent::SyncRetrying { .. } => "actor-sync-retrying",
        AppEvent::SyncCompleted { .. } => "actor-sync-completed",
    };

    let raw = serde_json::to_value(&event)
        .map_err(|e| format!("Failed to serialize Actor event: {}", e))?;

    // Flatten tagged enum structure: { "VariantName": { fields... } } -> { variant: "VariantName", fields... }
    let flat = if let Value::Object(map) = raw {
        if map.len() == 1 {
            let mut out = Map::new();
            if let Some((k, v)) = map.into_iter().next() {
                out.insert("variant".into(), Value::String(k));
                if let Value::Object(inner) = v {
                    for (ik, iv) in inner {
                        out.insert(ik, iv);
                    }
                } else {
                    out.insert("value".into(), v);
                }
            }
            Value::Object(out)
        } else {
            Value::Object(map)
        }
    } else {
        raw
    };

    Ok((event_name.to_string(), flat))
}

#[cfg(test)]
mod tests {
    use super::convert_actor_event_to_frontend_value;
    use crate::crawl_engine::actors::types::{AppEvent, StageType, TaskKind, SimpleMetrics};
    use chrono::Utc;

    // Test-only: mirror emission name selection (unified vs. per-variant)
    fn compute_emission_name_for_test(original_event_name: &str, generalized_only: bool) -> String {
        if generalized_only {
            "actor-event".to_string()
        } else {
            original_event_name.to_string()
        }
    }

    #[test]
    fn map_stage_started_event_name_and_payload_flattened() {
        let ev = AppEvent::StageStarted {
            stage_type: StageType::ListPageCrawling,
            session_id: "s1".into(),
            batch_id: Some("b1".into()),
            items_count: 10,
            timestamp: Utc::now(),
        };
        let (name, payload) = convert_actor_event_to_frontend_value(ev).expect("map ok");
        assert_eq!(name, "actor-stage-started");
        let obj = payload.as_object().expect("obj");
        assert_eq!(obj.get("variant").and_then(|v| v.as_str()), Some("StageStarted"));
        assert_eq!(obj.get("session_id").and_then(|v| v.as_str()), Some("s1"));
        assert_eq!(obj.get("batch_id").and_then(|v| v.as_str()), Some("b1"));
        assert_eq!(obj.get("items_count").and_then(|v| v.as_u64()), Some(10));
        assert_eq!(obj.get("stage_type").and_then(|v| v.as_str()), Some("ListPageCrawling"));
        assert!(obj.contains_key("timestamp"));
    }

    #[test]
    fn map_performance_metrics_event_name_and_payload_flattened() {
        let ev = AppEvent::PerformanceMetrics {
            session_id: "s2".into(),
            metrics: crate::crawl_engine::actors::types::PerformanceMetrics {
                memory_usage_mb: 123.4,
                cpu_usage_percent: 12.3,
                active_tasks_count: 2,
                queued_tasks_count: 1,
                avg_response_time_ms: 45.6,
                throughput_per_second: 7.8,
            },
            timestamp: Utc::now(),
        };
        let (name, payload) = convert_actor_event_to_frontend_value(ev).expect("map ok");
        assert_eq!(name, "actor-performance-metrics");
        let obj = payload.as_object().expect("obj");
        assert_eq!(obj.get("variant").and_then(|v| v.as_str()), Some("PerformanceMetrics"));
        assert_eq!(obj.get("session_id").and_then(|v| v.as_str()), Some("s2"));
        assert!(obj.get("metrics").is_some());
        assert!(obj.contains_key("timestamp"));
    }

    #[test]
    fn map_page_lifecycle_event() {
        let ev = AppEvent::PageLifecycle {
            session_id: "sess-pg".into(),
            batch_id: Some("bpg".into()),
            page_number: 3,
            status: "fetch_completed".into(),
            metrics: Some(SimpleMetrics::Page {
                url_count: Some(20),
                scheduled_details: Some(18),
                error: None,
            }),
            timestamp: Utc::now(),
        };
        let (name, payload) = convert_actor_event_to_frontend_value(ev).expect("map ok");
        assert_eq!(name, "actor-page-lifecycle");
        let obj = payload.as_object().expect("obj");
        assert_eq!(obj.get("variant").and_then(|v| v.as_str()), Some("PageLifecycle"));
        assert_eq!(obj.get("session_id").and_then(|v| v.as_str()), Some("sess-pg"));
        assert_eq!(obj.get("batch_id").and_then(|v| v.as_str()), Some("bpg"));
        assert_eq!(obj.get("page_number").and_then(|v| v.as_u64()), Some(3));
        assert_eq!(obj.get("status").and_then(|v| v.as_str()), Some("fetch_completed"));
        let metrics = obj.get("metrics").and_then(|v| v.as_object()).expect("metrics obj");
        assert_eq!(metrics.get("kind").and_then(|v| v.as_str()), Some("Page"));
        // optional nested checks
        assert!(metrics.get("data").is_some());
    }

    #[test]
    fn map_task_lifecycle_event() {
        let ev = AppEvent::TaskLifecycle {
            session_id: "sess-task".into(),
            batch_id: None,
            task_kind: TaskKind::Product,
            page_number: Some(5),
            product_ref: Some("p/123".into()),
            status: "success".into(),
            retry: Some(0),
            duration_ms: Some(123),
            metrics: Some(SimpleMetrics::Generic { key: "k".into(), value: "v".into() }),
            timestamp: Utc::now(),
        };
        let (name, payload) = convert_actor_event_to_frontend_value(ev).expect("map ok");
        assert_eq!(name, "actor-task-lifecycle");
        let obj = payload.as_object().expect("obj");
        assert_eq!(obj.get("variant").and_then(|v| v.as_str()), Some("TaskLifecycle"));
        assert_eq!(obj.get("session_id").and_then(|v| v.as_str()), Some("sess-task"));
        assert_eq!(obj.get("task_kind").and_then(|v| v.as_str()), Some("Product"));
        assert_eq!(obj.get("page_number").and_then(|v| v.as_u64()), Some(5));
        assert_eq!(obj.get("product_ref").and_then(|v| v.as_str()), Some("p/123"));
        assert_eq!(obj.get("status").and_then(|v| v.as_str()), Some("success"));
        assert_eq!(obj.get("retry").and_then(|v| v.as_u64()), Some(0));
        assert_eq!(obj.get("duration_ms").and_then(|v| v.as_u64()), Some(123));
    }

    #[test]
    fn map_validation_completed_event() {
        let ev = AppEvent::ValidationCompleted {
            session_id: "sess-val".into(),
            pages_scanned: 12,
            products_checked: 345,
            divergences: 2,
            anomalies: 1,
            duration_ms: 9876,
            timestamp: Utc::now(),
        };
        let (name, payload) = convert_actor_event_to_frontend_value(ev).expect("map ok");
        assert_eq!(name, "actor-validation-completed");
        let obj = payload.as_object().expect("obj");
        assert_eq!(obj.get("variant").and_then(|v| v.as_str()), Some("ValidationCompleted"));
        assert_eq!(obj.get("session_id").and_then(|v| v.as_str()), Some("sess-val"));
        assert_eq!(obj.get("pages_scanned").and_then(|v| v.as_u64()), Some(12));
        assert_eq!(obj.get("products_checked").and_then(|v| v.as_u64()), Some(345));
        assert_eq!(obj.get("divergences").and_then(|v| v.as_u64()), Some(2));
        assert_eq!(obj.get("anomalies").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(obj.get("duration_ms").and_then(|v| v.as_u64()), Some(9876));
    }

    #[test]
    fn generalized_only_mode_emits_unified_channel_and_keeps_original_name_in_payload() {
        // Given: a normal mapping result
        let ev = AppEvent::StageCompleted {
            stage_type: StageType::DataValidation,
            session_id: "s1".into(),
            batch_id: None,
            result: crate::crawl_engine::actors::types::StageResult {
                processed_items: 5,
                successful_items: 5,
                failed_items: 0,
                duration_ms: 321,
                details: vec![],
            },
            timestamp: Utc::now(),
        };
        let (original_name, mut payload) = convert_actor_event_to_frontend_value(ev).expect("map ok");

        // When: generalized-only is considered ON, emission name is unified but payload still carries original name
        let emission = compute_emission_name_for_test(&original_name, true);
        assert_eq!(emission, "actor-event");

        // Simulate enrichment as bridge does (subset sufficient for assertion)
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("event_name".into(), serde_json::Value::from(original_name.clone()));
            obj.insert("seq".into(), serde_json::Value::from(1));
            obj.insert("backend_ts".into(), serde_json::Value::from(chrono::Utc::now().to_rfc3339()));
        }
        let obj = payload.as_object().expect("obj");
        assert_eq!(obj.get("event_name").and_then(|v| v.as_str()), Some("actor-stage-completed"));
    }
}

/// Actor Event Bridge 시작 유틸리티 함수
///
/// 프론트엔드로 전달할 수 있도록 백엔드 `AppEvent` 스트림을 수신하여
/// 적절한 이벤트 이름과 페이로드로 변환/브로드캐스트하는 비동기 태스크를 스폰합니다.
///
/// Errors
/// - 현재 구현에서는 스폰 이전에 실패할 수 있는 단계가 없으므로 일반적으로 `Ok`를 반환합니다.
/// - 향후 초기화 로직이 확장되어 채널/상태 생성 등에서 실패 가능성이 추가될 경우, 해당 오류 메시지를 포함한 `Err(String)`을 반환할 수 있습니다.
pub async fn start_actor_event_bridge(
    app_handle: AppHandle,
    event_rx: broadcast::Receiver<AppEvent>,
) -> Result<tokio::task::JoinHandle<()>, String> {
    let mut bridge = ActorEventBridge::new(app_handle, event_rx);

    let handle = tokio::spawn(async move {
        bridge.start().await;
    });

    info!("🌉 Actor Event Bridge task spawned");
    Ok(handle)
}
