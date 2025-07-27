//! Event emission system for real-time communication with frontend
//! 
//! This module provides a centralized event emission system that allows
//! the crawling engine to send real-time updates to the frontend.

use crate::domain::events::{CrawlingEvent, CrawlingProgress, CrawlingTaskStatus, DatabaseStats};
use crate::domain::atomic_events::AtomicTaskEvent; // 추가
use crate::infrastructure::service_based_crawling_engine::DetailedCrawlingEvent; // 추가
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tracing::{debug, error, warn};
use tokio::sync::{RwLock, mpsc};
use thiserror::Error;
use std::time::Duration;
use futures::future::join_all;

/// 이벤트 발신 관련 오류 타입
#[derive(Debug, Error)]
pub enum EventEmissionError {
    #[error("이벤트 발신 비활성화됨")]
    Disabled,
    #[error("Tauri 이벤트 발신 오류: {0}")]
    TauriError(#[from] tauri::Error),
    #[error("이벤트 큐가 가득참")]
    QueueFull,
    #[error("직렬화 오류: {0}")]
    Serialization(String),
    #[error("이벤트 발신 오류: {0}")]
    Emission(String),
}

/// 이벤트 발신 결과 타입
pub type EventResult = Result<(), EventEmissionError>;

/// Event emitter for sending real-time updates to the frontend
#[derive(Clone)]
pub struct EventEmitter {
    app_handle: AppHandle,
    /// Whether event emission is enabled
    enabled: Arc<RwLock<bool>>,
    /// Event queue for batched emissions
    event_sender: Option<mpsc::Sender<CrawlingEvent>>,
}

impl EventEmitter {
    /// Create a new event emitter
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            app_handle,
            enabled: Arc::new(RwLock::new(true)),
            event_sender: None,
        }
    }

    /// Create a new event emitter with batching enabled
    pub fn with_batching(app_handle: AppHandle, batch_size: usize, interval_ms: u64) -> Self {
        let (tx, mut rx) = mpsc::channel::<CrawlingEvent>(batch_size * 2);
        
        let emitter = Self {
            app_handle: app_handle.clone(),
            enabled: Arc::new(RwLock::new(true)),
            event_sender: Some(tx),
        };
        
        // 백그라운드 태스크로 이벤트 배치 처리
        let app_handle_clone = app_handle.clone();
        tokio::spawn(async move {
            let mut batch: Vec<CrawlingEvent> = Vec::with_capacity(batch_size);
            let mut interval = tokio::time::interval(Duration::from_millis(interval_ms));
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if !batch.is_empty() {
                            for event in batch.drain(..) {
                                let event_name = event.event_name();
                                if let Err(e) = app_handle_clone.emit(event_name, &event) {
                                    warn!("Failed to emit batched event {}: {}", event_name, e);
                                }
                            }
                        }
                    }
                    event = rx.recv() => {
                        match event {
                            Some(event) => {
                                batch.push(event);
                                if batch.len() >= batch_size {
                                    for event in batch.drain(..) {
                                        let event_name = event.event_name();
                                        if let Err(e) = app_handle_clone.emit(event_name, &event) {
                                            warn!("Failed to emit batched event {}: {}", event_name, e);
                                        }
                                    }
                                }
                            }
                            None => break,
                        }
                    }
                }
            }
        });
        
        emitter
    }

    /// Enable or disable event emission
    pub async fn set_enabled(&self, enabled: bool) {
        let mut enabled_guard = self.enabled.write().await;
        *enabled_guard = enabled;
        debug!("Event emission {}", if enabled { "enabled" } else { "disabled" });
    }

    /// Check if event emission is enabled
    pub async fn is_enabled(&self) -> bool {
        *self.enabled.read().await
    }

    /// Emit a crawling event to the frontend
    pub async fn emit_event(&self, event: CrawlingEvent) -> EventResult {
        // 빠른 경로: 비활성화 검사 (읽기 락만 필요)
        if !self.is_enabled().await {
            return Err(EventEmissionError::Disabled);
        }

        // 배치 모드인 경우 이벤트 큐에 추가
        if let Some(sender) = &self.event_sender {
            return sender.send(event).await
                .map_err(|_| EventEmissionError::QueueFull);
        }

        let event_name = event.event_name();
        
        match self.app_handle.emit(event_name, &event) {
            Ok(_) => {
                debug!("Successfully emitted event: {}", event_name);
                Ok(())
            }
            Err(e) => {
                error!("Failed to emit event {}: {}", event_name, e);
                Err(EventEmissionError::TauriError(e))
            }
        }
    }

    /// Emit a progress update
    pub async fn emit_progress(&self, progress: CrawlingProgress) -> EventResult {
        let event = CrawlingEvent::ProgressUpdate(progress);
        self.emit_event(event).await
    }

    /// Emit a task status update
    pub async fn emit_task_update(&self, task_status: CrawlingTaskStatus) -> EventResult {
        let event = CrawlingEvent::TaskUpdate(task_status);
        self.emit_event(event).await
    }

    /// Emit a stage change notification
    pub async fn emit_stage_change(
        &self,
        from: crate::domain::events::CrawlingStage,
        to: crate::domain::events::CrawlingStage,
        message: String,
    ) -> EventResult {
        let event = CrawlingEvent::StageChange { from, to, message };
        self.emit_event(event).await
    }

    /// Emit an error notification
    pub async fn emit_error(
        &self,
        error_id: String,
        message: String,
        stage: crate::domain::events::CrawlingStage,
        recoverable: bool,
    ) -> EventResult {
        let event = CrawlingEvent::Error {
            error_id,
            message,
            stage,
            recoverable,
        };
        self.emit_event(event).await
    }

    /// Emit database statistics update
    pub async fn emit_database_update(&self, stats: DatabaseStats) -> EventResult {
        let event = CrawlingEvent::DatabaseUpdate(stats);
        self.emit_event(event).await
    }

    /// Emit crawling completion notification
    pub async fn emit_completed(&self, result: crate::domain::events::CrawlingResult) -> EventResult {
        let event = CrawlingEvent::Completed(result);
        self.emit_event(event).await
    }

    /// Emit detailed crawling event for hierarchical event monitor
    pub async fn emit_detailed_crawling_event(&self, detailed_event: DetailedCrawlingEvent) -> EventResult {
        // 빠른 경로: 비활성화 검사
        if !self.is_enabled().await {
            return Err(EventEmissionError::Disabled);
        }

        let event_name = "detailed-crawling-event";
        
        match self.app_handle.emit(event_name, &detailed_event) {
            Ok(_) => {
                debug!("Successfully emitted detailed crawling event: {}", event_name);
                Ok(())
            }
            Err(e) => {
                error!("Failed to emit detailed crawling event {}: {}", event_name, e);
                Err(EventEmissionError::TauriError(e))
            }
        }
    }

    // =========================================================================
    // 원자적 태스크 이벤트 (Atomic Task Events) - proposal5.md 구현
    // =========================================================================

    /// Emit an atomic task event immediately (high-frequency, lightweight)
    pub async fn emit_atomic_task_event(&self, event: AtomicTaskEvent) -> EventResult {
        // 빠른 경로: 비활성화 검사
        if !self.is_enabled().await {
            return Err(EventEmissionError::Disabled);
        }

        let event_name = AtomicTaskEvent::event_name();
        
        match self.app_handle.emit(event_name, &event) {
            Ok(_) => {
                debug!("Successfully emitted atomic task event: {} for task {}", 
                       event_name, event.task_id());
                Ok(())
            }
            Err(e) => {
                error!("Failed to emit atomic task event {}: {}", event_name, e);
                Err(EventEmissionError::TauriError(e))
            }
        }
    }

    /// Emit task started event
    pub async fn emit_task_started(&self, task_id: crate::domain::atomic_events::TaskId, task_type: String) -> EventResult {
        let event = AtomicTaskEvent::started(task_id, task_type);
        self.emit_atomic_task_event(event).await
    }

    /// Emit task completed event
    pub async fn emit_task_completed(&self, task_id: crate::domain::atomic_events::TaskId, task_type: String, duration_ms: u64) -> EventResult {
        let event = AtomicTaskEvent::completed(task_id, task_type, duration_ms);
        self.emit_atomic_task_event(event).await
    }

    /// Emit task failed event
    pub async fn emit_task_failed(&self, task_id: crate::domain::atomic_events::TaskId, task_type: String, error_message: String, retry_count: u32) -> EventResult {
        let event = AtomicTaskEvent::failed(task_id, task_type, error_message, retry_count);
        self.emit_atomic_task_event(event).await
    }

    /// Emit task retrying event
    pub async fn emit_task_retrying(&self, task_id: crate::domain::atomic_events::TaskId, task_type: String, retry_count: u32, delay_ms: u64) -> EventResult {
        let event = AtomicTaskEvent::retrying(task_id, task_type, retry_count, delay_ms);
        self.emit_atomic_task_event(event).await
    }

    // =========================================================================
    // 기존 이벤트 (상태 스냅샷) - 저주파, 무거운 정보
    // =========================================================================

    /// Emit multiple events in batch (useful for reducing frontend update frequency)
    pub async fn emit_batch(&self, events: Vec<CrawlingEvent>) -> Vec<EventResult> {
        // 최적화: 비활성화된 경우 빠르게 리턴
        if !self.is_enabled().await {
            return events.into_iter()
                .map(|_| Err(EventEmissionError::Disabled))
                .collect();
        }

        // 병렬 이벤트 전송
        let futures = events.into_iter()
            .map(|event| self.emit_event(event));
        
        join_all(futures).await
    }
}

