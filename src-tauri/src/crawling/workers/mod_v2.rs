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

/// Modern Rust 2024 - Generic Worker Trait
#[async_trait]
pub trait Worker<T>: Send + Sync + 'static
where
    T: Send + Sync + 'static,
{
    /// Worker 식별자
    fn worker_id(&self) -> &'static str;

    /// 최대 동시 실행 태스크 수
    fn max_concurrency(&self) -> usize;

    /// 태스크 처리 (핵심 비즈니스 로직)
    async fn process_task(
        &self,
        task: T,
        shared_state: Arc<SharedState>,
    ) -> Result<TaskResult, WorkerError>;

    /// 헬스 체크
    async fn health_check(&self) -> Result<WorkerHealthStatus, WorkerError> {
        Ok(WorkerHealthStatus::Healthy)
    }

    /// 워커 통계 정보
    fn get_metrics(&self) -> WorkerMetrics {
        WorkerMetrics::default()
    }
}

/// 워커 헬스 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerHealthStatus {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String },
}

/// 워커 메트릭스
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct WorkerMetrics {
    pub tasks_processed: u64,
    pub tasks_failed: u64,
    pub average_duration: Duration,
    pub last_task_completed: Option<chrono::DateTime<chrono::Utc>>,
}

/// Modern Rust 2024 - Type-Safe Worker Pool
#[derive(Clone)]
pub struct WorkerPool {
    list_page_fetcher: Arc<ListPageFetcher>,
    list_page_parser: Arc<ListPageParser>,
    product_detail_fetcher: Arc<ProductDetailFetcher>,
    product_detail_parser: Arc<ProductDetailParser>,
    db_saver: Arc<DbSaver>,
    max_total_concurrency: usize,
    metrics: Arc<tokio::sync::RwLock<WorkerPoolMetrics>>,
}

impl WorkerPool {
    /// Clean Architecture 생성자 - 의존성 주입
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
            metrics: Arc::new(tokio::sync::RwLock::new(WorkerPoolMetrics::default())),
        }
    }

    /// 타입 안전한 태스크 라우팅
    pub async fn process_task(
        &self,
        task: CrawlingTask,
        shared_state: Arc<SharedState>,
    ) -> Result<TaskResult, WorkerError> {
        let start_time = std::time::Instant::now();
        
        let result = match task {
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
        };

        // 메트릭스 업데이트
        self.update_metrics(start_time, &result).await;
        
        result
    }

    /// 워커 풀 헬스 체크
    pub async fn health_check(&self) -> WorkerPoolHealthStatus {
        let mut statuses = Vec::new();
        
        // 모든 워커의 헬스 체크
        statuses.push(("list_page_fetcher", self.list_page_fetcher.health_check().await));
        statuses.push(("list_page_parser", self.list_page_parser.health_check().await));
        statuses.push(("product_detail_fetcher", self.product_detail_fetcher.health_check().await));
        statuses.push(("product_detail_parser", self.product_detail_parser.health_check().await));
        statuses.push(("db_saver", self.db_saver.health_check().await));
        
        let healthy_count = statuses.iter()
            .filter(|(_, status)| matches!(status, Ok(WorkerHealthStatus::Healthy)))
            .count();
            
        WorkerPoolHealthStatus {
            total_workers: 5,
            healthy_workers: healthy_count,
            worker_statuses: statuses.into_iter()
                .map(|(name, status)| (name.to_string(), status.unwrap_or(WorkerHealthStatus::Unhealthy { reason: "Unknown error".to_string() })))
                .collect(),
        }
    }

    /// 워커 풀 통계
    pub async fn get_statistics(&self) -> WorkerPoolStats {
        let metrics = self.metrics.read().await;
        WorkerPoolStats {
            list_page_fetcher_concurrency: self.list_page_fetcher.max_concurrency(),
            list_page_parser_concurrency: self.list_page_parser.max_concurrency(),
            product_detail_fetcher_concurrency: self.product_detail_fetcher.max_concurrency(),
            product_detail_parser_concurrency: self.product_detail_parser.max_concurrency(),
            db_saver_concurrency: self.db_saver.max_concurrency(),
            total_max_concurrency: self.max_total_concurrency,
            total_tasks_processed: metrics.total_tasks_processed,
            total_tasks_failed: metrics.total_tasks_failed,
            average_processing_time: metrics.average_processing_time,
        }
    }

    /// 메트릭스 업데이트 (내부 메서드)
    async fn update_metrics(&self, start_time: std::time::Instant, result: &Result<TaskResult, WorkerError>) {
        let mut metrics = self.metrics.write().await;
        let duration = start_time.elapsed();
        
        metrics.total_tasks_processed += 1;
        if result.is_err() {
            metrics.total_tasks_failed += 1;
        }
        
        // 평균 처리 시간 계산
        if metrics.total_tasks_processed == 1 {
            metrics.average_processing_time = duration;
        } else {
            let total_time = metrics.average_processing_time * (metrics.total_tasks_processed - 1) + duration;
            metrics.average_processing_time = total_time / metrics.total_tasks_processed as u32;
        }
        
        metrics.last_task_completed = Some(chrono::Utc::now());
    }
}

