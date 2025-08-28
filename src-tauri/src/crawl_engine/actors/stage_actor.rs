//! StageActor: 개별 스테이지 작업 처리 Actor
//!
//! Phase 3: Actor 구현 - 스테이지 레벨 작업 실행 및 관리
//! Modern Rust 2024 준수: 함수형 원칙, 명시적 의존성, 상태 최소화

use chrono::Utc;
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::crawl_engine::actors::traits::{Actor, ActorHealth, ActorStatus, ActorType};
use crate::crawl_engine::actors::types::{
    ActorCommand, ActorError, AppEvent, SimpleMetrics, StageError, StageItemResult, StageItemType,
    StageResult, StageType,
};
use crate::crawl_engine::channels::types::StageItem;
use crate::crawl_engine::integrated_context::AppContext;
use crate::crawl_engine::stages::DefaultStageLogicFactory;
use crate::crawl_engine::stages::traits::StageLogicFactory;
use crate::domain::services::SiteStatus;
use crate::domain::services::{ProductDetailCollector, ProductListCollector, StatusChecker};
use crate::infrastructure::config::AppConfig;
use crate::infrastructure::crawling_service_impls::{
    CollectorConfig, ProductDetailCollectorImpl, ProductListCollectorImpl, StatusCheckerImpl,
};
use crate::infrastructure::{HttpClient, IntegratedProductRepository, MatterDataExtractor};

/// Dependency bundle for StageActor (to move construction out of the actor)
#[derive(Clone)]
pub struct StageDeps {
    pub http_client: Arc<HttpClient>,
    pub data_extractor: Arc<MatterDataExtractor>,
    pub product_repo: Arc<IntegratedProductRepository>,
    pub status_checker: Arc<dyn StatusChecker>,
    pub product_list_collector: Arc<dyn ProductListCollector>,
    pub product_detail_collector: Arc<dyn ProductDetailCollector>,
    pub app_config: AppConfig,
    /// 중복 URL 저장 정책 (수동 실행 등에서 제어)
    pub duplicate_policy: crate::crawl_engine::actors::types::DuplicatePersistencePolicy,
}

// Duplicate-execution guard for DataSaving stage (session+batch scoped)
static DATA_SAVING_RUN_GUARD: Lazy<StdMutex<HashSet<String>>> =
    Lazy::new(|| StdMutex::new(HashSet::new()));

/// 스테이지 상태 열거형 (local to StageActor)
#[derive(Debug, Clone, PartialEq)]
enum StageState {
    Idle,
    Starting,
    Processing,
    Completed,
    Failed { error: String },
    Timeout,
}

/// StageActor: 개별 스테이지 작업의 실행 및 관리
#[allow(clippy::struct_excessive_bools)]
pub struct StageActor {
    // 기본 메타데이터
    actor_id: String,
    pub batch_id: String,
    stage_id: Option<String>,
    stage_type: Option<StageType>,
    state: StageState,
    start_time: Option<Instant>,

    // 진행 카운터
    total_items: u32,
    completed_items: u32,
    success_count: u32,
    failure_count: u32,
    skipped_count: u32,
    item_results: Vec<StageItemResult>,

    // 실제 크롤링 엔진 의존성
    status_checker: Option<Arc<dyn StatusChecker>>, // 사이트 상태 체크
    product_list_collector: Option<Arc<dyn ProductListCollector>>, // 리스트 페이지 수집기
    product_detail_collector: Option<Arc<dyn ProductDetailCollector>>, // 상세 페이지 수집기
    _product_repo: Option<Arc<IntegratedProductRepository>>, // 저장소 (옵션)
    http_client: Option<Arc<HttpClient>>,           // HTTP 클라이언트
    data_extractor: Option<Arc<MatterDataExtractor>>, // HTML 파서
    app_config: Option<AppConfig>,                  // 앱 설정

    // 상위에서 주입되는 페이지네이션 힌트
    site_total_pages_hint: Option<u32>,
    products_on_last_page_hint: Option<u32>,

    // 전략 분기 (Phase 3)
    strategy_factory: Arc<dyn StageLogicFactory + Send + Sync>,
    duplicate_policy: crate::crawl_engine::actors::types::DuplicatePersistencePolicy,
}

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
            StageItem::Url(_u) => StageItemType::Url {
                url_type: "generic".into(),
            },
            StageItem::Product(_p) => StageItemType::Url {
                url_type: "product".into(),
            },
            StageItem::ProductList(_l) => StageItemType::ProductUrls { urls: vec![] },
            StageItem::ProductUrls(list) => StageItemType::ProductUrls {
                urls: list.urls.iter().map(|u| u.url.clone()).collect(),
            },
            StageItem::ProductDetails(_d) => StageItemType::Url {
                url_type: "product_details".into(),
            },
            StageItem::ValidatedProducts(_v) => StageItemType::Url {
                url_type: "validated_products".into(),
            },
            StageItem::ValidationTarget(_t) => StageItemType::Url {
                url_type: "validation_target".into(),
            },
        }
    }
}

