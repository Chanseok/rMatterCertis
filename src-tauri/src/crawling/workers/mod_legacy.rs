//! # Worker Pool Module v2.0
//!
//! Clean Architecture + Modern Rust 2024 패턴 적용
//! - 도메인 로직과 인프라스트럭처 분리
//! - 의존성 역전 원칙 준수
//! - 타입 안전성 및 테스트 가능한 구조

use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use thiserror::Error;
use serde::{Deserialize, Serialize};

use crate::crawling::tasks::{TaskId, TaskResult, TaskOutput, CrawlingTask};
use crate::crawling::state::SharedState;

// Re-export worker implementations
pub mod list_page_fetcher;
pub mod list_page_parser; 
pub mod product_detail_fetcher;
pub mod product_detail_parser;
pub mod db_saver;

pub use list_page_fetcher::ListPageFetcher;
pub use list_page_parser::ListPageParser;
pub use product_detail_fetcher::ProductDetailFetcher;
pub use product_detail_parser::ProductDetailParser;
pub use db_saver::DbSaver;

/// Clean Architecture 워커 에러 타입
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum WorkerError {
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    #[error("Timeout error: {0}")]
    TimeoutError(String),
    
    #[error("Rate limit error: {0}")]
    RateLimitError(String),
    
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    
    #[error("Validation error: {0}")]
    ValidationError(String),
    
    #[error("HTTP error {0}: {1}")]
    HttpError(u16, String),
    
    #[error("Initialization error: {0}")]
    InitializationError(String),
    
    #[error("Task was cancelled")]
    Cancelled,
    
    #[error("Worker unavailable: {0}")]
    WorkerUnavailable(String),
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}
    
    #[error("Initialization error: {0}")]
    InitializationError(String),
    
    #[error("Task was cancelled")]
    Cancelled,
}

/// Result type for task processing
#[derive(Debug, Clone)]
pub enum TaskResult {
    Success {
        task_id: TaskId,
        output: TaskOutput,
        duration: Duration,
    },
    Failure {
        task_id: TaskId,
        error: String,
        duration: Duration,
    },
}

/// Output types for different task results
#[derive(Debug, Clone)]
pub enum TaskOutput {
    /// HTML content from a fetched page
    Html(String),
    
    /// List of product URLs extracted from a page
    ProductUrls(Vec<String>),
    
    /// Product detail HTML with metadata
    ProductDetailHtml {
        product_id: String,
        html_content: String,
        source_url: String,
    },
    
    /// Parsed product data
    ProductData(crate::domain::value_objects::ProductData),
    
    /// Confirmation of successful save
    SaveConfirmation {
        product_id: String,
        saved_at: chrono::DateTime<chrono::Utc>,
    },
}

/// Trait for all workers in the system
#[async_trait]
pub trait Worker: Send + Sync {
    /// The type of task this worker processes
    type Task: Send + Sync;

    /// Process a single task
    async fn process_task(
        &self,
        task: Self::Task,
        shared_state: Arc<SharedState>,
    ) -> Result<TaskResult, WorkerError>;

    /// Get the worker's name for logging and monitoring
    fn worker_name(&self) -> &'static str;

    /// Get the maximum number of concurrent tasks this worker can handle
    fn max_concurrency(&self) -> usize;
}

/// Worker pool manager with task routing
pub struct WorkerPool {
    list_page_fetcher: Arc<ListPageFetcher>,
    list_page_parser: Arc<ListPageParser>,
    product_detail_fetcher: Arc<ProductDetailFetcher>,
    product_detail_parser: Arc<ProductDetailParser>,
    db_saver: Arc<DbSaver>,
    max_total_concurrency: usize,
}

impl WorkerPool {
    /// Create a new worker pool with all worker types
    pub fn new(
        list_page_fetcher: Arc<ListPageFetcher>,
        list_page_parser: Arc<ListPageParser>,
        product_detail_fetcher: Arc<ProductDetailFetcher>,
        product_detail_parser: Arc<ProductDetailParser>,
        db_saver: Arc<DbSaver>,
        max_total_concurrency: usize,
    ) -> Self {
        Self {
            list_page_fetcher,
            list_page_parser,
            product_detail_fetcher,
            product_detail_parser,
            db_saver,
            max_total_concurrency,
        }
    }

    /// Get the total number of worker types
    pub fn worker_count(&self) -> usize {
        5 // Number of different worker types
    }

    /// Get the maximum concurrency across all workers
    pub fn max_concurrency(&self) -> usize {
        self.max_total_concurrency
    }

    /// Process a task with the appropriate worker
    pub async fn process_task(
        &self,
        task: CrawlingTask,
        shared_state: Arc<SharedState>,
    ) -> Result<TaskResult, WorkerError> {
        match task {
            CrawlingTask::FetchListPage { .. } => {
                self.list_page_fetcher.process_task(task, shared_state).await
            }
            CrawlingTask::ParseListPage { .. } => {
                self.list_page_parser.process_task(task, shared_state).await
            }
            CrawlingTask::FetchProductDetail { .. } => {
                self.product_detail_fetcher.process_task(task, shared_state).await
            }
            CrawlingTask::ParseProductDetail { .. } => {
                self.product_detail_parser.process_task(task, shared_state).await
            }
            CrawlingTask::SaveProduct { .. } => {
                self.db_saver.process_task(task, shared_state).await
            }
        }
    }

    /// Get worker statistics
    pub fn get_worker_stats(&self) -> WorkerPoolStats {
        WorkerPoolStats {
            list_page_fetcher_concurrency: self.list_page_fetcher.max_concurrency(),
            list_page_parser_concurrency: self.list_page_parser.max_concurrency(),
            product_detail_fetcher_concurrency: self.product_detail_fetcher.max_concurrency(),
            product_detail_parser_concurrency: self.product_detail_parser.max_concurrency(),
            db_saver_concurrency: self.db_saver.max_concurrency(),
            total_max_concurrency: self.max_total_concurrency,
        }
    }
}

/// Statistics for the worker pool
#[derive(Debug, Clone)]
pub struct WorkerPoolStats {
    pub list_page_fetcher_concurrency: usize,
    pub list_page_parser_concurrency: usize,
    pub product_detail_fetcher_concurrency: usize,
    pub product_detail_parser_concurrency: usize,
    pub db_saver_concurrency: usize,
    pub total_max_concurrency: usize,
}
