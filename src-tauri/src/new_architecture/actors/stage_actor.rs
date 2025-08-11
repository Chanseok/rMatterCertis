//! StageActor: 개별 스테이지 작업 처리 Actor
//! 
//! Phase 3: Actor 구현 - 스테이지 레벨 작업 실행 및 관리
//! Modern Rust 2024 준수: 함수형 원칙, 명시적 의존성, 상태 최소화

use std::sync::Arc;
use std::time::{Duration, Instant};
use chrono::Utc;
use tokio::{sync::mpsc, time::timeout};
use tracing::{info, warn, error, debug};
use uuid::Uuid;
use once_cell::sync::Lazy;
use std::sync::Mutex as StdMutex;
use std::collections::HashSet;

use crate::infrastructure::{HttpClient, MatterDataExtractor, IntegratedProductRepository};
use crate::infrastructure::config::AppConfig;
use crate::new_architecture::actors::types::{AppEvent, StageResult, StageItemResult, StageItemType, ActorCommand, SimpleMetrics};
use crate::new_architecture::channels::types::{StageItem};
use crate::new_architecture::context::AppContext;
use crate::new_architecture::actors::traits::{Actor, ActorHealth, ActorStatus, ActorType};
use crate::new_architecture::actors::types::ActorError;
use crate::domain::services::SiteStatus;
use crate::infrastructure::crawling_service_impls::{StatusCheckerImpl, ProductListCollectorImpl, ProductDetailCollectorImpl, CollectorConfig};
use crate::domain::services::{StatusChecker, ProductListCollector, ProductDetailCollector};
use crate::new_architecture::actors::types::{StageType};
use crate::domain::pagination::PaginationCalculator;
use thiserror::Error;

// NOTE: 원래 파일 상단이 손상되어 재작성. 아래 기존 구현들과 연결되도록 동일한 타입/impl 유지.

// 내부 상태 enum (간단 재구성 - 원래 손상된 정의 복원)
#[derive(Clone, PartialEq, Debug)]
enum StageState {
    Idle,
    Starting,
    Processing,
    Completed,
    Timeout,
    Failed { error: String },
}

#[derive(Debug, Error, Clone)]
pub enum StageError {
    #[error("Stage already processing: {0}")]
    AlreadyProcessing(String),
    #[error("Stage not found: {0}")]
    StageNotFound(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
    #[error("Service initialization failed: {0}")]
    ServiceInitialization(String),
    #[error("Initialization failed: {0}")]
    InitializationFailed(String),
    #[error("Context communication error: {0}")]
    ContextError(String),
    #[error("Processing timeout: {timeout_secs}s")]
    ProcessingTimeout { timeout_secs: u64 },
    #[error("Unsupported stage type: {0:?}")]
    UnsupportedStageType(StageType),
    #[error("Item processing failed: {item_id} - {error}")]
    ItemProcessingFailed { item_id: String, error: String },
}

// StageActor 구조체 (필요 필드만 복원; 나머지 로직과 매칭)
// Removed automatic Debug derive due to non-Debug trait object fields; manual impl below
pub struct StageActor {
    pub actor_id: String,
    pub batch_id: String,
    pub stage_id: Option<String>,
    pub stage_type: Option<StageType>,
    state: StageState,
    start_time: Option<Instant>,
    total_items: u32,
    completed_items: u32,
    success_count: u32,
    failure_count: u32,
    skipped_count: u32,
    item_results: Vec<StageItemResult>,
    // 서비스 의존성
    status_checker: Option<Arc<dyn StatusChecker>>,
    product_list_collector: Option<Arc<dyn ProductListCollector>>,
    product_detail_collector: Option<Arc<dyn ProductDetailCollector>>,
    _product_repo: Option<Arc<IntegratedProductRepository>>,
    http_client: Option<Arc<HttpClient>>,
    data_extractor: Option<Arc<MatterDataExtractor>>,
    app_config: Option<AppConfig>,
    // 힌트
    site_total_pages_hint: Option<u32>,
    products_on_last_page_hint: Option<u32>,
}

// Prevent duplicate DataSaving executions per (session_id,batch_id)
static DATA_SAVING_RUN_GUARD: Lazy<StdMutex<HashSet<String>>> = Lazy::new(|| StdMutex::new(HashSet::new()));

// Manual Debug implementation to avoid requiring Debug on all service trait object dependencies
impl std::fmt::Debug for StageActor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StageActor")
            .field("actor_id", &self.actor_id)
            .field("batch_id", &self.batch_id)
            .field("stage_id", &self.stage_id)
            .field("stage_type", &self.stage_type)
            .field("state", &self.state)
            .field("total_items", &self.total_items)
            .field("completed_items", &self.completed_items)
            .field("success_count", &self.success_count)
            .field("failure_count", &self.failure_count)
            .field("skipped_count", &self.skipped_count)
            .finish()
    }
}

// Extension trait to restore helper methods expected by per-item task logic
trait StageItemExt {
    fn id_string(&self) -> String;
    fn item_type_enum(&self) -> StageItemType;
}

