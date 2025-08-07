//! StageActor: 개별 스테이지 작업 처리 Actor
//! 
//! Phase 3: Actor 구현 - 스테이지 레벨 작업 실행 및 관리
//! Modern Rust 2024 준수: 함수형 원칙, 명시적 의존성, 상태 최소화

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::{info, warn, error, debug};
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::new_architecture::actors::types::{StageItemResult, StageItemType};

use super::traits::{Actor, ActorHealth, ActorStatus, ActorType};
use super::types::{ActorCommand, StageType, StageItem, StageResult, ActorError};
use crate::new_architecture::channels::types::AppEvent;
use crate::new_architecture::context::{AppContext, EventEmitter};

// 실제 서비스 imports - ServiceBasedBatchCrawlingEngine 패턴 참조
use crate::domain::services::{StatusChecker, ProductListCollector, ProductDetailCollector};
use crate::infrastructure::{HttpClient, MatterDataExtractor, IntegratedProductRepository};
use crate::infrastructure::config::AppConfig;
use crate::domain::value_objects::ProductData;
use crate::domain::product_url::ProductUrl;
use crate::domain::integrated_product::ProductDetail;
use crate::infrastructure::crawling_service_impls::{StatusCheckerImpl, ProductListCollectorImpl, ProductDetailCollectorImpl};
use crate::infrastructure::CollectorConfig;

/// StageActor: 개별 스테이지 작업의 실행 및 관리
/// 
/// 책임:
/// - 특정 스테이지 타입의 작업 실행
/// - 아이템별 처리 및 결과 수집
/// - 스테이지 레벨 이벤트 발행
/// - 타임아웃 및 재시도 로직 관리
/// - 실제 크롤링 서비스와 통합
#[derive(Clone)]
pub struct StageActor {
    /// Actor 고유 식별자
    actor_id: String,
    /// 배치 ID (OneShot 호환성)
    pub batch_id: String,
    /// 현재 처리 중인 스테이지 ID
    stage_id: Option<String>,
    /// 스테이지 타입
    stage_type: Option<StageType>,
    /// 스테이지 상태
    state: StageState,
    /// 스테이지 시작 시간
    start_time: Option<Instant>,
    /// 총 아이템 수
    total_items: u32,
    /// 처리 완료된 아이템 수
    completed_items: u32,
    /// 성공한 아이템 수
    success_count: u32,
    /// 실패한 아이템 수
    failure_count: u32,
    /// 스키핑된 아이템 수
    skipped_count: u32,
    /// 처리 결과들
    item_results: Vec<StageItemResult>,
    
    // 실제 서비스 의존성들
    /// 상태 확인 서비스
    status_checker: Option<Arc<dyn StatusChecker>>,
    /// 제품 목록 수집 서비스
    product_list_collector: Option<Arc<dyn ProductListCollector>>,
    /// 제품 상세 수집 서비스
    product_detail_collector: Option<Arc<dyn ProductDetailCollector>>,
    /// 데이터베이스 레포지토리
    product_repo: Option<Arc<IntegratedProductRepository>>,
    /// HTTP 클라이언트
    http_client: Option<Arc<HttpClient>>,
    /// 데이터 추출기
    data_extractor: Option<Arc<MatterDataExtractor>>,
    /// 앱 설정
    app_config: Option<AppConfig>,
}

impl std::fmt::Debug for StageActor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StageActor")
            .field("actor_id", &self.actor_id)
            .field("state", &self.state)
            .field("has_real_services", &self.status_checker.is_some())
            .finish()
    }
}

/// 스테이지 상태 열거형
#[derive(Debug, Clone, PartialEq)]
pub enum StageState {
    Idle,
    Starting,
    Processing,
    Completing,
    Completed,
    Failed { error: String },
    Timeout,
}

/// 스테이지 관련 에러 타입
#[derive(Debug, thiserror::Error)]
pub enum StageError {
    #[error("Stage initialization failed: {0}")]
    InitializationFailed(String),
    
    #[error("Stage already processing: {0}")]
    AlreadyProcessing(String),
    
    #[error("Stage not found: {0}")]
    StageNotFound(String),
    
    #[error("Invalid stage configuration: {0}")]
    InvalidConfiguration(String),
    
    #[error("Service initialization failed: {0}")]
    ServiceInitialization(String),
    
    #[error("Stage processing timeout: {timeout_secs}s")]
    ProcessingTimeout { timeout_secs: u64 },
    
    #[error("Item processing failed: {item_id} - {error}")]
    ItemProcessingFailed { item_id: String, error: String },
    
    #[error("Context communication error: {0}")]
    ContextError(String),
    
    #[error("Unsupported stage type: {0:?}")]
    UnsupportedStageType(StageType),
}

