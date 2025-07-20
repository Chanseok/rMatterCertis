//! # Crawling Domain Module v2.0
//!
//! Modern Rust 2024 + Clean Architecture
//! - 명시적 모듈 구조 (mod.rs 비사용)
//! - 도메인 로직 중심 설계
//! - 의존성 역전 원칙 준수
//! - 테스트 가능한 구조
//! - 인프라스트럭처 분리

use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{info, error};

// Modern Rust 2024 - 명시적 모듈 선언 (mod.rs 비사용)
pub mod tasks;
pub mod state;
pub mod queues;
pub mod workers;
pub mod orchestrator;

// Clean re-exports
pub use tasks::{TaskId, CrawlingTask, TaskResult, TaskOutput, TaskProductData};
pub use state::{SharedState, CrawlingStats, CrawlingConfig};
pub use queues::{QueueManager, TaskQueue, QueueError};
pub use workers::{WorkerPool, WorkerError, WorkerPoolStats, WorkerPoolBuilder};
pub use orchestrator::{CrawlingOrchestrator, OrchestratorConfig, OrchestratorError};

/// Clean Architecture - 크롤링 엔진 도메인 서비스
#[derive(Clone)]
pub struct CrawlingEngine {
    orchestrator: Arc<CrawlingOrchestrator>,
    shared_state: Arc<SharedState>,
    is_running: Arc<RwLock<bool>>,
    config: CrawlingConfig,
}

/// 크롤링 엔진 에러 타입
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum CrawlingEngineError {
    #[error("Engine initialization failed: {0}")]
    InitializationError(String),
    
    #[error("Engine is already running")]
    AlreadyRunning,
    
    #[error("Engine is not running")]
    NotRunning,
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    
    #[error("Worker pool error: {0}")]
    WorkerPoolError(String),
    
    #[error("Orchestrator error: {0}")]
    OrchestratorError(String),
    
    #[error("Database connection error: {0}")]
    DatabaseError(String),
    
    #[error("Database connection error: {0}")]
    DatabaseConnectionError(String),
    
    #[error("Shutdown timeout exceeded")]
    ShutdownTimeout,
}

/// 크롤링 엔진 팩토리 - 의존성 주입 패턴
pub struct CrawlingEngineFactory;

impl CrawlingEngineFactory {
    /// Clean Architecture 기반 엔진 생성
    pub async fn create_engine(
        config: CrawlingConfig,
        dependencies: CrawlingEngineDependencies,
    ) -> Result<CrawlingEngine, CrawlingEngineError> {
        // 1. 공유 상태 초기화
        let shared_state = Arc::new(SharedState::new(config.clone()));
        
        // 2. 큐 매니저 초기화 (설정값 사용)
        let queue_manager = Arc::new(QueueManager::new_with_config(
            config.max_queue_size,
            config.backpressure_threshold,
        ));
        
        // 3. 워커 풀 생성 - 현재는 사용되지 않는 Factory 패턴 (주석 처리)
        // let worker_pool = WorkerPoolBuilder::new()
        //     .with_max_concurrency(config.max_concurrent_requests)
        //     .with_list_page_fetcher(Arc::new(workers::ListPageFetcher::new_simple()))
        //     .with_list_page_parser(Arc::new(workers::ListPageParser::new_simple()))
        //     .with_product_detail_fetcher(Arc::new(workers::ProductDetailFetcher::new_simple()))
        //     .with_product_detail_parser(Arc::new(workers::ProductDetailParser::new_simple()))
        //     .with_db_saver(Arc::new(workers::DbSaver::new(...)))  // 실제 DB 필요
        //     .build()
        //     .map_err(|e| CrawlingEngineError::WorkerPoolError(e.to_string()))?;
        
        // Factory 패턴은 deprecated됨. with_config 또는 with_config_and_db 사용 권장
        Err(CrawlingEngineError::ConfigurationError(
            "Factory pattern is deprecated. Use with_config or with_config_and_db instead.".to_string()
        ))
    }
}

