//! Memory-based crawling session state management
//! 
//! Implements the industry-standard approach: "State management layer + save only final results to DB"
//! This replaces the previous crawling_sessions table with in-memory state management for better performance.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use sqlx::{Encode, Decode, Type};

/// Current status of a crawling session
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SessionStatus {
    Initializing,
    Running,
    Paused,
    Completed,
    Failed,
    Stopped,
}

impl Type<sqlx::Sqlite> for SessionStatus {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <String as Type<sqlx::Sqlite>>::type_info()
    }
}

impl<'q> Encode<'q, sqlx::Sqlite> for SessionStatus {
    fn encode_by_ref(&self, buf: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>) -> sqlx::encode::IsNull {
        let s = match self {
            SessionStatus::Initializing => "Initializing",
            SessionStatus::Running => "Running",
            SessionStatus::Paused => "Paused",
            SessionStatus::Completed => "Completed",
            SessionStatus::Failed => "Failed",
            SessionStatus::Stopped => "Stopped",
        };
        <String as Encode<sqlx::Sqlite>>::encode(s.to_string(), buf)
    }
}

impl<'r> Decode<'r, sqlx::Sqlite> for SessionStatus {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <String as Decode<sqlx::Sqlite>>::decode(value)?;
        match s.as_str() {
            "Initializing" => Ok(SessionStatus::Initializing),
            "Running" => Ok(SessionStatus::Running),
            "Paused" => Ok(SessionStatus::Paused),
            "Completed" => Ok(SessionStatus::Completed),
            "Failed" => Ok(SessionStatus::Failed),
            "Stopped" => Ok(SessionStatus::Stopped),
            _ => Err(format!("Invalid SessionStatus: {s}").into()),
        }
    }
}

/// Current stage of crawling process
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CrawlingStage {
    ProductList,      // Stage 1: Collecting product URLs
    ProductDetails,   // Stage 2: Collecting detailed product information
    MatterDetails,    // Stage 3: Collecting Matter-specific details
}

impl Type<sqlx::Sqlite> for CrawlingStage {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <String as Type<sqlx::Sqlite>>::type_info()
    }
}

impl<'q> Encode<'q, sqlx::Sqlite> for CrawlingStage {
    fn encode_by_ref(&self, buf: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>) -> sqlx::encode::IsNull {
        let s = match self {
            CrawlingStage::ProductList => "ProductList",
            CrawlingStage::ProductDetails => "ProductDetails",
            CrawlingStage::MatterDetails => "MatterDetails",
        };
        <String as Encode<sqlx::Sqlite>>::encode(s.to_string(), buf)
    }
}

impl<'r> Decode<'r, sqlx::Sqlite> for CrawlingStage {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <String as Decode<sqlx::Sqlite>>::decode(value)?;
        match s.as_str() {
            "ProductList" => Ok(CrawlingStage::ProductList),
            "ProductDetails" => Ok(CrawlingStage::ProductDetails),
            "MatterDetails" => Ok(CrawlingStage::MatterDetails),
            _ => Err(format!("Invalid CrawlingStage: {s}").into()),
        }
    }
}

/// Real-time crawling session state (kept in memory)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingSessionState {
    pub session_id: String,
    pub status: SessionStatus,
    pub stage: CrawlingStage,
    pub current_page: u32,
    pub total_pages: u32,
    pub products_found: u32,
    pub products_processed: u32,
    pub errors_count: u32,
    pub started_at: DateTime<Utc>,
    pub last_updated_at: DateTime<Utc>,
    pub estimated_completion: Option<DateTime<Utc>>,
    pub config_snapshot: serde_json::Value,
    pub current_url: Option<String>,
    pub error_details: Vec<String>,
    pub start_url: String,
    pub target_domains: Vec<String>,
}

/// Final crawling result (saved to database)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingResult {
    pub session_id: String,
    pub status: SessionStatus,
    pub stage: CrawlingStage,
    pub total_pages: u32,
    pub products_found: u32,
    pub errors_count: u32,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub execution_time_seconds: u32,
    pub config_snapshot: serde_json::Value,
    pub error_details: Option<String>,
    pub details_fetched: u32,
    pub created_at: DateTime<Utc>,
}

/// Thread-safe session manager for in-memory state management
#[derive(Debug)]
pub struct SessionManager {
    /// Active sessions in memory
    sessions: Arc<RwLock<HashMap<String, CrawlingSessionState>>>,
    
    /// Performance metrics for estimation
    metrics: Arc<Mutex<SessionMetrics>>,
}

