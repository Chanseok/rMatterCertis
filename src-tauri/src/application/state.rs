//! Application state management for Tauri
//! 
//! This module defines the global application state that will be managed
//! by Tauri's state management system, providing access to core services
//! and components across the application.

use crate::application::events::EventEmitter;
// use crate::application::crawler_manager::CrawlerManager; // 임시 비활성화
use crate::domain::entities::CrawlingSession;
use crate::domain::events::{CrawlingProgress, CrawlingStatus, DatabaseStats};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use chrono::Utc;
use tracing::{info, debug};
use sqlx::SqlitePool;

/// Global application state managed by Tauri
pub struct AppState {
    /// Event emitter for real-time communication with frontend
    pub event_emitter: Arc<RwLock<Option<EventEmitter>>>,
    
    /// Shared database connection pool (Modern Rust 2024 - Backend-Only CRUD)
    pub database_pool: Arc<RwLock<Option<SqlitePool>>>,
    
    /// Integrated crawler manager - 통합 크롤링 매니저 (임시 비활성화)
    // pub crawler_manager: Arc<RwLock<Option<CrawlerManager>>>,
    
    /// Current crawling session
    pub current_session: Arc<RwLock<Option<CrawlingSession>>>,
    
    /// Current crawling progress
    pub current_progress: Arc<RwLock<CrawlingProgress>>,
    
    /// Database statistics
    pub database_stats: Arc<RwLock<Option<DatabaseStats>>>,
    
    /// Application configuration
    pub config: Arc<RwLock<crate::infrastructure::config::AppConfig>>,
    
    /// Session start time for calculating elapsed time
    pub session_start_time: Arc<RwLock<Option<chrono::DateTime<Utc>>>>,
    
    /// Cancellation token for stopping crawling operations
    pub crawling_cancellation_token: Arc<RwLock<Option<CancellationToken>>>,
}

impl AppState {
    /// Create a new application state
    pub fn new(config: crate::infrastructure::config::AppConfig) -> Self {
        Self {
            event_emitter: Arc::new(RwLock::new(None)),
            database_pool: Arc::new(RwLock::new(None)),
            // crawler_manager: Arc::new(RwLock::new(None)), // 임시 비활성화
            current_session: Arc::new(RwLock::new(None)),
            current_progress: Arc::new(RwLock::new(CrawlingProgress::default())),
            database_stats: Arc::new(RwLock::new(None)),
            config: Arc::new(RwLock::new(config)),
            session_start_time: Arc::new(RwLock::new(None)),
            crawling_cancellation_token: Arc::new(RwLock::new(None)),
        }
    }
    
    /// Initialize the shared database connection pool (Modern Rust 2024 - Backend-Only CRUD)
    pub async fn initialize_database_pool(&self) -> Result<(), String> {
        use crate::infrastructure::database_paths::get_main_database_url;
        
        let database_url = get_main_database_url();
        
        let pool = SqlitePool::connect(&database_url).await
            .map_err(|e| format!("Failed to connect to database: {}", e))?;
        
        let mut pool_guard = self.database_pool.write().await;
        *pool_guard = Some(pool);
        info!("✅ Shared database connection pool initialized");
        Ok(())
    }
    
    /// Get a cloned database pool for use in commands
    pub async fn get_database_pool(&self) -> Result<SqlitePool, String> {
        let pool_guard = self.database_pool.read().await;
        match pool_guard.as_ref() {
            Some(pool) => Ok(pool.clone()),
            None => Err("Database pool not initialized. Call initialize_database_pool() first.".to_string())
        }
    }
    
    /* ===== CrawlerManager 관련 메서드들 (임시 비활성화) =====
    
    /// Initialize the crawler manager
    pub async fn initialize_crawler_manager(&self, crawler_manager: CrawlerManager) -> Result<(), String> {
        let mut manager_guard = self.crawler_manager.write().await;
        *manager_guard = Some(crawler_manager);
        info!("Crawler manager initialized");
        Ok(())
    }
    
    /// Get the crawler manager
    pub async fn get_crawler_manager(&self) -> Option<CrawlerManager> {
        self.crawler_manager.read().await.clone()
    }
    
    */
    
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
    