#[allow(dead_code)]
impl StageActor {
    /// 공통 재시도 래퍼 (Exponential Backoff + Jitter) with telemetry
    async fn retry_with_backoff<T, Fut, Op>(
        &self,
        context: &AppContext,
        stage_type: StageType,
        start_attempt: u32,
        max_attempts: u32,
        base_delay_ms: u64,
        max_delay_ms: u64,
        op: Op,
    ) -> (Result<T, String>, u32)
    where
        Op: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, String>>,
    {
        let mut attempt = start_attempt;
        loop {
            if context.is_cancelled() {
                return (Err("Operation cancelled".to_string()), attempt);
            }

            match op().await {
                Ok(val) => return (Ok(val), attempt),
                Err(err) => {
                    if attempt < max_attempts {
                        let next = attempt + 1;
                        let _ = context.emit_event(AppEvent::StageRetrying {
                            stage_type: stage_type.clone(),
                            session_id: context.session_id.clone(),
                            batch_id: Some(self.batch_id.clone()),
                            attempt: next,
                            max_attempts,
                            reason: Some(err.clone()),
                            timestamp: Utc::now(),
                        });

                        // exponential backoff: base * 2^(next-1)
                        let factor = 1u64.checked_shl(next.saturating_sub(1)).unwrap_or(u64::MAX);
                        let exp = base_delay_ms.saturating_mul(factor);
                        let capped = std::cmp::min(exp, max_delay_ms);
                        let jitter = if capped >= 10 {
                            fastrand::u64(0..=(capped / 5))
                        } else {
                            0
                        };
                        let delay = capped.saturating_add(jitter);
                        warn!(
                            "🔁 {:?} attempt {}/{} after {}ms (reason: {})",
                            stage_type, next, max_attempts, delay, err
                        );
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                        attempt = next;
                        continue;
                    } else {
                        return (Err(err), attempt);
                    }
                }
            }
        }
    }
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
            strategy_factory: Arc::new(DefaultStageLogicFactory),
            duplicate_policy: crate::crawl_engine::actors::types::DuplicatePersistencePolicy::Skip,
        }
    }

    /// New constructor that takes explicit dependencies and a strategy factory.
    /// This supports proper DI and makes StageActor focused on orchestration only.
    #[must_use]
    pub fn new_with_deps(
        actor_id: String,
        batch_id: String,
        deps: StageDeps,
        strategy_factory: Arc<dyn StageLogicFactory + Send + Sync>,
    ) -> Self {
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
            status_checker: Some(deps.status_checker),
            product_list_collector: Some(deps.product_list_collector),
            product_detail_collector: Some(deps.product_detail_collector),
            _product_repo: Some(deps.product_repo),
            http_client: Some(deps.http_client),
            data_extractor: Some(deps.data_extractor),
            app_config: Some(deps.app_config),
            // 정책은 이후 execute_real_database_storage 에서 사용하기 위해 필요 시 전파
            site_total_pages_hint: None,
            products_on_last_page_hint: None,
            strategy_factory,
            duplicate_policy: deps.duplicate_policy,
        }
    }

    /// Inject a StageLogic factory (strategy dispatch). Safe no-op if not provided.
    pub fn with_strategy_factory(
        mut self,
        factory: Arc<dyn StageLogicFactory + Send + Sync>,
    ) -> Self {
        self.strategy_factory = factory;
        self
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
    #[must_use]
    #[deprecated(note = "Use StageActor::new_with_deps with StageDeps for proper DI")]
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
        let status_checker: Option<Arc<dyn StatusChecker>> =
            Some(Arc::new(StatusCheckerImpl::with_product_repo(
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

        let product_list_collector: Option<Arc<dyn ProductListCollector>> =
            Some(Arc::new(ProductListCollectorImpl::new(
                Arc::new(http_client_inner.clone()),
                Arc::new(data_extractor_inner.clone()),
                list_collector_config.clone(),
                status_checker_for_list,
            )));

        // ProductDetailCollector 생성
        let detail_collector_config = CollectorConfig {
            max_concurrent: app_config
                .user
                .crawling
                .workers
                .product_detail_max_concurrent as u32,
            concurrency: app_config
                .user
                .crawling
                .workers
                .product_detail_max_concurrent as u32,
            delay_between_requests: Duration::from_millis(app_config.user.request_delay_ms),
            delay_ms: app_config.user.request_delay_ms,
            batch_size: app_config.user.batch.batch_size,
            retry_attempts: app_config.user.crawling.workers.max_retries,
            retry_max: app_config.user.crawling.workers.max_retries,
        };

        let product_detail_collector: Option<Arc<dyn ProductDetailCollector>> =
            Some(Arc::new(ProductDetailCollectorImpl::new(
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
            strategy_factory: Arc::new(DefaultStageLogicFactory),
            duplicate_policy: crate::crawl_engine::actors::types::DuplicatePersistencePolicy::Skip,
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
        _config: Arc<crate::crawl_engine::config::SystemConfig>,
        _total_pages: u32,
        _products_on_last_page: u32,
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
            strategy_factory: Arc::new(DefaultStageLogicFactory),
            duplicate_policy: crate::crawl_engine::actors::types::DuplicatePersistencePolicy::Skip,
        }
    }

    /// 사이트 페이지네이션 힌트 설정 (StatusCheck 결과를 상위에서 주입)
    pub fn set_site_pagination_hints(&mut self, total_pages: u32, products_on_last_page: u32) {
        self.site_total_pages_hint = Some(total_pages);
        self.products_on_last_page_hint = Some(products_on_last_page);
        info!(
            "🔧 Applied site pagination hints: total_pages={}, products_on_last_page={}",
            total_pages, products_on_last_page
        );
    }

    /// 실제 서비스 초기화 - guide/re-arch-plan-final2.md 설계 기반
    /// ServiceBasedBatchCrawlingEngine 패턴 참조하되 Actor 모델에 맞게 구현
    #[deprecated(
        note = "Construct services outside and pass via StageDeps; keep for transitional compatibility"
    )]
    pub async fn initialize_real_services(
        &mut self,
        _context: &AppContext,
    ) -> Result<(), StageError> {
        info!(
            "🎯 [ACTOR] Initializing real services for StageActor: {}",
            self.actor_id
        );

        // AppConfig 로드 (설정 파일에서)
        let app_config = crate::infrastructure::config::AppConfig::default();

        // HTTP Client 생성 (ServiceBasedBatchCrawlingEngine과 동일한 방식)
        let http_client =
            app_config
                .create_http_client()
                .map_err(|e| StageError::GenericError {
                    message: format!("Failed to create HTTP client: {}", e),
                })?;

        // 데이터 추출기 생성
        let data_extractor = MatterDataExtractor::new().map_err(|e| StageError::GenericError {
            message: format!("Failed to create data extractor: {}", e),
        })?;

        // 데이터베이스 연결 생성 (기본 경로 사용)
        let pool = crate::infrastructure::database_connection::get_or_init_global_pool()
            .await
            .map_err(|e| StageError::GenericError {
                message: format!("Failed to obtain database pool: {}", e),
            })?;
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
            max_concurrent: app_config
                .user
                .crawling
                .workers
                .product_detail_max_concurrent as u32,
            concurrency: app_config
                .user
                .crawling
                .workers
                .product_detail_max_concurrent as u32,
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

        info!(
            "✅ [ACTOR] Real services initialized successfully for StageActor: {}",
            self.actor_id
        );
        Ok(())
    }

    /// 크롤링 엔진 초기화 (임시 구현)
    /// 현재는 시뮬레이션 모드이므로 실제 엔진 초기화는 건너뛰기
    pub fn initialize_default_engines(&mut self) -> Result<(), StageError> {
        // No-op in production. Historical simulation path kept for tests/benchmarks via feature.
        #[cfg(feature = "simulate-details")]
        info!(
            "🔧 StageActor {} initialized (simulate-details enabled)",
            self.actor_id
        );
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
        self.handle_execute_stage(stage_type, items, concurrency_limit, timeout_secs, context)
            .await?;

        Ok(StageResult {
            processed_items: self.completed_items,
            successful_items: self.success_count,
            failed_items: self.failure_count,
            duration_ms: self
                .start_time
                .map(|start| start.elapsed().as_millis() as u64)
                .unwrap_or(0),
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
            let sid = self
                .stage_id
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            return Err(StageError::GenericError {
                message: format!("Stage already processing: {}", sid),
            });
        }

        let stage_id = Uuid::new_v4().to_string();

        info!(
            "🎯 StageActor {} executing stage {:?} with {} items",
            self.actor_id,
            stage_type,
            items.len()
        );

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

        context
            .emit_event(start_event)
            .map_err(|e| StageError::GenericError {
                message: e.to_string(),
            })?;

        // 상태를 Processing으로 전환
        self.state = StageState::Processing;

        // 내부 타임아웃/취소 지원이 포함된 처리 실행 (tasks abort 포함)
        let processing_result = self
            .process_stage_items(
                stage_type.clone(),
                items,
                concurrency_limit,
                context,
                Duration::from_secs(timeout_secs),
            )
            .await;
        // TODO: Introduce a generic retry wrapper for stage-level retries and emit AppEvent::StageRetrying accordingly.

        match processing_result {
            Ok(stage_result) => {
                self.state = StageState::Completed;
                let completion_event = AppEvent::StageCompleted {
                    stage_type: stage_type.clone(),
                    session_id: context.session_id.clone(),
                    batch_id: Some(self.batch_id.clone()),
                    result: stage_result,
                    timestamp: Utc::now(),
                };
                context
                    .emit_event(completion_event)
                    .map_err(|e| StageError::GenericError {
                        message: e.to_string(),
                    })?;
                info!(
                    "✅ Stage {:?} completed successfully: {}/{} items processed",
                    stage_type, self.success_count, self.total_items
                );
                Ok(())
            }
            Err(StageError::TimeoutError { .. }) => {
                self.state = StageState::Timeout;
                let error = StageError::TimeoutError {
                    timeout_ms: timeout_secs * 1000,
                };
                let timeout_event = AppEvent::StageFailed {
                    stage_type: stage_type.clone(),
                    session_id: context.session_id.clone(),
                    batch_id: Some(self.batch_id.clone()),
                    error: format!("{:?}", error),
                    timestamp: Utc::now(),
                };
                context
                    .emit_event(timeout_event)
                    .map_err(|e| StageError::GenericError {
                        message: e.to_string(),
                    })?;
                Err(error)
            }
            Err(e) => {
                let error_msg = format!("{:?}", e);
                self.state = StageState::Failed {
                    error: error_msg.clone(),
                };
                let failure_event = AppEvent::StageFailed {
                    stage_type: stage_type.clone(),
                    session_id: context.session_id.clone(),
                    batch_id: Some(self.batch_id.clone()),
                    error: error_msg,
                    timestamp: Utc::now(),
                };
                context
                    .emit_event(failure_event)
                    .map_err(|er| StageError::GenericError {
                        message: er.to_string(),
                    })?;
                Err(e)
            }
        }
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
        overall_timeout: Duration,
    ) -> Result<StageResult, StageError> {
        debug!(
            "Processing {} items for stage {:?}",
            items.len(),
            stage_type
        );

        // 전략 필수 의존성 확인
        let (app_config_clone, http_client, data_extractor, product_repo) = match (
            self.app_config.clone(),
            self.http_client.clone(),
            self.data_extractor.clone(),
            self._product_repo.clone(),
        ) {
            (Some(cfg), Some(http), Some(extractor), Some(repo)) => (cfg, http, extractor, repo),
            _ => {
                return Err(StageError::GenericError {
                    message: "Missing required dependencies for strategy execution".into(),
                });
            }
        };

        // 동시성 제어를 위한 세마포어
        let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency_limit as usize));
        // 전략/설정 및 의존성 복사
        let strategy_factory_clone = self.strategy_factory.clone();
        // 페이지네이션 힌트 복사 (Copy types; safe to move into tasks)
        let site_total_pages_hint = self.site_total_pages_hint;
        let products_on_last_page_hint = self.products_on_last_page_hint;
        // Duplicate policy 사전 클론 (self를 태스크 내부에서 캡처하지 않기 위해)
        let duplicate_policy_base = self.duplicate_policy.clone();

        // 각 아이템을 병렬로 처리 (StageItemStarted를 먼저 emit하여 이벤트 순서 보장)
        let deadline = Instant::now() + overall_timeout;
        // join 전에 추후 abort 대상 추적을 위해 task handle 저장
        let mut handles: Vec<tokio::task::JoinHandle<Result<StageItemResult, StageError>>> =
            Vec::new();
        let batch_id_owned = self.batch_id.clone();
        for item in items {
            let sem = semaphore.clone();
            let base_item = item.clone(); // used for lifecycle pre-emits
            let stage_type_clone = stage_type.clone();
            let product_repo_clone = product_repo.clone();
            let http_client_clone = http_client.clone();
            let data_extractor_clone = data_extractor.clone();
            let session_id_clone = _context.session_id.clone();
            let batch_id_opt = Some(batch_id_owned.clone());
            let ctx_clone = _context.clone();
            // Per-iteration clones for moved values into async block
            let app_config_iter = app_config_clone.clone();
            let strategy_factory_iter = strategy_factory_clone.clone();
            let duplicate_policy = duplicate_policy_base.clone();
            // Copy pagination hints into the task scope (avoid referencing self)
            let tp_hint = site_total_pages_hint;
            let plp_hint = products_on_last_page_hint;
            let task = tokio::spawn(async move {
                // Separate handle for persistence path to avoid moved value issues
                let product_repo_for_persist = product_repo_clone.clone();
                let _permit = sem.acquire().await.map_err(|e| StageError::GenericError {
                    message: format!("Semaphore error: {}", e),
                })?;
                if let Err(e) = ctx_clone.emit_event(AppEvent::StageItemStarted {
                    session_id: session_id_clone.clone(),
                    batch_id: batch_id_opt.clone(),
                    stage_type: stage_type_clone.clone(),
                    item_id: base_item.id_string(),
                    item_type: base_item.item_type_enum(),
                    timestamp: Utc::now(),
                }) {
                    tracing::error!("StageItemStarted emit failed: {}", e);
                }

                // Lifecycle coarse events AFTER StageItemStarted to preserve ordering
                // Define a local alias used by subsequent lifecycle handling blocks
                let lifecycle_item = base_item.clone();
                match (&stage_type_clone, &base_item) {
                    (StageType::ListPageCrawling, StageItem::Page(pn)) => {
                        if let Err(e) = ctx_clone.emit_event(AppEvent::PageLifecycle {
                            session_id: session_id_clone.clone(),
                            batch_id: batch_id_opt.clone(),
                            page_number: *pn,
                            status: "fetch_started".into(),
                            metrics: None,
                            timestamp: Utc::now(),
                        }) {
                            error!(
                                "PageLifecycle fetch_started emit failed page={} err={}",
                                pn, e
                            );
                        } else {
                            debug!("Emitted PageLifecycle fetch_started page={}", pn);
                        }
                    }
                    (StageType::ProductDetailCrawling, StageItem::ProductUrls(urls_wrapper)) => {
                        // Estimate scheduled_details using DB state (best-effort) and emit mapping event once
                        let repo = &product_repo_clone;
                        let mut ct = 0u32;
                        for u in &urls_wrapper.urls {
                            match repo.get_product_detail_by_url(&u.url).await {
                                Ok(existing) => {
                                    if existing.is_none() {
                                        ct += 1;
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "[DetailFilter] DB check failed for url={} err={}",
                                        u.url,
                                        e
                                    );
                                    ct += 1;
                                }
                            }
                        }
                        let scheduled = ct;
                        if let Some(first) = urls_wrapper.urls.first() {
                            let page_hint = first.page_id as u32;
                            let _ = ctx_clone.emit_event(AppEvent::PageLifecycle {
                                session_id: session_id_clone.clone(),
                                batch_id: batch_id_opt.clone(),
                                page_number: page_hint,
                                status: "detail_mapping_emitted".into(),
                                metrics: Some(SimpleMetrics::Page {
                                    url_count: Some(urls_wrapper.urls.len() as u32),
                                    scheduled_details: Some(scheduled),
                                    error: None,
                                }),
                                timestamp: Utc::now(),
                            });
                        }
                    }
                    _ => {}
                }

                let item_start = Instant::now();

                // Strategy-only execution
                let factory = strategy_factory_iter;
                let result = if let Some(logic) = factory.logic_for(&stage_type_clone) {
                    let deps = crate::crawl_engine::stages::traits::Deps {
                        http: http_client_clone.clone(),
                        extractor: data_extractor_clone.clone(),
                        repo: product_repo_clone.clone(),
                        duplicate_policy: duplicate_policy.clone(),
                    };
                    let input = crate::crawl_engine::stages::traits::StageInput {
                        stage_type: stage_type_clone.clone(),
                        item: base_item.clone(),
                        config: app_config_iter.clone(),
                        deps,
                        total_pages_hint: tp_hint,
                        products_on_last_page_hint: plp_hint,
                    };
                    match logic.execute(input).await {
                        Ok(crate::crawl_engine::stages::traits::StageOutput { result }) => {
                            Ok(result)
                        }
                        Err(e) => Err(StageError::GenericError {
                            message: format!("Strategy error: {}", e),
                        }),
                    }
                } else {
                    Err(StageError::GenericError {
                        message: format!("No strategy registered for stage {:?}", stage_type_clone),
                    })
                };
                match &result {
                    Ok(r) => {
                        // Emit Validation events in aggregate for DataValidation stage
                        if matches!(stage_type_clone, StageType::DataValidation) {
                            // Attempt to decode collected data to count items
                            let (products_found, products_checked, divergences, anomalies) =
                                (|| {
                                    if let Some(json) = &r.collected_data {
                                        // collected_data for DataValidation is serialized validated products Vec<ProductDetail>
                                        let parsed: Result<
                                            Vec<crate::domain::product::ProductDetail>,
                                            _,
                                        > = serde_json::from_str(json);
                                        if let Ok(validated) = parsed {
                                            let found = validated.len() as u32;
                                            // Derive anomalies/divergences from DataQualityReport
                                            let report = crate::crawl_engine::services::data_quality_analyzer::DataQualityAnalyzer::new()
                                            .analyze_product_quality(&validated)
                                            .ok();
                                            let (div_ct, anom_ct) = if let Some(rep) = report {
                                                let dup = rep
                                                .issues
                                                .iter()
                                                .filter(|i| matches!(i.issue_type, crate::crawl_engine::services::data_quality_analyzer::IssueType::Duplicate))
                                                .count() as u32;
                                                let anom = rep
                                                .issues
                                                .iter()
                                                .filter(|i| matches!(i.severity, crate::crawl_engine::services::data_quality_analyzer::IssueSeverity::Critical | crate::crawl_engine::services::data_quality_analyzer::IssueSeverity::Warning))
                                                .count() as u32;
                                                (dup, anom)
                                            } else {
                                                (0u32, 0u32)
                                            };
                                            return (found, found as u64, div_ct, anom_ct);
                                        }
                                    }
                                    (0u32, 0u64, 0u32, 0u32)
                                })();
                            let _ = ctx_clone.emit_event(AppEvent::ValidationStarted {
                                session_id: session_id_clone.clone(),
                                scan_pages: 1,
                                total_pages_site: None,
                                timestamp: Utc::now(),
                            });
                            let _ = ctx_clone.emit_event(AppEvent::ValidationPageScanned {
                                session_id: session_id_clone.clone(),
                                physical_page: 0,
                                products_found,
                                assigned_start_offset: 0,
                                assigned_end_offset: products_found.saturating_sub(1) as u64,
                                timestamp: Utc::now(),
                            });
                            let _ = ctx_clone.emit_event(AppEvent::ValidationCompleted {
                                session_id: session_id_clone.clone(),
                                pages_scanned: 1,
                                products_checked,
                                divergences,
                                anomalies,
                                duration_ms: item_start.elapsed().as_millis() as u64,
                                timestamp: Utc::now(),
                            });
                            // Emit a few anomaly details to console if present
                            if let Some(json) = &r.collected_data {
                                if let Ok(validated) = serde_json::from_str::<
                                    Vec<crate::domain::product::ProductDetail>,
                                >(json)
                                {
                                    if let Ok(rep) = crate::crawl_engine::services::data_quality_analyzer::DataQualityAnalyzer::new().analyze_product_quality(&validated) {
                                        for issue in rep.issues.iter().take(3) {
                                            let code = match issue.issue_type {
                                                crate::crawl_engine::services::data_quality_analyzer::IssueType::Duplicate => "duplicate_index",
                                                crate::crawl_engine::services::data_quality_analyzer::IssueType::MissingRequired => "missing_required",
                                                crate::crawl_engine::services::data_quality_analyzer::IssueType::InvalidFormat => "invalid_format",
                                                crate::crawl_engine::services::data_quality_analyzer::IssueType::EmptyValue => "empty_value",
                                            };
                                            let detail = format!(
                                                "{} {} in '{}'",
                                                match issue.severity { crate::crawl_engine::services::data_quality_analyzer::IssueSeverity::Critical => "CRIT", crate::crawl_engine::services::data_quality_analyzer::IssueSeverity::Warning => "WARN", crate::crawl_engine::services::data_quality_analyzer::IssueSeverity::Info => "INFO" },
                                                match issue.issue_type { crate::crawl_engine::services::data_quality_analyzer::IssueType::MissingRequired => "Missing", crate::crawl_engine::services::data_quality_analyzer::IssueType::InvalidFormat => "Format", crate::crawl_engine::services::data_quality_analyzer::IssueType::EmptyValue => "Empty", crate::crawl_engine::services::data_quality_analyzer::IssueType::Duplicate => "Dup" },
                                                issue.field_name
                                            );
                                            let _ = ctx_clone.emit_event(AppEvent::ValidationAnomaly {
                                                session_id: session_id_clone.clone(),
                                                code: code.into(),
                                                detail,
                                                timestamp: Utc::now(),
                                            });
                                        }
                                        // Emit some divergence events (duplicates) for live counting
                                        let mut dup_emitted = 0u32;
                                        for issue in rep.issues.iter() {
                                            if let crate::crawl_engine::services::data_quality_analyzer::IssueType::Duplicate = issue.issue_type {
                                                let detail = format!("Duplicate in '{}' (url={})", issue.field_name, issue.product_url);
                                                let _ = ctx_clone.emit_event(AppEvent::ValidationDivergenceFound {
                                                    session_id: session_id_clone.clone(),
                                                    physical_page: 0,
                                                    kind: "duplicate".into(),
                                                    detail,
                                                    expected_offset: 0,
                                                    timestamp: Utc::now(),
                                                });
                                                dup_emitted += 1;
                                                if dup_emitted >= 5 { break; }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        // For grouped product detail crawling, emit a grouped completion event summarizing counts
                        if let StageType::ProductDetailCrawling = stage_type_clone {
                            if let StageItem::ProductUrls(ref urls) = lifecycle_item {
                                let page_hint =
                                    urls.urls.first().map(|u| u.page_id as u32).unwrap_or(0u32);
                                let total = urls.urls.len() as u32;
                                let duration_ms = item_start.elapsed().as_millis() as u64;
                                debug!(
                                    "[GroupedEmit] fetch_completed_group total_urls={} duration_ms={}",
                                    total, duration_ms
                                );
                                if let Err(e) =
                                    ctx_clone.emit_event(AppEvent::ProductLifecycleGroup {
                                        session_id: session_id_clone.clone(),
                                        batch_id: batch_id_opt.clone(),
                                        page_number: Some(page_hint),
                                        group_size: total,
                                        started: total,
                                        succeeded: total,
                                        failed: 0,
                                        duplicates: 0,
                                        duration_ms: duration_ms,
                                        phase: "fetch".into(),
                                        timestamp: Utc::now(),
                                    })
                                {
                                    error!(
                                        "ProductLifecycleGroup fetch_completed emit failed err={}",
                                        e
                                    );
                                }
                            }
                        }
                        // If this is DataSaving stage, perform persistence now (previous arm returned Ok(()))
                        // NOTE: Previous pattern used value patterns against references &StageType/&StageItem and never matched.
                        // We now explicitly match references to ensure the block executes.
                        if matches!(stage_type_clone, StageType::DataSaving) {
                            // DataSaving 단계에서는 ProductDetails 또는 ValidatedProducts 둘 다 저장 대상이 될 수 있음
                            let is_persist_target =
                                matches!(lifecycle_item, StageItem::ProductDetails(_))
                                    || matches!(lifecycle_item, StageItem::ValidatedProducts(_));
                            if is_persist_target {
                                info!(
                                    "[Persist] Enter DataSaving block batch_id={:?} variant={}",
                                    batch_id_opt,
                                    match &lifecycle_item {
                                        StageItem::ProductDetails(_) => "ProductDetails",
                                        StageItem::ValidatedProducts(_) => "ValidatedProducts",
                                        _ => "Other",
                                    }
                                );
                                // Duplicate guard
                                let guard_key = format!(
                                    "{}:{}:data_saving",
                                    session_id_clone,
                                    batch_id_opt.clone().unwrap_or_else(|| "none".into())
                                );
                                if let Ok(mut guard) =
                                    crate::crawl_engine::actors::stage_actor::DATA_SAVING_RUN_GUARD
                                        .lock()
                                {
                                    if guard.contains(&guard_key) {
                                        info!(
                                            "[PersistGuard] duplicate DataSaving suppression key={}",
                                            guard_key
                                        );
                                        return Ok(StageItemResult {
                                            item_id: "data_saving_guard".into(),
                                            item_type: StageItemType::Url {
                                                url_type: "data_saving".into(),
                                            },
                                            success: true,
                                            error: None,
                                            duration_ms: item_start.elapsed().as_millis() as u64,
                                            retry_count: 0,
                                            collected_data: None,
                                        });
                                    } else {
                                        guard.insert(guard_key);
                                        info!(
                                            "[PersistGuard] first DataSaving execution proceeding"
                                        );
                                    }
                                }
                                // decide skip via env inside task scope
                                let skip_save = std::env::var("MC_SKIP_DB_SAVE")
                                    .ok()
                                    .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                                    .unwrap_or(false);
                                if skip_save {
                                    info!("[Persist] Skipped by env MC_SKIP_DB_SAVE");
                                    if let Err(e) =
                                        ctx_clone.emit_event(AppEvent::ProductLifecycle {
                                            session_id: session_id_clone.clone(),
                                            batch_id: batch_id_opt.clone(),
                                            page_number: None,
                                            product_ref: "_batch_persist".into(),
                                            status: "persist_skipped".into(),
                                            retry: None,
                                            duration_ms: None,
                                            metrics: Some(SimpleMetrics::Generic {
                                                key: "reason".into(),
                                                value: "MC_SKIP_DB_SAVE".into(),
                                            }),
                                            timestamp: Utc::now(),
                                        })
                                    {
                                        error!(
                                            "ProductLifecycle persist_skipped emit failed err={}",
                                            e
                                        );
                                    }
                                } else {
                                    let attempted_count = match &lifecycle_item {
                                        StageItem::ValidatedProducts(v) => v.products.len() as u32,
                                        StageItem::ProductDetails(d) => d.products.len() as u32,
                                        _ => 0,
                                    };
                                    if let Err(e) =
                                        ctx_clone.emit_event(AppEvent::ProductLifecycle {
                                            session_id: session_id_clone.clone(),
                                            batch_id: batch_id_opt.clone(),
                                            page_number: None,
                                            product_ref: "_batch_persist".into(),
                                            status: "persist_started".into(),
                                            retry: None,
                                            duration_ms: None,
                                            metrics: Some(SimpleMetrics::Generic {
                                                key: "attempted_count".into(),
                                                value: attempted_count.to_string(),
                                            }),
                                            timestamp: Utc::now(),
                                        })
                                    {
                                        error!(
                                            "ProductLifecycle persist_started emit failed err={}",
                                            e
                                        );
                                    }
                                    let persist_start = Instant::now();
                                    if attempted_count == 0 {
                                        info!(
                                            "[Persist] Empty batch (attempted_count=0) -> emit persist_empty and skip storage call"
                                        );
                                        let _ = ctx_clone.emit_event(AppEvent::ProductLifecycle {
                                            session_id: session_id_clone.clone(),
                                            batch_id: batch_id_opt.clone(),
                                            page_number: None,
                                            product_ref: "_batch_persist".into(),
                                            status: "persist_empty".into(),
                                            retry: None,
                                            duration_ms: Some(0),
                                            metrics: Some(SimpleMetrics::Generic {
                                                key: "persist_result".into(),
                                                value: "attempted=0".into(),
                                            }),
                                            timestamp: Utc::now(),
                                        });
                                        return Ok(StageItemResult {
                                            item_id: "data_saving_empty".into(),
                                            item_type: StageItemType::Url {
                                                url_type: "data_saving".into(),
                                            },
                                            success: true,
                                            error: None,
                                            duration_ms: item_start.elapsed().as_millis() as u64,
                                            retry_count: 0,
                                            collected_data: None,
                                        });
                                    }
                                    {
                                        let repo = product_repo_for_persist.clone();
                                        info!(
                                            "[PersistExec] starting storage call variant={}",
                                            match &lifecycle_item {
                                                StageItem::ProductDetails(_) => "ProductDetails",
                                                StageItem::ValidatedProducts(_) =>
                                                    "ValidatedProducts",
                                                _ => "Other",
                                            }
                                        );
                                        match StageActor::execute_real_database_storage(
                                            &lifecycle_item,
                                            repo.clone(),
                                        )
                                        .await
                                        {
                                            Ok((inserted, updated, duplicates_ct)) => {
                                                // total = inserted + updated (실제 DB 변경된 row 수)
                                                let total_changed = inserted + updated;
                                                let attempted = attempted_count; // from outer scope (입력 product 개수)
                                                // unchanged = attempted - (inserted+updated+duplicates) 로 해석: duplicates 는 이미 동일 데이터 존재
                                                let consumed = inserted + updated + duplicates_ct;
                                                let unchanged = if attempted > consumed {
                                                    attempted - consumed
                                                } else {
                                                    0
                                                };
                                                let status = if inserted > 0 && updated == 0 {
                                                    "persist_inserted"
                                                } else if updated > 0 && inserted == 0 {
                                                    "persist_updated"
                                                } else if inserted == 0 && updated == 0 {
                                                    if duplicates_ct == attempted {
                                                        "persist_noop_all_duplicate"
                                                    } else {
                                                        "persist_noop"
                                                    }
                                                } else {
                                                    "persist_mixed"
                                                };
                                                // 일관성 검증
                                                let logical_sum =
                                                    inserted + updated + duplicates_ct + unchanged;
                                                if logical_sum != attempted {
                                                    warn!(
                                                        "[Persist] metric_inconsistency attempted={} != inserted+updated+duplicates+unchanged={} ({}+{}+{}+{})",
                                                        attempted,
                                                        logical_sum,
                                                        inserted,
                                                        updated,
                                                        duplicates_ct,
                                                        unchanged
                                                    );
                                                }
                                                info!(
                                                    "[Persist] Result status={} inserted={} updated={} duplicates={} changed={} unchanged={} attempted={} duration_ms={}",
                                                    status,
                                                    inserted,
                                                    updated,
                                                    duplicates_ct,
                                                    total_changed,
                                                    unchanged,
                                                    attempted,
                                                    persist_start.elapsed().as_millis()
                                                );
                                                let metrics = SimpleMetrics::Generic {
                                                    key: "persist_result".into(),
                                                    value: format!(
                                                        "attempted={},inserted={},updated={},duplicates={},changed={},unchanged={}",
                                                        attempted,
                                                        inserted,
                                                        updated,
                                                        duplicates_ct,
                                                        total_changed,
                                                        unchanged
                                                    ),
                                                };
                                                let emit_res = ctx_clone.emit_event(
                                                    AppEvent::ProductLifecycle {
                                                        session_id: session_id_clone.clone(),
                                                        batch_id: batch_id_opt.clone(),
                                                        page_number: None,
                                                        product_ref: "_batch_persist".into(),
                                                        status: status.into(),
                                                        retry: None,
                                                        duration_ms: Some(
                                                            persist_start.elapsed().as_millis()
                                                                as u64,
                                                        ),
                                                        metrics: Some(metrics),
                                                        timestamp: Utc::now(),
                                                    },
                                                );

                                                // Emit DatabaseStats event for Stage 4 UI visibility
                                                {
                                                    let repo = product_repo_for_persist.clone();
                                                    if let Ok((
                                                        total_count,
                                                        min_page,
                                                        max_page,
                                                        _,
                                                    )) = repo.get_product_detail_stats().await
                                                    {
                                                        let note = if inserted > 0 || updated > 0 {
                                                            Some(format!(
                                                                "Batch persisted: {} inserted, {} updated",
                                                                inserted, updated
                                                            ))
                                                        } else {
                                                            Some(
                                                                "Batch persisted: no changes"
                                                                    .into(),
                                                            )
                                                        };
                                                        let _ = ctx_clone.emit_event(
                                                            AppEvent::DatabaseStats {
                                                                session_id: session_id_clone
                                                                    .clone(),
                                                                batch_id: batch_id_opt.clone(),
                                                                total_product_details: total_count,
                                                                min_page,
                                                                max_page,
                                                                note,
                                                                timestamp: Utc::now(),
                                                            },
                                                        );
                                                        // Emit grouped persistence lifecycle snapshot for UI animation (Stage 5)
                                                        let _ = ctx_clone.emit_event(
                                                            AppEvent::ProductLifecycleGroup {
                                                                session_id: session_id_clone
                                                                    .clone(),
                                                                batch_id: batch_id_opt.clone(),
                                                                page_number: None,
                                                                group_size: attempted_count,
                                                                started: attempted_count,
                                                                succeeded: inserted + updated,
                                                                failed: (attempted_count
                                                                    .saturating_sub(
                                                                        inserted + updated,
                                                                    )),
                                                                duplicates: duplicates_ct,
                                                                duration_ms: persist_start
                                                                    .elapsed()
                                                                    .as_millis()
                                                                    as u64,
                                                                phase: "persist".into(),
                                                                timestamp: Utc::now(),
                                                            },
                                                        );
                                                    }
                                                }
                                                match emit_res {
                                                    Ok(_) => info!(
                                                        "[PersistEmit] lifecycle emitted status={}",
                                                        status
                                                    ),
                                                    Err(e) => error!(
                                                        "ProductLifecycle {} emit failed err={}",
                                                        status, e
                                                    ),
                                                }
                                                if status == "persist_noop" {
                                                    // emit anomaly diagnostic + (future) logical drift probe
                                                    {
                                                        let repo = product_repo_for_persist.clone();
                                                        if let Ok((cnt, minp, maxp, _)) =
                                                            repo.get_product_detail_stats().await
                                                        {
                                                            let attempted = attempted_count;
                                                            let _ = ctx_clone.emit_event(AppEvent::PersistenceAnomaly {
                                                                session_id: session_id_clone.clone(),
                                                                batch_id: batch_id_opt.clone(),
                                                                kind: "all_noop".into(),
                                                                detail: format!("All attempted saves were noop: attempted={} inserted={} updated={} db_total={} db_page_range={:?}-{:?}", attempted, inserted, updated, cnt, minp, maxp),
                                                                attempted,
                                                                inserted,
                                                                updated,
                                                                timestamp: Utc::now(),
                                                            });
                                                            // LogicalMappingDrift (placeholder): detect unexpected min/max inversion or large gap
                                                            if let (Some(min_p), Some(max_p)) =
                                                                (minp, maxp)
                                                            {
                                                                if min_p > max_p {
                                                                    // impossible condition -> drift
                                                                    let _ = ctx_clone.emit_event(AppEvent::PersistenceAnomaly {
                                                                        session_id: session_id_clone.clone(),
                                                                        batch_id: batch_id_opt.clone(),
                                                                        kind: "logical_mapping_drift".into(),
                                                                        detail: format!("Inverted page range detected min_page={} > max_page={} during noop persistence", min_p, max_p),
                                                                        attempted,
                                                                        inserted,
                                                                        updated,
                                                                        timestamp: Utc::now(),
                                                                    });
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                error!(
                                                    "[Persist] Failed error={} duration_ms={}",
                                                    e,
                                                    persist_start.elapsed().as_millis()
                                                );
                                                let emit_res = ctx_clone.emit_event(
                                                    AppEvent::ProductLifecycle {
                                                        session_id: session_id_clone.clone(),
                                                        batch_id: batch_id_opt.clone(),
                                                        page_number: None,
                                                        product_ref: "_batch_persist".into(),
                                                        status: "persist_failed".into(),
                                                        retry: None,
                                                        duration_ms: Some(
                                                            persist_start.elapsed().as_millis()
                                                                as u64,
                                                        ),
                                                        metrics: Some(SimpleMetrics::Generic {
                                                            key: "error".into(),
                                                            value: e.clone(),
                                                        }),
                                                        timestamp: Utc::now(),
                                                    },
                                                );
                                                if let Err(e2) = emit_res {
                                                    error!(
                                                        "ProductLifecycle persist_failed emit failed err={}",
                                                        e2
                                                    );
                                                } else {
                                                    info!(
                                                        "[PersistEmit] lifecycle emitted status=persist_failed"
                                                    );
                                                }
                                            }
                                        }
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
                                if d.starts_with('[') {
                                    d.matches("\"").count() as u32 / 2
                                } else {
                                    1
                                }
                            }),
                            timestamp: Utc::now(),
                        }) {
                            tracing::error!("StageItemCompleted emit failed: {}", e);
                        }
                        // Emit lifecycle completion for page or product aggregated result
                        if let (StageType::ListPageCrawling, StageItem::Page(pn)) =
                            (&stage_type_clone, &lifecycle_item)
                        {
                            let metrics = crate::crawl_engine::actors::types::SimpleMetrics::Page {
                                url_count: Some(
                                    r.collected_data
                                        .as_ref()
                                        .map(|d| d.len() as u32)
                                        .unwrap_or(0),
                                ),
                                scheduled_details: None,
                                error: None,
                            };
                            if let Err(e) = ctx_clone.emit_event(AppEvent::PageLifecycle {
                                session_id: session_id_clone.clone(),
                                batch_id: batch_id_opt.clone(),
                                page_number: *pn,
                                status: "fetch_completed".into(),
                                metrics: Some(metrics),
                                timestamp: Utc::now(),
                            }) {
                                error!(
                                    "PageLifecycle fetch_completed emit failed page={} err={}",
                                    pn, e
                                );
                            } else {
                                debug!("Emitted PageLifecycle fetch_completed page={}", pn);
                            }
                        }
                        // Product detail lifecycle completion (group success)
                        if let (StageType::ProductDetailCrawling, StageItem::ProductUrls(urls)) =
                            (&stage_type_clone, &lifecycle_item)
                        {
                            if let Err(e) = ctx_clone.emit_event(AppEvent::ProductLifecycle {
                                session_id: session_id_clone.clone(),
                                batch_id: batch_id_opt.clone(),
                                page_number: urls.urls.first().map(|u| u.page_id as u32),
                                product_ref: format!("_batch_urls_{}", urls.urls.len()),
                                status: "fetch_completed_group".into(),
                                retry: None,
                                duration_ms: Some(item_start.elapsed().as_millis() as u64),
                                metrics: Some(SimpleMetrics::Generic {
                                    key: "group_size".into(),
                                    value: urls.urls.len().to_string(),
                                }),
                                timestamp: Utc::now(),
                            }) {
                                error!(
                                    "ProductLifecycle fetch_completed_group emit failed err={}",
                                    e
                                );
                            }
                        }
                    }
                    Err(err) => {
                        if let Err(e) = ctx_clone.emit_event(AppEvent::StageItemCompleted {
                            session_id: session_id_clone.clone(),
                            batch_id: batch_id_opt.clone(),
                            stage_type: stage_type_clone.clone(),
                            item_id: "unknown".into(),
                            item_type: StageItemType::Url {
                                url_type: "unknown".into(),
                            },
                            success: false,
                            error: Some(format!("{:?}", err)),
                            duration_ms: item_start.elapsed().as_millis() as u64,
                            retry_count: 0,
                            collected_count: None,
                            timestamp: Utc::now(),
                        }) {
                            tracing::error!("StageItemCompleted emit failed: {}", e);
                        }
                        if let (StageType::ListPageCrawling, StageItem::Page(pn)) =
                            (&stage_type_clone, &lifecycle_item)
                        {
                            let metrics = crate::crawl_engine::actors::types::SimpleMetrics::Page {
                                url_count: None,
                                scheduled_details: None,
                                error: Some(format!("{:?}", err)),
                            };
                            if let Err(e2) = ctx_clone.emit_event(AppEvent::PageLifecycle {
                                session_id: session_id_clone.clone(),
                                batch_id: batch_id_opt.clone(),
                                page_number: *pn,
                                status: "failed".into(),
                                metrics: Some(metrics),
                                timestamp: Utc::now(),
                            }) {
                                error!("PageLifecycle failed emit failed page={} err={}", pn, e2);
                            } else {
                                debug!("Emitted PageLifecycle failed page={}", pn);
                            }
                        }
                        // Product detail lifecycle failure
                        if let (StageType::ProductDetailCrawling, StageItem::ProductUrls(urls)) =
                            (&stage_type_clone, &lifecycle_item)
                        {
                            for pu in &urls.urls {
                                let metrics =
                                    crate::crawl_engine::actors::types::SimpleMetrics::Product {
                                        fields: None,
                                        size_bytes: None,
                                        error: Some(format!("{:?}", err)),
                                    };
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
                                }) {
                                    error!(
                                        "ProductLifecycle failed emit failed product={} err={}",
                                        pu.url, e2
                                    );
                                }
                            }
                        }
                    }
                }
                result
            });
            handles.push(task);
        }

        // 모든 태스크 완료 대기 (전체 타임아웃 관리 및 잔여 task abort)
        let mut results = Vec::new();
        let mut timeout_triggered = false;
        while let Some(task) = handles.pop() {
            let now = Instant::now();
            if now >= deadline {
                timeout_triggered = true;
                // 남은 작업들 중단
                for h in handles.drain(..) {
                    h.abort();
                }
                break;
            }
            let remaining = deadline.saturating_duration_since(now);
            // 개별 task join에 대해 남은 전체 시간만 허용
            let join_res = tokio::time::timeout(remaining, task).await;
            let join_outcome = match join_res {
                Ok(j) => j,
                Err(_) => {
                    timeout_triggered = true;
                    break;
                }
            };
            match join_outcome {
                Ok(Ok(result)) => {
                    results.push(result);
                }
                Ok(Err(e)) => {
                    error!("Item processing failed: {:?}", e);
                    results.push(StageItemResult {
                        item_id: "unknown".to_string(),
                        item_type: StageItemType::Url {
                            url_type: "unknown".to_string(),
                        },
                        success: false,
                        error: Some(format!("{:?}", e)),
                        duration_ms: 0,
                        retry_count: 0,
                        collected_data: None,
                    });
                }
                Err(e) => {
                    error!("Task join error: {}", e);
                    results.push(StageItemResult {
                        item_id: "unknown".to_string(),
                        item_type: StageItemType::Url {
                            url_type: "unknown".to_string(),
                        },
                        success: false,
                        error: Some(format!("Task join error: {}", e)),
                        duration_ms: 0,
                        retry_count: 0,
                        collected_data: None,
                    });
                }
            }
        }

        // 타임아웃 이후 남아있는 handle abort
        if timeout_triggered {
            return Err(StageError::TimeoutError {
                timeout_ms: overall_timeout.as_millis() as u64,
            });
        }

        // 결과 집계
        self.item_results = results;
        self.completed_items = self.item_results.len() as u32;
        self.success_count = self.item_results.iter().filter(|r| r.success).count() as u32;
        self.failure_count = self.item_results.iter().filter(|r| !r.success).count() as u32;

        let duration = self
            .start_time
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

    // Legacy per-item path removed; strategy-only execution is enforced in process_stage_items.

    // === 실제 서비스 기반 처리 함수들 (Critical Issue #1) ===

    /// 실제 상태 확인 처리
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    async fn execute_real_list_page_processing(
        &self,
        item: &StageItem,
        product_list_collector: Arc<dyn ProductListCollector>,
    ) -> Result<Vec<crate::domain::product_url::ProductUrl>, String> {
        match item {
            StageItem::Page(page_number) => {
                // 실제 리스트 페이지 크롤링
                // 페이지네이션 힌트 사용, 없으면 필요 시 상태 재확인
                let (total_pages, products_on_last_page) = match (
                    self.site_total_pages_hint,
                    self.products_on_last_page_hint,
                ) {
                    (Some(tp), Some(plp)) => (tp, plp),
                    _ => {
                        if let Some(checker) = &self.status_checker {
                            // StageActor는 AppHandle에 접근하지 않으므로 여기서는 직접 체크만 수행
                            match checker.check_site_status().await {
                                Ok(s) => (s.total_pages, s.products_on_last_page),
                                Err(e) => {
                                    warn!(
                                        "⚠️ Failed to get site status for list processing, using conservative defaults: {}",
                                        e
                                    );
                                    (100u32, 10u32)
                                }
                            }
                        } else {
                            warn!(
                                "⚠️ No StatusChecker available; using conservative defaults for pagination"
                            );
                            (100u32, 10u32)
                        }
                    }
                };

                // 단일 페이지 수집 API를 사용하여 실패 시 에러를 그대로 전파
                match product_list_collector
                    .collect_single_page(*page_number, total_pages, products_on_last_page)
                    .await
                {
                    Ok(urls) => {
                        // 빈 결과는 실패로 간주하여 재시도를 유도
                        if urls.is_empty() {
                            warn!(
                                "⚠️ Page {} returned 0 URLs — treating as failure to trigger retry",
                                page_number
                            );
                            Err("Empty result from list page".to_string())
                        } else {
                            info!(
                                "✅ Real list page processing successful for page {}: {} URLs collected",
                                page_number,
                                urls.len()
                            );
                            for (index, url) in urls.iter().enumerate() {
                                debug!("  📄 Collected URL {}: {}", index + 1, url.url);
                            }
                            Ok(urls)
                        }
                    }
                    Err(e) => {
                        warn!(
                            "❌ Real list page processing failed for page {}: {}",
                            page_number, e
                        );
                        Err(format!("List page processing failed: {}", e))
                    }
                }
            }
            _ => Ok(vec![]), // 다른 타입은 빈 벡터 반환
        }
    }

    /// 실제 제품 상세 처리
    #[allow(dead_code)]
    async fn execute_real_product_detail_processing(
        product_urls: &crate::crawl_engine::channels::types::ProductUrls,
        product_detail_collector: Arc<dyn ProductDetailCollector>,
    ) -> Result<Vec<crate::domain::product::ProductDetail>, String> {
        debug!(
            "Processing {} product URLs for detail crawling",
            product_urls.urls.len()
        );

        // ProductUrls 구조체에서 ProductUrl 객체들을 직접 사용
        match product_detail_collector
            .collect_details(&product_urls.urls)
            .await
        {
            Ok(details) => {
                info!(
                    "✅ Real product detail processing successful: {} details collected",
                    details.len()
                );

                // 수집된 ProductDetail들을 로그로 확인
                for (index, detail) in details.iter().enumerate() {
                    debug!(
                        "  📄 Collected detail {}: {} (page_id: {:?}, index: {:?})",
                        index + 1,
                        detail.url,
                        detail.page_id,
                        detail.index_in_page
                    );
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
                info!(
                    "🔍 Starting data validation for {} ProductDetails",
                    product_details.products.len()
                );

                // DataQualityAnalyzer 사용하여 실제 검증 수행
                use crate::crawl_engine::services::data_quality_analyzer::DataQualityAnalyzer;
                let analyzer = DataQualityAnalyzer::new();

                match analyzer.validate_before_storage(&product_details.products) {
                    Ok(validated_products) => {
                        info!(
                            "✅ Data quality validation completed: {} products validated",
                            validated_products.len()
                        );
                        if validated_products.len() != product_details.products.len() {
                            warn!(
                                "⚠️  Data validation filtered out {} products",
                                product_details.products.len() - validated_products.len()
                            );
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
                info!(
                    "✅ ValidatedProducts already validated: {} products",
                    products.products.len()
                );
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
    ) -> Result<(u32, u32, u32), String> {
        // (inserted, updated, duplicates)
        match item {
            StageItem::ProductDetails(wrapper) => {
                info!(
                    "[PersistExec] handling ProductDetails count={} extraction_stats=attempted:{} success:{} failed:{}",
                    wrapper.products.len(),
                    wrapper.extraction_stats.attempted,
                    wrapper.extraction_stats.successful,
                    wrapper.extraction_stats.failed
                );
                // Debug a small sample of pagination coordinates to verify propagation
                for (i, d) in wrapper.products.iter().take(3).enumerate() {
                    debug!(
                        "[PersistExecSample] idx={} url={} page_id={:?} index_in_page={:?}",
                        i, d.url, d.page_id, d.index_in_page
                    );
                }
                let products = &wrapper.products;
                if products.is_empty() {
                    return Ok((0, 0, 0));
                }
                // Duplicate detection by URL
                let mut seen = std::collections::HashSet::new();
                let mut duplicates: Vec<String> = Vec::new();
                for d in products {
                    if !seen.insert(d.url.clone()) {
                        duplicates.push(d.url.clone());
                    }
                }
                if !duplicates.is_empty() {
                    warn!(
                        "[PersistExec] duplicate urls detected count={} urls={:?}",
                        duplicates.len(),
                        duplicates
                    );
                }
                let mut inserted = 0u32;
                let mut updated = 0u32;
                let mut duplicates_ct = 0u32;
                for (idx, detail) in products.iter().enumerate() {
                    let start = std::time::Instant::now();
                    debug!(
                        "[PersistExec] upsert detail idx={} url={} page_id={:?} index_in_page={:?}",
                        idx, detail.url, detail.page_id, detail.index_in_page
                    );
                    match product_repo.create_or_update_product_detail(detail).await {
                        Ok((was_updated, was_created)) => {
                            if was_created {
                                inserted += 1;
                            }
                            if was_updated {
                                updated += 1;
                            }
                            if !was_created && !was_updated {
                                // 중복(no-op) → 정책 적용: UpdateIdIndexOnly면 위치 강제 업데이트 시도
                                let policy_env = std::env::var("MC_DUPLICATE_POLICY").ok();
                                if matches!(policy_env.as_deref(), Some("UpdateIdIndexOnly")) {
                                    if let (Some(pid), Some(idx)) =
                                        (detail.page_id, detail.index_in_page)
                                    {
                                        if let Ok((prod_rows, det_rows)) = product_repo
                                            .force_update_position_by_url(&detail.url, pid, idx)
                                            .await
                                        {
                                            if det_rows > 0 || prod_rows > 0 {
                                                updated += 1;
                                                debug!(
                                                    "[PersistExecDetail] forced pos update idx={} url={} pid={} idx_in_page={} (prod_rows={}, det_rows={})",
                                                    idx, detail.url, pid, idx, prod_rows, det_rows
                                                );
                                                continue;
                                            }
                                        }
                                    }
                                }
                                duplicates_ct += 1;
                            }
                            debug!(
                                "[PersistExecDetail] idx={} url={} created={} updated={} elapsed_ms={}",
                                idx,
                                detail.url,
                                was_created,
                                was_updated,
                                start.elapsed().as_millis()
                            );
                        }
                        Err(e) => return Err(format!("Database save failed: {}", e)),
                    }
                }
                Ok((inserted, updated, duplicates_ct))
            }
            StageItem::ValidatedProducts(wrapper) => {
                info!(
                    "[PersistExec] handling ValidatedProducts count={}",
                    wrapper.products.len()
                );
                for (i, d) in wrapper.products.iter().take(3).enumerate() {
                    debug!(
                        "[PersistExecSample] validated idx={} url={} page_id={:?} index_in_page={:?}",
                        i, d.url, d.page_id, d.index_in_page
                    );
                }
                let products = &wrapper.products;
                if products.is_empty() {
                    return Ok((0, 0, 0));
                }
                let mut seen = std::collections::HashSet::new();
                let mut duplicates: Vec<String> = Vec::new();
                for d in products {
                    if !seen.insert(d.url.clone()) {
                        duplicates.push(d.url.clone());
                    }
                }
                if !duplicates.is_empty() {
                    warn!(
                        "[PersistExec] duplicate validated urls detected count={} urls={:?}",
                        duplicates.len(),
                        duplicates
                    );
                }
                let mut inserted = 0u32;
                let mut updated = 0u32;
                let mut duplicates_ct = 0u32;
                for (idx, detail) in products.iter().enumerate() {
                    let start = std::time::Instant::now();
                    debug!(
                        "[PersistExec] upsert validated detail idx={} url={} page_id={:?} index_in_page={:?}",
                        idx, detail.url, detail.page_id, detail.index_in_page
                    );
                    match product_repo.create_or_update_product_detail(detail).await {
                        Ok((was_updated, was_created)) => {
                            if was_created {
                                inserted += 1;
                            }
                            if was_updated {
                                updated += 1;
                            }
                            if !was_created && !was_updated {
                                let policy_env = std::env::var("MC_DUPLICATE_POLICY").ok();
                                if matches!(policy_env.as_deref(), Some("UpdateIdIndexOnly")) {
                                    if let (Some(pid), Some(idx)) =
                                        (detail.page_id, detail.index_in_page)
                                    {
                                        if let Ok((prod_rows, det_rows)) = product_repo
                                            .force_update_position_by_url(&detail.url, pid, idx)
                                            .await
                                        {
                                            if det_rows > 0 || prod_rows > 0 {
                                                updated += 1;
                                                debug!(
                                                    "[PersistExecDetail] forced pos update(validated) idx={} url={} pid={} idx_in_page={} (prod_rows={}, det_rows={})",
                                                    idx, detail.url, pid, idx, prod_rows, det_rows
                                                );
                                                continue;
                                            }
                                        }
                                    }
                                }
                                duplicates_ct += 1;
                            }
                            debug!(
                                "[PersistExecDetail] validated idx={} url={} created={} updated={} elapsed_ms={}",
                                idx,
                                detail.url,
                                was_created,
                                was_updated,
                                start.elapsed().as_millis()
                            );
                        }
                        Err(e) => return Err(format!("Database save failed: {}", e)),
                    }
                }
                Ok((inserted, updated, duplicates_ct))
            }
            _ => Ok((0, 0, 0)),
        }
    }

    // === 시뮬레이션 함수들 (기존) ===

    /// 리스트 페이지 처리 시뮬레이션 (test/dev only)
    #[cfg(feature = "simulate-details")]
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
                biased;
                // 취소 신호 확인 (우선 처리)
                _ = context.cancellation_token.changed() => {
                    if *context.cancellation_token.borrow() {
                        warn!("🚫 StageActor {} received cancellation signal", self.actor_id);
                        break;
                    }
                }
                // 명령 처리
                maybe_cmd = command_rx.recv() => {
                    match maybe_cmd {
                        Some(cmd) => {
                            debug!("📨 StageActor {} received command: {:?}", self.actor_id, cmd);
                            match cmd {
                                ActorCommand::ExecuteStage { stage_type, items: _, concurrency_limit, timeout_secs } => {
                                    // Temporary: this control path is not used in production; ignore payload type and run with empty items
                                    let empty: Vec<StageItem> = Vec::new();
                                    if let Err(e) = self.execute_stage(stage_type.clone(), empty, concurrency_limit, timeout_secs, &context).await {
                                        error!("Failed to execute stage: {:?}", e);
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
            errors_count: 0,       // TODO: 실제 에러 수 계산
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
            })
            .to_string(),
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
    async fn extract_product_detail_from_url(
        &self,
        url: &str,
    ) -> Result<crate::domain::product::ProductDetail, ActorError> {
        // HTTP 클라이언트 확인
        let http_client = self
            .http_client
            .as_ref()
            .ok_or_else(|| ActorError::RequestFailed("HTTP client not available".to_string()))?;

        // HTTP 클라이언트로 URL에서 HTML 가져오기
        let response = http_client
            .fetch_response_with_options(
                url,
                &crate::infrastructure::simple_http_client::RequestOptions {
                    user_agent_override: None, // could be overridden at call site if needed
                    referer: Some(
                        crate::infrastructure::config::csa_iot::PRODUCTS_BASE.to_string(),
                    ),
                    skip_robots_check: false,
                    attempt: None,
                    max_attempts: None,
                },
            )
            .await
            .map_err(|e| ActorError::RequestFailed(format!("HTTP request failed: {}", e)))?;

        let html_content = response.text().await.map_err(|e| {
            ActorError::ParsingFailed(format!("Failed to get response text: {}", e))
        })?;

        if html_content.trim().is_empty() {
            return Err(ActorError::ParsingFailed(format!(
                "Empty HTML content from {}",
                url
            )));
        }

        // 데이터 추출기 확인
        let data_extractor = self
            .data_extractor
            .as_ref()
            .ok_or_else(|| ActorError::ParsingFailed("Data extractor not available".to_string()))?;

        // 데이터 추출기로 HTML 파싱
        let product_data_json =
            data_extractor
                .extract_product_data(&html_content)
                .map_err(|e| {
                    ActorError::ParsingFailed(format!("Failed to extract product data: {}", e))
                })?;

        // JSON에서 필드들을 안전하게 추출
        let manufacturer = product_data_json
            .get("manufacturer")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let model = product_data_json
            .get("model")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let certificate_id = product_data_json
            .get("certificate_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let pid = product_data_json
            .get("pid")
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
