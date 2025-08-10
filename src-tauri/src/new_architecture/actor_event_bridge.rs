//! Actor 이벤트 프론트엔드 브릿지
//! 
//! Actor 시스템의 AppEvent를 실제 Tauri 프론트엔드로 전달하는 브릿지 컴포넌트
//! 설계 의도: 각 Actor, Task 레벨에서 독립적으로 이벤트 발행을 가능하게 하여 
//! 낮은 복잡성의 구현으로도 모든 경우를 다 커버할 수 있도록 함

use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use tokio::sync::broadcast;
use tauri::{AppHandle, Emitter};
use tracing::{info, warn, error, debug};
use crate::new_architecture::actors::types::AppEvent;
use crate::domain::events::CrawlingEvent;

/// Actor 이벤트를 프론트엔드로 전달하는 브릿지
pub struct ActorEventBridge {
    /// Tauri AppHandle
    app_handle: AppHandle,
    /// Actor 이벤트 수신기
    event_rx: broadcast::Receiver<AppEvent>,
    /// 브릿지 활성화 상태
    is_active: Arc<std::sync::atomic::AtomicBool>,
    /// 단조 증가 시퀀스 번호
    seq: Arc<AtomicU64>,
}

impl ActorEventBridge {
    /// 새로운 브릿지 생성
    pub fn new(app_handle: AppHandle, event_rx: broadcast::Receiver<AppEvent>) -> Self {
        Self {
            app_handle,
            event_rx,
            is_active: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            seq: Arc::new(AtomicU64::new(1)),
        }
    }