    /// Update the current crawling progress with calculated fields
    pub async fn update_progress(&self, mut progress: CrawlingProgress) -> Result<(), String> {
        // Get start time for calculations
        let start_time = {
            let start_time_guard = self.session_start_time.read().await;
            start_time_guard.unwrap_or_else(|| Utc::now())
        };
        
        // Calculate derived fields
        progress.calculate_derived_fields(start_time);
        
        // Update stored progress
        {
            let mut progress_guard = self.current_progress.write().await;
            *progress_guard = progress.clone();
        }
        
        // Emit progress update event
        if let Some(emitter) = self.get_event_emitter().await {
            let _ = emitter.emit_progress(progress).await;
        }
        
        Ok(())
    }
    
    /// Get the current crawling progress
    pub async fn get_progress(&self) -> CrawlingProgress {
        self.current_progress.read().await.clone()
    }
    
    /// Start a new crawling session
    pub async fn start_session(&self, session: CrawlingSession) -> Result<(), String> {
        let now = Utc::now();
        
        // Create a new cancellation token for this session
        let cancellation_token = CancellationToken::new();
        
        // Store the cancellation token
        {
            let mut token_guard = self.crawling_cancellation_token.write().await;
            *token_guard = Some(cancellation_token);
        }
        
        // Set session start time
        {
            let mut start_time_guard = self.session_start_time.write().await;
            *start_time_guard = Some(now);
        }
        
        {
            let mut session_guard = self.current_session.write().await;
            *session_guard = Some(session);
        }
        
        // Reset progress for new session with calculated fields
        let initial_progress = CrawlingProgress::new_with_calculation(
            0,
            0,
            crate::domain::events::CrawlingStage::Idle,
            "크롤링 세션을 시작합니다".to_string(),
            CrawlingStatus::Running,
            "크롤링 세션을 시작합니다".to_string(),
            now,
            0,
            0,
            0,
        );
        
        self.update_progress(initial_progress).await?;
        info!("Crawling session started");
        Ok(())
    }
    
    /// Stop the current crawling session
    pub async fn stop_session(&self) -> Result<(), String> {
        // Cancel all ongoing operations
        {
            let token_guard = self.crawling_cancellation_token.read().await;
            if let Some(token) = token_guard.as_ref() {
                token.cancel();
                info!("🛑 Cancellation token activated - all HTTP requests will be cancelled");
            }
        }
        
        // Clear the cancellation token
        {
            let mut token_guard = self.crawling_cancellation_token.write().await;
            *token_guard = None;
        }
        
        {
            let mut session_guard = self.current_session.write().await;
            *session_guard = None;
        }
        
        // Clear session start time
        {
            let mut start_time_guard = self.session_start_time.write().await;
            *start_time_guard = None;
        }
        
        // Update progress to stopped state
        let mut stopped_progress = self.get_progress().await;
        stopped_progress.status = CrawlingStatus::Cancelled;
        stopped_progress.message = "크롤링이 중단되었습니다".to_string();
        stopped_progress.timestamp = Utc::now();
        
        self.update_progress(stopped_progress).await?;
        info!("Crawling session stopped");
        Ok(())
    }
    
    /// Get the current cancellation token
    pub async fn get_cancellation_token(&self) -> Option<CancellationToken> {
        let token_guard = self.crawling_cancellation_token.read().await;
        token_guard.clone()
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
            let _ = emitter.emit_database_update(stats).await;
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
        debug!("Application configuration updated");
        Ok(())
    }
    
    /// Emit an error event
    pub async fn emit_error(&self, error_id: String, message: String, recoverable: bool) {
        if let Some(emitter) = self.get_event_emitter().await {
            let current_progress = self.get_progress().await;
            let _ = emitter.emit_error(error_id, message, current_progress.current_stage, recoverable).await;
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
            let _ = emitter.emit_stage_change(from, to, message).await;
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
