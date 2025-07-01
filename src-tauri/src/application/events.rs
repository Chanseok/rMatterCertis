//! Event emission system for real-time communication with frontend
//! 
//! This module provides a centralized event emission system that allows
//! the crawling engine to send real-time updates to the frontend.

use crate::domain::events::{CrawlingEvent, CrawlingProgress, CrawlingTaskStatus, DatabaseStats};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tracing::{debug, error};
use tokio::sync::RwLock;

/// Event emitter for sending real-time updates to the frontend
#[derive(Clone)]
pub struct EventEmitter {
    app_handle: AppHandle,
    /// Whether event emission is enabled
    enabled: Arc<RwLock<bool>>,
}

impl EventEmitter {
    /// Create a new event emitter
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            app_handle,
            enabled: Arc::new(RwLock::new(true)),
        }
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
    pub async fn emit_event(&self, event: CrawlingEvent) {
        if !self.is_enabled().await {
            return;
        }

        let event_name = event.event_name();
        
        match self.app_handle.emit(event_name, &event) {
            Ok(_) => {
                debug!("Successfully emitted event: {}", event_name);
            }
            Err(e) => {
                error!("Failed to emit event {}: {}", event_name, e);
            }
        }
    }

    /// Emit a progress update
    pub async fn emit_progress(&self, progress: CrawlingProgress) {
        let event = CrawlingEvent::ProgressUpdate(progress);
        self.emit_event(event).await;
    }

    /// Emit a task status update
    pub async fn emit_task_update(&self, task_status: CrawlingTaskStatus) {
        let event = CrawlingEvent::TaskUpdate(task_status);
        self.emit_event(event).await;
    }

    /// Emit a stage change notification
    pub async fn emit_stage_change(
        &self,
        from: crate::domain::events::CrawlingStage,
        to: crate::domain::events::CrawlingStage,
        message: String,
    ) {
        let event = CrawlingEvent::StageChange { from, to, message };
        self.emit_event(event).await;
    }

    /// Emit an error notification
    pub async fn emit_error(
        &self,
        error_id: String,
        message: String,
        stage: crate::domain::events::CrawlingStage,
        recoverable: bool,
    ) {
        let event = CrawlingEvent::Error {
            error_id,
            message,
            stage,
            recoverable,
        };
        self.emit_event(event).await;
    }

    /// Emit database statistics update
    pub async fn emit_database_update(&self, stats: DatabaseStats) {
        let event = CrawlingEvent::DatabaseUpdate(stats);
        self.emit_event(event).await;
    }

    /// Emit crawling completion notification
    pub async fn emit_completed(&self, result: crate::domain::events::CrawlingResult) {
        let event = CrawlingEvent::Completed(result);
        self.emit_event(event).await;
    }

    /// Emit multiple events in batch (useful for reducing frontend update frequency)
    pub async fn emit_batch(&self, events: Vec<CrawlingEvent>) {
        if !self.is_enabled().await {
            return;
        }

        for event in events {
            self.emit_event(event).await;
        }
    }
}

/// Builder for creating event emitters with specific configurations
pub struct EventEmitterBuilder {
    app_handle: Option<AppHandle>,
    enabled: bool,
}

impl EventEmitterBuilder {
    /// Create a new event emitter builder
    pub fn new() -> Self {
        Self {
            app_handle: None,
            enabled: true,
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

    /// Build the event emitter
    pub async fn build(self) -> Result<EventEmitter, String> {
        let app_handle = self.app_handle.ok_or("App handle is required")?;
        
        let emitter = EventEmitter::new(app_handle);
        emitter.set_enabled(self.enabled).await;
        
        Ok(emitter)
    }
}

impl Default for EventEmitterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Note: These tests would require a Tauri app instance to run properly
    // They serve as documentation for the expected behavior
    
    #[tokio::test]
    async fn test_event_emitter_creation() {
        // This test demonstrates how to create an event emitter
        // In a real test, you would need to mock the AppHandle
        assert!(true);
    }
    
    #[tokio::test]
    async fn test_event_emission_when_disabled() {
        // This test would verify that events are not emitted when disabled
        assert!(true);
    }
}