#[derive(Debug, Default)]
struct SessionMetrics {
    avg_pages_per_second: f64,
    avg_products_per_page: f64,
    last_updated: Option<DateTime<Utc>>,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(Mutex::new(SessionMetrics::default())),
        }
    }

    /// Start a new crawling session
    pub async fn start_session(
        &self,
        config: serde_json::Value,
        total_pages: u32,
        stage: CrawlingStage,
    ) -> String {
        let session_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        let session_state = CrawlingSessionState {
            session_id: session_id.clone(),
            status: SessionStatus::Initializing,
            stage,
            current_page: 0,
            total_pages,
            products_found: 0,
            products_processed: 0,
            errors_count: 0,
            started_at: now,
            last_updated_at: now,
            estimated_completion: None,
            config_snapshot: config,
            current_url: None,
            error_details: Vec::new(),
            start_url: String::new(),
            target_domains: Vec::new(),
        };

        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.clone(), session_state);

        tracing::info!("Started new crawling session: {}", session_id);
        session_id
    }

    /// Start a new crawling session with simplified parameters
    pub async fn start_session_simple(
        &self,
        session_id: &str,
        start_url: &str,
        target_domains: Vec<String>,
    ) -> Result<(), anyhow::Error> {
        let now = Utc::now();

        let session_state = CrawlingSessionState {
            session_id: session_id.to_string(),
            status: SessionStatus::Initializing,
            stage: CrawlingStage::ProductList,
            current_page: 0,
            total_pages: 0, // Will be updated during crawling
            products_found: 0,
            products_processed: 0,
            errors_count: 0,
            current_url: Some(start_url.to_string()),
            start_url: start_url.to_string(),
            target_domains: target_domains.clone(),
            started_at: now,
            last_updated_at: now,
            estimated_completion: None,
            error_details: Vec::new(),
            config_snapshot: serde_json::json!({
                "start_url": start_url,
                "target_domains": target_domains
            }),
        };

        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.to_string(), session_state);
        Ok(())
    }

    /// Update session progress (memory-only, very fast)
    pub async fn update_progress(
        &self,
        session_id: &str,
        current_page: u32,
        products_found: u32,
        current_url: Option<String>,
    ) -> Result<(), String> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(session) = sessions.get_mut(session_id) {
            session.current_page = current_page;
            session.products_found = products_found;
            session.current_url = current_url;
            session.last_updated_at = Utc::now();
            session.status = SessionStatus::Running;
            
            // Update estimated completion
            if current_page > 0 {
                session.estimated_completion = self.calculate_eta(session).await;
            }
            
            Ok(())
        } else {
            Err(format!("Session not found: {session_id}"))
        }
    }

    /// Update session progress with simplified parameters
    pub async fn update_session_progress(
        &self,
        session_id: &str,
        progress: u32,
        current_step: String,
    ) -> Result<(), anyhow::Error> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.products_found = progress;
            session.current_url = Some(current_step);
            session.last_updated_at = Utc::now();
            session.status = SessionStatus::Running;
        }
        Ok(())
    }

    /// Add error to session
    pub async fn add_error(&self, session_id: &str, error: String) -> Result<(), String> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(session) = sessions.get_mut(session_id) {
            session.errors_count += 1;
            session.error_details.push(error);
            session.last_updated_at = Utc::now();
            Ok(())
        } else {
            Err(format!("Session not found: {session_id}"))
        }
    }

    /// Change session status
    pub async fn set_status(&self, session_id: &str, status: SessionStatus) -> Result<(), String> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(session) = sessions.get_mut(session_id) {
            session.status = status;
            session.last_updated_at = Utc::now();
            Ok(())
        } else {
            Err(format!("Session not found: {session_id}"))
        }
    }

    /// Complete a session with final status
    pub async fn complete_session_simple(
        &self,
        session_id: &str,
    ) -> Result<(), anyhow::Error> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.status = SessionStatus::Completed;
            session.last_updated_at = Utc::now();
        }
        Ok(())
    }

    /// Complete session and prepare final result
    pub async fn complete_session(&self, session_id: &str, status: SessionStatus) -> Option<CrawlingResult> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(mut session) = sessions.remove(session_id) {
            let completed_at = Utc::now();
            let execution_time = (completed_at - session.started_at).num_seconds() as u32;
            
            session.status = status.clone();
            session.last_updated_at = completed_at;

            // Update performance metrics
            self.update_metrics(&session, execution_time).await;

            Some(CrawlingResult {
                session_id: session.session_id,
                status,
                stage: session.stage,
                total_pages: session.total_pages,
                products_found: session.products_found,
                errors_count: session.errors_count,
                started_at: session.started_at,
                completed_at,
                execution_time_seconds: execution_time,
                config_snapshot: session.config_snapshot,
                error_details: if session.error_details.is_empty() {
                    None
                } else {
                    Some(session.error_details.join("\n"))
                },
                details_fetched: session.products_processed,
                created_at: Utc::now(),
            })
        } else {
            None
        }
    }

    /// Remove session from memory (cleanup)
    pub async fn remove_session(&self, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
        tracing::info!("Removed session from memory: {}", session_id);
    }

    /// Get session statistics
    pub async fn get_session_stats(&self) -> SessionStats {
        let sessions = self.sessions.read().await;
        let total_sessions = sessions.len();
        let mut by_status = HashMap::new();

        for session in sessions.values() {
            *by_status.entry(session.status.clone()).or_insert(0) += 1;
        }

        SessionStats {
            total_active_sessions: total_sessions,
            sessions_by_status: by_status,
        }
    }

    /// Get current session state (instant response from memory)
    pub async fn get_session(&self, session_id: &str) -> Option<CrawlingSessionState> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }

    /// Get session status by ID
    pub async fn get_session_state(&self, session_id: &str) -> Result<Option<CrawlingSessionState>, anyhow::Error> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(session_id).cloned())
    }

    /// Get all active sessions
    pub async fn get_active_sessions(&self) -> Vec<CrawlingSessionState> {
        let sessions = self.sessions.read().await;
        sessions.values()
            .filter(|session| matches!(session.status, 
                SessionStatus::Initializing | 
                SessionStatus::Running | 
                SessionStatus::Paused
            ))
            .cloned()
            .collect()
    }

    /// Get all sessions (both active and inactive)
    pub async fn get_all_sessions(&self) -> Vec<CrawlingSessionState> {
        let sessions = self.sessions.read().await;
        sessions.values().cloned().collect()
    }

    /// Calculate estimated time of arrival
    async fn calculate_eta(&self, session: &CrawlingSessionState) -> Option<DateTime<Utc>> {
        if session.current_page == 0 || session.total_pages == 0 {
            return None;
        }

        let elapsed = (Utc::now() - session.started_at).num_seconds() as f64;
        let progress_ratio = session.current_page as f64 / session.total_pages as f64;
        
        if progress_ratio > 0.0 {
            let estimated_total_time = elapsed / progress_ratio;
            let remaining_time = estimated_total_time - elapsed;
            
            Some(Utc::now() + chrono::Duration::seconds(remaining_time as i64))
        } else {
            None
        }
    }

    /// Update performance metrics for better ETA calculation
    async fn update_metrics(&self, session: &CrawlingSessionState, execution_time: u32) {
        let mut metrics = self.metrics.lock().await;
        
        if session.current_page > 0 {
            let pages_per_second = session.current_page as f64 / execution_time as f64;
            metrics.avg_pages_per_second = if metrics.avg_pages_per_second == 0.0 {
                pages_per_second
            } else {
                (metrics.avg_pages_per_second + pages_per_second) / 2.0
            };
        }
        
        if session.current_page > 0 {
            let products_per_page = session.products_found as f64 / session.current_page as f64;
            metrics.avg_products_per_page = if metrics.avg_products_per_page == 0.0 {
                products_per_page
            } else {
                (metrics.avg_products_per_page + products_per_page) / 2.0
            };
        }
        
        metrics.last_updated = Some(Utc::now());
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionStats {
    pub total_active_sessions: usize,
    pub sessions_by_status: HashMap<SessionStatus, usize>,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_lifecycle() {
        let manager = SessionManager::new();
        let config = serde_json::json!({"test": true});

        // Start session
        let session_id = manager.start_session(config, 100, CrawlingStage::ProductList).await;
        
        // Check initial state
        let session = manager.get_session(&session_id).await.unwrap();
        assert_eq!(session.status, SessionStatus::Initializing);
        assert_eq!(session.total_pages, 100);

        // Update progress
        manager.update_progress(&session_id, 10, 50, Some("test-url".to_string())).await.unwrap();
        
        // Check updated state
        let session = manager.get_session(&session_id).await.unwrap();
        assert_eq!(session.status, SessionStatus::Running);
        assert_eq!(session.current_page, 10);
        assert_eq!(session.products_found, 50);

        // Complete session
        let result = manager.complete_session(&session_id, SessionStatus::Completed).await.unwrap();
        assert_eq!(result.status, SessionStatus::Completed);
        assert_eq!(result.products_found, 50);

        // Session should be removed from memory
        assert!(manager.get_session(&session_id).await.is_none());
    }

    #[tokio::test]
    async fn test_concurrent_sessions() {
        let manager = SessionManager::new();
        let config = serde_json::json!({"test": true});

        // Start multiple sessions
        let session1 = manager.start_session(config.clone(), 50, CrawlingStage::ProductList).await;
        let session2 = manager.start_session(config, 100, CrawlingStage::ProductDetails).await;

        // Update both sessions
        manager.update_progress(&session1, 5, 25, None).await.unwrap();
        manager.update_progress(&session2, 10, 100, None).await.unwrap();

        // Check active sessions
        let active = manager.get_active_sessions().await;
        assert_eq!(active.len(), 2);

        // Complete one session
        manager.complete_session(&session1, SessionStatus::Completed).await.unwrap();
        
        let active = manager.get_active_sessions().await;
        assert_eq!(active.len(), 1);
    }
}
