//! Event types for real-time communication between backend and frontend
//! 
//! This module defines all event types that will be emitted from the Rust backend
//! to the SolidJS frontend for real-time updates during crawling operations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// Represents the current stage of the crawling process
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CrawlingStage {
    /// System is idle, no crawling in progress
    Idle,
    /// Discovering total number of pages to crawl
    TotalPages,
    /// Collecting product list from pages
    ProductList,
    /// Collecting detailed product information
    ProductDetail,
    /// Saving data to database
    Database,
}

impl std::fmt::Display for CrawlingStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CrawlingStage::Idle => write!(f, "대기"),
            CrawlingStage::TotalPages => write!(f, "총 페이지 수 확인"),
            CrawlingStage::ProductList => write!(f, "제품 목록 수집"),
            CrawlingStage::ProductDetail => write!(f, "제품 상세정보 수집"),
            CrawlingStage::Database => write!(f, "데이터베이스 저장"),
        }
    }
}

/// Overall status of the crawling operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CrawlingStatus {
    /// No crawling operation is running
    Idle,
    /// Crawling is actively running
    Running,
    /// Crawling is temporarily paused
    Paused,
    /// Crawling completed successfully
    Completed,
    /// Crawling stopped due to error
    Error,
    /// Crawling was cancelled by user
    Cancelled,
}

/// Detailed progress information for the entire crawling operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingProgress {
    /// Current progress count
    pub current: u32,
    /// Total expected items to process
    pub total: u32,
    /// Progress percentage (0.0 to 100.0)
    pub percentage: f64,
    /// Current stage of crawling
    pub current_stage: CrawlingStage,
    /// Human-readable description of current step
    pub current_step: String,
    /// Overall status
    pub status: CrawlingStatus,
    /// Status message for display
    pub message: String,
    /// Estimated remaining time in seconds
    pub remaining_time: Option<u64>,
    /// Elapsed time in seconds since start
    pub elapsed_time: u64,
    /// Number of new items discovered
    pub new_items: u32,
    /// Number of items updated
    pub updated_items: u32,
    /// Current batch being processed
    pub current_batch: Option<u32>,
    /// Total number of batches
    pub total_batches: Option<u32>,
    /// Number of errors encountered
    pub errors: u32,
    /// Timestamp of this progress update
    pub timestamp: DateTime<Utc>,
}

impl Default for CrawlingProgress {
    fn default() -> Self {
        Self {
            current: 0,
            total: 0,
            percentage: 0.0,
            current_stage: CrawlingStage::Idle,
            current_step: "대기 중".to_string(),
            status: CrawlingStatus::Idle,
            message: "크롤링이 시작되지 않았습니다".to_string(),
            remaining_time: None,
            elapsed_time: 0,
            new_items: 0,
            updated_items: 0,
            current_batch: None,
            total_batches: None,
            errors: 0,
            timestamp: Utc::now(),
        }
    }
}

/// Individual task status within a parallel crawling operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingTaskStatus {
    /// Unique identifier for the task
    pub task_id: String,
    /// URL being processed by this task
    pub url: String,
    /// Current status of the task
    pub status: TaskStatus,
    /// Status message
    pub message: String,
    /// Timestamp when status was updated
    pub timestamp: DateTime<Utc>,
    /// Current stage this task is in
    pub stage: CrawlingStage,
    /// Additional details specific to the task
    pub details: Option<HashMap<String, serde_json::Value>>,
}

/// Status of an individual crawling task
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    /// Task is waiting to be processed
    Pending,
    /// Task is currently being processed
    Running,
    /// Task completed successfully
    Completed,
    /// Task failed with error
    Failed,
    /// Task was cancelled
    Cancelled,
}

/// Database statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStats {
    /// Total number of products in database
    pub total_products: u64,
    /// Total number of devices in database  
    pub total_devices: u64,
    /// Last update timestamp
    pub last_updated: DateTime<Utc>,
    /// Estimated storage size
    pub storage_size: String,
    /// Number of incomplete records
    pub incomplete_records: u64,
    /// Database health status
    pub health_status: DatabaseHealth,
}

/// Database health indicators
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DatabaseHealth {
    /// Database is operating normally
    Healthy,
    /// Database has minor issues
    Warning,
    /// Database has serious issues
    Critical,
}

/// Summary of crawling results after completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingResult {
    /// Total number of items processed
    pub total_processed: u32,
    /// Number of new items added
    pub new_items: u32,
    /// Number of existing items updated
    pub updated_items: u32,
    /// Number of errors encountered
    pub errors: u32,
    /// Duration of crawling operation in milliseconds
    pub duration_ms: u64,
    /// Stages that were completed
    pub stages_completed: Vec<CrawlingStage>,
    /// Start time of the operation
    pub start_time: DateTime<Utc>,
    /// End time of the operation
    pub end_time: DateTime<Utc>,
    /// Performance metrics
    pub performance_metrics: PerformanceMetrics,
}

/// Performance metrics for crawling operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Average processing time per item in milliseconds
    pub avg_processing_time_ms: f64,
    /// Items processed per second
    pub items_per_second: f64,
    /// Memory usage in MB
    pub memory_usage_mb: f64,
    /// Network requests made
    pub network_requests: u64,
    /// Cache hit rate percentage
    pub cache_hit_rate: f64,
}

/// Event types that can be emitted to the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum CrawlingEvent {
    /// Progress update event
    ProgressUpdate(CrawlingProgress),
    /// Individual task status update
    TaskUpdate(CrawlingTaskStatus),
    /// Stage change notification
    StageChange {
        from: CrawlingStage,
        to: CrawlingStage,
        message: String,
    },
    /// Error notification
    Error {
        error_id: String,
        message: String,
        stage: CrawlingStage,
        recoverable: bool,
    },
    /// Database statistics update
    DatabaseUpdate(DatabaseStats),
    /// Final results notification
    Completed(CrawlingResult),
}

impl CrawlingEvent {
    /// Get the event type as a string for Tauri event emission
    pub fn event_name(&self) -> &'static str {
        match self {
            CrawlingEvent::ProgressUpdate(_) => "crawling-progress",
            CrawlingEvent::TaskUpdate(_) => "crawling-task-update",
            CrawlingEvent::StageChange { .. } => "crawling-stage-change",
            CrawlingEvent::Error { .. } => "crawling-error",
            CrawlingEvent::DatabaseUpdate(_) => "database-update",
            CrawlingEvent::Completed(_) => "crawling-completed",
        }
    }
}