impl StageActor {
    /// 새로운 StageActor 인스턴스 생성
    /// 
    /// # Arguments
    /// * `actor_id` - Actor 고유 식별자
    /// 
    /// # Returns
    /// * `Self` - 새로운 StageActor 인스턴스
    pub fn new(actor_id: String) -> Self {
        let batch_id = Uuid::new_v4().to_string();
        Self {
            actor_id,
            batch_id,
            stage_id: None,
            stage_type: None,
            state: StageState::Idle,
            start_time: None,
            total_items: 0,
            completed_items: 0,
            success_count: 0,
            failure_count: 0,
            skipped_count: 0,
            item_results: Vec::new(),
            status_checker: None,
            product_list_collector: None,
            product_detail_collector: None,
            product_repo: None,
            http_client: None,
            data_extractor: None,
            app_config: None,
        }
    }
    
    /// 🔥 Phase 1: 실제 서비스들과 함께 StageActor 생성
    /// 
    /// # Arguments
    /// * `actor_id` - Actor 고유 식별자
    /// * `batch_id` - 배치 식별자
    /// * `http_client` - HTTP 클라이언트
    /// * `data_extractor` - 데이터 추출기
    /// * `product_repo` - 제품 레포지토리
    /// * `app_config` - 앱 설정
    /// 
    /// # Returns
    /// * `Self` - 서비스가 주입된 StageActor 인스턴스
    pub fn new_with_services(
        actor_id: String,
        batch_id: String,
        http_client: Arc<HttpClient>,
        data_extractor: Arc<MatterDataExtractor>,
        product_repo: Arc<IntegratedProductRepository>,
        app_config: AppConfig,
    ) -> Self {
        // Arc에서 클론을 통해 실제 값 추출
        let http_client_inner = (*http_client).clone();
        let data_extractor_inner = (*data_extractor).clone();
        
        // 실제 서비스들을 사용하여 컬렉터 생성 (ServiceBasedBatchCrawlingEngine 패턴 참조)
        let status_checker_impl = StatusCheckerImpl::new(
            http_client_inner.clone(),
            data_extractor_inner.clone(),
            app_config.clone(),
        );
        let status_checker = Some(Arc::new(status_checker_impl));
        
        // ProductListCollector 생성
        let list_collector_config = CollectorConfig {
            max_concurrent: app_config.user.crawling.workers.list_page_max_concurrent as u32,
            concurrency: app_config.user.crawling.workers.list_page_max_concurrent as u32,
            delay_between_requests: Duration::from_millis(app_config.user.request_delay_ms),
            delay_ms: app_config.user.request_delay_ms,
            batch_size: app_config.user.batch.batch_size,
            retry_attempts: app_config.user.crawling.workers.max_retries,
            retry_max: app_config.user.crawling.workers.max_retries,
        };
        
        // StatusCheckerImpl을 다시 생성 (ProductListCollector가 StatusCheckerImpl을 요구)
        let status_checker_for_list = Arc::new(StatusCheckerImpl::new(
            http_client_inner.clone(),
            data_extractor_inner.clone(),
            app_config.clone(),
        ));
        
        let product_list_collector = Some(Arc::new(ProductListCollectorImpl::new(
            Arc::new(http_client_inner.clone()),
            Arc::new(data_extractor_inner.clone()),
            list_collector_config.clone(),
            status_checker_for_list,
        )) as Arc<dyn ProductListCollector>);
        
        // ProductDetailCollector 생성
        let detail_collector_config = CollectorConfig {
            max_concurrent: app_config.user.crawling.workers.product_detail_max_concurrent as u32,
            concurrency: app_config.user.crawling.workers.product_detail_max_concurrent as u32,
            delay_between_requests: Duration::from_millis(app_config.user.request_delay_ms),
            delay_ms: app_config.user.request_delay_ms,
            batch_size: app_config.user.batch.batch_size,
            retry_attempts: app_config.user.crawling.workers.max_retries,
            retry_max: app_config.user.crawling.workers.max_retries,
        };
        
        let product_detail_collector = Some(Arc::new(ProductDetailCollectorImpl::new(
            Arc::new(http_client_inner.clone()),
            Arc::new(data_extractor_inner.clone()),
            detail_collector_config,
        )) as Arc<dyn ProductDetailCollector>);
        
        Self {
            actor_id,
            batch_id,
            stage_id: None,
            stage_type: None,
            state: StageState::Idle,
            start_time: None,
            total_items: 0,
            completed_items: 0,
            success_count: 0,
            failure_count: 0,
            skipped_count: 0,
            item_results: Vec::new(),
            // 실제 서비스들 주입
            status_checker: status_checker.map(|s| s as Arc<dyn StatusChecker>),
            product_list_collector,
            product_detail_collector,
            product_repo: Some(product_repo),
            http_client: Some(http_client),
            data_extractor: Some(data_extractor),
            app_config: Some(app_config),
        }
    }
    
