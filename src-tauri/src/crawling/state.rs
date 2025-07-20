//! # Shared State Management
//!
//! This module implements thread-safe shared state for the crawling system.
//! Using modern Rust patterns with Arc, Mutex, and channels for state management.

#![allow(missing_docs)]
#![allow(clippy::unnecessary_operation)]
#![allow(unused_must_use)]

use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock, Semaphore};
use tokio_util::sync::CancellationToken;

use crate::crawling::tasks::{TaskId, CrawlingTask};
use crate::domain::CrawlingStatus;

// Export CrawlingStatus for public API
pub use crate::domain::CrawlingStatus as PublicCrawlingStatus;

/// Thread-safe shared state for the entire crawling system
#[derive(Debug)]
pub struct SharedState {
    /// Cancellation token for graceful shutdown
    pub cancellation_token: CancellationToken,
    
    /// HTTP request rate limiter
    pub http_semaphore: Arc<Semaphore>,
    
    /// Real-time crawling statistics
    pub stats: Arc<RwLock<CrawlingStats>>,
    
    /// Task tracking and status
    pub task_tracker: Arc<Mutex<TaskTracker>>,
    
    /// Configuration settings
    pub config: Arc<RwLock<CrawlingConfig>>,
}

impl SharedState {
    /// Creates a new shared state instance
    #[must_use]
    pub fn new(config: CrawlingConfig) -> Self {
        Self {
            cancellation_token: CancellationToken::new(),
            http_semaphore: Arc::new(Semaphore::new(config.max_concurrent_requests)),
            stats: Arc::new(RwLock::new(CrawlingStats::default())),
            task_tracker: Arc::new(Mutex::new(TaskTracker::new())),
            config: Arc::new(RwLock::new(config)),
        }
    }
    
    /// Requests graceful shutdown of all operations
    pub fn request_shutdown(&self) {
        self.cancellation_token.cancel();
    }
    
    /// Checks if shutdown has been requested
    #[must_use]
    pub fn is_shutdown_requested(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }

    /// Record task success
    pub async fn record_task_success(&self, task_id: TaskId, duration: Duration) {
        let mut stats = self.stats.write().await;
        stats.tasks_completed += 1;
        if stats.tasks_in_progress > 0 {
            stats.tasks_in_progress -= 1;
        }
        
        let mut tracker = self.task_tracker.lock().await;
        tracker.complete_task(task_id, duration);
    }

    /// Record task failure
    pub async fn record_task_failure(&self, task_id: TaskId, error: String, duration: Duration) {
        let mut stats = self.stats.write().await;
        stats.tasks_failed += 1;
        if stats.tasks_in_progress > 0 {
            stats.tasks_in_progress -= 1;
        }
        
        let mut tracker = self.task_tracker.lock().await;
        tracker.fail_task(task_id, error, duration);
    }
}

/// Real-time crawling statistics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CrawlingStats {
    /// Total number of tasks created
    pub total_tasks_created: u64,
    
    /// Number of tasks completed successfully
    pub tasks_completed: u64,
    
    /// Number of tasks that failed
    pub tasks_failed: u64,
    
    /// Number of tasks currently in progress
    pub tasks_in_progress: u64,
    
    /// Number of currently active tasks
    pub active_tasks: usize,
    
    /// List pages processed
    pub list_pages_processed: u32,
    
    /// List pages fetched
    pub list_pages_fetched: u64,
    
    /// Product URLs discovered
    pub product_urls_discovered: u64,
    
    /// Product details fetched
    pub product_details_fetched: u64,
    
    /// Product details parsed
    pub product_details_parsed: u64,
    
    /// Products saved to database
    pub products_saved: u64,
    
    /// Current processing rate (tasks per second)
    pub processing_rate: f64,
    
    /// Average task duration by type
    pub avg_task_duration: HashMap<String, Duration>,
    
    /// Queue sizes for monitoring
    pub queue_sizes: QueueSizes,
    
    /// System health status
    pub is_healthy: bool,
    
    /// Timestamp of last update
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Queue sizes for all queues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueSizes {
    pub list_page_fetch: usize,
    pub list_page_parse: usize,
    pub product_detail_fetch: usize,
    pub product_detail_parse: usize,
    pub product_save: usize,
}