    /// 브릿지 시작 - Actor 이벤트를 프론트엔드로 전달
    pub async fn start(&mut self) {
        if self.is_active.swap(true, std::sync::atomic::Ordering::SeqCst) {
            warn!("ActorEventBridge is already running");
            return;
        }

        info!("🌉 Starting Actor Event Bridge - connecting Actor events to Frontend");

        while self.is_active.load(std::sync::atomic::Ordering::SeqCst) {
            match self.event_rx.recv().await {
                Ok(actor_event) => {
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

        self.is_active.store(false, std::sync::atomic::Ordering::SeqCst);
        info!("🌉 Actor Event Bridge stopped");
    }

    /// 브릿지 중지
    pub fn stop(&self) {
        self.is_active.store(false, std::sync::atomic::Ordering::SeqCst);
    }

    /// Actor 이벤트를 프론트엔드로 전달
    async fn forward_to_frontend(&self, actor_event: AppEvent) -> Result<(), String> {
        // AppEvent를 프론트엔드가 이해할 수 있는 형태로 변환
        let (event_name, event_data) = self.convert_actor_event_to_frontend(actor_event)?;
        
        // 시퀀스 & backend_ts 주입 (RFC3339)
        let enriched = {
            let mut v = event_data;
            if let Some(obj) = v.as_object_mut() {
                obj.insert("seq".into(), serde_json::Value::from(self.seq.fetch_add(1, Ordering::SeqCst)));
                obj.insert("backend_ts".into(), serde_json::Value::from(chrono::Utc::now().to_rfc3339()));
                obj.insert("event_name".into(), serde_json::Value::from(event_name.clone()));
            }
            v
        };

        // Tauri emit을 통해 프론트엔드로 전송
        self.app_handle
            .emit(&event_name, &enriched)
            .map_err(|e| format!("Tauri emit failed: {}", e))?;

        debug!("✅ Forwarded Actor event '{}' to Frontend", event_name);
        // Always emit a concise info-level line so users see forwarding even if debug is filtered.
        if let Some(obj) = enriched.as_object() {
            let variant = obj.get("variant").and_then(|v| v.as_str()).unwrap_or("?");
            let seq_val = obj.get("seq").and_then(|v| v.as_u64()).unwrap_or(0);
            let session_id = obj.get("session_id").and_then(|v| v.as_str());
            let batch_id = obj.get("batch_id").and_then(|v| v.as_str());
            info!(
                "🌉 actor-event seq={} name={} variant={} session_id={:?} batch_id={:?}",
                seq_val, event_name, variant, session_id, batch_id
            );
        }
        Ok(())
    }

    /// AppEvent를 프론트엔드 이벤트로 변환
    fn convert_actor_event_to_frontend(&self, event: AppEvent) -> Result<(String, serde_json::Value), String> {
        use serde_json::{Value, Map};
        // Determine event name (use .. to ignore future fields)
        let event_name = match &event {
            AppEvent::SessionStarted { .. } => "actor-session-started",
            AppEvent::SessionPaused { .. } => "actor-session-paused",
            AppEvent::SessionResumed { .. } => "actor-session-resumed",
            AppEvent::SessionCompleted { .. } => "actor-session-completed",
            AppEvent::SessionFailed { .. } => "actor-session-failed",
            AppEvent::SessionTimeout { .. } => "actor-session-timeout",
            AppEvent::BatchStarted { .. } => "actor-batch-started",
            AppEvent::BatchCompleted { .. } => "actor-batch-completed",
            AppEvent::BatchFailed { .. } => "actor-batch-failed",
            AppEvent::StageStarted { .. } => "actor-stage-started",
            AppEvent::StageCompleted { .. } => "actor-stage-completed",
            AppEvent::StageFailed { .. } => "actor-stage-failed",
            AppEvent::Progress { .. } => "actor-progress",
            AppEvent::PerformanceMetrics { .. } => "actor-performance-metrics",
            AppEvent::BatchReport { .. } => "actor-batch-report",
            AppEvent::CrawlReportSession { .. } => "actor-session-report",
            AppEvent::PhaseStarted { .. } => "actor-phase-started",
            AppEvent::PhaseCompleted { .. } => "actor-phase-completed",
            AppEvent::PhaseAborted { .. } => "actor-phase-aborted",
            AppEvent::ShutdownRequested { .. } => "actor-shutdown-requested",
            AppEvent::ShutdownCompleted { .. } => "actor-shutdown-completed",
            AppEvent::PageTaskStarted { .. } => "actor-page-task-started",
            AppEvent::PageTaskCompleted { .. } => "actor-page-task-completed",
            AppEvent::PageTaskFailed { .. } => "actor-page-task-failed",
            AppEvent::DetailTaskStarted { .. } => "actor-detail-task-started",
            AppEvent::DetailTaskCompleted { .. } => "actor-detail-task-completed",
            AppEvent::DetailTaskFailed { .. } => "actor-detail-task-failed",
            AppEvent::DetailConcurrencyDownshifted { .. } => "actor-detail-concurrency-downshifted",
        };

        let raw = serde_json::to_value(&event)
            .map_err(|e| format!("Failed to serialize Actor event: {}", e))?;

        // Flatten tagged enum structure: { "VariantName": { fields... } } -> { variant: "VariantName", fields... }
        let flat = if let Value::Object(map) = raw {
            if map.len() == 1 {
                let mut out = Map::new();
                if let Some((k,v)) = map.into_iter().next() {
                    out.insert("variant".into(), Value::String(k.clone()));
                    if let Value::Object(inner) = v {
                        for (ik,iv) in inner.into_iter() { out.insert(ik, iv); }
                    } else {
                        out.insert("value".into(), v);
                    }
                }
                Value::Object(out)
            } else {
                Value::Object(map)
            }
        } else { raw };

        Ok((event_name.to_string(), flat))
    }

    /// CrawlingEvent 호환성을 위한 변환 (필요시)
    #[allow(dead_code)]
    fn convert_to_crawling_event(&self, actor_event: &AppEvent) -> Option<CrawlingEvent> {
        match actor_event {
            AppEvent::SessionStarted { session_id, .. } => {
                Some(CrawlingEvent::SessionEvent {
                    session_id: session_id.clone(),
                    event_type: crate::domain::events::SessionEventType::Started,
                    message: "Actor session started".to_string(),
                    timestamp: chrono::Utc::now(),
                })
            }
            AppEvent::SessionCompleted {  summary, .. } => {
                let result = crate::domain::events::CrawlingResult {
                    total_processed: summary.total_pages_processed,
                    new_items: summary.total_pages_processed, // TODO: 실제 새 아이템 수
                    updated_items: 0, // TODO: 실제 업데이트된 아이템 수
                    errors: 0, // TODO: 실제 에러 수
                    duration_ms: summary.total_duration_ms,
                    stages_completed: vec![], // TODO: 완료된 스테이지들
                    start_time: chrono::Utc::now() - chrono::Duration::milliseconds(summary.total_duration_ms as i64),
                    end_time: chrono::Utc::now(),
                    performance_metrics: crate::domain::events::PerformanceMetrics {
                        avg_processing_time_ms: summary.avg_page_processing_time as f64,
                        items_per_second: if summary.total_duration_ms > 0 {
                            (summary.total_pages_processed as f64 * 1000.0) / summary.total_duration_ms as f64
                        } else { 0.0 },
                        memory_usage_mb: 0.0, // TODO: 실제 메모리 사용량
                        network_requests: summary.total_pages_processed as u64, // 근사치
                        cache_hit_rate: 0.0, // TODO: 실제 캐시 히트율
                    },
                };
                Some(CrawlingEvent::Completed(result))
            }
            AppEvent::Progress {  current_step, total_steps, percentage, message, .. } => {
                let progress = crate::domain::events::CrawlingProgress {
                    current: *current_step,
                    total: *total_steps,
                    percentage: *percentage,
                    current_stage: crate::domain::events::CrawlingStage::ProductList, // 진행 중이므로 ProductList 단계로 가정
                    current_step: message.clone(),
                    status: crate::domain::events::CrawlingStatus::Running,
                    message: format!("Processing step {} of {}", current_step, total_steps),
                    remaining_time: None,
                    elapsed_time: 0, // TODO: 실제 경과 시간
                    new_items: 0,
                    updated_items: 0,
                    current_batch: None,
                    total_batches: None,
                    errors: 0,
                    timestamp: chrono::Utc::now(),
                };
                Some(CrawlingEvent::ProgressUpdate(progress))
            }
            _ => None, // 다른 이벤트들은 필요시 추가
        }
    }

    /// 브릿지 상태 확인
    pub fn is_active(&self) -> bool {
        self.is_active.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Actor Event Bridge 시작 유틸리티 함수
pub async fn start_actor_event_bridge(
    app_handle: AppHandle, 
    event_rx: broadcast::Receiver<AppEvent>
) -> Result<tokio::task::JoinHandle<()>, String> {
    let mut bridge = ActorEventBridge::new(app_handle, event_rx);
    
    let handle = tokio::spawn(async move {
        bridge.start().await;
    });

    info!("🌉 Actor Event Bridge task spawned");
    Ok(handle)
}