    /// OneShot Actor 시스템 호환성을 위한 생성자
    /// 
    /// # Arguments
    /// * `batch_id` - 배치 식별자
    /// * `config` - 시스템 설정
    /// * `total_pages` - 총 페이지 수 (선택적)
    /// * `products_on_last_page` - 마지막 페이지 제품 수 (선택적)
    /// 
    /// # Returns
    /// * `Self` - 새로운 StageActor 인스턴스
    pub fn new_with_oneshot(
        batch_id: String, 
        _config: Arc<crate::new_architecture::config::SystemConfig>,
        _total_pages: u32,
        _products_on_last_page: u32
    ) -> Self {
        let actor_id = Uuid::new_v4().to_string();
        Self {
            actor_id,
            batch_id,
            stage_id: None,
            stage_type: None,
            state: StageState::Idle,
            start_time: None,
            total_items: 0,
            completed_items: 0,
            success_count: 0,
            failure_count: 0,
            skipped_count: 0,
            item_results: Vec::new(),
            status_checker: None,
            product_list_collector: None,
            product_detail_collector: None,
            product_repo: None,
            http_client: None,
            data_extractor: None,
            app_config: None,
        }
    }
    
    /// 실제 서비스 초기화 - guide/re-arch-plan-final2.md 설계 기반
    /// ServiceBasedBatchCrawlingEngine 패턴 참조하되 Actor 모델에 맞게 구현
    pub async fn initialize_real_services(&mut self, context: &AppContext) -> Result<(), StageError> {
        info!("🎯 [ACTOR] Initializing real services for StageActor: {}", self.actor_id);
        
        // AppConfig 로드 (설정 파일에서)
        let app_config = crate::infrastructure::config::AppConfig::default();
        
        // HTTP Client 생성 (ServiceBasedBatchCrawlingEngine과 동일한 방식)
        let http_client = app_config.create_http_client()
            .map_err(|e| StageError::ServiceInitialization(format!("Failed to create HTTP client: {}", e)))?;
        
        // 데이터 추출기 생성
        let data_extractor = MatterDataExtractor::new()
            .map_err(|e| StageError::ServiceInitialization(format!("Failed to create data extractor: {}", e)))?;
        
        // 데이터베이스 연결 생성 (기본 경로 사용)
        let database_url = crate::infrastructure::database_paths::get_main_database_url();
        let pool = sqlx::SqlitePool::connect(&database_url).await
            .map_err(|e| StageError::ServiceInitialization(format!("Failed to connect to database: {}", e)))?;
        let product_repo = Arc::new(IntegratedProductRepository::new(pool));
        
        // StatusChecker 생성 (ServiceBasedBatchCrawlingEngine과 동일한 방식)
        let status_checker = Arc::new(StatusCheckerImpl::new(
            http_client.clone(),
            data_extractor.clone(),
            app_config.clone(),
        ));
        
        // List Collector Config (ServiceBasedBatchCrawlingEngine 패턴 참조)
        let list_collector_config = CollectorConfig {
            max_concurrent: app_config.user.crawling.workers.list_page_max_concurrent as u32,
            concurrency: app_config.user.crawling.workers.list_page_max_concurrent as u32,
            delay_between_requests: Duration::from_millis(app_config.user.request_delay_ms),
            delay_ms: app_config.user.request_delay_ms,
            batch_size: app_config.user.batch.batch_size,
            retry_attempts: app_config.user.crawling.workers.max_retries,
            retry_max: app_config.user.crawling.workers.max_retries,
        };
        
        let detail_collector_config = CollectorConfig {
            max_concurrent: app_config.user.crawling.workers.product_detail_max_concurrent as u32,
            concurrency: app_config.user.crawling.workers.product_detail_max_concurrent as u32,
            delay_between_requests: Duration::from_millis(app_config.user.request_delay_ms),
            delay_ms: app_config.user.request_delay_ms,
            batch_size: app_config.user.batch.batch_size,
            retry_attempts: app_config.user.crawling.workers.max_retries,
            retry_max: app_config.user.crawling.workers.max_retries,
        };
        
        // Status checker를 concrete type으로 생성 (ProductListCollector에 필요)
        let status_checker_impl = Arc::new(StatusCheckerImpl::new(
            http_client.clone(),
            data_extractor.clone(),
            app_config.clone(),
        ));
        
        // ProductListCollector 생성 (ServiceBasedBatchCrawlingEngine과 동일한 방식)
        let product_list_collector = Arc::new(ProductListCollectorImpl::new(
            Arc::new(http_client.clone()),
            Arc::new(data_extractor.clone()),
            list_collector_config,
            status_checker_impl,
        ));
        
        // ProductDetailCollector 생성 (ServiceBasedBatchCrawlingEngine과 동일한 방식)
        let product_detail_collector = Arc::new(ProductDetailCollectorImpl::new(
            Arc::new(http_client.clone()),
            Arc::new(data_extractor.clone()),
            detail_collector_config,
        ));
        
        // 서비스들을 StageActor에 할당
        self.status_checker = Some(status_checker);
        self.product_list_collector = Some(product_list_collector);
        self.product_detail_collector = Some(product_detail_collector);
        self.product_repo = Some(product_repo);
        self.http_client = Some(Arc::new(http_client));
        self.data_extractor = Some(Arc::new(data_extractor));
        self.app_config = Some(app_config);
        
        info!("✅ [ACTOR] Real services initialized successfully for StageActor: {}", self.actor_id);
        Ok(())
    }
    