impl CrawlingStats {
    /// Updates processing rate based on recent activity
    pub fn update_processing_rate(&mut self, completed_tasks: u64, time_window: Duration) {
        if time_window.as_secs() > 0 {
            self.processing_rate = completed_tasks as f64 / time_window.as_secs() as f64;
        }
        self.last_updated = chrono::Utc::now();
    }
    
    /// Records completion of a task
    pub fn record_task_completion(&mut self, task_type: &str, duration: Duration) {
        self.tasks_completed += 1;
        self.tasks_in_progress = self.tasks_in_progress.saturating_sub(1);
        
        // Update average duration for this task type
        let current_avg = self.avg_task_duration.get(task_type).copied().unwrap_or_default();
        let new_avg = Duration::from_nanos(
            (current_avg.as_nanos() as u64 + duration.as_nanos() as u64) / 2
        );
        self.avg_task_duration.insert(task_type.to_string(), new_avg);
        
        self.last_updated = chrono::Utc::now();
    }
    
    /// Records failure of a task
    pub fn record_task_failure(&mut self, _task_type: &str) {
        self.tasks_failed += 1;
        self.tasks_in_progress = self.tasks_in_progress.saturating_sub(1);
        self.last_updated = chrono::Utc::now();
    }
    
    /// Records start of a task
    pub fn record_task_start(&mut self) {
        self.tasks_in_progress += 1;
        self.last_updated = chrono::Utc::now();
    }
    
    /// Gets total number of tasks processed (completed + failed)
    pub fn total_tasks_processed(&self) -> u64 {
        self.tasks_completed + self.tasks_failed
    }
}

/// Task tracking system for monitoring individual tasks
#[derive(Debug)]
pub struct TaskTracker {
    /// Currently active tasks
    active_tasks: HashMap<TaskId, TaskStatus>,
    
    /// Task execution history (limited size)
    task_history: Vec<TaskRecord>,
    
    /// Maximum history size
    max_history_size: usize,
}

impl TaskTracker {
    /// Creates a new task tracker
    #[must_use]
    pub fn new() -> Self {
        Self {
            active_tasks: HashMap::new(),
            task_history: Vec::new(),
            max_history_size: 1000,
        }
    }
    
    /// Records the start of a task
    pub fn start_task(&mut self, task: &CrawlingTask) {
        let status = TaskStatus {
            task_id: task.task_id(),
            task_type: task.task_type().to_string(),
            started_at: Instant::now(),
            status: TaskState::Running,
        };
        
        self.active_tasks.insert(task.task_id(), status);
    }
    
    /// Records successful completion of a task
    pub fn complete_task(&mut self, task_id: TaskId, duration: Duration) {
        if let Some(mut status) = self.active_tasks.remove(&task_id) {
            status.status = TaskState::Completed;
            
            let record = TaskRecord {
                task_id,
                task_type: status.task_type.clone(),
                started_at: chrono::Utc::now(), // Convert from instant to DateTime
                completed_at: Some(chrono::Utc::now()),
                duration,
                success: true,
                error_message: None,
            };
            
            self.add_to_history(record);
        }
    }
    
    /// Records failure of a task
    pub fn fail_task(&mut self, task_id: TaskId, error: String, duration: Duration) {
        if let Some(mut status) = self.active_tasks.remove(&task_id) {
            status.status = TaskState::Failed;
            
            let record = TaskRecord {
                task_id,
                task_type: status.task_type.clone(),
                started_at: chrono::Utc::now(), // Convert from instant to DateTime
                completed_at: Some(chrono::Utc::now()),
                duration,
                success: false,
                error_message: Some(error),
            };
            
            self.add_to_history(record);
        }
    }
    
    /// Adds a record to history, maintaining size limit
    fn add_to_history(&mut self, record: TaskRecord) {
        self.task_history.push(record);
        
        if self.task_history.len() > self.max_history_size {
            self.task_history.remove(0);
        }
    }
    