/// 의존성 주입을 위한 구조체
pub struct CrawlingEngineDependencies {
    pub list_page_fetcher: Arc<workers::ListPageFetcher>,
    pub list_page_parser: Arc<workers::ListPageParser>,
    pub product_detail_fetcher: Arc<workers::ProductDetailFetcher>,
    pub product_detail_parser: Arc<workers::ProductDetailParser>,
    pub db_saver: Arc<workers::DbSaver>,
}

impl CrawlingEngine {
    /// 크롤링 시작 (도메인 로직)
    pub async fn start(&self) -> Result<(), CrawlingEngineError> {
        let mut is_running = self.is_running.write().await;
        
        if *is_running {
            return Err(CrawlingEngineError::AlreadyRunning);
        }
        
        tracing::info!("Starting crawling engine...");
        
        // 오케스트레이터 시작
        self.orchestrator.start().await
            .map_err(|e| CrawlingEngineError::OrchestratorError(e.to_string()))?;
        
        *is_running = true;
        
        tracing::info!("Crawling engine started successfully");
        Ok(())
    }
    
    /// 크롤링 중지 (도메인 로직)
    pub async fn stop(&self) -> Result<(), CrawlingEngineError> {
        let mut is_running = self.is_running.write().await;
        
        if !*is_running {
            tracing::warn!("🟡 Crawling engine is already stopped");
            return Ok(()); // 이미 중지된 상태면 성공으로 처리
        }
        
        tracing::info!("🛑 Stopping crawling engine...");
        
        // 오케스트레이터 중지
        match self.orchestrator.stop().await {
            Ok(()) => {
                tracing::info!("✅ Orchestrator stopped successfully");
            }
            Err(e) => {
                tracing::error!("❌ Error stopping orchestrator: {}", e);
                // 오케스트레이터 중지 실패해도 엔진 상태는 중지로 설정
            }
        }
        
        *is_running = false;
        
        tracing::info!("✅ Crawling engine stopped successfully");
        Ok(())
    }
    