/// 워커 풀 헬스 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerPoolHealthStatus {
    pub total_workers: usize,
    pub healthy_workers: usize,
    pub worker_statuses: std::collections::HashMap<String, WorkerHealthStatus>,
}

/// 워커 풀 통계
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerPoolStats {
    pub list_page_fetcher_concurrency: usize,
    pub list_page_parser_concurrency: usize,
    pub product_detail_fetcher_concurrency: usize,
    pub product_detail_parser_concurrency: usize,
    pub db_saver_concurrency: usize,
    pub total_max_concurrency: usize,
    pub total_tasks_processed: u64,
    pub total_tasks_failed: u64,
    pub average_processing_time: Duration,
}

/// 워커 풀 메트릭스 (내부 사용)
#[derive(Debug, Default, Clone)]
struct WorkerPoolMetrics {
    total_tasks_processed: u64,
    total_tasks_failed: u64,
    average_processing_time: Duration,
    last_task_completed: Option<chrono::DateTime<chrono::Utc>>,
}

/// Modern Rust 2024 - Builder Pattern for WorkerPool
pub struct WorkerPoolBuilder {
    max_total_concurrency: usize,
    list_page_fetcher: Option<Arc<ListPageFetcher>>,
    list_page_parser: Option<Arc<ListPageParser>>,
    product_detail_fetcher: Option<Arc<ProductDetailFetcher>>,
    product_detail_parser: Option<Arc<ProductDetailParser>>,
    db_saver: Option<Arc<DbSaver>>,
}

impl WorkerPoolBuilder {
    pub fn new() -> Self {
        Self {
            max_total_concurrency: crate::infrastructure::config::defaults::MAX_CONCURRENT_REQUESTS as usize,
            list_page_fetcher: None,
            list_page_parser: None,
            product_detail_fetcher: None,
            product_detail_parser: None,
            db_saver: None,
        }
    }

    pub fn with_max_concurrency(mut self, max_concurrency: usize) -> Self {
        self.max_total_concurrency = max_concurrency;
        self
    }

    pub fn with_list_page_fetcher(mut self, worker: Arc<ListPageFetcher>) -> Self {
        self.list_page_fetcher = Some(worker);
        self
    }

    pub fn with_list_page_parser(mut self, worker: Arc<ListPageParser>) -> Self {
        self.list_page_parser = Some(worker);
        self
    }

    pub fn with_product_detail_fetcher(mut self, worker: Arc<ProductDetailFetcher>) -> Self {
        self.product_detail_fetcher = Some(worker);
        self
    }

    pub fn with_product_detail_parser(mut self, worker: Arc<ProductDetailParser>) -> Self {
        self.product_detail_parser = Some(worker);
        self
    }

    pub fn with_db_saver(mut self, worker: Arc<DbSaver>) -> Self {
        self.db_saver = Some(worker);
        self
    }

    pub fn build(self) -> Result<WorkerPool, WorkerError> {
        let list_page_fetcher = self.list_page_fetcher
            .ok_or_else(|| WorkerError::ConfigurationError("ListPageFetcher not configured".to_string()))?;
        let list_page_parser = self.list_page_parser
            .ok_or_else(|| WorkerError::ConfigurationError("ListPageParser not configured".to_string()))?;
        let product_detail_fetcher = self.product_detail_fetcher
            .ok_or_else(|| WorkerError::ConfigurationError("ProductDetailFetcher not configured".to_string()))?;
        let product_detail_parser = self.product_detail_parser
            .ok_or_else(|| WorkerError::ConfigurationError("ProductDetailParser not configured".to_string()))?;
        let db_saver = self.db_saver
            .ok_or_else(|| WorkerError::ConfigurationError("DbSaver not configured".to_string()))?;

        Ok(WorkerPool::new(
            list_page_fetcher,
            list_page_parser,
            product_detail_fetcher,
            product_detail_parser,
            db_saver,
            self.max_total_concurrency,
        ))
    }
}

impl Default for WorkerPoolBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_pool_builder() {
        let builder = WorkerPoolBuilder::new()
            .with_max_concurrency(20);
        
        // 빌더 패턴이 올바르게 동작하는지 확인
        assert_eq!(builder.max_total_concurrency, 20);
    }

    #[test]
    fn test_worker_error_serialization() {
        let error = WorkerError::NetworkError("Test error".to_string());
        let serialized = serde_json::to_string(&error).unwrap();
        let deserialized: WorkerError = serde_json::from_str(&serialized).unwrap();
        
        match deserialized {
            WorkerError::NetworkError(msg) => assert_eq!(msg, "Test error"),
            _ => panic!("Wrong error type"),
        }
    }
}