    /// Returns the number of active tasks
    #[must_use]
    pub fn active_task_count(&self) -> usize {
        self.active_tasks.len()
    }
    
    /// Returns task history
    #[must_use]
    pub fn task_history(&self) -> &[TaskRecord] {
        &self.task_history
    }
}

impl Default for TaskTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Status of an individual task
#[derive(Debug, Clone)]
pub struct TaskStatus {
    pub task_id: TaskId,
    pub task_type: String,
    pub started_at: Instant,
    pub status: TaskState,
}

/// Task execution state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskState {
    Running,
    Completed,
    Failed,
}

/// Historical record of task execution
#[derive(Debug, Clone)]
pub struct TaskRecord {
    pub task_id: TaskId,
    pub task_type: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub duration: Duration,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Configuration for the crawling system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingConfig {
    /// Maximum number of concurrent HTTP requests
    pub max_concurrent_requests: usize,
    
    /// Maximum number of concurrent tasks
    pub max_concurrent_tasks: usize,
    
    /// Request timeout in seconds
    pub request_timeout_seconds: u64,
    
    /// Maximum number of retries for failed requests
    pub max_retries: u32,
    
    /// Base URL for the target website
    pub base_url: String,
    
    /// User agent string for HTTP requests
    pub user_agent: String,
    
    /// Delay between requests (in milliseconds)
    pub request_delay_ms: u64,
    
    /// Maximum number of pages to process
    pub max_pages: Option<u32>,
    
    /// 개발 편의성을 위한 추가 필드들
    pub max_queue_size: usize,
    pub backpressure_threshold: usize,
    pub scheduler_interval_ms: u64,
    pub shutdown_timeout_seconds: u64,
    pub stats_interval_seconds: u64,
    pub auto_retry_enabled: bool,
    pub retry_delay_ms: u64,
    pub backpressure_enabled: bool,
}

impl Default for CrawlingConfig {
    fn default() -> Self {
        Self {
            max_concurrent_requests: 10,
            max_concurrent_tasks: 20,
            request_timeout_seconds: 30,
            max_retries: 3,
            base_url: "https://csa-iot.org/csa-iot_products/?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver".to_string(),
            user_agent: "Mozilla/5.0 (compatible; RMatterCertis/1.0)".to_string(),
            request_delay_ms: 1000,
            max_pages: Some(100),
            // 개발 편의성을 위한 기본값들
            max_queue_size: 1000,
            backpressure_threshold: 800,
            scheduler_interval_ms: 100,
            shutdown_timeout_seconds: 30,
            stats_interval_seconds: 10,
            auto_retry_enabled: true,
            retry_delay_ms: 1000,
            backpressure_enabled: false, // 개발 단계에서는 비활성화
        }
    }
}

impl Default for QueueSizes {
    fn default() -> Self {
        Self {
            list_page_fetch: 0,
            list_page_parse: 0,
            product_detail_fetch: 0,
            product_detail_parse: 0,
            product_save: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crawling::tasks::CrawlingTask;

    #[test]
    fn shared_state_creation() {
        let config = CrawlingConfig::default();
        let state = SharedState::new(config);
        
        assert!(!state.is_shutdown_requested());
    }

    #[test]
    fn task_tracker_lifecycle() {
        let mut tracker = TaskTracker::new();
        let task = CrawlingTask::FetchListPage {
            task_id: crate::crawling::tasks::TaskId::new(),
            page_number: 1,
            url: "https://example.com".to_string(),
        };
        
        tracker.start_task(&task);
        assert_eq!(tracker.active_task_count(), 1);
        
        tracker.complete_task(task.task_id(), Duration::from_millis(100));
        assert_eq!(tracker.active_task_count(), 0);
        assert_eq!(tracker.task_history().len(), 1);
    }

    #[test]
    fn crawling_stats_update() {
        let mut stats = CrawlingStats::default();
        
        stats.record_task_start();
        assert_eq!(stats.tasks_in_progress, 1);
        
        stats.record_task_completion("fetch_list_page", Duration::from_millis(100));
        assert_eq!(stats.tasks_completed, 1);
        assert_eq!(stats.tasks_in_progress, 0);
    }
}