    /// 크롤링 엔진 초기화 (임시 구현)
    /// 현재는 시뮬레이션 모드이므로 실제 엔진 초기화는 건너뛰기
    pub async fn initialize_default_engines(&mut self) -> Result<(), StageError> {
        // Phase 3에서는 시뮬레이션 모드로 동작
        // 실제 크롤링 엔진 초기화는 향후 구현
        info!("🔧 StageActor {} initialized with simulation engines", self.actor_id);
        Ok(())
    }
    
    /// 공개 스테이지 실행 메서드 (BatchActor에서 사용)
    /// 
    /// # Arguments
    /// * `stage_type` - 실행할 스테이지 타입
    /// * `items` - 처리할 아이템 리스트
    /// * `concurrency_limit` - 동시성 제한
    /// * `timeout_secs` - 타임아웃 (초)
    /// * `context` - Actor 컨텍스트
    pub async fn execute_stage(
        &mut self,
        stage_type: StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
        timeout_secs: u64,
        context: &AppContext,
    ) -> Result<StageResult, StageError> {
        self.handle_execute_stage(
            stage_type,
            items,
            concurrency_limit,
            timeout_secs,
            context,
        ).await?;
        
        Ok(StageResult {
            processed_items: self.completed_items,
            successful_items: self.success_count,
            failed_items: self.failure_count,
            duration_ms: self.start_time.map(|start| start.elapsed().as_millis() as u64).unwrap_or(0),
            details: self.item_results.clone(),
        })
    }
    
    /// 스테이지 실행 처리
    /// 
    /// # Arguments
    /// * `stage_type` - 실행할 스테이지 타입
    /// * `items` - 처리할 아이템 리스트
    /// * `concurrency_limit` - 동시성 제한
    /// * `timeout_secs` - 타임아웃 (초)
    /// * `context` - Actor 컨텍스트
    async fn handle_execute_stage(
        &mut self,
        stage_type: StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
        timeout_secs: u64,
        context: &AppContext,
    ) -> Result<(), StageError> {
        // 상태 검증
        if !matches!(self.state, StageState::Idle) {
            return Err(StageError::AlreadyProcessing(
                self.stage_id.clone().unwrap_or_else(|| "unknown".to_string())
            ));
        }
        
        let stage_id = Uuid::new_v4().to_string();
        
        info!("🎯 StageActor {} executing stage {:?} with {} items", 
              self.actor_id, stage_type, items.len());
        
        // 상태 초기화
        self.stage_id = Some(stage_id.clone());
        self.stage_type = Some(stage_type.clone());
        self.state = StageState::Starting;
        self.start_time = Some(Instant::now());
        self.total_items = items.len() as u32;
        self.completed_items = 0;
        self.success_count = 0;
        self.failure_count = 0;
        self.skipped_count = 0;
        self.item_results.clear();
        
        // 스테이지 시작 이벤트 발행
        let start_event = AppEvent::StageStarted {
            stage_type: stage_type.clone(),
            session_id: context.session_id.clone(),
            items_count: items.len() as u32,
            timestamp: Utc::now(),
        };
        
        context.emit_event(start_event).await
            .map_err(|e| StageError::ContextError(e.to_string()))?;
        
        // 상태를 Processing으로 전환
        self.state = StageState::Processing;
        
        // 타임아웃과 함께 스테이지 처리
        let processing_result = timeout(
            Duration::from_secs(timeout_secs),
            self.process_stage_items(stage_type.clone(), items, concurrency_limit, context)
        ).await;
        
        match processing_result {
            Ok(result) => {
                match result {
                    Ok(stage_result) => {
                        self.state = StageState::Completed;
                        
                        // 완료 이벤트 발행
                        let completion_event = AppEvent::StageCompleted {
                            stage_type: stage_type.clone(),
                            session_id: context.session_id.clone(),
                            result: stage_result,
                            timestamp: Utc::now(),
                        };
                        
                        context.emit_event(completion_event).await
                            .map_err(|e| StageError::ContextError(e.to_string()))?;
                        
                        info!("✅ Stage {:?} completed successfully: {}/{} items processed", 
                              stage_type, self.success_count, self.total_items);
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        self.state = StageState::Failed { error: error_msg.clone() };
                        
                        // 실패 이벤트 발행
                        let failure_event = AppEvent::StageFailed {
                            stage_type: stage_type.clone(),
                            session_id: context.session_id.clone(),
                            error: error_msg,
                            timestamp: Utc::now(),
                        };
                        
                        context.emit_event(failure_event).await
                            .map_err(|e| StageError::ContextError(e.to_string()))?;
                        
                        return Err(e);
                    }
                }
            }
            Err(_) => {
                // 타임아웃 발생
                self.state = StageState::Timeout;
                
                let error = StageError::ProcessingTimeout { timeout_secs };
                
                // 타임아웃 이벤트 발행
                let timeout_event = AppEvent::StageFailed {
                    stage_type: stage_type.clone(),
                    session_id: context.session_id.clone(),
                    error: error.to_string(),
                    timestamp: Utc::now(),
                };
                
                context.emit_event(timeout_event).await
                    .map_err(|e| StageError::ContextError(e.to_string()))?;
                
                return Err(error);
            }
        }
        
        Ok(())
    }
    