    /// 엔진 상태 확인
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }
    
    /// 크롤링 통계 조회
    pub async fn get_stats(&self) -> CrawlingStats {
        let stats = self.shared_state.stats.read().await;
        stats.clone()
    }
    
    /// 설정 조회
    pub fn get_config(&self) -> &CrawlingConfig {
        &self.config
    }
    
    /// 설정 업데이트
    pub async fn update_config(&mut self, new_config: CrawlingConfig) -> Result<(), CrawlingEngineError> {
        if self.is_running().await {
            return Err(CrawlingEngineError::ConfigurationError(
                "Cannot update configuration while engine is running".to_string()
            ));
        }
        
        self.config = new_config.clone();
        // Update shared state configuration as needed
        // For now, we'll just update local config
        Ok(())
    }
    
    /// 태스크 추가
    pub async fn add_task(&self, task: CrawlingTask) -> Result<(), CrawlingEngineError> {
        if !self.is_running().await {
            return Err(CrawlingEngineError::NotRunning);
        }
        
        self.orchestrator.add_initial_task(task).await
            .map_err(|e| CrawlingEngineError::OrchestratorError(e.to_string()))?;
        
        Ok(())
    }
    
    /// 크롤링 세션 시작
    pub async fn start_crawling_session(
        &self,
        start_page: u32,
        end_page: u32,
    ) -> Result<(), CrawlingEngineError> {
        if !self.is_running().await {
            return Err(CrawlingEngineError::NotRunning);
        }
        
        tracing::info!("🚀 Starting crawling session: pages {} to {}", start_page, end_page);
        
        // 페이지 범위 유효성 검사
        if start_page == 0 || end_page == 0 {
            return Err(CrawlingEngineError::ConfigurationError(
                "Page numbers must be greater than 0".to_string()
            ));
        }
        
        // Matter Certification Products URL 패턴 사용
        const MATTER_PRODUCTS_URL_PATTERN: &str = "https://csa-iot.org/csa-iot_products/page/{}/?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver";
        
        // 페이지 범위 처리: start_page가 end_page보다 클 수 있음 (최신 페이지부터 크롤링)
        let (min_page, max_page) = if start_page <= end_page {
            (start_page, end_page)
        } else {
            (end_page, start_page)
        };
        
        let total_pages = max_page - min_page + 1;
        tracing::info!("📋 Creating {} tasks for pages {} to {}", total_pages, min_page, max_page);
        
        // 페이지 범위에 따른 태스크 생성
        for page in min_page..=max_page {
            let url = MATTER_PRODUCTS_URL_PATTERN.replace("{}", &page.to_string());
            let task = CrawlingTask::FetchListPage {
                task_id: TaskId::new(),
                page_number: page,
                url: url.clone(),
            };
            
            tracing::info!("✅ Created task for page {}: {}", page, url);
            self.add_task(task).await?;
        }
        
        tracing::info!("🎯 All {} tasks created successfully", total_pages);
        Ok(())
    }
    
    /// 응급 중지
    pub async fn emergency_stop(&self) -> Result<(), CrawlingEngineError> {
        tracing::warn!("Emergency stop requested");
        
        // 강제 중지
        *self.is_running.write().await = false;
        
        // 오케스트레이터 중지
        self.orchestrator.stop().await
            .map_err(|e| CrawlingEngineError::OrchestratorError(e.to_string()))?;
        
        tracing::warn!("Emergency stop completed");
        Ok(())
    }
    
    /// 시스템 헬스 체크
    pub async fn health_check(&self) -> CrawlingEngineHealth {
        let is_running = self.is_running().await;
        let stats = self.get_stats().await;
        
        CrawlingEngineHealth {
            is_running,
            stats,
            orchestrator_health: self.orchestrator.get_stats().await,
            last_check: chrono::Utc::now(),
        }
    }
    
    /// 새로운 크롤링 엔진을 생성합니다
    pub async fn new(
        _database_pool: sqlx::Pool<sqlx::Sqlite>, 
        config: CrawlingConfig
    ) -> Result<Self, CrawlingEngineError> {
        // 개발 용이성을 위해 Mock 데이터베이스 사용
        info!("개발 모드: Mock 데이터베이스를 사용합니다");
        
        Self::with_config(config).await
    }
    
    /// 설정과 데이터베이스 연결을 사용하여 크롤링 엔진을 생성합니다
    pub async fn with_config_and_db(
        config: CrawlingConfig,
        database_pool: sqlx::SqlitePool,
    ) -> Result<Self, CrawlingEngineError> {
        // 1. 공유 상태 초기화
        let shared_state = Arc::new(SharedState::new(config.clone()));
        
        // 2. 큐 매니저 초기화
        let queue_manager = Arc::new(QueueManager::new_with_config(
            config.max_queue_size,
            config.backpressure_threshold,
        ));
        
        // 3. 워커 풀 생성 (실제 데이터베이스 연결 사용)
        let db_saver = Arc::new(workers::DbSaver::new(
            database_pool,
            100, // batch_size
            Duration::from_secs(30), // flush_interval
        ));
        
        let worker_pool = WorkerPoolBuilder::new()
            .with_max_concurrency(config.max_concurrent_tasks)
            .with_list_page_fetcher(Arc::new(workers::ListPageFetcher::new_simple()))
            .with_list_page_parser(Arc::new(workers::ListPageParser::new_simple()))
            .with_product_detail_fetcher(Arc::new(workers::ProductDetailFetcher::new_simple()))
            .with_product_detail_parser(Arc::new(workers::ProductDetailParser::new_simple()))
            .with_db_saver(db_saver)
            .build()
            .map_err(|e| CrawlingEngineError::WorkerPoolError(e.to_string()))?;
        
        // 4. 오케스트레이터 설정
        let orchestrator_config = OrchestratorConfig {
            max_global_concurrency: config.max_concurrent_tasks,
            scheduler_interval: Duration::from_millis(config.scheduler_interval_ms),
            shutdown_timeout: Duration::from_secs(config.shutdown_timeout_seconds),
            stats_interval: Duration::from_secs(config.stats_interval_seconds),
            auto_retry_enabled: config.auto_retry_enabled,
            max_retries: config.max_retries,
            retry_delay: Duration::from_millis(config.retry_delay_ms),
            backpressure_enabled: config.backpressure_enabled,
            backpressure_threshold: 1000, // Default value
        };
        
        // 5. 오케스트레이터 생성
        let orchestrator = Arc::new(CrawlingOrchestrator::new(
            Arc::new(worker_pool),
            queue_manager,
            shared_state.clone(),
            orchestrator_config,
        ));
        
        // 6. 엔진 생성
        let engine = CrawlingEngine {
            orchestrator,
            shared_state,
            is_running: Arc::new(RwLock::new(false)),
            config,
        };
        
        tracing::info!("크롤링 엔진 생성 완료 (실제 데이터베이스 연결)");
        Ok(engine)
    }

    /// 설정을 사용하여 크롤링 엔진을 생성합니다 (인메모리 SQLite DB 사용)
    pub async fn with_config(config: CrawlingConfig) -> Result<Self, CrawlingEngineError> {
        // 개발용 인메모리 SQLite 데이터베이스 연결
        let database_pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .map_err(|e| CrawlingEngineError::DatabaseConnectionError(e.to_string()))?;
        
        // 테이블 생성 (기본 스키마)
        sqlx::query(r"
            CREATE TABLE IF NOT EXISTS products (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                product_id TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                category TEXT,
                manufacturer TEXT,
                model TEXT,
                certification_number TEXT,
                certification_date TEXT,
                source_url TEXT NOT NULL,
                extracted_at TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
        ")
        .execute(&database_pool)
        .await
        .map_err(|e| CrawlingEngineError::DatabaseConnectionError(format!("Failed to create tables: {}", e)))?;
        
        // 실제 데이터베이스 연결을 사용하여 엔진 생성
        Self::with_config_and_db(config, database_pool).await
    }
}

/// 크롤링 엔진 헬스 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingEngineHealth {
    pub is_running: bool,
    pub stats: CrawlingStats,
    pub orchestrator_health: orchestrator::OrchestratorStats,
    pub last_check: chrono::DateTime<chrono::Utc>,
}

