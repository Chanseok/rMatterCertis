//! Application state management for Tauri
//!
//! This module defines the global application state that will be managed
//! by Tauri's state management system, providing access to core services
//! and components across the application.

use crate::application::events::EventEmitter;
// use crate::application::crawler_manager::CrawlerManager; // 임시 비활성화
use crate::domain::entities::CrawlingSession;
// Use frontend-facing progress/info types to avoid legacy domain::events coupling
use crate::api::frontend_api as fe_types;
use crate::api::frontend_api::DatabaseStats;
use chrono::Utc;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

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

    /// Current crawling progress (frontend-friendly summary)
    pub current_progress: Arc<RwLock<fe_types::CrawlingProgressInfo>>,

    /// Database statistics
    pub database_stats: Arc<RwLock<Option<DatabaseStats>>>,

    /// Application configuration
    pub config: Arc<RwLock<crate::infrastructure::config::AppConfig>>,

    /// Shared HTTP client (unified, rate-limited)
    pub http_client: Arc<RwLock<Option<crate::infrastructure::simple_http_client::HttpClient>>>,

    /// Session start time for calculating elapsed time
    pub session_start_time: Arc<RwLock<Option<chrono::DateTime<Utc>>>>,

    /// Cancellation token for stopping crawling operations
    pub crawling_cancellation_token: Arc<RwLock<Option<CancellationToken>>>,
}

impl AppState {
    /// Create a new application state
    #[must_use]
    pub fn new(config: crate::infrastructure::config::AppConfig) -> Self {
        Self {
            event_emitter: Arc::new(RwLock::new(None)),
            database_pool: Arc::new(RwLock::new(None)),
            // crawler_manager: Arc::new(RwLock::new(None)), // 임시 비활성화
            current_session: Arc::new(RwLock::new(None)),
            current_progress: Arc::new(RwLock::new(fe_types::CrawlingProgressInfo {
                stage: 0,
                stage_name: "Idle".to_string(),
                progress_percentage: 0.0,
                items_processed: 0,
                current_message: "대기 중".to_string(),
                estimated_remaining_time: None,
                session_id: "".to_string(),
                timestamp: Utc::now(),
            })),
            database_stats: Arc::new(RwLock::new(None)),
            config: Arc::new(RwLock::new(config)),
            http_client: Arc::new(RwLock::new(None)),
            session_start_time: Arc::new(RwLock::new(None)),
            crawling_cancellation_token: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize the shared database connection pool (Modern Rust 2024 - Backend-Only CRUD)
    pub async fn initialize_database_pool(&self) -> Result<(), String> {
        let pool = crate::infrastructure::database_connection::get_or_init_global_pool()
            .await
            .map_err(|e| format!("Failed to obtain database pool: {}", e))?;

        let mut pool_guard = self.database_pool.write().await;
        *pool_guard = Some(pool);
        // Note: Log message moved to lib.rs setup to avoid duplication
        Ok(())
    }

    /// Initialize the shared HTTP client from the current configuration
    pub async fn initialize_http_client(&self) -> Result<(), String> {
        let cfg = { self.config.read().await.clone() };
        let client = cfg
            .create_http_client()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
        let mut guard = self.http_client.write().await;
        *guard = Some(client);
        Ok(())
    }

    /// Get a cloned HTTP client for use in commands
    pub async fn get_http_client(
        &self,
    ) -> Result<crate::infrastructure::simple_http_client::HttpClient, String> {
        let guard = self.http_client.read().await;
        match guard.as_ref() {
            Some(c) => Ok(c.clone()),
            None => {
                Err("HTTP client not initialized. Call initialize_http_client() first.".to_string())
            }
        }
    }

    /// Get a cloned database pool for use in commands
    pub async fn get_database_pool(&self) -> Result<SqlitePool, String> {
        let pool_guard = self.database_pool.read().await;
        match pool_guard.as_ref() {
            Some(pool) => Ok(pool.clone()),
            None => Err(
                "Database pool not initialized. Call initialize_database_pool() first.".to_string(),
            ),
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
        // Note: Log message moved to lib.rs setup to avoid duplication
        Ok(())
    }

    /// Get the event emitter
    pub async fn get_event_emitter(&self) -> Option<EventEmitter> {
        self.event_emitter.read().await.clone()
    }

    /// Update the current crawling progress with calculated fields
    pub async fn update_progress(
        &self,
        progress: fe_types::CrawlingProgressInfo,
    ) -> Result<(), String> {
        // Update stored progress (already frontend-friendly)
        let mut progress_guard = self.current_progress.write().await;
        *progress_guard = progress;
        Ok(())
    }

    /// Get the current crawling progress
    pub async fn get_progress(&self) -> fe_types::CrawlingProgressInfo {
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

        // Clone to avoid move, needed below for session_id
        let session_clone = session.clone();
        {
            let mut session_guard = self.current_session.write().await;
            *session_guard = Some(session);
        }

        // Reset progress for new session with calculated fields
        // Initialize minimal progress info for UI summary
        self.update_progress(fe_types::CrawlingProgressInfo {
            stage: 0,
            stage_name: "Started".to_string(),
            progress_percentage: 0.0,
            items_processed: 0,
            current_message: "크롤링 세션을 시작합니다".to_string(),
            estimated_remaining_time: None,
            session_id: session_clone.id.clone(),
            timestamp: now,
        })
        .await?;
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
        let now = Utc::now();
        let mut stopped = self.get_progress().await;
        stopped.current_message = "크롤링이 중단되었습니다".to_string();
        stopped.stage_name = "Cancelled".to_string();
        stopped.timestamp = now;
        self.update_progress(stopped).await?;
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

        // DB 통계의 FE 알림은 Actor 이벤트/대시보드 경로로 대체됩니다. 여기서는 별도 emit 하지 않습니다.

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
    pub async fn update_config(
        &self,
        config: crate::infrastructure::config::AppConfig,
    ) -> Result<(), String> {
        let mut config_guard = self.config.write().await;
        *config_guard = config;
        debug!("Application configuration updated");
        Ok(())
    }

    /// Emit an error event
    pub async fn emit_error(&self, error_id: String, message: String, recoverable: bool) {
        // Unified actor-event flow handles errors; this is a no-op placeholder for compatibility.
        let _ = (error_id, message, recoverable);
        tracing::debug!("emit_error called (no-op under unified event system)");
    }

    /// Emit a stage change event
    pub async fn emit_stage_change(&self, from: &str, to: &str, message: String) {
        // Unified actor-event flow handles stage lifecycle; this is a no-op placeholder.
        let _ = (from, to, message);
        tracing::debug!("emit_stage_change called (no-op under unified event system)");
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
    // use crate::domain::events::CrawlingStage;

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

        let progress = fe_types::CrawlingProgressInfo {
            stage: 1,
            stage_name: "ProductList".to_string(),
            progress_percentage: 10.0,
            items_processed: 10,
            current_message: "Test progress".to_string(),
            estimated_remaining_time: None,
            session_id: "test-session".to_string(),
            timestamp: Utc::now(),
        };

        state.update_progress(progress.clone()).await.unwrap();
        let stored_progress = state.get_progress().await;

        assert_eq!(stored_progress.items_processed, 10);
        assert_eq!(stored_progress.progress_percentage, 10.0);
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
