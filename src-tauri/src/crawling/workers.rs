//! # Worker Pool Module v2.0
//!
//! Modern Rust 2024 + Clean Architecture 패턴 적용
//! - 명시적 모듈 구조 (mod.rs 비사용)
//! - 도메인 로직과 인프라스트럭처 분리
//! - 의존성 역전 원칙 준수
//! - 타입 안전성 및 테스트 가능한 구조

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

use crate::crawling::state::SharedState;
use crate::crawling::tasks::{CrawlingTask, TaskResult};

// Re-export for public API (without duplicates)
pub use crate::crawling::tasks::TaskOutput as PublicTaskOutput;
pub use crate::crawling::tasks::TaskResult as PublicTaskResult;

// Modern Rust 2024 - 명시적 모듈 선언 (하위 모듈들)
pub mod db_saver_sqlx; // 실제 SQLX 구현
pub mod list_page_fetcher;
pub mod list_page_parser;
pub mod mock_db_saver;
pub mod product_detail_fetcher;
pub mod product_detail_parser; // 개발용 Mock (임시 유지)

pub use db_saver_sqlx::DbSaver; // 실제 SQLX 구현 사용
pub use list_page_fetcher::ListPageFetcher;
pub use list_page_parser::ListPageParser;
pub use mock_db_saver::MockDbSaver;
pub use product_detail_fetcher::ProductDetailFetcher;
pub use product_detail_parser::ProductDetailParser; // Mock 추가 (단계적 제거 예정)

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

    #[error("Task timeout: {message}")]
    Timeout { message: String },

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
    /// Associated task type
    type Task;

    /// Worker 식별자
    fn worker_id(&self) -> &'static str;

    /// Worker 이름 (디버깅용)
    fn worker_name(&self) -> &'static str;

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
    db_saver: Arc<DbSaver>, // 구체적 타입으로 변경
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
        db_saver: Arc<DbSaver>, // 구체적 타입으로 변경
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
                self.list_page_fetcher
                    .process_task(task, shared_state)
                    .await
            }
            CrawlingTask::ParseListPage { .. } => {
                self.list_page_parser.process_task(task, shared_state).await
            }
            CrawlingTask::FetchProductDetail { .. } => {
                self.product_detail_fetcher
                    .process_task(task, shared_state)
                    .await
            }
            CrawlingTask::ParseProductDetail { .. } => {
                self.product_detail_parser
                    .process_task(task, shared_state)
                    .await
            }
            CrawlingTask::SaveProduct { .. } => {
                self.db_saver.process_task(task, shared_state).await
            }
        };

        // 메트릭스 업데이트
        self.update_metrics(start_time, &result).await;

        result
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

    /// 워커 접근자 메서드들
    pub fn list_page_fetcher(&self) -> &Arc<ListPageFetcher> {
        &self.list_page_fetcher
    }

    pub fn list_page_parser(&self) -> &Arc<ListPageParser> {
        &self.list_page_parser
    }

    pub fn product_detail_fetcher(&self) -> &Arc<ProductDetailFetcher> {
        &self.product_detail_fetcher
    }

    pub fn product_detail_parser(&self) -> &Arc<ProductDetailParser> {
        &self.product_detail_parser
    }

    pub fn db_saver(&self) -> &Arc<DbSaver> {
        &self.db_saver
    }

    /// 메트릭스 업데이트 (내부 메서드)
    async fn update_metrics(
        &self,
        start_time: std::time::Instant,
        result: &Result<TaskResult, WorkerError>,
    ) {
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
            // u32로 안전하게 캐스팅하여 Duration 산술 연산 수행
            let count_minus_one =
                std::cmp::min(metrics.total_tasks_processed - 1, u32::MAX as u64) as u32;
            let count = std::cmp::min(metrics.total_tasks_processed, u32::MAX as u64) as u32;

            let total_time = metrics.average_processing_time * count_minus_one + duration;
            metrics.average_processing_time = total_time / count;
        }

        metrics.last_task_completed = Some(chrono::Utc::now());
    }
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
    db_saver: Option<Arc<DbSaver>>, // 구체적 타입으로 변경
}

impl WorkerPoolBuilder {
    pub fn new() -> Self {
        Self {
            max_total_concurrency: 10,
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
        let list_page_fetcher = self.list_page_fetcher.ok_or_else(|| {
            WorkerError::ConfigurationError("ListPageFetcher not configured".to_string())
        })?;
        let list_page_parser = self.list_page_parser.ok_or_else(|| {
            WorkerError::ConfigurationError("ListPageParser not configured".to_string())
        })?;
        let product_detail_fetcher = self.product_detail_fetcher.ok_or_else(|| {
            WorkerError::ConfigurationError("ProductDetailFetcher not configured".to_string())
        })?;
        let product_detail_parser = self.product_detail_parser.ok_or_else(|| {
            WorkerError::ConfigurationError("ProductDetailParser not configured".to_string())
        })?;
        let db_saver = self
            .db_saver
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
        let builder = WorkerPoolBuilder::new().with_max_concurrency(20);

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