    /// 스테이지 아이템들 처리
    /// 
    /// # Arguments
    /// * `stage_type` - 스테이지 타입
    /// * `items` - 처리할 아이템들
    /// * `concurrency_limit` - 동시성 제한
    /// * `context` - Actor 컨텍스트
    async fn process_stage_items(
        &mut self,
        stage_type: StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
        _context: &AppContext,
    ) -> Result<StageResult, StageError> {
        debug!("Processing {} items for stage {:?}", items.len(), stage_type);
        
        // 동시성 제어를 위한 세마포어
        let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency_limit as usize));
        let mut tasks = Vec::new();
        
        // 서비스 의존성 복사
        let status_checker = self.status_checker.clone();
        let product_list_collector = self.product_list_collector.clone();
        let product_detail_collector = self.product_detail_collector.clone();
        let product_repo = self.product_repo.clone();
        
        // 각 아이템을 병렬로 처리
        for item in items {
            let sem = semaphore.clone();
            let item_clone = item.clone();
            let stage_type_clone = stage_type.clone();
            let status_checker_clone = status_checker.clone();
            let product_list_collector_clone = product_list_collector.clone();
            let product_detail_collector_clone = product_detail_collector.clone();
            let product_repo_clone = product_repo.clone();
            
            let task = tokio::spawn(async move {
                let _permit = sem.acquire().await.map_err(|e| 
                    StageError::InitializationFailed(format!("Semaphore error: {}", e))
                )?;
                
                Self::process_single_item(
                    stage_type_clone, 
                    item_clone,
                    status_checker_clone,
                    product_list_collector_clone,
                    product_detail_collector_clone,
                    product_repo_clone,
                ).await
            });
            
            tasks.push(task);
        }
        
        // 모든 태스크 완료 대기
        let mut results = Vec::new();
        for task in tasks {
            match task.await {
                Ok(Ok(result)) => {
                    results.push(result);
                }
                Ok(Err(e)) => {
                    error!("Item processing failed: {}", e);
                    results.push(StageItemResult {
                        item_id: "unknown".to_string(),
                        item_type: StageItemType::Url { url_type: "unknown".to_string() },
                        success: false,
                        error: Some(e.to_string()),
                        duration_ms: 0,
                        retry_count: 0,
                        collected_data: None,
                    });
                }
                Err(e) => {
                    error!("Task join error: {}", e);
                    results.push(StageItemResult {
                        item_id: "unknown".to_string(),
                        item_type: StageItemType::Url { url_type: "unknown".to_string() },
                        success: false,
                        error: Some(format!("Task join error: {}", e)),
                        duration_ms: 0,
                        retry_count: 0,
                        collected_data: None,
                    });
                }
            }
        }
        
        // 결과 집계
        self.item_results = results;
        self.completed_items = self.item_results.len() as u32;
        self.success_count = self.item_results.iter().filter(|r| r.success).count() as u32;
        self.failure_count = self.item_results.iter().filter(|r| !r.success).count() as u32;
        
        let duration = self.start_time
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);
        
        Ok(StageResult {
            processed_items: self.completed_items,
            successful_items: self.success_count,
            failed_items: self.failure_count,
            duration_ms: duration.as_millis() as u64,
            details: self.item_results.clone(),
        })
    }
    
    /// 개별 아이템 처리 (실제 서비스 사용)
    /// 
    /// # Arguments
    /// * `stage_type` - 스테이지 타입
    /// * `item` - 처리할 아이템
    async fn process_single_item(
        stage_type: StageType,
        item: StageItem,
        status_checker: Option<Arc<dyn StatusChecker>>,
        product_list_collector: Option<Arc<dyn ProductListCollector>>,
        product_detail_collector: Option<Arc<dyn ProductDetailCollector>>,
        product_repo: Option<Arc<IntegratedProductRepository>>,
    ) -> Result<StageItemResult, StageError> {
        let start_time = Instant::now();
        
        debug!("Processing item {} for stage {:?}", item.id, stage_type);
        
        // 스테이지 타입별 처리 로직 - 수집된 데이터와 성공 여부를 함께 반환
        let (success, collected_data) = match stage_type {
            StageType::StatusCheck => {
                if let Some(checker) = status_checker {
                    match Self::execute_real_status_check(&item, checker).await {
                        Ok(()) => (Ok(()), None),
                        Err(e) => (Err(e), None),
                    }
                } else {
                    // StatusChecker가 없으면 에러
                    (Err("StatusChecker not available".to_string()), None)
                }
            }
            StageType::ListPageCrawling => {
                if let Some(collector) = product_list_collector {
                    match Self::execute_real_list_page_processing(&item, collector).await {
                        Ok(urls) => {
                            // ProductURL들을 JSON으로 직렬화하여 저장
                            match serde_json::to_string(&urls) {
                                Ok(json_data) => (Ok(()), Some(json_data)),
                                Err(e) => (Err(format!("JSON serialization failed: {}", e)), None),
                            }
                        }
                        Err(e) => (Err(e), None),
                    }
                } else {
                    // ProductListCollector가 없으면 에러
                    (Err("ProductListCollector not available".to_string()), None)
                }
            }
            StageType::ProductDetailCrawling => {
                if let Some(collector) = product_detail_collector {
                    match Self::execute_real_product_detail_processing(&item, collector).await {
                        Ok(products) => {
                            // Product들을 JSON으로 직렬화하여 저장
                            match serde_json::to_string(&products) {
                                Ok(json_data) => (Ok(()), Some(json_data)),
                                Err(e) => (Err(format!("JSON serialization failed: {}", e)), None),
                            }
                        }
                        Err(e) => (Err(e), None),
                    }
                } else {
                    // ProductDetailCollector가 없으면 에러
                    (Err("ProductDetailCollector not available".to_string()), None)
                }
            }
            StageType::DataValidation => {
                match Self::execute_real_data_validation(&item).await {
                    Ok(()) => (Ok(()), None),
                    Err(e) => (Err(e), None),
                }
            }
            StageType::DataSaving => {
                if let Some(repo) = product_repo {
                    match Self::execute_real_database_storage(&item, repo).await {
                        Ok(()) => (Ok(()), None),
                        Err(e) => (Err(e), None),
                    }
                } else {
                    // Product repository가 없으면 에러
                    (Err("Product repository not available".to_string()), None)
                }
            }
        };
        
        let duration = start_time.elapsed();
        
        match success {
            Ok(()) => Ok(StageItemResult {
                item_id: item.id,
                item_type: item.item_type,
                success: true,
                error: None,
                duration_ms: duration.as_millis() as u64,
                retry_count: 0,
                collected_data,
            }),
            Err(error) => Ok(StageItemResult {
                item_id: item.id.clone(),
                item_type: item.item_type,
                success: false,
                error: Some(error.clone()),
                duration_ms: duration.as_millis() as u64,
                retry_count: 0,
                collected_data: None,
            }),
        }
    }
    
    // === 실제 서비스 기반 처리 함수들 (Critical Issue #1) ===
    
    /// 실제 상태 확인 처리
    async fn execute_real_status_check(
        item: &StageItem,
        status_checker: Arc<dyn StatusChecker>,
    ) -> Result<(), String> {
        match &item.item_type {
            StageItemType::Url { .. } => {
                // 실제 사이트 상태 확인
                match status_checker.check_site_status().await {
                    Ok(_status) => {
                        info!("✅ Real status check successful for item {}", item.id);
                        Ok(())
                    }
                    Err(e) => {
                        warn!("❌ Real status check failed for item {}: {}", item.id, e);
                        Err(format!("Status check failed: {}", e))
                    }
                }
            }
            _ => Ok(()), // 다른 타입은 성공으로 처리
        }
    }
    
    /// 실제 리스트 페이지 처리
    async fn execute_real_list_page_processing(
        item: &StageItem,
        product_list_collector: Arc<dyn ProductListCollector>,
    ) -> Result<Vec<crate::domain::product_url::ProductUrl>, String> {
        match &item.item_type {
            StageItemType::Page { page_number } => {
                // 실제 리스트 페이지 크롤링
                match product_list_collector.collect_page_range(
                    *page_number, *page_number, 1000, 20  // 임시 값들 - TODO: 실제 설정 사용
                ).await {
                    Ok(urls) => {
                        info!("✅ Real list page processing successful for page {}: {} URLs collected", 
                              page_number, urls.len());
                        
                        // 수집된 ProductURL들을 반환
                        for (index, url) in urls.iter().enumerate() {
                            debug!("  📄 Collected URL {}: {}", index + 1, url.url);
                        }
                        
                        Ok(urls)
                    }
                    Err(e) => {
                        warn!("❌ Real list page processing failed for page {}: {}", page_number, e);
                        Err(format!("List page processing failed: {}", e))
                    }
                }
            }
            _ => Ok(vec![]), // 다른 타입은 빈 벡터 반환
        }
    }
    
    /// 실제 제품 상세 처리
    async fn execute_real_product_detail_processing(
        item: &StageItem,
        product_detail_collector: Arc<dyn ProductDetailCollector>,
    ) -> Result<Vec<crate::domain::product::ProductDetail>, String> {
        match &item.item_type {
            StageItemType::ProductUrls { urls } => {
                // URL 문자열을 ProductURL 객체로 변환
                let product_urls: Vec<crate::domain::product_url::ProductUrl> = urls
                    .iter()
                    .enumerate()
                    .map(|(index, url)| crate::domain::product_url::ProductUrl::new(url.clone(), index as i32, 0))
                    .collect();
                
                info!("🎯 Processing {} product URLs for detail crawling", product_urls.len());
                
                match product_detail_collector.collect_details(&product_urls).await {
                    Ok(details) => {
                        info!("✅ Real product detail processing successful for item {}: {} details collected", 
                              item.id, details.len());
                        Ok(details)
                    }
                    Err(e) => {
                        warn!("❌ Real product detail processing failed for item {}: {}", item.id, e);
                        Err(format!("Product detail processing failed: {}", e))
                    }
                }
            }
            StageItemType::Url { .. } => {
                // 단일 URL 처리를 위한 fallback
                warn!("⚠️ Single URL processing not fully implemented, using placeholder");
                let sample_urls = vec![crate::domain::product_url::ProductUrl::new(item.url.clone(), 1, 0)];
                match product_detail_collector.collect_details(&sample_urls).await {
                    Ok(details) => {
                        info!("✅ Fallback product detail processing successful for item {}: {} details collected", 
                              item.id, details.len());
                        Ok(details)
                    }
                    Err(e) => {
                        warn!("❌ Fallback product detail processing failed for item {}: {}", item.id, e);
                        Err(format!("Product detail processing failed: {}", e))
                    }
                }
            }
            _ => {
                warn!("⚠️ Unsupported item type for product detail processing: {:?}", item.item_type);
                Ok(vec![]) // 다른 타입은 빈 벡터 반환
            }
        }
    }
    
    /// 실제 데이터 검증 처리
    async fn execute_real_data_validation(item: &StageItem) -> Result<(), String> {
        // 기본적인 데이터 검증 로직
        if item.id.is_empty() {
            return Err("Invalid item: empty ID".to_string());
        }
        
        // 추가 검증 로직을 여기에 구현
        info!("✅ Real data validation successful for item {}", item.id);
        Ok(())
    }
    
    /// 실제 데이터베이스 저장 처리
    async fn execute_real_database_storage(
        item: &StageItem,
        product_repo: Arc<IntegratedProductRepository>,
    ) -> Result<(), String> {
        // 실제 데이터베이스 저장 로직 - ServiceBasedBatchCrawlingEngine 패턴 참조
        match &item.item_type {
            StageItemType::Product { page_number } => {
                info!("💾 Saving product data for page {} to database", page_number);
                
                // TODO: 실제 제품 데이터를 받아서 저장하는 로직 필요
                // 현재는 구조상 제품 데이터가 StageItem에 포함되지 않음
                warn!("⚠️ Product data saving not fully implemented - requires Stage간 데이터 전달 메커니즘");
                
                Ok(())
            }
            StageItemType::ProductUrls { urls } => {
                // ProductURL 정보는 저장하지 않고 스킵
                info!("🔧 Skipping database storage for ProductURL collection ({})", urls.len());
                Ok(())
            }
            _ => {
                // URL 타입의 경우 아직 제품 데이터가 없으므로 스킵
                info!("🔧 Skipping database storage for item {} (type: {:?})", 
                     item.id, item.item_type);
                Ok(())
            }
        }
    }
    
    // === 시뮬레이션 함수들 (기존) ===
    
    /// 리스트 페이지 처리 시뮬레이션 (Phase 3 임시)
    async fn simulate_list_page_processing(item: &StageItem) -> Result<(), String> {
        // 임시: 간단한 처리 시뮬레이션
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // 90% 성공률 시뮬레이션
        if item.id.chars().last().unwrap_or('0').to_digit(10).unwrap_or(0) < 9 {
            Ok(())
        } else {
            Err("Simulated network error".to_string())
        }
    }
    
    /// 스테이지 정리
    fn cleanup_stage(&mut self) {
        self.stage_id = None;
        self.stage_type = None;
        self.state = StageState::Idle;
        self.start_time = None;
        self.total_items = 0;
        self.completed_items = 0;
        self.success_count = 0;
        self.failure_count = 0;
        self.skipped_count = 0;
        self.item_results.clear();
    }
    
    /// 진행 상황 계산
    /// 
    /// # Returns
    /// * `f64` - 진행률 (0.0 ~ 1.0)
    fn calculate_progress(&self) -> f64 {
        if self.total_items == 0 {
            0.0
        } else {
            f64::from(self.completed_items) / f64::from(self.total_items)
        }
    }
    
    /// 성공률 계산
    /// 
    /// # Returns
    /// * `f64` - 성공률 (0.0 ~ 1.0)
    fn calculate_success_rate(&self) -> f64 {
        if self.completed_items == 0 {
            0.0
        } else {
            f64::from(self.success_count) / f64::from(self.completed_items)
        }
    }
}

