//! Actor 이벤트 프론트엔드 브릿지
//!
//! Actor 시스템의 `AppEvent를` 실제 Tauri 프론트엔드로 전달하는 브릿지 컴포넌트
//! 설계 의도: 각 Actor, Task 레벨에서 독립적으로 이벤트 발행을 가능하게 하여
//! 낮은 복잡성의 구현으로도 모든 경우를 다 커버할 수 있도록 함

use crate::crawl_engine::actors::types::{AppEvent, SimpleMetrics};
use crate::infrastructure::features::feature_events_generalized_only;
use std::collections::VecDeque;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
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
    #[must_use] pub fn new(app_handle: AppHandle, event_rx: broadcast::Receiver<AppEvent>) -> Self {
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
        // Generalized-only 모드: 단일 채널로 통일된 이벤트를 방출하고 종료
    if feature_events_generalized_only() {
            let unified_name = "actor-event";
            self.app_handle
                .emit(unified_name, &enriched)
                .map_err(|e| format!("Tauri emit failed: {}", e))?;
            // Also write a concise info-level line to events.log so stage/page/detail events are visible
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
            // Specialized concise lines per important variants to improve ProductDetail visibility
            match &actor_event {
                // Native TaskLifecycle is now emitted directly by actors; no synthetic re-emit needed
                AppEvent::ProductLifecycle { .. } => {}
                AppEvent::TaskLifecycle {
                    session_id,
                    batch_id,
                    task_kind,
                    page_number,
                    product_ref,
                    status,
                    duration_ms,
                    ..
                } => {
                    tracing::info!(target: "actor-event",
                        "[TaskLifecycle] kind={:?} status={} page={:?} ref={:?} dur_ms={:?} batch={:?} session={}",
                        task_kind, status, page_number, product_ref, duration_ms, batch_id, session_id
                    );
                }
                // ProductLifecycle logging is covered by synthetic TaskLifecycle above; keep concise log via that path
                AppEvent::ProductLifecycleGroup {
                    session_id,
                    batch_id,
                    page_number,
                    group_size,
                    started,
                    succeeded,
                    failed,
                    duplicates,
                    duration_ms,
                    phase,
                    ..
                } => {
                    tracing::info!(target: "actor-event",
                        "[ProductLifecycleGroup] phase={} size={} started={} ok={} fail={} dup={} page={:?} batch={:?} dur_ms={} session={}",
                        phase, group_size, started, succeeded, failed, duplicates, page_number, batch_id, duration_ms, session_id
                    );
                }
                // DetailTask* events deprecated and no longer emitted
                AppEvent::DatabaseStats {
                    session_id,
                    batch_id,
                    total_product_details,
                    min_page,
                    max_page,
                    note,
                    ..
                } => {
                    tracing::info!(target: "actor-event",
                        "[DatabaseStats] total={} range={:?}-{:?} note={:?} batch={:?} session={}",
                        total_product_details, min_page, max_page, note, batch_id, session_id
                    );
                }
                AppEvent::PageLifecycle {
                    session_id,
                    batch_id,
                    page_number,
                    status,
                    metrics,
                    ..
                } => {
                    // Record native PageLifecycle key in recent cache
                    self.push_recent_page(session_id, batch_id.as_ref(), *page_number).await;
                    // Extract a couple key metrics if available
                    let (urls, scheduled, err) = match metrics {
                        Some(SimpleMetrics::Page {
                            url_count,
                            scheduled_details,
                            error,
                        }) => (
                            url_count.unwrap_or(0),
                            scheduled_details.unwrap_or(0),
                            error.as_deref().unwrap_or(""),
                        ),
                        _ => (0, 0, ""),
                    };
                    tracing::info!(target: "actor-event",
                        "[PageLifecycle] status={} page={} urls={} scheduled={} err='{}' batch={:?} session={}",
                        status, page_number, urls, scheduled, err, batch_id, session_id
                    );
                }
                AppEvent::StageStarted {
                    stage_type,
                    session_id,
                    batch_id,
                    items_count,
                    ..
                } => {
                    tracing::info!(target: "actor-event",
                        "[Stage] started stage={} items={} batch={:?} session={}",
                        stage_type.as_str(), items_count, batch_id, session_id
                    );
                }
                AppEvent::StageCompleted {
                    stage_type,
                    session_id,
                    batch_id,
                    result,
                    ..
                } => {
                    tracing::info!(target: "actor-event",
                        "[Stage] completed stage={} processed={} ok={} fail={} dur_ms={} batch={:?} session={}",
                        stage_type.as_str(), result.processed_items, result.successful_items, result.failed_items, result.duration_ms, batch_id, session_id
                    );
                }
                _ => {}
            }
            debug!(
                "✅ Forwarded generalized Actor event '{}' (original={})",
                unified_name, event_name
            );
            return Ok(());
        }

        // 레거시 호환: 기존 이벤트명으로 전송
        self.app_handle
            .emit(&event_name, &enriched)
            .map_err(|e| format!("Tauri emit failed: {}", e))?;

        debug!("✅ Forwarded Actor event '{}' to Frontend", event_name);
        // Always emit a concise info-level line so users see forwarding even if debug is filtered.
        if let Some(obj) = enriched.as_object() {
            let variant = obj.get("variant").and_then(|v| v.as_str()).unwrap_or("?");
            let seq_val = obj.get("seq").and_then(serde_json::Value::as_u64).unwrap_or(0);
            let session_id = obj.get("session_id").and_then(|v| v.as_str());
            let batch_id = obj.get("batch_id").and_then(|v| v.as_str());
            // Route this concise line to events.log by using the dedicated target
            tracing::info!(target: "actor-event",
                "🌉 actor-event seq={} name={} variant={} session_id={:?} batch_id={:?}",
                seq_val, event_name, variant, session_id, batch_id
            );
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

        // 보강: CrawlReportSession 이 없고 SessionCompleted 로만 종료되는 경로(레거시 오케스트레이션 포함)를 위해
        // SessionCompleted(summary) 수신 시에도 메인 로그에 인간 친화적 요약을 남긴다.
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

        // 레거시 PageTask* 이벤트를 사용하는 경로(구 actor_system_commands 기반)에서도
        // UI가 통합된 actor-page-lifecycle 스트림을 받을 수 있도록 합성 이벤트 생성
        // 단, 새로운 파이프라인(StageActor)이 PageLifecycle을 직접 방출하는 경우에는 합성하지 않음
        if matches!(actor_event, AppEvent::PageLifecycle { .. }) {
            return Ok(());
        }
        if let Some((derived_name, mut derived_payload)) =
            self.create_synthetic_page_lifecycle(&actor_event).await
        {
            if let Some(obj) = derived_payload.as_object_mut() {
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
                    serde_json::Value::from(derived_name.clone()),
                );
            }
            if let Err(e) = self.app_handle.emit(&derived_name, &derived_payload) {
                warn!("Failed to emit synthetic page lifecycle event: {}", e);
            } else {
                debug!(
                    "✅ Emitted synthetic page lifecycle event '{}': {:?}",
                    derived_name, derived_payload
                );
            }
        }

        Ok(())
    }

    /// `AppEvent를` 프론트엔드 이벤트로 변환
    fn convert_actor_event_to_frontend(
        &self,
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
            AppEvent::PhaseStarted { .. } => "actor-phase-started",
            AppEvent::PhaseCompleted { .. } => "actor-phase-completed",
            AppEvent::PhaseAborted { .. } => "actor-phase-aborted",
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

    // NOTE: Legacy `CrawlingEvent` conversion removed. Frontend should consume unified `actor-event` only.

    /// 브릿지 상태 확인
    #[must_use] pub fn is_active(&self) -> bool {
        self.is_active.load(std::sync::atomic::Ordering::SeqCst)
    }

    // PageTask* removed; synthetic conversion no longer needed. If needed later, we could synthesize TaskLifecycle from Page/Product lifecycles.
    async fn create_synthetic_page_lifecycle(
        &self,
        _event: &AppEvent,
    ) -> Option<(String, serde_json::Value)> {
        None
    }

    // Build synthetic TaskLifecycle payload from PageLifecycle/ProductLifecycle
    fn create_task_lifecycle_payload(&self, event: &AppEvent) -> Option<serde_json::Value> {
    use serde_json::{Map, Value};
        match event {
            AppEvent::PageLifecycle { session_id, batch_id, page_number, status, metrics, timestamp } => {
                let mut obj = Map::new();
                obj.insert("variant".into(), Value::String("TaskLifecycle".into()));
                obj.insert("session_id".into(), Value::String(session_id.clone()));
                obj.insert("batch_id".into(), batch_id.as_ref().map(|s| Value::String(s.clone())).unwrap_or(Value::Null));
                obj.insert("task_kind".into(), Value::String("Page".into()));
                obj.insert("page_number".into(), Value::Number((*page_number).into()));
                obj.insert("product_ref".into(), Value::Null);
                obj.insert("status".into(), Value::String(status.clone()));
                obj.insert("retry".into(), Value::Null);
                obj.insert("duration_ms".into(), Value::Null);
                let m = metrics.as_ref().map(|m| serde_json::to_value(m).unwrap_or(Value::Null)).unwrap_or(Value::Null);
                obj.insert("metrics".into(), m);
                obj.insert("timestamp".into(), Value::String(timestamp.to_rfc3339()));
                Some(Value::Object(obj))
            }
            AppEvent::ProductLifecycle { session_id, batch_id, page_number, product_ref, status, retry, duration_ms, metrics, timestamp } => {
                let mut obj = Map::new();
                obj.insert("variant".into(), Value::String("TaskLifecycle".into()));
                obj.insert("session_id".into(), Value::String(session_id.clone()));
                obj.insert("batch_id".into(), batch_id.as_ref().map(|s| Value::String(s.clone())).unwrap_or(Value::Null));
                obj.insert("task_kind".into(), Value::String("Product".into()));
                obj.insert("page_number".into(), page_number.map(|n| Value::Number(n.into())).unwrap_or(Value::Null));
                obj.insert("product_ref".into(), Value::String(product_ref.clone()));
                obj.insert("status".into(), Value::String(status.clone()));
                obj.insert("retry".into(), retry.map(|n| Value::Number(n.into())).unwrap_or(Value::Null));
                obj.insert("duration_ms".into(), duration_ms.map(|n| Value::Number(n.into())).unwrap_or(Value::Null));
                let m = metrics.as_ref().map(|m| serde_json::to_value(m).unwrap_or(Value::Null)).unwrap_or(Value::Null);
                obj.insert("metrics".into(), m);
                obj.insert("timestamp".into(), Value::String(timestamp.to_rfc3339()));
                Some(Value::Object(obj))
            }
            _ => None,
        }
    }

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

    async fn is_recent_page(&self, session_id: &str, batch_id: Option<&String>, page: u32) -> bool {
        use std::time::{Duration, Instant};
        let q = self.recent_pages.lock().await;
        let cutoff = Instant::now() - Duration::from_secs(15);
        q.iter().rev().take(64).any(|(s, b, p, t)| s == session_id && b.as_ref() == batch_id && *p == page && *t >= cutoff)
    }
}

/// Actor Event Bridge 시작 유틸리티 함수
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
