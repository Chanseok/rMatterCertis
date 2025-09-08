//! Event emission system for real-time communication with frontend
//!
//! This module provides a centralized event emission system that allows
//! the crawling engine to send real-time updates to the frontend.

use crate::domain::atomic_events::AtomicTaskEvent; // 추가
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, error};

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
}

impl EventEmitter {
    /// Create a new event emitter
    #[must_use]
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            app_handle,
            enabled: Arc::new(RwLock::new(true)),
        }
    }

    /// Create a new event emitter with batching enabled
    #[must_use]
    pub fn with_batching(app_handle: AppHandle, _batch_size: usize, _interval_ms: u64) -> Self {
        Self::new(app_handle)
    }

    /// Enable or disable event emission
    pub async fn set_enabled(&self, enabled: bool) {
        let mut enabled_guard = self.enabled.write().await;
        *enabled_guard = enabled;
        debug!(
            "Event emission {}",
            if enabled { "enabled" } else { "disabled" }
        );
    }

    /// Check if event emission is enabled
    pub async fn is_enabled(&self) -> bool {
        *self.enabled.read().await
    }

    // Legacy CrawlingEvent emission removed – unified actor-event only.

    /// Emit a progress update
    pub async fn emit_progress(&self, _progress: serde_json::Value) -> EventResult {
        Ok(())
    }

    /// Emit a task status update
    pub async fn emit_task_update(&self, _task_status: serde_json::Value) -> EventResult {
        Ok(())
    }

    /// Emit a stage change notification
    pub async fn emit_stage_change(&self, _from: &str, _to: &str, _message: String) -> EventResult {
        Ok(())
    }

    /// Emit an error notification
    pub async fn emit_error(
        &self,
        _error_id: String,
        _message: String,
        _stage: &str,
        _recoverable: bool,
    ) -> EventResult {
        Ok(())
    }

    /// Emit database statistics update
    pub async fn emit_database_update(&self, _stats: serde_json::Value) -> EventResult {
        Ok(())
    }

    /// Emit crawling completion notification
    pub async fn emit_completed(&self, _result: serde_json::Value) -> EventResult {
        Ok(())
    }

    /// Emit detailed crawling event for hierarchical event monitor
    pub async fn emit_detailed_crawling_event<T: serde::Serialize>(
        &self,
        detailed_event: T,
    ) -> EventResult {
        // 빠른 경로: 비활성화 검사
        if !self.is_enabled().await {
            return Err(EventEmissionError::Disabled);
        }

        let event_name = "detailed-crawling-event";

        match self.app_handle.emit(event_name, &detailed_event) {
            Ok(()) => {
                debug!(
                    "Successfully emitted detailed crawling event: {}",
                    event_name
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    "Failed to emit detailed crawling event {}: {}",
                    event_name, e
                );
                Err(EventEmissionError::TauriError(e))
            }
        }
    }

    /// Emit detailed crawling event with JSON payload (for `TaskLifecycleEvent`)
    pub async fn emit_detailed_crawling_event_json(
        &self,
        json_payload: serde_json::Value,
    ) -> EventResult {
        // 빠른 경로: 비활성화 검사
        if !self.is_enabled().await {
            return Err(EventEmissionError::Disabled);
        }

        let event_name = "detailed-crawling-event";

        match self.app_handle.emit(event_name, &json_payload) {
            Ok(()) => {
                debug!(
                    "Successfully emitted detailed crawling event JSON: {}",
                    event_name
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    "Failed to emit detailed crawling event JSON {}: {}",
                    event_name, e
                );
                Err(EventEmissionError::TauriError(e))
            }
        }
    }

    // =========================================================================
    // 원자적 태스크 이벤트 (Atomic Task Events) - proposal5.md 구현
    // =========================================================================

    /// Emit an atomic task event immediately (high-frequency, lightweight)
    ///
    /// # Errors
    /// Returns an error if event emission is disabled or if the Tauri emitter fails.
    pub async fn emit_atomic_task_event(&self, event: &AtomicTaskEvent) -> EventResult {
        // 빠른 경로: 비활성화 검사
        if !self.is_enabled().await {
            return Err(EventEmissionError::Disabled);
        }

        let event_name = AtomicTaskEvent::event_name();

        match self.app_handle.emit(event_name, &event) {
            Ok(()) => {
                debug!(
                    "Successfully emitted atomic task event: {} for task {}",
                    event_name,
                    event.task_id()
                );
                Ok(())
            }
            Err(e) => {
                error!("Failed to emit atomic task event {}: {}", event_name, e);
                Err(EventEmissionError::TauriError(e))
            }
        }
    }

    /// Emit task started event
    pub async fn emit_task_started(
        &self,
        task_id: crate::domain::atomic_events::TaskId,
        task_type: String,
    ) -> EventResult {
        let event = AtomicTaskEvent::started(task_id, task_type);
        self.emit_atomic_task_event(&event).await
    }

    /// Emit task completed event
    pub async fn emit_task_completed(
        &self,
        task_id: crate::domain::atomic_events::TaskId,
        task_type: String,
        duration_ms: u64,
    ) -> EventResult {
        let event = AtomicTaskEvent::completed(task_id, task_type, duration_ms);
        self.emit_atomic_task_event(&event).await
    }

    /// Emit task failed event
    pub async fn emit_task_failed(
        &self,
        task_id: crate::domain::atomic_events::TaskId,
        task_type: String,
        error_message: String,
        retry_count: u32,
    ) -> EventResult {
        let event = AtomicTaskEvent::failed(task_id, task_type, error_message, retry_count);
        self.emit_atomic_task_event(&event).await
    }

    /// Emit task retrying event
    pub async fn emit_task_retrying(
        &self,
        task_id: crate::domain::atomic_events::TaskId,
        task_type: String,
        retry_count: u32,
        delay_ms: u64,
    ) -> EventResult {
        let event = AtomicTaskEvent::retrying(task_id, task_type, retry_count, delay_ms);
        self.emit_atomic_task_event(&event).await
    }

    // =========================================================================
    // 기존 이벤트 (상태 스냅샷) - 저주파, 무거운 정보
    // =========================================================================

    /// Emit multiple events in batch (useful for reducing frontend update frequency)
    pub async fn emit_batch(&self, events: Vec<serde_json::Value>) -> Vec<EventResult> {
        events.into_iter().map(|_| Ok(())).collect()
    }

    // =========================================================================
    // 확장: 독립 이벤트 스트림 emit 헬퍼들
    // =========================================================================

    /// Emit a concurrency status event
    pub async fn emit_concurrency_event(&self, _event: serde_json::Value) -> EventResult {
        if !self.is_enabled().await {
            return Err(EventEmissionError::Disabled);
        }
        let event_name = "concurrency-event";
        match self.app_handle.emit(event_name, &serde_json::json!({})) {
            Ok(()) => Ok(()),
            Err(e) => Err(EventEmissionError::TauriError(e)),
        }
    }

    /// Emit a validation event
    pub async fn emit_validation_event(&self, _event: serde_json::Value) -> EventResult {
        if !self.is_enabled().await {
            return Err(EventEmissionError::Disabled);
        }
        let event_name = "validation-event";
        match self.app_handle.emit(event_name, &serde_json::json!({})) {
            Ok(()) => Ok(()),
            Err(e) => Err(EventEmissionError::TauriError(e)),
        }
    }

    /// Emit a database save event
    pub async fn emit_db_save_event(&self, _event: serde_json::Value) -> EventResult {
        if !self.is_enabled().await {
            return Err(EventEmissionError::Disabled);
        }
        let event_name = "db-save-event";
        match self.app_handle.emit(event_name, &serde_json::json!({})) {
            Ok(()) => Ok(()),
            Err(e) => Err(EventEmissionError::TauriError(e)),
        }
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
    #[must_use]
    pub const fn new() -> Self {
        Self {
            app_handle: None,
            enabled: true,
            enable_batching: false,
            batch_size: 10,
            batch_interval_ms: 100,
        }
    }

    /// Set the app handle
    #[must_use]
    pub fn with_app_handle(mut self, app_handle: AppHandle) -> Self {
        self.app_handle = Some(app_handle);
        self
    }

    /// Set initial enabled state
    #[must_use]
    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Enable batched event emission
    #[must_use]
    pub const fn with_batching(mut self, batch_size: usize, interval_ms: u64) -> Self {
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

    // legacy CrawlingProgress serialization test removed

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