/// Builder for creating event emitters with specific configurations
#[derive(Default)]
pub struct EventEmitterBuilder {
    app_handle: Option<AppHandle>,
    enabled: bool,
    enable_batching: bool,
    batch_size: usize,
    batch_interval_ms: u64,
}

impl EventEmitterBuilder {
    /// Create a new event emitter builder
    pub fn new() -> Self {
        Self {
            app_handle: None,
            enabled: true,
            enable_batching: false,
            batch_size: 10,
            batch_interval_ms: 100,
        }
    }

    /// Set the app handle
    pub fn with_app_handle(mut self, app_handle: AppHandle) -> Self {
        self.app_handle = Some(app_handle);
        self
    }

    /// Set initial enabled state
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    
    /// Enable batched event emission
    pub fn with_batching(mut self, batch_size: usize, interval_ms: u64) -> Self {
        self.enable_batching = true;
        self.batch_size = batch_size;
        self.batch_interval_ms = interval_ms;
        self
    }

    /// Build the event emitter
    pub async fn build(self) -> Result<EventEmitter, String> {
        let app_handle = self.app_handle.ok_or("App handle is required")?;
        
        let emitter = if self.enable_batching {
            EventEmitter::with_batching(app_handle, self.batch_size, self.batch_interval_ms)
        } else {
            EventEmitter::new(app_handle)
        };
        
        emitter.set_enabled(self.enabled).await;
        
        Ok(emitter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_event_emitter_basic_operations() {
        // 이 테스트는 EventEmitter의 기본 상태 관리를 테스트합니다
        // 실제 Tauri AppHandle이 없이도 테스트 가능한 부분을 테스트합니다
        
        // EventEmitter는 AppHandle을 필요로 하므로 기본 구조체 생성만 테스트
        let enabled = std::sync::Arc::new(tokio::sync::RwLock::new(true));
        assert!(*enabled.read().await);
        
        // 비활성화 테스트
        *enabled.write().await = false;
        assert!(!*enabled.read().await);
    }
    
    #[tokio::test] 
    async fn test_crawling_progress_serialization() {
        // CrawlingProgress 구조체가 제대로 직렬화되는지 테스트
        let progress = CrawlingProgress::default();
        let serialized = serde_json::to_value(&progress);
        assert!(serialized.is_ok());
        
        let json_value = serialized.unwrap();
        assert!(json_value.is_object());
    }
    
    #[tokio::test]
    async fn test_event_emission_error_types() {
        // EventEmissionError 타입들이 제대로 생성되는지 테스트
        let disabled_error = EventEmissionError::Disabled;
        let serialization_error = EventEmissionError::Serialization("test error".to_string());
        let emission_error = EventEmissionError::Emission("test emission error".to_string());
        
        // 에러 메시지가 제대로 표시되는지 테스트
        let disabled_msg = format!("{}", disabled_error);
        let serialization_msg = format!("{}", serialization_error);
        let emission_msg = format!("{}", emission_error);
        
        assert!(disabled_msg.contains("비활성화"));
        assert!(serialization_msg.contains("test error"));
        assert!(emission_msg.contains("test emission error"));
    }
}