/// 크롤링 엔진 메트릭스
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingEngineMetrics {
    pub uptime: Duration,
    pub total_tasks_processed: u64,
    pub success_rate: f64,
    pub average_task_duration: Duration,
    pub current_queue_size: usize,
}

/// 크롤링 엔진 빌더 패턴
pub struct CrawlingEngineBuilder {
    config: Option<CrawlingConfig>,
    dependencies: Option<CrawlingEngineDependencies>,
}

impl CrawlingEngineBuilder {
    pub fn new() -> Self {
        Self {
            config: None,
            dependencies: None,
        }
    }
    
    pub fn with_config(mut self, config: CrawlingConfig) -> Self {
        self.config = Some(config);
        self
    }
    
    pub fn with_dependencies(mut self, dependencies: CrawlingEngineDependencies) -> Self {
        self.dependencies = Some(dependencies);
        self
    }
    
    pub async fn build(self) -> Result<CrawlingEngine, CrawlingEngineError> {
        let config = self.config.ok_or_else(|| 
            CrawlingEngineError::ConfigurationError("Config not provided".to_string()))?;
        let dependencies = self.dependencies.ok_or_else(|| 
            CrawlingEngineError::ConfigurationError("Dependencies not provided".to_string()))?;
        
        CrawlingEngineFactory::create_engine(config, dependencies).await
    }
}

impl Default for CrawlingEngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_crawling_engine_lifecycle() {
        // Mock dependencies would be injected here
        // This is a placeholder for actual tests
        
        // Test engine creation
        let builder = CrawlingEngineBuilder::new();
        assert!(builder.config.is_none());
        assert!(builder.dependencies.is_none());
    }
    
    #[test]
    fn test_crawling_engine_error_serialization() {
        let error = CrawlingEngineError::AlreadyRunning;
        let serialized = serde_json::to_string(&error).unwrap();
        let deserialized: CrawlingEngineError = serde_json::from_str(&serialized).unwrap();
        
        match deserialized {
            CrawlingEngineError::AlreadyRunning => {},
            _ => panic!("Wrong error type"),
        }
    }
}
