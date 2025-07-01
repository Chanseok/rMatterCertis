//! Application state management for Tauri
//! 
//! This module defines the global application state that will be managed
//! by Tauri's state management system, providing access to core services
//! and components across the application.

use crate::application::events::EventEmitter;
use crate::domain::entities::CrawlingSession;
use crate::domain::events::{CrawlingProgress, CrawlingStatus, DatabaseStats};
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::Utc;
use tracing::info;

/// Global application state managed by Tauri
pub struct AppState {
    /// Event emitter for real-time communication with frontend
    pub event_emitter: Arc<RwLock<Option<EventEmitter>>>,
    
    /// Current crawling session
    pub current_session: Arc<RwLock<Option<CrawlingSession>>>,
    
    /// Current crawling progress
    pub current_progress: Arc<RwLock<CrawlingProgress>>,
    
    /// Database statistics
    pub database_stats: Arc<RwLock<Option<DatabaseStats>>>,
    
    /// Application configuration
    pub config: Arc<RwLock<crate::infrastructure::config::AppConfig>>,
}

impl AppState {
    /// Create a new application state
    pub fn new(config: crate::infrastructure::config::AppConfig) -> Self {
        Self {
            event_emitter: Arc::new(RwLock::new(None)),
            current_session: Arc::new(RwLock::new(None)),
            current_progress: Arc::new(RwLock::new(CrawlingProgress::default())),
            database_stats: Arc::new(RwLock::new(None)),
            config: Arc::new(RwLock::new(config)),
        }
    }
    
    /// Initialize the event emitter with the app handle
    pub async fn initialize_event_emitter(&self, emitter: EventEmitter) -> Result<(), String> {
        let mut emitter_guard = self.event_emitter.write().await;
        *emitter_guard = Some(emitter);
        info!("Event emitter initialized");
        Ok(())
    }
    
    /// Get the event emitter
    pub async fn get_event_emitter(&self) -> Option<EventEmitter> {
        self.event_emitter.read().await.clone()
    }
    
    /// Update the current crawling progress
    pub async fn update_progress(&self, progress: CrawlingProgress) -> Result<(), String> {
        // Update stored progress
        {
            let mut progress_guard = self.current_progress.write().await;
            *progress_guard = progress.clone();
        }
        
        // Emit progress update event
        if let Some(emitter) = self.get_event_emitter().await {
            emitter.emit_progress(progress).await;
        }
        
        Ok(())
    }
    
    /// Get the current crawling progress
    pub async fn get_progress(&self) -> CrawlingProgress {
        self.current_progress.read().await.clone()
    }
    
    /// Start a new crawling session
    pub async fn start_session(&self, session: CrawlingSession) -> Result<(), String> {
        {
            let mut session_guard = self.current_session.write().await;
            *session_guard = Some(session);
        }
        
        // Reset progress for new session
        let initial_progress = CrawlingProgress {
            status: CrawlingStatus::Running,
            message: "크롤링 세션을 시작합니다".to_string(),
            timestamp: Utc::now(),
            ..Default::default()
        };
        
        self.update_progress(initial_progress).await?;
        info!("Crawling session started");
        Ok(())
    }
    
    /// Stop the current crawling session
    pub async fn stop_session(&self) -> Result<(), String> {
        {
            let mut session_guard = self.current_session.write().await;
            *session_guard = None;
        }
        
        // Update progress to stopped state
        let stopped_progress = CrawlingProgress {
            status: CrawlingStatus::Cancelled,
            message: "크롤링이 중단되었습니다".to_string(),
            timestamp: Utc::now(),
            ..self.get_progress().await
        };
        
        self.update_progress(stopped_progress).await?;
        info!("Crawling session stopped");
        Ok(())
    }
    
    /// Get the current crawling session
    pub async fn get_current_session(&self) -> Option<CrawlingSession> {
        self.current_session.read().await.clone()
    }
    
    /// Update database statistics
    pub async fn update_database_stats(&self, stats: DatabaseStats) -> Result<(), String> {
        {
            let mut stats_guard = self.database_stats.write().await;
            *stats_guard = Some(stats.clone());
        }
        
        // Emit database update event
        if let Some(emitter) = self.get_event_emitter().await {
            emitter.emit_database_update(stats).await;
        }
        
        Ok(())
    }
    
    /// Get current database statistics
    pub async fn get_database_stats(&self) -> Option<DatabaseStats> {
        self.database_stats.read().await.clone()
    }
    
    /// Check if a crawling session is currently active
    pub async fn is_crawling_active(&self) -> bool {
        self.current_session.read().await.is_some()
    }
    
    /// Get the current application configuration
    pub async fn get_config(&self) -> crate::infrastructure::config::AppConfig {
        self.config.read().await.clone()
    }
    
    /// Update the application configuration
    pub async fn update_config(&self, config: crate::infrastructure::config::AppConfig) -> Result<(), String> {
        let mut config_guard = self.config.write().await;
        *config_guard = config;
        info!("Application configuration updated");
        Ok(())
    }
    
    /// Emit an error event
    pub async fn emit_error(&self, error_id: String, message: String, recoverable: bool) {
        if let Some(emitter) = self.get_event_emitter().await {
            let current_progress = self.get_progress().await;
            emitter.emit_error(error_id, message, current_progress.current_stage, recoverable).await;
        }
    }
    
    /// Emit a stage change event
    pub async fn emit_stage_change(
        &self,
        from: crate::domain::events::CrawlingStage,
        to: crate::domain::events::CrawlingStage,
        message: String,
    ) {
        if let Some(emitter) = self.get_event_emitter().await {
            emitter.emit_stage_change(from, to, message).await;
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(crate::infrastructure::config::AppConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::events::CrawlingStage;
    
    #[tokio::test]
    async fn test_app_state_creation() {
        let config = crate::infrastructure::config::AppConfig::default();
        let state = AppState::new(config);
        
        assert!(state.get_current_session().await.is_none());
        assert!(!state.is_crawling_active().await);
    }
    
    #[tokio::test]
    async fn test_progress_update() {
        let state = AppState::default();
        
        let progress = CrawlingProgress {
            current: 10,
            total: 100,
            percentage: 10.0,
            current_stage: CrawlingStage::ProductList,
            status: CrawlingStatus::Running,
            message: "Test progress".to_string(),
            ..Default::default()
        };
        
        state.update_progress(progress.clone()).await.unwrap();
        let stored_progress = state.get_progress().await;
        
        assert_eq!(stored_progress.current, 10);
        assert_eq!(stored_progress.total, 100);
        assert_eq!(stored_progress.percentage, 10.0);
    }
    
    #[tokio::test]
    async fn test_session_lifecycle() {
        let state = AppState::default();
        
        // Initially no active session
        assert!(!state.is_crawling_active().await);
        
        // Start a session
        let session = CrawlingSession {
            id: "test-session".to_string(),
            url: "https://example.com".to_string(),
            start_page: 1,
            end_page: 10,
            created_at: Utc::now(),
            ..Default::default()
        };
        
        state.start_session(session).await.unwrap();
        assert!(state.is_crawling_active().await);
        
        // Stop the session
        state.stop_session().await.unwrap();
        assert!(!state.is_crawling_active().await);
    }
}