impl StageItemExt for StageItem {
    fn id_string(&self) -> String {
        match self {
            StageItem::Page(p) => format!("page_{}", p),
            StageItem::Url(u) => u.clone(),
            StageItem::Product(p) => p.url.clone(),
            StageItem::ProductList(l) => format!("list_page_{}", l.page_number),
            StageItem::ProductUrls(urls) => format!("product_urls_{}", urls.urls.len()),
            StageItem::ProductDetails(d) => format!("product_details_{}", d.products.len()),
            StageItem::ValidatedProducts(v) => format!("validated_products_{}", v.products.len()),
            StageItem::ValidationTarget(v) => format!("validation_target_{}", v.len()),
        }
    }
    fn item_type_enum(&self) -> StageItemType {
        match self {
            StageItem::Page(page) => StageItemType::Page { page_number: *page },
            StageItem::Url(_u) => StageItemType::Url { url_type: "generic".into() },
            StageItem::Product(_p) => StageItemType::Url { url_type: "product".into() },
            StageItem::ProductList(_l) => StageItemType::ProductUrls { urls: vec![] },
            StageItem::ProductUrls(list) => StageItemType::ProductUrls { urls: list.urls.iter().map(|u| u.url.clone()).collect() },
            StageItem::ProductDetails(_d) => StageItemType::Url { url_type: "product_details".into() },
            StageItem::ValidatedProducts(_v) => StageItemType::Url { url_type: "validated_products".into() },
            StageItem::ValidationTarget(_t) => StageItemType::Url { url_type: "validation_target".into() },
        }
    }
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
            _product_repo: None,
            http_client: None,
            data_extractor: None,
            app_config: None,
            site_total_pages_hint: None,
            products_on_last_page_hint: None,
        }
    }

    // (Early duplicate progress helpers removed; canonical versions near file end)
    
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
        let status_checker: Option<Arc<dyn StatusChecker>> = Some(Arc::new(StatusCheckerImpl::with_product_repo(
            http_client_inner.clone(),
            data_extractor_inner.clone(),
            app_config.clone(),
            Arc::clone(&product_repo),
        )));
        
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
        let status_checker_for_list = Arc::new(StatusCheckerImpl::with_product_repo(
            http_client_inner.clone(),
            data_extractor_inner.clone(),
            app_config.clone(),
            Arc::clone(&product_repo),
        ));
        
        let product_list_collector: Option<Arc<dyn ProductListCollector>> = Some(Arc::new(ProductListCollectorImpl::new(
            Arc::new(http_client_inner.clone()),
            Arc::new(data_extractor_inner.clone()),
            list_collector_config.clone(),
            status_checker_for_list,
        )));
        
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
        
        let product_detail_collector: Option<Arc<dyn ProductDetailCollector>> = Some(Arc::new(ProductDetailCollectorImpl::new(
            Arc::new(http_client_inner.clone()),
            Arc::new(data_extractor_inner.clone()),
            detail_collector_config,
        )));
        
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
            status_checker,
            product_list_collector,
            product_detail_collector,
            _product_repo: Some(product_repo),
            http_client: Some(http_client),
            data_extractor: Some(data_extractor),
            app_config: Some(app_config),
            site_total_pages_hint: None,
            products_on_last_page_hint: None,
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
            _product_repo: None,
            http_client: None,
            data_extractor: None,
            app_config: None,
            site_total_pages_hint: None,
            products_on_last_page_hint: None,
        }
    }

    /// 사이트 페이지네이션 힌트 설정 (StatusCheck 결과를 상위에서 주입)
    pub fn set_site_pagination_hints(&mut self, total_pages: u32, products_on_last_page: u32) {
        self.site_total_pages_hint = Some(total_pages);
        self.products_on_last_page_hint = Some(products_on_last_page);
        info!("🔧 Applied site pagination hints: total_pages={}, products_on_last_page={}", total_pages, products_on_last_page);
    }
    
    /// 실제 서비스 초기화 - guide/re-arch-plan-final2.md 설계 기반
    /// ServiceBasedBatchCrawlingEngine 패턴 참조하되 Actor 모델에 맞게 구현
    pub async fn initialize_real_services(&mut self, _context: &AppContext) -> Result<(), StageError> {
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
        let status_checker = Arc::new(StatusCheckerImpl::with_product_repo(
            http_client.clone(),
            data_extractor.clone(),
            app_config.clone(),
            Arc::clone(&product_repo),
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
        let status_checker_impl = Arc::new(StatusCheckerImpl::with_product_repo(
            http_client.clone(),
            data_extractor.clone(),
            app_config.clone(),
            Arc::clone(&product_repo),
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
    self._product_repo = Some(product_repo);
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
            batch_id: Some(self.batch_id.clone()),
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
                            batch_id: Some(self.batch_id.clone()),
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
                            batch_id: Some(self.batch_id.clone()),
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
                    batch_id: Some(self.batch_id.clone()),
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
        
        // 동시성 상한을 Collector에도 반영하도록 CollectorConfig를 재구성(필요 시)
        if let (Some(http_client), Some(data_extractor), Some(app_config)) = (&self.http_client, &self.data_extractor, &self.app_config) {
            // 리스트 수집기
            if let Some(repo) = &self._product_repo {
                let list_cfg = crate::infrastructure::crawling_service_impls::CollectorConfig {
                    max_concurrent: concurrency_limit,
                    concurrency: concurrency_limit,
                    delay_between_requests: std::time::Duration::from_millis(app_config.user.request_delay_ms),
                    delay_ms: app_config.user.request_delay_ms,
                    batch_size: app_config.user.batch.batch_size,
                    retry_attempts: app_config.user.crawling.workers.max_retries,
                    retry_max: app_config.user.crawling.workers.max_retries,
                };
                let status_checker_for_list = Arc::new(crate::infrastructure::crawling_service_impls::StatusCheckerImpl::with_product_repo(
                    (**http_client).clone(),
                    (**data_extractor).clone(),
                    app_config.clone(),
                    Arc::clone(repo),
                ));
                self.product_list_collector = Some(Arc::new(crate::infrastructure::crawling_service_impls::ProductListCollectorImpl::new(
                    Arc::clone(http_client),
                    Arc::clone(data_extractor),
                    list_cfg,
                    status_checker_for_list,
                )));
            }

            // 상세 수집기
            let detail_cfg = crate::infrastructure::crawling_service_impls::CollectorConfig {
                max_concurrent: concurrency_limit,
                concurrency: concurrency_limit,
                delay_between_requests: std::time::Duration::from_millis(app_config.user.request_delay_ms),
                delay_ms: app_config.user.request_delay_ms,
                batch_size: app_config.user.batch.batch_size,
                retry_attempts: app_config.user.crawling.workers.max_retries,
                retry_max: app_config.user.crawling.workers.max_retries,
            };
            self.product_detail_collector = Some(Arc::new(crate::infrastructure::crawling_service_impls::ProductDetailCollectorImpl::new(
                Arc::clone(http_client),
                Arc::clone(data_extractor),
                detail_cfg,
            )));
        }

        // 동시성 제어를 위한 세마포어
        let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency_limit as usize));
        let mut tasks = Vec::new();
        
        // 서비스 의존성 복사
        let status_checker = self.status_checker.clone();
        let product_list_collector = self.product_list_collector.clone();
        let product_detail_collector = self.product_detail_collector.clone();
    let product_repo = self._product_repo.clone();
        let http_client = self.http_client.clone();
        let data_extractor = self.data_extractor.clone();
    // 페이지네이션 힌트 복사
    let site_total_pages_hint = self.site_total_pages_hint;
    let products_on_last_page_hint = self.products_on_last_page_hint;
        
        // 각 아이템을 병렬로 처리
        for item in items {
            let sem = semaphore.clone();
            let base_item = item.clone(); // used for lifecycle pre-emits
            let stage_type_clone = stage_type.clone();
            let status_checker_clone = status_checker.clone();
            let product_list_collector_clone = product_list_collector.clone();
            let product_detail_collector_clone = product_detail_collector.clone();
            let product_repo_clone = product_repo.clone();
            let http_client_clone = http_client.clone();
            let data_extractor_clone = data_extractor.clone();
            let session_id_clone = _context.session_id.clone();
            let batch_id_opt = Some(self.batch_id.clone());
            let ctx_clone = _context.clone();
            // Pre-emit lifecycle coarse events (page/product) before spawning heavy work
            match (&stage_type, &base_item) {
                (StageType::ListPageCrawling, StageItem::Page(pn)) => {
                    if let Err(e) = ctx_clone.emit_event(AppEvent::PageLifecycle { session_id: session_id_clone.clone(), batch_id: batch_id_opt.clone(), page_number: *pn, status: "fetch_started".into(), metrics: None, timestamp: Utc::now() }).await {
                        error!("PageLifecycle fetch_started emit failed page={} err={}", pn, e);
                    } else {
                        debug!("Emitted PageLifecycle fetch_started page={}", pn);
                    }
                }
                (StageType::ProductDetailCrawling, StageItem::ProductUrls(urls)) => {
                    let count = urls.urls.len() as u32;
                    let metrics = crate::new_architecture::actors::types::SimpleMetrics::Page { url_count: None, scheduled_details: Some(count), error: None };
                    let page_hint = urls.urls.first().map(|u| u.page_id as u32).unwrap_or(0u32);
                    if let Err(e) = ctx_clone.emit_event(AppEvent::PageLifecycle { session_id: session_id_clone.clone(), batch_id: batch_id_opt.clone(), page_number: page_hint, status: "detail_scheduled".into(), metrics: Some(metrics), timestamp: Utc::now() }).await {
                        error!("PageLifecycle detail_scheduled emit failed page={} err={}", page_hint, e);
                    } else {
                        debug!("Emitted PageLifecycle detail_scheduled page={} scheduled_details={}", page_hint, count);
                    }
                    for pu in &urls.urls { // per-product fetch_started
                        if let Err(e) = ctx_clone.emit_event(AppEvent::ProductLifecycle {
                            session_id: session_id_clone.clone(),
                            batch_id: batch_id_opt.clone(),
                            page_number: Some(pu.page_id as u32),
                            product_ref: pu.url.clone(),
                            status: "fetch_started".into(),
                            retry: None,
                            duration_ms: None,
                            metrics: None,
                            timestamp: Utc::now(),
                        }).await {
                            error!("ProductLifecycle fetch_started emit failed product={} err={}", pu.url, e);
                        }
                    }
                }
                _ => {}
            }
            let task = tokio::spawn(async move {
                let _permit = sem.acquire().await.map_err(|e|
                    StageError::InitializationFailed(format!("Semaphore error: {}", e))
                )?;
                if let Err(e) = ctx_clone.emit_event(AppEvent::StageItemStarted {
                    session_id: session_id_clone.clone(),
                    batch_id: batch_id_opt.clone(),
                    stage_type: stage_type_clone.clone(),
                    item_id: base_item.id_string(),
                    item_type: base_item.item_type_enum(),
                    timestamp: Utc::now(),
                }).await { tracing::error!("StageItemStarted emit failed: {}", e); }
                let item_start = Instant::now();
                let temp_actor = StageActor {
                    actor_id: "temp".to_string(),
                    batch_id: "temp".to_string(),
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
                    status_checker: status_checker_clone.clone(),
                    product_list_collector: product_list_collector_clone.clone(),
                    product_detail_collector: product_detail_collector_clone.clone(),
                    _product_repo: product_repo_clone.clone(),
                    http_client: http_client_clone,
                    data_extractor: data_extractor_clone,
                    app_config: None,
                    site_total_pages_hint,
                    products_on_last_page_hint,
                };
                let lifecycle_item = base_item.clone();
                let processing_item = base_item.clone();
                let result = temp_actor.process_single_item(
                    stage_type_clone.clone(),
                    processing_item,
                    status_checker_clone,
                    product_list_collector_clone,
                    product_detail_collector_clone,
                    product_repo_clone,
                ).await;
                match &result {
                    Ok(r) => {
                        // If this is DataSaving stage, perform persistence now (previous arm returned Ok(()))
                        // NOTE: Previous pattern used value patterns against references &StageType/&StageItem and never matched.
                        // We now explicitly match references to ensure the block executes.
                        if matches!(stage_type_clone, StageType::DataSaving) {
                            // DataSaving 단계에서는 ProductDetails 또는 ValidatedProducts 둘 다 저장 대상이 될 수 있음
                            let is_persist_target = matches!(lifecycle_item, StageItem::ProductDetails(_)) || matches!(lifecycle_item, StageItem::ValidatedProducts(_));
                            if is_persist_target {
                                info!("[Persist] Enter DataSaving block batch_id={:?} variant={}", batch_id_opt, match &lifecycle_item { StageItem::ProductDetails(_) => "ProductDetails", StageItem::ValidatedProducts(_) => "ValidatedProducts", _ => "Other" });
                                // Duplicate guard
                                let guard_key = format!("{}:{}:data_saving", session_id_clone, batch_id_opt.clone().unwrap_or_else(|| "none".into()));
                                if let Ok(mut guard) = DATA_SAVING_RUN_GUARD.lock() {
                                    if guard.contains(&guard_key) {
                                        info!("[PersistGuard] duplicate DataSaving suppression key={}", guard_key);
                                        return Ok(StageItemResult { item_id: "data_saving_guard".into(), item_type: StageItemType::Url { url_type: "data_saving".into() }, success: true, error: None, duration_ms: item_start.elapsed().as_millis() as u64, retry_count: 0, collected_data: None });
                                    } else {
                                        guard.insert(guard_key);
                                        info!("[PersistGuard] first DataSaving execution proceeding");
                                    }
                                }
                                // decide skip via env inside task scope
                                let skip_save = std::env::var("MC_SKIP_DB_SAVE")
                                    .ok()
                                    .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                                    .unwrap_or(false);
                                if skip_save {
                                    info!("[Persist] Skipped by env MC_SKIP_DB_SAVE");
                                    if let Err(e) = ctx_clone.emit_event(AppEvent::ProductLifecycle {
                                        session_id: session_id_clone.clone(),
                                        batch_id: batch_id_opt.clone(),
                                        page_number: None,
                                        product_ref: "_batch_persist".into(),
                                        status: "persist_skipped".into(),
                                        retry: None,
                                        duration_ms: None,
                                        metrics: Some(SimpleMetrics::Generic { key: "reason".into(), value: "MC_SKIP_DB_SAVE".into() }),
                                        timestamp: Utc::now(),
                                    }).await { error!("ProductLifecycle persist_skipped emit failed err={}", e); }
                                } else {
                                    let attempted_count = match &lifecycle_item { StageItem::ValidatedProducts(v) => v.products.len() as u32, StageItem::ProductDetails(d) => d.products.len() as u32, _ => 0 };
                                    if let Err(e) = ctx_clone.emit_event(AppEvent::ProductLifecycle {
                                        session_id: session_id_clone.clone(),
                                        batch_id: batch_id_opt.clone(),
                                        page_number: None,
                                        product_ref: "_batch_persist".into(),
                                        status: "persist_started".into(),
                                        retry: None,
                                        duration_ms: None,
                                        metrics: Some(SimpleMetrics::Generic { key: "attempted_count".into(), value: attempted_count.to_string() }),
                                        timestamp: Utc::now(),
                                    }).await { error!("ProductLifecycle persist_started emit failed err={}", e); }
                                    let persist_start = Instant::now();
                                    if attempted_count == 0 {
                                        info!("[Persist] Empty batch (attempted_count=0) -> emit persist_empty and skip storage call");
                                        let _ = ctx_clone.emit_event(AppEvent::ProductLifecycle {
                                            session_id: session_id_clone.clone(),
                                            batch_id: batch_id_opt.clone(),
                                            page_number: None,
                                            product_ref: "_batch_persist".into(),
                                            status: "persist_empty".into(),
                                            retry: None,
                                            duration_ms: Some(0),
                                            metrics: Some(SimpleMetrics::Generic { key: "persist_result".into(), value: "attempted=0".into() }),
                                            timestamp: Utc::now(),
                                        }).await;
                                        return Ok(StageItemResult { item_id: "data_saving_empty".into(), item_type: StageItemType::Url { url_type: "data_saving".into() }, success: true, error: None, duration_ms: item_start.elapsed().as_millis() as u64, retry_count: 0, collected_data: None });
                                    }
                                    if let Some(repo) = &temp_actor._product_repo {
                                        info!("[PersistExec] starting storage call variant={}", match &lifecycle_item { StageItem::ProductDetails(_) => "ProductDetails", StageItem::ValidatedProducts(_) => "ValidatedProducts", _ => "Other" });
                                        match StageActor::execute_real_database_storage(&lifecycle_item, repo.clone()).await {
                                            Ok((inserted, updated, duplicates_ct)) => {
                                                let total = inserted + updated;
                                                let attempted = attempted_count; // from outer scope
                                                let unchanged = if attempted > total { attempted - total } else { 0 };
                                                let status = if inserted > 0 && updated == 0 { "persist_inserted" } else if updated > 0 && inserted == 0 { "persist_updated" } else if inserted == 0 && updated == 0 { if duplicates_ct == attempted { "persist_noop_all_duplicate" } else { "persist_noop" } } else { "persist_mixed" };
                                                info!("[Persist] Result status={} inserted={} updated={} duplicates={} total={} unchanged={} attempted={} duration_ms={}", status, inserted, updated, duplicates_ct, total, unchanged, attempted, persist_start.elapsed().as_millis());
                                                let metrics = SimpleMetrics::Generic { key: "persist_result".into(), value: format!("attempted={},inserted={},updated={},duplicates={},total={},unchanged={}", attempted, inserted, updated, duplicates_ct, total, unchanged) };
                                                let emit_res = ctx_clone.emit_event(AppEvent::ProductLifecycle {
                                                    session_id: session_id_clone.clone(),
                                                    batch_id: batch_id_opt.clone(),
                                                    page_number: None,
                                                    product_ref: "_batch_persist".into(),
                                                    status: status.into(),
                                                    retry: None,
                                                    duration_ms: Some(persist_start.elapsed().as_millis() as u64),
                                                    metrics: Some(metrics),
                                                    timestamp: Utc::now(),
                                                }).await;
                                                match emit_res {
                                                    Ok(_) => info!("[PersistEmit] lifecycle emitted status={}", status),
                                                    Err(e) => error!("ProductLifecycle {} emit failed err={}", status, e)
                                                }
                                                if status == "persist_noop" {
                                                    // emit anomaly diagnostic + (future) logical drift probe
                                                    if let Some(repo) = &temp_actor._product_repo {
                                                        if let Ok((cnt, minp, maxp, _)) = repo.get_product_detail_stats().await {
                                                            let attempted = match &lifecycle_item { StageItem::ValidatedProducts(v) => v.products.len() as u32, StageItem::ProductDetails(d) => d.products.len() as u32, _ => total };
                                                            let _ = ctx_clone.emit_event(AppEvent::PersistenceAnomaly {
                                                                session_id: session_id_clone.clone(),
                                                                batch_id: batch_id_opt.clone(),
                                                                kind: "all_noop".into(),
                                                                detail: format!("All attempted saves were noop: attempted={} inserted={} updated={} db_total={} db_page_range={:?}-{:?}", attempted, inserted, updated, cnt, minp, maxp),
                                                                attempted,
                                                                inserted,
                                                                updated,
                                                                timestamp: Utc::now(),
                                                            }).await;
                                                            // LogicalMappingDrift (placeholder): detect unexpected min/max inversion or large gap
                                                            if let (Some(min_p), Some(max_p)) = (minp, maxp) {
                                                                if min_p > max_p { // impossible condition -> drift
                                                                    let _ = ctx_clone.emit_event(AppEvent::PersistenceAnomaly {
                                                                        session_id: session_id_clone.clone(),
                                                                        batch_id: batch_id_opt.clone(),
                                                                        kind: "logical_mapping_drift".into(),
                                                                        detail: format!("Inverted page range detected min_page={} > max_page={} during noop persistence", min_p, max_p),
                                                                        attempted,
                                                                        inserted,
                                                                        updated,
                                                                        timestamp: Utc::now(),
                                                                    }).await;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                error!("[Persist] Failed error={} duration_ms={}", e, persist_start.elapsed().as_millis());
                                                let emit_res = ctx_clone.emit_event(AppEvent::ProductLifecycle {
                                                    session_id: session_id_clone.clone(),
                                                    batch_id: batch_id_opt.clone(),
                                                    page_number: None,
                                                    product_ref: "_batch_persist".into(),
                                                    status: "persist_failed".into(),
                                                    retry: None,
                                                    duration_ms: Some(persist_start.elapsed().as_millis() as u64),
                                                    metrics: Some(SimpleMetrics::Generic { key: "error".into(), value: e.clone() }),
                                                    timestamp: Utc::now(),
                                                }).await;
                                                if let Err(e2) = emit_res { error!("ProductLifecycle persist_failed emit failed err={}", e2); } else { info!("[PersistEmit] lifecycle emitted status=persist_failed"); }
                                            }
                                        }
                                    } else {
                                        error!("Product repository not available during DataSaving persistence stage");
                                    }
                                }
                            }
                        }
                        if let Err(e) = ctx_clone.emit_event(AppEvent::StageItemCompleted {
                            session_id: session_id_clone.clone(),
                            batch_id: batch_id_opt.clone(),
                            stage_type: stage_type_clone.clone(),
                            item_id: r.item_id.clone(),
                            item_type: r.item_type.clone(),
                            success: true,
                            error: None,
                            duration_ms: item_start.elapsed().as_millis() as u64,
                            retry_count: r.retry_count,
                            collected_count: r.collected_data.as_ref().map(|d| {
                                // JSON 배열일 가능성 높음 → 대략 길이 추정 (간단 처리)
                                if d.starts_with('[') { d.matches("\"").count() as u32 / 2 } else { 1 }
                            }),
                            timestamp: Utc::now(),
                        }).await { tracing::error!("StageItemCompleted emit failed: {}", e); }
                        // Emit lifecycle completion for page or product aggregated result
                        if let (StageType::ListPageCrawling, StageItem::Page(pn)) = (&stage_type_clone, &lifecycle_item) {
                            let metrics = crate::new_architecture::actors::types::SimpleMetrics::Page { url_count: Some(r.collected_data.as_ref().map(|d| d.len() as u32).unwrap_or(0)), scheduled_details: None, error: None };
                            if let Err(e) = ctx_clone.emit_event(AppEvent::PageLifecycle { session_id: session_id_clone.clone(), batch_id: batch_id_opt.clone(), page_number: *pn, status: "fetch_completed".into(), metrics: Some(metrics), timestamp: Utc::now() }).await { error!("PageLifecycle fetch_completed emit failed page={} err={}", pn, e); } else { debug!("Emitted PageLifecycle fetch_completed page={}", pn); }
                        }
                        // Product detail lifecycle completion (group success)
                        if let (StageType::ProductDetailCrawling, StageItem::ProductUrls(urls)) = (&stage_type_clone, &lifecycle_item) {
                            for pu in &urls.urls {
                                if let Err(e) = ctx_clone.emit_event(AppEvent::ProductLifecycle {
                                    session_id: session_id_clone.clone(),
                                    batch_id: batch_id_opt.clone(),
                                    page_number: Some(pu.page_id as u32),
                                    product_ref: pu.url.clone(),
                                    status: "fetch_completed".into(),
                                    retry: None,
                                    duration_ms: Some(item_start.elapsed().as_millis() as u64),
                                    metrics: None,
                                    timestamp: Utc::now(),
                                }).await { error!("ProductLifecycle fetch_completed emit failed product={} err={}", pu.url, e); }
                            }
                        }
                    }
                    Err(err) => {
                        if let Err(e) = ctx_clone.emit_event(AppEvent::StageItemCompleted {
                            session_id: session_id_clone.clone(),
                            batch_id: batch_id_opt.clone(),
                            stage_type: stage_type_clone.clone(),
                            item_id: "unknown".into(),
                            item_type: StageItemType::Url { url_type: "unknown".into() },
                            success: false,
                            error: Some(err.to_string()),
                            duration_ms: item_start.elapsed().as_millis() as u64,
                            retry_count: 0,
                            collected_count: None,
                            timestamp: Utc::now(),
                        }).await { tracing::error!("StageItemCompleted emit failed: {}", e); }
                        if let (StageType::ListPageCrawling, StageItem::Page(pn)) = (&stage_type_clone, &lifecycle_item) {
                            let metrics = crate::new_architecture::actors::types::SimpleMetrics::Page { url_count: None, scheduled_details: None, error: Some(err.to_string()) };
                            if let Err(e2) = ctx_clone.emit_event(AppEvent::PageLifecycle { session_id: session_id_clone.clone(), batch_id: batch_id_opt.clone(), page_number: *pn, status: "failed".into(), metrics: Some(metrics), timestamp: Utc::now() }).await { error!("PageLifecycle failed emit failed page={} err={}", pn, e2); } else { debug!("Emitted PageLifecycle failed page={}", pn); }
                        }
                        // Product detail lifecycle failure
                        if let (StageType::ProductDetailCrawling, StageItem::ProductUrls(urls)) = (&stage_type_clone, &lifecycle_item) {
                            for pu in &urls.urls {
                                let metrics = crate::new_architecture::actors::types::SimpleMetrics::Product { fields: None, size_bytes: None, error: Some(err.to_string()) };
                                if let Err(e2) = ctx_clone.emit_event(AppEvent::ProductLifecycle {
                                    session_id: session_id_clone.clone(),
                                    batch_id: batch_id_opt.clone(),
                                    page_number: Some(pu.page_id as u32),
                                    product_ref: pu.url.clone(),
                                    status: "failed".into(),
                                    retry: None,
                                    duration_ms: Some(item_start.elapsed().as_millis() as u64),
                                    metrics: Some(metrics),
                                    timestamp: Utc::now(),
                                }).await { error!("ProductLifecycle failed emit failed product={} err={}", pu.url, e2); }
                            }
                        }
                    }
                }
                result
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
        &self,
        stage_type: StageType,
        item: StageItem,
        status_checker: Option<Arc<dyn StatusChecker>>,
        product_list_collector: Option<Arc<dyn ProductListCollector>>,
    _product_detail_collector: Option<Arc<dyn ProductDetailCollector>>,
    _product_repo: Option<Arc<IntegratedProductRepository>>,
    ) -> Result<StageItemResult, StageError> {
        let start_time = Instant::now();
        
        let item_id = match &item {
            StageItem::Page(page_num) => format!("page_{}", page_num),
            StageItem::Url(url) => url.clone(),
            StageItem::Product(product) => product.url.clone(),
            StageItem::ProductList(list) => format!("page_{}", list.page_number),
            StageItem::ProductUrls(urls) => format!("urls_{}", urls.urls.len()),
            StageItem::ProductDetails(details) => format!("details_{}", details.products.len()),
            StageItem::ValidatedProducts(products) => format!("validated_{}", products.products.len()),
            _ => "unknown".to_string(),
        };
        
        debug!("Processing item {} for stage {:?}", item_id, stage_type);
        
        // 스테이지 타입별 처리 로직 - 수집된 데이터와 성공 여부를 함께 반환
        let (success, collected_data, retries_used) = match stage_type {
            StageType::StatusCheck => {
                if let Some(checker) = status_checker {
                    match Self::execute_real_status_check(&item, checker).await {
                        Ok(site_status) => {
                            match serde_json::to_string(&site_status) {
                                Ok(json) => (Ok(()), Some(json), 0),
                                Err(e) => (Err(format!("JSON serialization failed: {}", e)), None, 0),
                            }
                        }
                        Err(e) => (Err(e), None, 0),
                    }
                } else {
                    // StatusChecker가 없으면 에러
                    (Err("StatusChecker not available".to_string()), None, 0)
                }
            }
            StageType::ListPageCrawling => {
                if let Some(collector) = product_list_collector {
                    // 재시도 설정 로드
                    // 권장 기본 재시도 값 (설명된 스펙 기반): 리스트 페이지 4회
                    const RECOMMENDED_MAX_RETRIES_LIST: u32 = 4;
                    let (cfg_retries, base_delay_ms) = if let Some(cfg) = &self.app_config {
                        (cfg.user.crawling.workers.max_retries, cfg.user.crawling.timing.retry_delay_ms)
                    } else {
                        (3u32, 1000u64)
                    };
                    // 설정값과 권장값 중 큰 값 적용
                    let max_retries = std::cmp::max(cfg_retries, RECOMMENDED_MAX_RETRIES_LIST);
                    // 지수 백오프 + 지터를 위한 파라미터
                    let base_delay_ms = base_delay_ms.max(200); // 안전한 최소값
                    let max_delay_ms: u64 = 30_000; // 30초 상한

                    let mut attempt: u32 = 0;
                    loop {
                        match self.execute_real_list_page_processing(&item, Arc::clone(&collector)).await {
                            Ok(urls) => {
                                // ProductURL들을 JSON으로 직렬화하여 저장
                                match serde_json::to_string(&urls) {
                                    Ok(json_data) => break (Ok(()), Some(json_data), attempt),
                                    Err(e) => break (Err(format!("JSON serialization failed: {}", e)), None, attempt),
                                }
                            }
                            Err(e) => {
                                if attempt < max_retries {
                                    attempt += 1;
                                    // 지수 백오프: base * 2^(attempt-1)
                                    // Note: use checked_shl to avoid panics for large shifts
                                    let factor = 1u64
                                        .checked_shl(attempt - 1)
                                        .unwrap_or(u64::MAX);
                                    let exp = base_delay_ms.saturating_mul(factor);
                                    let capped = std::cmp::min(exp, max_delay_ms);
                                    // 지터: 최대 20% 랜덤 가산
                                    let jitter = if capped >= 10 {
                                        let range = capped / 5; // 20%
                                        fastrand::u64(0..=range)
                                    } else { 0 };
                                    let delay = capped.saturating_add(jitter);
                                    warn!(
                                        "🔁 ListPageCrawling attempt {}/{} after {}ms (reason: {})",
                                        attempt,
                                        max_retries,
                                        delay,
                                        e
                                    );
                                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                                    continue;
                                } else {
                                    error!("❌ ListPageCrawling final failure: {}", e);
                                    break (Err(e), None, attempt);
                                }
                            }
                        }
                    }
                } else {
                    // ProductListCollector가 없으면 에러
                    (Err("ProductListCollector not available".to_string()), None, 0)
                }
            }
            StageType::ProductDetailCrawling => {
                // Stage 2의 결과로 받은 ProductUrls에서 실제 제품 상세 정보 수집
        info!("🔍 ProductDetailCrawling: processing ProductUrls from item {}", item_id);

                match &item {
                    StageItem::ProductUrls(product_urls) => {
            // Compact: log once at start of detail crawling for this item
            info!("📋 Detail crawling for {} product URLs", product_urls.urls.len());
                        
                        if let Some(collector) = &self.product_detail_collector {
                            // 실제 ProductDetailCollector를 사용하여 상세 정보 수집
                            match Self::execute_real_product_detail_processing(product_urls, Arc::clone(collector)).await {
                                Ok(product_details) => {
                                    info!("✅ Successfully collected {} product details", product_details.len());
                                    
                                    // ProductDetails 래퍼 생성
                                    use crate::new_architecture::channels::types::{ProductDetails, ExtractionStats};
                                    let product_details_wrapper = ProductDetails {
                                        products: product_details.clone(),
                                        source_urls: product_urls.urls.clone(),
                                        extraction_stats: ExtractionStats {
                                            attempted: product_urls.urls.len() as u32,
                                            successful: product_details.len() as u32,
                                            failed: (product_urls.urls.len() - product_details.len()) as u32,
                                            empty_responses: 0, // 현재는 0으로 설정
                                        },
                                    };
                                    
                                    // ProductDetails 래퍼를 JSON으로 직렬화하여 저장
                    debug!("Serializing ProductDetails wrapper with {} products", product_details_wrapper.products.len());
                                    match serde_json::to_string(&product_details_wrapper) {
                                        Ok(json_data) => {
                        debug!("ProductDetails JSON serialization successful: {} chars", json_data.len());
                                            (Ok(()), Some(json_data), 0)
                                        },
                                        Err(e) => {
                                            error!("❌ ProductDetails JSON serialization failed: {}", e);
                                            (Err(format!("JSON serialization failed: {}", e)), None, 0)
                                        },
                                    }
                                }
                                Err(e) => {
                                    error!("❌ Product detail crawling failed: {}", e);
                                    (Err(e), None, 0)
                                }
                            }
                        } else {
                            error!("❌ ProductDetailCollector not available");
                            (Err("ProductDetailCollector not available".to_string()), None, 0)
                        }
                    }
                    StageItem::ProductList(product_list) => {
                        // Legacy: ProductList에서 ProductUrl 객체 변환하여 처리
                        info!("📋 Converting {} products from page {} to ProductUrls for detail crawling", 
                              product_list.products.len(), product_list.page_number);
                        
                        // ⭐ 중요: Product -> ProductUrl로 변환 시 메타데이터 보존
                        // 실제 사이트 정보를 가져와서 PageIdCalculator 초기화
                        // StatusChecker trait에 discover_total_pages가 없으므로 fallback 값 사용
                        let (total_pages, products_on_last_page) = match (self.site_total_pages_hint, self.products_on_last_page_hint) {
                            (Some(tp), Some(plp)) => (tp, plp),
                            _ => {
                                // 최후의 수단으로 알려진 값 사용
                                let fallback = (498u32, 8u32);
                                info!("✅ Using fallback site info: total_pages={}, products_on_last_page={}", fallback.0, fallback.1);
                                fallback
                            }
                        };

                        // Canonical 계산: 페이지 enumerate index 는 0-based
                        let canonical_calc = PaginationCalculator::default(); // total_pages 는 호출 시 세 번째 인자로 전달
                        let product_urls: Vec<crate::domain::product_url::ProductUrl> = product_list.products
                            .iter()
                            .enumerate()
                            .map(|(zero_idx, product)| {
                                let c = canonical_calc.calculate(product_list.page_number, zero_idx as u32, total_pages);
                                crate::domain::product_url::ProductUrl { url: product.url.clone(), page_id: c.page_id, index_in_page: c.index_in_page }
                            })
                            .collect();

                        // 🔎 Debug a compact summary of calculated mappings for this page
                        if !product_urls.is_empty() {
                            let min_page_id = product_urls.iter().map(|p| p.page_id).min().unwrap_or(0);
                            let max_page_id = product_urls.iter().map(|p| p.page_id).max().unwrap_or(0);
                            let min_index = product_urls.iter().map(|p| p.index_in_page).min().unwrap_or(0);
                            let max_index = product_urls.iter().map(|p| p.index_in_page).max().unwrap_or(0);
                            let sample: Vec<String> = product_urls
                                .iter()
                                .take(6)
                                .enumerate()
                                .map(|(i, p)| format!("i{}=>p{}_i{}", i, p.page_id, p.index_in_page))
                                .collect();
                            debug!(
                                "📐 Stage mapping summary (page {}): count={}, page_id=[{}..{}], index_in_page=[{}..{}], sample={:?}",
                                product_list.page_number,
                                product_urls.len(),
                                min_page_id,
                                max_page_id,
                                min_index,
                                max_index,
                                sample
                            );
                        }
                        
                        if let Some(collector) = &self.product_detail_collector {
                            use crate::new_architecture::channels::types::ProductUrls;
                            let product_urls_wrapper = ProductUrls {
                                urls: product_urls, // ProductUrl 객체들 직접 저장
                                batch_id: Some(format!("batch_{}", product_list.page_number)),
                            };
                            
                            match Self::execute_real_product_detail_processing(&product_urls_wrapper, Arc::clone(collector)).await {
                                Ok(product_details) => {
                                    info!("✅ Successfully collected {} product details", product_details.len());
                                    match serde_json::to_string(&product_details) {
                                        Ok(json_data) => (Ok(()), Some(json_data), 0),
                                        Err(e) => (Err(format!("JSON serialization failed: {}", e)), None, 0),
                                    }
                                }
                                Err(e) => {
                                    error!("❌ Product detail crawling failed: {}", e);
                                    (Err(e), None, 0)
                                }
                            }
                        } else {
                            error!("❌ ProductDetailCollector not available");
                            (Err("ProductDetailCollector not available".to_string()), None, 0)
                        }
                    }
                    other => {
                        warn!("⚠️ ProductDetailCrawling stage received unexpected item type: {:?}", other);
                        (Err("Unexpected item type for ProductDetailCrawling".to_string()), None, 0)
                    }
                }
            }
            StageType::DataValidation => {
                // Stage 3 (ProductDetailCrawling)에서 수집된 ProductDetail들을 검증
                info!("🔍 DataValidation: validating ProductDetails from item {}", item_id);
                
                let product_details: Vec<crate::domain::product::ProductDetail> = match &item {
                    // Stage 3에서 ProductDetails 데이터를 받음
                    StageItem::ProductDetails(product_details_wrapper) => {
                        info!("📋 Processing ProductDetails with {} products", product_details_wrapper.products.len());
                        product_details_wrapper.products.clone()
                    }
                    StageItem::ProductUrls(_product_urls) => {
                        warn!("⚠️ DataValidation received ProductUrls instead of ProductDetails - Stage 3 may have failed");
                        Vec::new()
                    }
                    other => {
                        warn!("⚠️ DataValidation stage received unexpected item type: {:?}", other);
                        Vec::new()
                    }
                };
                
                info!("✅ Extracted {} ProductDetails for validation", product_details.len());
                
                // DataQualityAnalyzer로 품질 검증
                use crate::new_architecture::services::data_quality_analyzer::DataQualityAnalyzer;
                let quality_analyzer = DataQualityAnalyzer::new();
                
                match quality_analyzer.validate_before_storage(&product_details).await {
                    Ok(validated_products) => {
                        // 검증된 제품들을 JSON으로 직렬화
                        match serde_json::to_string(&validated_products) {
                            Ok(json_data) => (Ok(()), Some(json_data), 0),
                            Err(e) => (Err(format!("JSON serialization failed: {}", e)), None, 0),
                        }
                    }
                    Err(e) => {
                        error!("❌ Data validation failed: {}", e);
                        (Err(format!("Data validation failed: {}", e)), None, 0)
                    }
                }
            }
            StageType::DataSaving => {
                // DataSaving persistence 로직은 비동기 task 내부에서 처리 (ctx_clone 스코프 확보 위해 여기서는 패스스루)
                // 여기서는 StageItem 처리 결과를 (Ok, None, 0) 로 넘기고 실제 저장은 아래 task match 분기에서 수행
                (Ok(()), None, 0)
            }
        };
        
        let duration = start_time.elapsed();
        
        // StageItem을 StageItemType으로 변환하는 헬퍼 함수
        let item_type = match &item {
            StageItem::Page(page_num) => StageItemType::Page { page_number: *page_num },
            StageItem::Url(_url) => StageItemType::Url { url_type: "site_check".to_string() },
            StageItem::Product(_product) => StageItemType::Url { url_type: "product".to_string() },
            StageItem::ProductList(_) => StageItemType::ProductUrls { urls: vec![] },
            StageItem::ProductUrls(urls) => StageItemType::ProductUrls { urls: urls.urls.iter().map(|u| u.url.clone()).collect() },
            _ => StageItemType::Url { url_type: "unknown".to_string() },
        };
        
        match success {
            Ok(()) => Ok(StageItemResult {
                item_id: item_id,
                item_type,
                success: true,
                error: None,
                duration_ms: duration.as_millis() as u64,
                retry_count: retries_used,
                collected_data,
            }),
            Err(error) => {
                let error_item_type = match &item {
                    StageItem::Page(page_num) => StageItemType::Page { page_number: *page_num },
                    StageItem::Url(_url) => StageItemType::Url { url_type: "site_check".to_string() },
                    StageItem::Product(_product) => StageItemType::Url { url_type: "product".to_string() },
                    StageItem::ProductList(_) => StageItemType::ProductUrls { urls: vec![] },
                    StageItem::ProductUrls(urls) => StageItemType::ProductUrls { urls: urls.urls.iter().map(|u| u.url.clone()).collect() },
                    _ => StageItemType::Url { url_type: "unknown".to_string() },
                };
                
                Ok(StageItemResult {
                    item_id: item_id.clone(),
                    item_type: error_item_type,
                    success: false,
                    error: Some(error.clone()),
                    duration_ms: duration.as_millis() as u64,
                    retry_count: retries_used,
                    collected_data: None,
                })
            }
        }
    }
    
    // === 실제 서비스 기반 처리 함수들 (Critical Issue #1) ===
    
    /// 실제 상태 확인 처리
    async fn execute_real_status_check(
        item: &StageItem,
        status_checker: Arc<dyn StatusChecker>,
    ) -> Result<SiteStatus, String> {
        // 새로운 StageItem 구조에 맞게 수정
        let item_desc = match item {
            StageItem::Page(page_num) => format!("page_{}", page_num),
            StageItem::Url(url) => url.clone(),
            _ => "unknown".to_string(),
        };
        
        // 실제 사이트 상태 확인
        match status_checker.check_site_status().await {
            Ok(status) => {
                info!("✅ Real status check successful for item {}", item_desc);
                Ok(status)
            }
            Err(e) => {
                warn!("❌ Real status check failed for item {}: {}", item_desc, e);
                Err(format!("Status check failed: {}", e))
            }
        }
    }
    
    /// 실제 리스트 페이지 처리
    async fn execute_real_list_page_processing(
        &self,
        item: &StageItem,
        product_list_collector: Arc<dyn ProductListCollector>,
    ) -> Result<Vec<crate::domain::product_url::ProductUrl>, String> {
        match item {
            StageItem::Page(page_number) => {
                // 실제 리스트 페이지 크롤링
                // 페이지네이션 힌트 사용, 없으면 필요 시 상태 재확인
                let (total_pages, products_on_last_page) = match (self.site_total_pages_hint, self.products_on_last_page_hint) {
                    (Some(tp), Some(plp)) => (tp, plp),
                    _ => {
                        if let Some(checker) = &self.status_checker {
                            match checker.check_site_status().await {
                                Ok(s) => (s.total_pages, s.products_on_last_page),
                                Err(e) => {
                                    warn!("⚠️ Failed to get site status for list processing, using conservative defaults: {}", e);
                                    (100u32, 10u32)
                                }
                            }
                        } else {
                            warn!("⚠️ No StatusChecker available; using conservative defaults for pagination");
                            (100u32, 10u32)
                        }
                    }
                };

                // 단일 페이지 수집 API를 사용하여 실패 시 에러를 그대로 전파
                match product_list_collector.collect_single_page(
                    *page_number, total_pages, products_on_last_page
                ).await {
                    Ok(urls) => {
                        // 빈 결과는 실패로 간주하여 재시도를 유도
                        if urls.is_empty() {
                            warn!("⚠️ Page {} returned 0 URLs — treating as failure to trigger retry", page_number);
                            Err("Empty result from list page".to_string())
                        } else {
                            info!(
                                "✅ Real list page processing successful for page {}: {} URLs collected",
                                page_number, urls.len()
                            );
                            for (index, url) in urls.iter().enumerate() {
                                debug!("  📄 Collected URL {}: {}", index + 1, url.url);
                            }
                            Ok(urls)
                        }
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
        product_urls: &crate::new_architecture::channels::types::ProductUrls,
        product_detail_collector: Arc<dyn ProductDetailCollector>,
    ) -> Result<Vec<crate::domain::product::ProductDetail>, String> {
    debug!("Processing {} product URLs for detail crawling", product_urls.urls.len());
        
        // ProductUrls 구조체에서 ProductUrl 객체들을 직접 사용
        match product_detail_collector.collect_details(&product_urls.urls).await {
            Ok(details) => {
                info!("✅ Real product detail processing successful: {} details collected", details.len());
                
                // 수집된 ProductDetail들을 로그로 확인
                for (index, detail) in details.iter().enumerate() {
                    debug!("  📄 Collected detail {}: {} (page_id: {:?}, index: {:?})", 
                           index + 1, detail.url, detail.page_id, detail.index_in_page);
                }
                
                Ok(details)
            }
            Err(e) => {
                warn!("❌ Real product detail processing failed: {}", e);
                Err(format!("Product detail processing failed: {}", e))
            }
        }
    }
    
    /// 실제 데이터 검증 처리 (현재 외부에서 직접 호출하지 않아 dead_code 경고 발생 가능)
    #[allow(dead_code)]
    async fn execute_real_data_validation(item: &StageItem) -> Result<(), String> {
        match item {
            StageItem::ProductDetails(product_details) => {
                info!("🔍 Starting data validation for {} ProductDetails", product_details.products.len());
                
                // DataQualityAnalyzer 사용하여 실제 검증 수행
                use crate::new_architecture::services::data_quality_analyzer::DataQualityAnalyzer;
                let analyzer = DataQualityAnalyzer::new();
                
                match analyzer.validate_before_storage(&product_details.products).await {
                    Ok(validated_products) => {
                        info!("✅ Data quality validation completed: {} products validated", validated_products.len());
                        if validated_products.len() != product_details.products.len() {
                            warn!("⚠️  Data validation filtered out {} products", 
                                  product_details.products.len() - validated_products.len());
                        }
                        Ok(())
                    }
                    Err(e) => {
                        error!("❌ Data quality validation failed: {}", e);
                        Err(format!("Data validation failed: {}", e))
                    }
                }
            }
            StageItem::ValidatedProducts(products) => {
                info!("✅ ValidatedProducts already validated: {} products", products.products.len());
                Ok(())
            }
            _ => {
                warn!("⚠️  DataValidation received unexpected item type, skipping validation");
                Ok(())
            }
        }
    }
    
    /// 실제 데이터베이스 저장 처리
    async fn execute_real_database_storage(
        item: &StageItem,
        product_repo: Arc<IntegratedProductRepository>,
    ) -> Result<(u32,u32,u32), String> { // (inserted, updated, duplicates)
        match item {
            StageItem::ProductDetails(wrapper) => {
                info!("[PersistExec] handling ProductDetails count={} extraction_stats=attempted:{} success:{} failed:{}", wrapper.products.len(), wrapper.extraction_stats.attempted, wrapper.extraction_stats.successful, wrapper.extraction_stats.failed);
                let products = &wrapper.products;
                if products.is_empty() { return Ok((0,0,0)); }
                // Duplicate detection by URL
                let mut seen = std::collections::HashSet::new();
                let mut duplicates: Vec<String> = Vec::new();
                for d in products { if !seen.insert(d.url.clone()) { duplicates.push(d.url.clone()); } }
                if !duplicates.is_empty() { warn!("[PersistExec] duplicate urls detected count={} urls={:?}", duplicates.len(), duplicates); }
                let mut inserted = 0u32; let mut updated = 0u32; let mut duplicates_ct = 0u32;
                for (idx, detail) in products.iter().enumerate() {
                    let start = std::time::Instant::now();
                    debug!("[PersistExec] upsert detail idx={} url={} page_id={:?}", idx, detail.url, detail.page_id);
                    match product_repo.create_or_update_product_detail(&detail).await {
                        Ok((was_updated, was_created)) => {
                            if was_created { inserted += 1; }
                            if was_updated { updated += 1; }
                            if !was_created && !was_updated { duplicates_ct += 1; }
                            debug!("[PersistExecDetail] idx={} url={} created={} updated={} elapsed_ms={}", idx, detail.url, was_created, was_updated, start.elapsed().as_millis());
                        },
                        Err(e) => return Err(format!("Database save failed: {}", e))
                    }
                }
                Ok((inserted, updated, duplicates_ct))
            }
            StageItem::ValidatedProducts(wrapper) => {
                info!("[PersistExec] handling ValidatedProducts count={}", wrapper.products.len());
                let products = &wrapper.products;
                if products.is_empty() { return Ok((0,0,0)); }
                let mut seen = std::collections::HashSet::new();
                let mut duplicates: Vec<String> = Vec::new();
                for d in products { if !seen.insert(d.url.clone()) { duplicates.push(d.url.clone()); } }
                if !duplicates.is_empty() { warn!("[PersistExec] duplicate validated urls detected count={} urls={:?}", duplicates.len(), duplicates); }
                let mut inserted = 0u32; let mut updated = 0u32; let mut duplicates_ct = 0u32;
                for (idx, detail) in products.iter().enumerate() {
                    let start = std::time::Instant::now();
                    debug!("[PersistExec] upsert validated detail idx={} url={} page_id={:?}", idx, detail.url, detail.page_id);
                    match product_repo.create_or_update_product_detail(&detail).await {
                        Ok((was_updated, was_created)) => {
                            if was_created { inserted += 1; }
                            if was_updated { updated += 1; }
                            if !was_created && !was_updated { duplicates_ct += 1; }
                            debug!("[PersistExecDetail] validated idx={} url={} created={} updated={} elapsed_ms={}", idx, detail.url, was_created, was_updated, start.elapsed().as_millis());
                        },
                        Err(e) => return Err(format!("Database save failed: {}", e))
                    }
                }
                Ok((inserted, updated, duplicates_ct))
            }
            _ => Ok((0,0,0))
        }
    }
    
    // === 시뮬레이션 함수들 (기존) ===
    
    /// 리스트 페이지 처리 시뮬레이션 (Phase 3 임시)
    #[allow(dead_code)]
    async fn simulate_list_page_processing(item: &StageItem) -> Result<(), String> {
        // 임시: 간단한 처리 시뮬레이션
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // 90% 성공률 시뮬레이션 - 간단한 방법 사용
        let success = match item {
            StageItem::Page(_) => true,
            StageItem::Url(_) => true,
            StageItem::Product(_) => true,
            StageItem::ValidationTarget(_) => true,
            StageItem::ProductList(_) => true, // 대부분 성공으로 가정
            StageItem::ProductUrls(_) => true,
            StageItem::ProductDetails(_) => true,
            StageItem::ValidatedProducts(_) => true,
        };
        
        if success {
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

// (기존 상세 Actor trait 구현은 파일 하단 원본 영역 유지)

#[async_trait::async_trait]
impl Actor for StageActor {
    type Command = ActorCommand;
    type Error = ActorError;

    fn actor_id(&self) -> &str {
        self.stage_id.as_deref().unwrap_or("unknown")
    }

    fn actor_type(&self) -> ActorType {
        ActorType::Stage
    }

    async fn run(
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
                                    items: _, // TODO: 적절한 타입 변환 필요
                                    concurrency_limit, 
                                    timeout_secs 
                                } => {
                                    // 임시: 빈 벡터로 처리하여 컴파일 에러 해결
                                    let empty_items = Vec::new();
                                    if let Err(e) = self.handle_execute_stage(
                                        stage_type, 
                                        empty_items, 
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

    /// 데이터 품질 분석 실행
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

impl StageActor {
    /// 실제 URL에서 ProductDetail을 추출하는 헬퍼 함수
    /// ServiceBasedBatchCrawlingEngine의 로직을 참조하여 구현
    /// 실제 HTTP 요청으로 제품 상세 정보 추출
    /// DataValidation 스테이지에서 ProductUrls -> ProductDetails 변환에 사용
    #[allow(dead_code)]
    async fn extract_product_detail_from_url(&self, url: &str) -> Result<crate::domain::product::ProductDetail, ActorError> {
        // HTTP 클라이언트 확인
        let http_client = self.http_client.as_ref()
            .ok_or_else(|| ActorError::RequestFailed("HTTP client not available".to_string()))?;
            
        // HTTP 클라이언트로 URL에서 HTML 가져오기
        let response = http_client.fetch_response(url).await
            .map_err(|e| ActorError::RequestFailed(format!("HTTP request failed: {}", e)))?;
        
        let html_content = response.text().await
            .map_err(|e| ActorError::ParsingFailed(format!("Failed to get response text: {}", e)))?;

        if html_content.trim().is_empty() {
            return Err(ActorError::ParsingFailed(format!("Empty HTML content from {}", url)));
        }

        // 데이터 추출기 확인
        let data_extractor = self.data_extractor.as_ref()
            .ok_or_else(|| ActorError::ParsingFailed("Data extractor not available".to_string()))?;
            
        // 데이터 추출기로 HTML 파싱
        let product_data_json = data_extractor.extract_product_data(&html_content)
            .map_err(|e| ActorError::ParsingFailed(format!("Failed to extract product data: {}", e)))?;

        // JSON에서 필드들을 안전하게 추출
        let manufacturer = product_data_json.get("manufacturer")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let model = product_data_json.get("model")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let certificate_id = product_data_json.get("certificate_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let pid = product_data_json.get("pid")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<i32>().ok());

        // ProductDetail 구조체 생성
        use crate::domain::product::ProductDetail;
        Ok(ProductDetail {
            url: url.to_string(),
            page_id: None,
            index_in_page: None,
            id: None,
            manufacturer,
            model,
            device_type: None,
            certificate_id: certificate_id,
            certification_date: None,
            software_version: None,
            hardware_version: None,
            firmware_version: None,
            specification_version: None,
            vid: None,
            pid,
            family_sku: None,
            family_variant_sku: None,
            family_id: None,
            tis_trp_tested: None,
            transport_interface: None,
            primary_device_type_id: None,
            application_categories: None,
            description: None,
            compliance_document_url: None,
            program_type: Some("Matter".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
    }
}