#[async_trait::async_trait]
impl Actor for StageActor {
    type Command = ActorCommand;
    type Error = ActorError;

    fn actor_id(&self) -> &str {
        self.stage_id.as_deref().unwrap_or("unknown")
    }

    fn actor_type(&self) -> ActorType {
        ActorType::Stage
    }    async fn run(
        &mut self,
        mut context: AppContext,
        mut command_rx: mpsc::Receiver<Self::Command>,
    ) -> Result<(), Self::Error> {
        info!("🎯 StageActor {} starting execution loop", self.actor_id);
        
        loop {
            tokio::select! {
                // 명령 처리
                command = command_rx.recv() => {
                    match command {
                        Some(cmd) => {
                            debug!("📨 StageActor {} received command: {:?}", self.actor_id, cmd);
                            
                            match cmd {
                                ActorCommand::ExecuteStage { 
                                    stage_type, 
                                    items, 
                                    concurrency_limit, 
                                    timeout_secs 
                                } => {
                                    if let Err(e) = self.handle_execute_stage(
                                        stage_type, 
                                        items, 
                                        concurrency_limit, 
                                        timeout_secs, 
                                        &context
                                    ).await {
                                        error!("Failed to execute stage: {}", e);
                                    }
                                }
                                
                                ActorCommand::Shutdown => {
                                    info!("🛑 StageActor {} received shutdown command", self.actor_id);
                                    break;
                                }
                                
                                _ => {
                                    debug!("StageActor {} ignoring non-stage command", self.actor_id);
                                }
                            }
                        }
                        None => {
                            warn!("📪 StageActor {} command channel closed", self.actor_id);
                            break;
                        }
                    }
                }
                
                // 취소 신호 확인
                _ = context.cancellation_token.changed() => {
                    if *context.cancellation_token.borrow() {
                        warn!("🚫 StageActor {} received cancellation signal", self.actor_id);
                        break;
                    }
                }
            }
        }
        
        info!("🏁 StageActor {} execution loop ended", self.actor_id);
        Ok(())
    }
    
    async fn health_check(&self) -> Result<ActorHealth, Self::Error> {
        let status = match &self.state {
            StageState::Idle => ActorStatus::Healthy,
            StageState::Processing => ActorStatus::Healthy,
            StageState::Completed => ActorStatus::Healthy,
            StageState::Timeout => ActorStatus::Degraded { 
                reason: "Stage timed out".to_string(),
                since: Utc::now(),
            },
            StageState::Failed { error } => ActorStatus::Unhealthy { 
                error: error.clone(),
                since: Utc::now(),
            },
            _ => ActorStatus::Degraded { 
                reason: format!("In transition state: {:?}", self.state),
                since: Utc::now(),
            },
        };
        
        Ok(ActorHealth {
            actor_id: self.stage_id.clone().unwrap_or_default(),
            actor_type: ActorType::Stage,
            status,
            last_activity: Utc::now(),
            memory_usage_mb: 0, // TODO: 실제 메모리 사용량 계산
            active_tasks: if matches!(self.state, StageState::Processing) { 
                self.total_items - self.completed_items 
            } else { 
                0 
            },
            commands_processed: 0, // TODO: 실제 처리된 명령 수 계산
            errors_count: 0, // TODO: 실제 에러 수 계산
            avg_command_processing_time_ms: 0.0, // TODO: 실제 평균 처리 시간 계산
            metadata: serde_json::json!({
                "stage_id": self.stage_id,
                "stage_type": self.stage_type,
                "state": format!("{:?}", self.state),
                "total_items": self.total_items,
                "completed_items": self.completed_items,
                "success_count": self.success_count,
                "failure_count": self.failure_count,
                "skipped_count": self.skipped_count,
                "progress": self.calculate_progress(),
                "success_rate": self.calculate_success_rate()
            }).to_string(),
        })
    }
    
    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        info!("🔌 StageActor {} shutting down", self.actor_id);
        
        // 활성 스테이지가 있다면 정리
        if self.stage_id.is_some() {
            warn!("Cleaning up active stage during shutdown");
            self.cleanup_stage();
        }
        
        Ok(())
    }
}
