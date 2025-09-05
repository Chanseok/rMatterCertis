// Removed an unused pipeline-oriented execute_stage variant in BatchActor; the active flow uses StageActor::execute_stage.

use chrono::Utc;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Semaphore};
use tracing::{debug, error, info, warn};

use crate::crawl_engine::actors::traits::{Actor, ActorHealth, ActorStatus, ActorType};
use crate::crawl_engine::actors::types::{ActorCommand, ActorError, AppEvent, StageResult, StageType, BatchConfig};
use crate::crawl_engine::channels::types::{ProductUrls, StageItem};
use crate::crawl_engine::integrated_context::AppContext;
use crate::infrastructure::config::AppConfig;
use crate::infrastructure::{HttpClient, IntegratedProductRepository, MatterDataExtractor};
use crate::domain::services::SiteStatus;
use crate::crawl_engine::actors::stage_actor::StageActor;

/// 배치 상태 열거형
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchState {
    Idle,
    Starting,
    Processing,
    Paused,
    Completing,
    Completed,
    Failed { error: String },
}

/// 배치 실행을 담당하는 Actor
#[allow(clippy::struct_excessive_bools)]
pub struct BatchActor {
    pub(crate) actor_id: String,
    pub(crate) batch_id: Option<String>,
    pub(crate) state: BatchState,
    pub(crate) start_time: Option<Instant>,
    pub(crate) total_pages: u32,
    pub(crate) completed_pages: u32,
    pub(crate) success_count: u32,
    pub(crate) failure_count: u32,
    pub(crate) concurrency_limiter: Option<Arc<Semaphore>>,
    #[allow(dead_code)]
    pub(crate) config: Option<Arc<crate::crawl_engine::config::SystemConfig>>,
    // Real service dependencies (DI)
    pub(crate) http_client: Option<Arc<HttpClient>>,
    pub(crate) data_extractor: Option<Arc<MatterDataExtractor>>,
    pub(crate) product_repo: Option<Arc<IntegratedProductRepository>>,
    pub(crate) app_config: Option<AppConfig>,

    // Batch-scoped tracking
    pub(crate) failed_list_pages: Vec<u32>,
    pub(crate) recent_product_urls: VecDeque<String>,
    pub(crate) recent_product_set: HashSet<String>,
    pub(crate) recent_capacity: usize,
    pub(crate) skip_duplicate_urls: bool,
    pub(crate) duplicates_skipped: u32,

    // Insert/Update accounting shared back to SessionActor
    pub(crate) products_inserted: u32,
    pub(crate) products_updated: u32,
    pub(crate) shared_metrics: Option<Arc<std::sync::Mutex<(u32, u32)>>>,

    // Deferred detail crawling accumulation
    pub(crate) collected_product_urls: Vec<crate::domain::product_url::ProductUrl>,
    pub(crate) defer_detail_crawling: bool,
}

// Debug 수동 구현 (의존성들이 Debug를 구현하지 않아서)
impl std::fmt::Debug for BatchActor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BatchActor")
            .field("actor_id", &self.actor_id)
            .field("batch_id", &self.batch_id)
            .field("state", &self.state)
            .field("start_time", &self.start_time)
            .field("total_pages", &self.total_pages)
            .field("completed_pages", &self.completed_pages)
            .field("success_count", &self.success_count)
            .field("failure_count", &self.failure_count)
            .field("has_http_client", &self.http_client.is_some())
            .field("has_data_extractor", &self.data_extractor.is_some())
            .field("has_product_repo", &self.product_repo.is_some())
            .field("has_app_config", &self.app_config.is_some())
            .field("failed_list_pages", &self.failed_list_pages)
            .finish()
    }
}

// NOTE: BatchState moved above struct for early visibility in this module

/// 배치 관련 에러 타입
#[derive(Debug, thiserror::Error)]
pub enum BatchError {
    #[error("Batch initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Batch already processing: {0}")]
    AlreadyProcessing(String),

    #[error("Batch not found: {0}")]
    BatchNotFound(String),

    #[error("Invalid batch configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Concurrency limit exceeded: requested {requested}, max {max}")]
    ConcurrencyLimitExceeded { requested: u32, max: u32 },

    #[error("Context communication error: {0}")]
    ContextError(String),

    #[error("Stage processing error: {0}")]
    StageError(String),

    #[error("Stage processing failed: {stage} - {error}")]
    StageProcessingFailed { stage: String, error: String },

    #[error("Stage execution failed: {0}")]
    StageExecutionFailed(String),

    #[error("Service not available: {0}")]
    ServiceNotAvailable(String),
}

impl BatchActor {
    /// Small helper to emit an AppEvent with unified error mapping
    fn emit(
        &self,
        context: &AppContext,
        evt: AppEvent,
    ) -> Result<(), BatchError> {
        context
            .emit_event(evt)
            .map(|_| ())
            .map_err(|e| BatchError::ContextError(e.to_string()))
    }

    /// Configure whether to skip duplicate product URLs within this batch.
    pub const fn set_skip_duplicate_urls(&mut self, flag: bool) {
        self.skip_duplicate_urls = flag;
    }

    /// 내부 보조: Stage 2/3 per-item duration 합계 산출
    pub(crate) fn compute_stage_duration_sums(
        list_page_result: &StageResult,
        detail_result_opt: Option<&StageResult>,
        defer_detail_crawling: bool,
    ) -> (u64, u64) {
        let stage2_duration_sum: u64 = list_page_result.details.iter().map(|d| d.duration_ms).sum();
        let stage3_duration_sum: u64 = if defer_detail_crawling {
            0
        } else {
            detail_result_opt
                .map_or(0, |r| r.details.iter().map(|d| d.duration_ms).sum())
        };
        (stage2_duration_sum, stage3_duration_sum)
    }

    /// 내부 보조: 전체 `retries_used` 산출 (Stage 2 + Stage 3, 지연 수집 시 Stage 2만)
    pub(crate) fn compute_retries_used(
        list_page_result: &StageResult,
        detail_result_opt: Option<&StageResult>,
        defer_detail_crawling: bool,
    ) -> u32 {
        let stage2_retries: u32 = list_page_result.details.iter().map(|d| d.retry_count).sum();
        if defer_detail_crawling {
            stage2_retries
        } else {
            let stage3_retries: u32 = detail_result_opt
                .map_or(0, |r| r.details.iter().map(|d| d.retry_count).sum());
            stage2_retries.saturating_add(stage3_retries)
        }
    }
    /// 새로운 `BatchActor` 인스턴스 생성 (기본)
    ///
    /// # Arguments
    /// * `actor_id` - Actor 고유 식별자
    ///
    /// # Returns
    /// * `Self` - 새로운 `BatchActor` 인스턴스
    #[must_use] pub fn new(actor_id: String) -> Self {
        Self {
            actor_id,
            batch_id: None,
            state: BatchState::Idle,
            start_time: None,
            total_pages: 0,
            completed_pages: 0,
            success_count: 0,
            failure_count: 0,
            concurrency_limiter: None,
            config: None,
            http_client: None,
            data_extractor: None,
            product_repo: None,
            app_config: None,
            failed_list_pages: Vec::new(),
            recent_product_urls: VecDeque::new(),
            recent_product_set: HashSet::new(),
            recent_capacity: 2000,
            skip_duplicate_urls: true,
            duplicates_skipped: 0,
            products_inserted: 0,
            products_updated: 0,
            shared_metrics: None,
            collected_product_urls: Vec::new(),
            defer_detail_crawling: std::env::var("MC_UNIFIED_DETAIL")
                .map(|v| {
                    let t = v.trim();
                    !(t.eq("0") || t.eq_ignore_ascii_case("false"))
                })
                .unwrap_or(false),
        }
    }

    /// 🔥 Phase 1: 실제 서비스들과 함께 `BatchActor` 생성
    ///
    /// # Arguments
    /// * `actor_id` - Actor 고유 식별자
    /// * `batch_id` - 배치 ID
    /// * `http_client` - HTTP 클라이언트
    /// * `data_extractor` - 데이터 추출기
    /// * `product_repo` - 제품 레포지토리
    /// * `app_config` - 앱 설정
    ///
    /// # Returns
    /// * `Self` - 서비스가 주입된 `BatchActor` 인스턴스
    #[must_use]
    pub fn new_with_services(
        actor_id: String,
        batch_id: String,
        http_client: Arc<HttpClient>,
        data_extractor: Arc<MatterDataExtractor>,
        product_repo: Arc<IntegratedProductRepository>,
        app_config: AppConfig,
    ) -> Self {
        Self {
            actor_id,
            batch_id: Some(batch_id),
            state: BatchState::Idle,
            start_time: None,
            total_pages: 0,
            completed_pages: 0,
            success_count: 0,
            failure_count: 0,
            concurrency_limiter: None,
            // Provide a default SystemConfig so template/bridge execution paths have required settings
            config: Some(Arc::new(
                crate::crawl_engine::config::SystemConfig::default(),
            )),
            // 실제 서비스 의존성 주입
            http_client: Some(http_client),
            data_extractor: Some(data_extractor),
            product_repo: Some(product_repo),
            app_config: Some(app_config),
            failed_list_pages: Vec::new(),
            recent_product_urls: VecDeque::new(),
            recent_product_set: HashSet::new(),
            recent_capacity: 2000,
            skip_duplicate_urls: true,
            duplicates_skipped: 0,
            products_inserted: 0,
            products_updated: 0,
            shared_metrics: None,
            collected_product_urls: Vec::new(),
            defer_detail_crawling: std::env::var("MC_UNIFIED_DETAIL")
                .map(|v| {
                    let t = v.trim();
                    !(t.eq("0") || t.eq_ignore_ascii_case("false"))
                })
                .unwrap_or(false),
        }
    }

    /// 배치 처리 시작
    ///
    /// # Arguments
    /// * `batch_id` - 배치 ID
    /// * `pages` - 처리할 페이지 번호 리스트
    /// * `config` - 배치 설정
    /// * `context` - Actor 컨텍스트
    async fn process_list_page_batch(
        &mut self,
        batch_id: String,
        _stage_type: StageType,
        pages: Vec<u32>,
        config: BatchConfig,
        _batch_size: u32,
        concurrency_limit: u32,
        _total_pages: u32,
        _products_on_last_page: u32,
        context: &AppContext,
    ) -> Result<(), BatchError> {
        // 상태 검증
        if !matches!(self.state, BatchState::Idle) {
            return Err(BatchError::AlreadyProcessing(batch_id));
        }

        // 설정 검증
        self.validate_batch_config(&config, concurrency_limit)?;

        info!(
            "🔄 [Batch START] actor={}, batch_id={}, pages={}, range={:?}",
            self.actor_id,
            batch_id,
            pages.len(),
            if pages.is_empty() {
                None
            } else {
                Some((pages.first().copied(), pages.last().copied()))
            }
        );

        // 계획 대비 적용 값 검증 로그 (plan vs applied)
        let planned_range = (config.start_page, config.end_page);
        let applied_range = if pages.is_empty() {
            None
        } else {
            Some((pages.first().copied(), pages.last().copied()))
        };
        info!(
            "🧭 [Batch PLAN/APPLIED] planned={:?} applied={:?} count={}",
            planned_range,
            applied_range,
            pages.len()
        );

        // 상태 초기화
        self.batch_id = Some(batch_id.clone());
        self.state = BatchState::Starting;
        self.start_time = Some(Instant::now());
        self.total_pages = pages.len() as u32;
        self.completed_pages = 0;
        self.success_count = 0;
        self.failure_count = 0;
        // 새 배치마다 중복 방지 캐시 초기화 (세션 전체 유지가 아니라 배치 단위로 격리)
        self.recent_product_urls.clear();
        self.recent_product_set.clear();
        debug!("♻️ Cleared recent product URL dedupe caches for new batch");

        // 동시성 제어 설정
        self.concurrency_limiter = Some(Arc::new(Semaphore::new(concurrency_limit as usize)));

        // 배치 시작 이벤트 발행
        let start_event = AppEvent::BatchStarted {
            batch_id: batch_id.clone(),
            session_id: context.session_id.clone(),
            plan_id: context.plan_id.clone(),
            pages_count: pages.len() as u32,
            timestamp: Utc::now(),
        };

    self.emit(context, start_event)?;

        // KPI: 배치 시작 (구조화 로그)
        let plan_id_json = if let Some(pid) = &context.plan_id { format!("\"{}\"", pid) } else { "null".to_string() };
        info!(target: "kpi.batch",
            "{{\"event\":\"batch_started\",\"session_id\":\"{}\",\"batch_id\":\"{}\",\"plan_id\":{},\"pages_count\":{},\"ts\":\"{}\"}}",
            context.session_id,
            batch_id,
            plan_id_json,
            pages.len(),
            chrono::Utc::now()
        );

        // 상태를 Processing으로 전환
        self.state = BatchState::Processing;

        // 실제 StageActor 기반 처리 구현
        info!(
            "🎭 Using real StageActor-based processing for batch {}",
            batch_id
        );

        // 초기 Stage Items 생성 - 페이지 기반 아이템들
        let initial_items: Vec<StageItem> = pages
            .iter()
            .map(|&page_number| StageItem::Page(page_number))
            .collect();

        // Stage 1: StatusCheck - 사이트 상태 확인 (세션 힌트가 없을 때만 실행)
        let mut total_pages_hint: Option<u32> = None;
        let mut last_page_products_hint: Option<u32> = None;
        if _total_pages > 0 && _products_on_last_page > 0 {
            info!(
                "🧠 Using provided SiteStatus hints from Session: total_pages={}, products_on_last_page={}",
                _total_pages, _products_on_last_page
            );
            total_pages_hint = Some(_total_pages);
            last_page_products_hint = Some(_products_on_last_page);
        } else {
            info!("🔍 Starting Stage 1: StatusCheck (no valid session hints)");
            // StatusCheck는 사이트 전체 상태를 확인하므로 특별한 URL 아이템으로 처리
            let status_check_items = vec![StageItem::Url(
                "https://csa-iot.org/csa-iot_products/".to_string(),
            )];
            let status_check_result = self
                .execute_stage_with_actor(
                    StageType::StatusCheck,
                    status_check_items,
                    concurrency_limit,
                    context,
                )
                .await?;
            info!(
                "✅ Stage 1 (StatusCheck) completed: {} success, {} failed",
                status_check_result.successful_items, status_check_result.failed_items
            );

            // 성공적으로 완료되었다면 (처리된 아이템이 있다면) 다음 단계로 진행
            if status_check_result.processed_items == 0 {
                error!("❌ Stage 1 (StatusCheck) failed completely - no status check performed");
                self.state = BatchState::Failed {
                    error: "StatusCheck stage failed - no status check performed".to_string(),
                };
                // 배치 실패 이벤트 발행
                let fail_event = AppEvent::BatchFailed {
                    batch_id: batch_id.clone(),
                    session_id: context.session_id.clone(),
                    plan_id: context.plan_id.clone(),
                    error: "StatusCheck stage failed - no status check performed".to_string(),
                    final_failure: true,
                    timestamp: Utc::now(),
                };
                self.emit(context, fail_event)?;
                return Err(BatchError::StageExecutionFailed(
                    "StatusCheck stage failed - no status check performed".to_string(),
                ));
            }
            // StatusCheck에서 사이트 접근 불가능한 경우에만 중단
            if status_check_result.failed_items > 0 && status_check_result.successful_items == 0 {
                error!("❌ Stage 1 (StatusCheck) failed completely - site is not accessible");
                self.state = BatchState::Failed {
                    error: "StatusCheck stage failed - site is not accessible".to_string(),
                };
                // 배치 실패 이벤트 발행
                let fail_event = AppEvent::BatchFailed {
                    batch_id: batch_id.clone(),
                    session_id: context.session_id.clone(),
                    plan_id: context.plan_id.clone(),
                    error: "StatusCheck stage failed - site is not accessible".to_string(),
                    final_failure: true,
                    timestamp: Utc::now(),
                };
                self.emit(context, fail_event)?;
                return Err(BatchError::StageExecutionFailed(
                    "StatusCheck stage failed - site is not accessible".to_string(),
                ));
            }

            // StatusCheck에서 수집된 SiteStatus JSON을 파싱하여 페이지네이션 힌트로 사용
            if let Some(first) = status_check_result.details.first() {
                if let Some(json) = &first.collected_data {
                    match serde_json::from_str::<SiteStatus>(json) {
                        Ok(site_status) => {
                            total_pages_hint = Some(site_status.total_pages);
                            last_page_products_hint = Some(site_status.products_on_last_page);
                            info!(
                                "📊 SiteStatus hints from Stage 1: total_pages={}, products_on_last_page={}",
                                site_status.total_pages, site_status.products_on_last_page
                            );
                        }
                        Err(e) => {
                            warn!(
                                "⚠️ Failed to parse SiteStatus from Stage 1 collected_data: {}",
                                e
                            );
                        }
                    }
                }
            }
        }

        // Stage 2: ListPageCrawling - ProductURL 수집
        info!("🔍 Starting Stage 2: ListPageCrawling");

        // Stage 2는 페이지네이션 힌트를 StageActor에 주입해야 함 → 전용 실행 경로 사용
        let list_page_result = self
            .execute_stage_with_actor_with_hints(
                StageType::ListPageCrawling,
                initial_items.clone(),
                concurrency_limit,
                context,
                total_pages_hint,
                last_page_products_hint,
            )
            .await?;

        info!(
            "✅ Stage 2 (ListPageCrawling) completed: {} success, {} failed",
            list_page_result.successful_items, list_page_result.failed_items
        );

        // Stage 2에서 최종 실패한 페이지 수집
        self.failed_list_pages.clear();
        for (idx, item) in initial_items.iter().enumerate() {
            if let StageItem::Page(page_no) = item {
                if let Some(detail) = list_page_result.details.get(idx) {
                    if !detail.success {
                        self.failed_list_pages.push(*page_no);
                    }
                }
            }
        }
        if !self.failed_list_pages.is_empty() {
            warn!(
                "⚠️ Stage 2 ended with {} failed pages after retries: {:?}",
                self.failed_list_pages.len(),
                self.failed_list_pages
            );
            // 진행 상황 이벤트로 실패 페이지 정보 요약 발행 (샘플 최대 10)
            let sample_len = self.failed_list_pages.len().min(10);
            let sample: Vec<u32> = self
                .failed_list_pages
                .iter()
                .copied()
                .take(sample_len)
                .collect();
            let progress_event = AppEvent::Progress {
                session_id: context.session_id.clone(),
                current_step: 2,
                total_steps: 5,
                message: format!(
                    "Stage 2 retries exhausted for {} pages (sample: {:?})",
                    self.failed_list_pages.len(),
                    sample
                ),
                percentage: 40.0,
                timestamp: Utc::now(),
            };
            self.emit(context, progress_event)?;
        }

        // Stage 실패 시 파이프라인 중단 검증
        if list_page_result.successful_items == 0 {
            error!("❌ Stage 2 (ListPageCrawling) failed completely - aborting pipeline");
            self.state = BatchState::Failed {
                error: "ListPageCrawling stage failed completely".to_string(),
            };
            // Emit KPI-style structured summary for failure
            info!(target: "kpi.batch",
                "{{\"event\":\"batch_stage_summary\",\"batch_id\":\"{}\",\"stage\":\"list_page\",\"success\":{},\"failed\":{},\"pages\":{},\"ts\":\"{}\"}}",
                batch_id,
                list_page_result.successful_items,
                list_page_result.failed_items,
                pages.len(),
                chrono::Utc::now()
            );
            // 배치 실패 이벤트 발행
            let fail_event = AppEvent::BatchFailed {
                batch_id: batch_id.clone(),
                session_id: context.session_id.clone(),
                plan_id: context.plan_id.clone(),
                error: "ListPageCrawling stage failed completely".to_string(),
                final_failure: true,
                timestamp: Utc::now(),
            };
            self.emit(context, fail_event)?;
            return Err(BatchError::StageExecutionFailed(
                "ListPageCrawling stage failed completely".to_string(),
            ));
        }

        // Stage 2 결과를 Stage 3 입력으로 변환 또는 누적
        let product_detail_items = self.transform_stage_output(
            StageType::ListPageCrawling,
            initial_items.clone(),
            &list_page_result,
        )?;

        let mut detail_result_opt: Option<StageResult> = None;
        if self.defer_detail_crawling {
            for item in &product_detail_items {
                if let StageItem::ProductUrls(wrapper) = item {
                    self.collected_product_urls.extend(wrapper.urls.clone());
                }
            }
            info!(
                "🧺 Deferred product detail crawling: accumulated_urls={} (batch pages={})",
                self.collected_product_urls.len(),
                pages.len()
            );
        } else {
            // 즉시 상세 수집
            info!("🔍 Starting Stage 3: ProductDetailCrawling");
        let detail_result = match self
                .execute_stage_with_actor(
                    StageType::ProductDetailCrawling,
                    product_detail_items.clone(),
                    concurrency_limit,
                    context,
                )
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    let fail_event = AppEvent::BatchFailed {
                        batch_id: batch_id.clone(),
                        session_id: context.session_id.clone(),
                        plan_id: context.plan_id.clone(),
                        error: format!("Stage 3 failed: {e}"),
                        final_failure: true,
                        timestamp: Utc::now(),
                    };
            self.emit(context, fail_event)?;
                    self.state = BatchState::Failed {
                        error: format!("Stage 3 failed: {e}"),
                    };
                    return Err(e);
                }
            };
            info!(
                "✅ Stage 3 (ProductDetailCrawling) completed: {} success, {} failed",
                detail_result.successful_items, detail_result.failed_items
            );
            info!(target: "kpi.batch", "{{\"event\":\"batch_stage_summary\",\"batch_id\":\"{}\",\"stage\":\"product_detail\",\"success\":{},\"failed\":{},\"pages\":{},\"ts\":\"{}\"}}",
                batch_id, detail_result.successful_items, detail_result.failed_items, pages.len(), chrono::Utc::now());
            detail_result_opt = Some(detail_result);
        }

        // Summarize failed product detail URLs by correlating input items with Stage 3 results
        if !self.defer_detail_crawling {
            // detail_result only exists in non-deferred path; re-execute minimal failure inspection via executing stage again not desired.
            // TODO: Refactor to hold detail_result outside branch if failure summary needed.
            // Skipping failure summary in deferred mode.
            /*
                if detail_result.failed_items > 0 {
                    let mut failed_detail_urls: Vec<String> = Vec::new();
                    for (idx, item) in product_detail_items.iter().enumerate() {
                        if let Some(item_result) = detail_result.details.get(idx) {
                            if !item_result.success {
                                match item {
                                    StageItem::ProductUrls(urls_wrapper) => {
                                        // If this item was a batch of URLs, log the count rather than each URL
                                        warn!("⚠️ ProductDetailCrawling subtask failed for a batch of {} urls (index={})", urls_wrapper.urls.len(), idx);
                                    }
                                    StageItem::Url(_url_str) => {
                                        // No direct URL payload here
                                    }
                                    StageItem::Product(product) => {
                                        failed_detail_urls.push(product.url.clone());
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    if !failed_detail_urls.is_empty() {
                        let sample_len = failed_detail_urls.len().min(10);
                        let sample = &failed_detail_urls[..sample_len];
                        warn!("⚠️ Stage 3 ended with {} failed detail URLs (sample up to 10): {:?}",
                            failed_detail_urls.len(), sample);
                        // 진행 상황 이벤트로 실패 URL 요약 발행
                        let progress_event = AppEvent::Progress {
                            session_id: context.session_id.clone(),
                            current_step: 3,
                            total_steps: 5,
                            message: format!(
                                "Stage 3 retries exhausted for {} product details (sample: {:?})",
                                failed_detail_urls.len(), sample
                            ),
                            percentage: 60.0,
                            timestamp: Utc::now(),
                        };
                        context.emit_event(progress_event)
                            .map_err(|e| BatchError::ContextError(e.to_string()))?;
                    }
            }
            */
        }

        // Stage 3 결과를 Stage 4 입력으로 변환
        let data_validation_items = if self.defer_detail_crawling {
            // Deferred mode: skip (no items)
            Vec::new()
        } else {
            // 기존 변환 결과 (각 ProductDetail 단위) → 하나의 ProductDetails StageItem 으로 합쳐 1회 실행
            let detail_result =
                detail_result_opt
                    .as_ref()
                    .ok_or_else(|| BatchError::StageProcessingFailed {
                        stage: "ProductDetailCrawling".to_string(),
                        error: "detail_result missing when not deferred".to_string(),
                    })?;
            let per_item = self.transform_stage_output(
                StageType::ProductDetailCrawling,
                product_detail_items,
                detail_result,
            )?;
            if per_item.is_empty() {
                Vec::new()
            } else {
                // 모든 Product 또는 ProductDetails 아이템에서 상세 제품들을 추출해 하나로 병합
                use crate::crawl_engine::channels::types::{ProductDetails, StageItem as SItem};
                let mut all_products: Vec<crate::domain::product::ProductDetail> = Vec::new();
                let mut all_source_urls: Vec<crate::domain::product_url::ProductUrl> = Vec::new();
                for it in per_item {
                    match it {
                        SItem::ProductDetails(pd) => {
                            all_source_urls.extend(pd.source_urls.into_iter());
                            all_products.extend(pd.products.into_iter());
                        }
                        SItem::Product(_p) => {
                            // 단일 ProductInfo 로 표현된 경우 ProductDetail 변환 불가 → 무시 또는 향후 변환 로직 추가
                            debug!("Skipping non-detailed product item in aggregation");
                        }
                        _ => {}
                    }
                }
                if all_products.is_empty() {
                    Vec::new()
                } else {
                    let stats = crate::crawl_engine::channels::types::ExtractionStats {
                        attempted: all_products.len() as u32,
                        successful: all_products.len() as u32,
                        failed: 0,
                        empty_responses: 0,
                    };
                    let merged = ProductDetails {
                        products: all_products,
                        source_urls: all_source_urls,
                        extraction_stats: stats,
                    };
                    vec![SItem::ProductDetails(merged)]
                }
            }
        };

        // Stage 4: DataValidation - 데이터 품질 분석
        info!("🔍 Starting Stage 4: DataValidation");
    let validation_result = match self
            .execute_stage_with_actor(
                StageType::DataValidation,
                data_validation_items.clone(),
                concurrency_limit,
                context,
            )
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let fail_event = AppEvent::BatchFailed {
                    batch_id: batch_id.clone(),
                    session_id: context.session_id.clone(),
                    plan_id: context.plan_id.clone(),
                    error: format!("Stage 4 failed: {e}"),
                    final_failure: true,
                    timestamp: Utc::now(),
                };
        self.emit(context, fail_event)?;
                self.state = BatchState::Failed {
                    error: format!("Stage 4 failed: {e}"),
                };
                return Err(e);
            }
        };

        info!(
            "✅ Stage 4 (DataValidation) completed: {} success, {} failed",
            validation_result.successful_items, validation_result.failed_items
        );
        info!(target: "kpi.batch", "{{\"event\":\"batch_stage_summary\",\"batch_id\":\"{}\",\"stage\":\"data_validation\",\"success\":{},\"failed\":{},\"pages\":{},\"ts\":\"{}\"}}",
          batch_id, validation_result.successful_items, validation_result.failed_items, pages.len(), chrono::Utc::now());

        // Stage 4 결과를 Stage 5 입력으로 변환
        let data_saving_items = if self.defer_detail_crawling {
            Vec::new()
        } else {
            self.transform_stage_output(
                StageType::DataValidation,
                data_validation_items,
                &validation_result,
            )?
        };

        // Stage 5: DataSaving - 데이터 저장
        info!("🔍 Starting Stage 5: DataSaving");
    let saving_result = match self
            .execute_stage_with_actor(
                StageType::DataSaving,
                data_saving_items,
                concurrency_limit,
                context,
            )
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let fail_event = AppEvent::BatchFailed {
                    batch_id: batch_id.clone(),
                    session_id: context.session_id.clone(),
                    plan_id: context.plan_id.clone(),
                    error: format!("Stage 5 failed: {e}"),
                    final_failure: true,
                    timestamp: Utc::now(),
                };
        self.emit(context, fail_event)?;
                self.state = BatchState::Failed {
                    error: format!("Stage 5 failed: {e}"),
                };
                return Err(e);
            }
        };

        info!(
            "✅ Stage 5 (DataSaving) completed: {} success, {} failed",
            saving_result.successful_items, saving_result.failed_items
        );
        info!(target: "kpi.batch", "{{\"event\":\"batch_stage_summary\",\"batch_id\":\"{}\",\"stage\":\"data_saving\",\"success\":{},\"failed\":{},\"pages\":{},\"ts\":\"{}\"}}",
          batch_id, saving_result.successful_items, saving_result.failed_items, pages.len(), chrono::Utc::now());

        // DataSaving 단계에서 product insert/update 메트릭 추출
        // StageItemResult.collected_data 에 JSON { products_inserted, products_updated, total_affected }
        let mut inserted_sum = 0u32;
        let mut updated_sum = 0u32;
        for item in &saving_result.details {
            if let Some(data) = &item.collected_data {
                if data.starts_with('{') {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                        if let Some(pi) = v.get("products_inserted").and_then(serde_json::Value::as_u64) {
                            inserted_sum = inserted_sum.saturating_add(pi as u32);
                        }
                        if let Some(pu) = v.get("products_updated").and_then(serde_json::Value::as_u64) {
                            updated_sum = updated_sum.saturating_add(pu as u32);
                        }
                    }
                }
            }
        }
        self.products_inserted = inserted_sum;
        self.products_updated = updated_sum;
        if let Some(shared) = &self.shared_metrics {
            if let Ok(mut g) = shared.lock() {
                *g = (self.products_inserted, self.products_updated);
            }
        }

        // 배치 결과 집계 (성공 카운트는 saving 단계 성공 기준)
        self.success_count = saving_result.successful_items;
        self.completed_pages = pages.len() as u32;
        self.state = BatchState::Completed;
        info!(
            "🏁 [Batch COMPLETE] actor={}, batch_id={}, pages_processed={}",
            self.actor_id,
            batch_id,
            pages.len()
        );

        // 추가: 배치 요약 한 줄 로그로 핵심 지표 집계 출력
        let duration_ms = self
            .start_time
            .map_or(0, |s| s.elapsed().as_millis() as u64);

        // 보조 지표: Stage 2/3 per-item duration 합계 집계 및 구조화 로그
        let (stage2_duration_sum, stage3_duration_sum) = Self::compute_stage_duration_sums(
            &list_page_result,
            detail_result_opt.as_ref(),
            self.defer_detail_crawling,
        );
        info!(target: "kpi.batch", "{{\"event\":\"batch_stage_durations\",\"batch_id\":\"{}\",\"stage2_duration_ms_total\":{},\"stage3_duration_ms_total\":{},\"ts\":\"{}\"}}",
            batch_id,
            stage2_duration_sum,
            stage3_duration_sum,
            chrono::Utc::now()
        );
        info!(
            "📦 [Batch SUMMARY] actor={}, batch_id={}, pages_total={}, success_items={}, failed_items={}, retries_used≈{}, duration_ms={}, products_inserted={}, products_updated={}, deferred_detail={}",
            self.actor_id,
            batch_id,
            self.total_pages,
            saving_result.successful_items,
            saving_result.failed_items,
            Self::compute_retries_used(
                &list_page_result,
                detail_result_opt.as_ref(),
                self.defer_detail_crawling
            ),
            duration_ms,
            self.products_inserted,
            self.products_updated,
            self.defer_detail_crawling
        );

        let completion_event = AppEvent::BatchCompleted {
            batch_id: batch_id.clone(),
            session_id: context.session_id.clone(),
            plan_id: context.plan_id.clone(),
            success_count: self.success_count,
            failed_count: saving_result.failed_items,
            duration: self
                .start_time
                .map_or(0, |s| s.elapsed().as_millis() as u64),
            timestamp: Utc::now(),
        };

    self.emit(context, completion_event)?;

        // KPI: 배치 완료 (구조화 로그)
        let plan_id_json = if let Some(pid) = &context.plan_id { format!("\"{}\"", pid) } else { "null".to_string() };
        info!(target: "kpi.batch",
            "{{\"event\":\"batch_completed\",\"session_id\":\"{}\",\"batch_id\":\"{}\",\"plan_id\":{},\"pages_total\":{},\"pages_success\":{},\"pages_failed\":{},\"duration_ms\":{},\"products_inserted\":{},\"products_updated\":{},\"ts\":\"{}\"}}",
            context.session_id,
            batch_id,
            plan_id_json,
            self.total_pages,
            self.success_count.max(list_page_result.successful_items),
            list_page_result.failed_items,
            self.start_time.map_or(0, |s| s.elapsed().as_millis() as u64),
            self.products_inserted,
            self.products_updated,
            chrono::Utc::now()
        );

        // TODO: Integrate real DataSaving inserted/updated metrics: requires StageActor -> BatchActor callback.
        // Current workaround: StageActor logs metrics; future enhancement: channel message carrying counts.

        // === 추가: 배치 리포트 이벤트 발행 ===
        let duration_ms = self
            .start_time
            .map_or(0, |s| s.elapsed().as_millis() as u64);
        // Stage 2/3 결과는 상단 스코프의 변수들에서 가져옴. 사용 가능 시 집계, 없으면 보수적 기본값.
        let pages_total = self.total_pages;
        let pages_success = self.success_count.max(list_page_result.successful_items);
        let pages_failed = list_page_result.failed_items;
        let (details_success, details_failed) = if self.defer_detail_crawling {
            (0, 0)
        } else {
            (
                validation_result.successful_items,
                validation_result.failed_items,
            )
        };
        let retries_used = Self::compute_retries_used(
            &list_page_result,
            detail_result_opt.as_ref(),
            self.defer_detail_crawling,
        );
        let report_event = AppEvent::BatchReport {
            session_id: context.session_id.clone(),
            batch_id: batch_id.clone(),
            plan_id: context.plan_id.clone(),
            pages_total,
            pages_success,
            pages_failed,
            list_pages_failed: self.failed_list_pages.clone(),
            details_success,
            details_failed,
            retries_used,
            duration_ms,
            duplicates_skipped: self.duplicates_skipped,
            products_inserted: self.products_inserted,
            products_updated: self.products_updated,
            timestamp: Utc::now(),
        };
    self.emit(context, report_event)?;

        Ok(())
    }

    /// 배치 설정 검증
    ///
    /// # Arguments
    /// * `config` - 배치 설정
    /// * `concurrency_limit` - 동시성 제한
    fn validate_batch_config(
        &self,
        config: &BatchConfig,
        concurrency_limit: u32,
    ) -> Result<(), BatchError> {
        if config.batch_size == 0 {
            return Err(BatchError::InvalidConfiguration(
                "Batch size cannot be zero".to_string(),
            ));
        }

        if concurrency_limit == 0 {
            return Err(BatchError::InvalidConfiguration(
                "Concurrency limit cannot be zero".to_string(),
            ));
        }

        const MAX_CONCURRENCY: u32 = 100;
        if concurrency_limit > MAX_CONCURRENCY {
            return Err(BatchError::ConcurrencyLimitExceeded {
                requested: concurrency_limit,
                max: MAX_CONCURRENCY,
            });
        }

        Ok(())
    }

    // Removed unused validate_batch helper; validation happens via state checks at call sites.

    /// 배치 정리
    fn cleanup_batch(&mut self) {
        self.batch_id = None;
        self.state = BatchState::Idle;
        self.start_time = None;
        self.total_pages = 0;
        self.completed_pages = 0;
        self.success_count = 0;
        self.failure_count = 0;
        self.concurrency_limiter = None;
    }

    /// 진행 상황 계산
    ///
    /// # Returns
    /// * `f64` - 진행률 (0.0 ~ 1.0)
    fn calculate_progress(&self) -> f64 {
        if self.total_pages == 0 {
            0.0
        } else {
            f64::from(self.completed_pages) / f64::from(self.total_pages)
        }
    }

    /// 처리 속도 계산 (페이지/초)
    ///
    /// # Returns
    /// * `f64` - 처리 속도
    fn calculate_processing_rate(&self) -> f64 {
        if let Some(start_time) = self.start_time {
            let elapsed = start_time.elapsed();
            if elapsed.as_secs() > 0 {
                f64::from(self.completed_pages) / elapsed.as_secs_f64()
            } else {
                0.0
            }
        } else {
            0.0
        }
    }
}

#[async_trait::async_trait]
impl Actor for BatchActor {
    type Command = ActorCommand;
    type Error = ActorError;

    fn actor_id(&self) -> &str {
        &self.actor_id
    }

    fn actor_type(&self) -> ActorType {
        ActorType::Batch
    }
    async fn run(
        &mut self,
        mut context: AppContext,
        mut command_rx: mpsc::Receiver<Self::Command>,
    ) -> Result<(), Self::Error> {
        info!("🔄 BatchActor {} starting execution loop", self.actor_id);

        loop {
            tokio::select! {
                // 명령 처리
                command = command_rx.recv() => {
                    if let Some(cmd) = command {
                        debug!("📨 BatchActor {} received command: {:?}", self.actor_id, cmd);

                        match cmd {
                            ActorCommand::ProcessBatch {
                                batch_id,
                                pages,
                                config,
                                batch_size,
                                concurrency_limit,
                                total_pages,
                                products_on_last_page
                            } => {
                                if let Err(e) = self.process_list_page_batch(
                                    batch_id,
                                    StageType::ListPageCrawling, // stage_type 추가
                                    pages,
                                    config,
                                    batch_size,
                                    concurrency_limit,
                                    total_pages,
                                    products_on_last_page,
                                    &context
                                ).await {
                                    error!("Failed to process batch: {}", e);
                                    info!("[BatchActorRun] exiting early after failed batch");
                                    info!("🏁 BatchActor {} execution loop ended (failure)", self.actor_id);
                                    return Err(ActorError::CommandProcessingFailed(format!("batch failed: {e}")));
                                }
                                // 단일 배치 모드: 추가 명령을 기다리지 않고 즉시 종료하여 상위 await가 풀리도록 한다.
                                info!("[BatchActorRun] single batch processed successfully — returning to caller");
                                info!("🏁 BatchActor {} execution loop ended (single batch success)", self.actor_id);
                                return Ok(());
                            }

                            ActorCommand::Shutdown => {
                                info!("🛑 BatchActor {} received shutdown command", self.actor_id);
                                break;
                            }

                            _ => {
                                debug!("BatchActor {} ignoring non-batch command", self.actor_id);
                            }
                        }
                    } else {
                        warn!("📪 BatchActor {} command channel closed", self.actor_id);
                        break;
                    }
                }

                // 취소 신호 확인
                _ = context.cancellation_token.changed() => {
                    // Cancellation 감지
                    if *context.cancellation_token.borrow() {
                        warn!("🚫 BatchActor {} received cancellation signal", self.actor_id);
                        break;
                    }
                }
            }
        }

        info!("🏁 BatchActor {} execution loop ended", self.actor_id);
        Ok(())
    }

    async fn health_check(&self) -> Result<ActorHealth, Self::Error> {
        let status = match &self.state {
            BatchState::Idle => ActorStatus::Healthy,
            BatchState::Processing => ActorStatus::Healthy,
            BatchState::Completed => ActorStatus::Healthy,
            BatchState::Paused => ActorStatus::Degraded {
                reason: "Batch paused".to_string(),
                since: Utc::now(),
            },
            BatchState::Failed { error } => ActorStatus::Unhealthy {
                error: error.clone(),
                since: Utc::now(),
            },
            _ => ActorStatus::Degraded {
                reason: format!("In transition state: {:?}", self.state),
                since: Utc::now(),
            },
        };

        Ok(ActorHealth {
            actor_id: self.actor_id.clone(),
            actor_type: ActorType::Batch,
            status,
            last_activity: Utc::now(),
            memory_usage_mb: 0, // TODO: 실제 메모리 사용량 계산
            active_tasks: if matches!(self.state, BatchState::Processing) {
                self.total_pages - self.completed_pages
            } else {
                0
            },
            commands_processed: 0, // TODO: 실제 처리된 명령 수 계산
            errors_count: 0,       // TODO: 실제 에러 수 계산
            avg_command_processing_time_ms: 0.0, // TODO: 실제 평균 처리 시간 계산
            metadata: serde_json::json!({
                "batch_id": self.batch_id,
                "state": format!("{:?}", self.state),
                "total_pages": self.total_pages,
                "completed_pages": self.completed_pages,
                "success_count": self.success_count,
                "failure_count": self.failure_count,
                "progress": self.calculate_progress(),
                "processing_rate": self.calculate_processing_rate()
            })
            .to_string(),
        })
    }

    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        info!("🔌 BatchActor {} shutting down", self.actor_id);

        // 활성 배치가 있다면 정리
        if self.batch_id.is_some() {
            warn!("Cleaning up active batch during shutdown");
            self.cleanup_batch();
        }

        Ok(())
    }
}

impl BatchActor {
    /// 개별 Stage를 `StageActor로` 실행
    /// TODO: `StageItemCompleted` 이벤트 수신 채널 도입하여 `products_inserted/products_updated` 실시간 반영
    /// # Arguments
    /// * `stage_type` - 실행할 스테이지 타입
    /// * `items` - 처리할 아이템들
    /// * `concurrency_limit` - 동시 실행 제한
    /// * `context` - Actor 컨텍스트
    async fn execute_stage_with_actor(
        &self,
        stage_type: StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
        context: &AppContext,
    ) -> Result<StageResult, BatchError> {
        // 서비스 의존성이 있는지 확인
        let http_client = self.http_client.as_ref().ok_or_else(|| {
            BatchError::ServiceNotAvailable("HttpClient not initialized".to_string())
        })?;
        let data_extractor = self.data_extractor.as_ref().ok_or_else(|| {
            BatchError::ServiceNotAvailable("MatterDataExtractor not initialized".to_string())
        })?;
        let product_repo = self.product_repo.as_ref().ok_or_else(|| {
            BatchError::ServiceNotAvailable(
                "IntegratedProductRepository not initialized".to_string(),
            )
        })?;
        let app_config = self.app_config.as_ref().ok_or_else(|| {
            BatchError::ServiceNotAvailable("AppConfig not initialized".to_string())
        })?;

        // Canonical path: StageActor only

        // StageActor 생성 (DI 경로: StageDeps + StrategyFactory)
        let deps = {
            // 중복 정책: 환경 변수 힌트(MC_DUPLICATE_POLICY)로 제어 (manual 경로에서 설정)
            let dup_policy = match std::env::var("MC_DUPLICATE_POLICY").ok().as_deref() {
                Some("UpdateIdIndexOnly") => crate::crawl_engine::actors::types::DuplicatePersistencePolicy::UpdateIdIndexOnly,
                Some("FullUpdate") => crate::crawl_engine::actors::types::DuplicatePersistencePolicy::FullUpdate,
                _ => crate::crawl_engine::actors::types::DuplicatePersistencePolicy::Skip,
            };
            crate::crawl_engine::actors::stage_actor::StageDeps {
                http_client: Arc::clone(http_client),
                data_extractor: Arc::clone(data_extractor),
                product_repo: Arc::clone(product_repo),
                app_config: app_config.clone(),
                duplicate_policy: dup_policy,
            }
        };

        let mut stage_actor = StageActor::new_with_deps(
            format!("stage_{}_{}", stage_type.as_str(), self.actor_id),
            self.batch_id.clone().unwrap_or_default(),
            deps,
            Arc::new(crate::crawl_engine::stages::DefaultStageLogicFactory),
        );

        // StageActor로 Stage 실행 (실제 items 전달)
        // Use configurable operation timeout instead of hard-coded 30s
        let timeout_secs = app_config.user.crawling.timing.operation_timeout_seconds;
        let stage_result = stage_actor
            .execute_stage(stage_type, items, concurrency_limit, timeout_secs, context)
            .await
            .map_err(|e| {
                BatchError::StageExecutionFailed(format!("Stage execution failed: {e:?}"))
            })?;

        Ok(stage_result)
    }

    /// 힌트를 주입할 수 있는 Stage 실행 도우미 (`ListPage` 등)
    async fn execute_stage_with_actor_with_hints(
        &self,
        stage_type: StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
        context: &AppContext,
        total_pages_hint: Option<u32>,
        products_on_last_page_hint: Option<u32>,
    ) -> Result<StageResult, BatchError> {
        // 기본 실행 준비는 동일
        let http_client = self.http_client.as_ref().ok_or_else(|| {
            BatchError::ServiceNotAvailable("HttpClient not initialized".to_string())
        })?;
        let data_extractor = self.data_extractor.as_ref().ok_or_else(|| {
            BatchError::ServiceNotAvailable("MatterDataExtractor not initialized".to_string())
        })?;
        let product_repo = self.product_repo.as_ref().ok_or_else(|| {
            BatchError::ServiceNotAvailable(
                "IntegratedProductRepository not initialized".to_string(),
            )
        })?;
        let app_config = self.app_config.as_ref().ok_or_else(|| {
            BatchError::ServiceNotAvailable("AppConfig not initialized".to_string())
        })?;

        // Template bridge disabled: always use StageActor path

        // canonical StageActor path below
        let deps = {
            let dup_policy = match std::env::var("MC_DUPLICATE_POLICY").ok().as_deref() {
                Some("UpdateIdIndexOnly") => crate::crawl_engine::actors::types::DuplicatePersistencePolicy::UpdateIdIndexOnly,
                Some("FullUpdate") => crate::crawl_engine::actors::types::DuplicatePersistencePolicy::FullUpdate,
                _ => crate::crawl_engine::actors::types::DuplicatePersistencePolicy::Skip,
            };
            crate::crawl_engine::actors::stage_actor::StageDeps {
                http_client: Arc::clone(http_client),
                data_extractor: Arc::clone(data_extractor),
                product_repo: Arc::clone(product_repo),
                app_config: app_config.clone(),
                duplicate_policy: dup_policy,
            }
        };
        let mut stage_actor = StageActor::new_with_deps(
            format!("stage_{}_{}", stage_type.as_str(), self.actor_id),
            self.batch_id.clone().unwrap_or_default(),
            deps,
            Arc::new(crate::crawl_engine::stages::DefaultStageLogicFactory),
        );

        if let (Some(tp), Some(plp)) = (total_pages_hint, products_on_last_page_hint) {
            stage_actor.set_site_pagination_hints(tp, plp);
        }

        // Use configurable operation timeout instead of hard-coded 30s
        let timeout_secs = app_config.user.crawling.timing.operation_timeout_seconds;
        let stage_result = stage_actor
            .execute_stage(stage_type, items, concurrency_limit, timeout_secs, context)
            .await
            .map_err(|e| {
                BatchError::StageExecutionFailed(format!("Stage execution failed: {e:?}"))
            })?;

        Ok(stage_result)
    }

    // Removed: Unused pipeline-oriented execute_stage method. The canonical path runs individual
    // stages via execute_stage_with_actor, driven by process_list_page_batch.

    /// Stage 출력을 다음 Stage 입력으로 변환
    fn transform_stage_output(
        &mut self,
        completed_stage: StageType,
        input_items: Vec<StageItem>,
        stage_result: &StageResult,
    ) -> Result<Vec<StageItem>, BatchError> {
        match completed_stage {
            StageType::StatusCheck => {
                // StatusCheck → ListPageCrawling: Page 아이템 그대로 전달
                info!(
                    "🔄 StatusCheck → ListPageCrawling: passing {} Page items",
                    input_items.len()
                );
                Ok(input_items)
            }
            StageType::ListPageCrawling => {
                // ListPageCrawling → ProductDetailCrawling: 실제 수집된 ProductUrls 사용
                info!(
                    "🔄 ListPageCrawling → ProductDetailCrawling: extracting ProductUrls from collected data"
                );

                let mut transformed_items = Vec::new();
                let mut total_urls_collected = 0;
                let mut total_urls_after_dedupe = 0;
                let enable_dedupe = self.skip_duplicate_urls;
                let mut total_duplicates_skipped = 0u32;

                for (item_index, item) in input_items.iter().enumerate() {
                    if let StageItem::Page(page_number) = item {
                        // stage_result에서 해당 페이지의 실행 결과 확인
                        if let Some(stage_item_result) = stage_result.details.get(item_index) {
                            if stage_item_result.success {
                                // 실제 수집된 데이터가 있는지 확인
                                if let Some(collected_data_json) = &stage_item_result.collected_data
                                {
                                    // JSON에서 ProductURL들을 파싱
                                    match serde_json::from_str::<
                                        Vec<crate::domain::product_url::ProductUrl>,
                                    >(collected_data_json)
                                    {
                                        Ok(product_urls_vec) => {
                                            if product_urls_vec.is_empty() {
                                                warn!(
                                                    "⚠️  Page {} crawling succeeded but no ProductURLs were collected",
                                                    page_number
                                                );
                                            } else {
                                                let original_count = product_urls_vec.len();
                                                total_urls_collected += original_count;
                                                let filtered_vec = if enable_dedupe {
                                                    let mut filtered =
                                                        Vec::with_capacity(original_count);
                                                    for pu in product_urls_vec {
                                                        let key = pu.url.clone();
                                                        if self.recent_product_set.contains(&key) {
                                                            total_duplicates_skipped += 1;
                                                        } else {
                                                            // LRU eviction if needed
                                                            if self.recent_product_urls.len()
                                                                >= self.recent_capacity
                                                            {
                                                                if let Some(old) = self
                                                                    .recent_product_urls
                                                                    .pop_front()
                                                                {
                                                                    self.recent_product_set
                                                                        .remove(&old);
                                                                }
                                                            }
                                                            self.recent_product_set
                                                                .insert(key.clone());
                                                            self.recent_product_urls.push_back(key);
                                                            filtered.push(pu);
                                                        }
                                                    }
                                                    filtered
                                                } else {
                                                    product_urls_vec
                                                };

                                                let after_count = filtered_vec.len();
                                                total_urls_after_dedupe += after_count;

                                                if after_count > 0 {
                                                    let product_urls = ProductUrls {
                                                        urls: filtered_vec,
                                                        batch_id: Some(self.actor_id.clone()),
                                                    };
                                                    transformed_items
                                                        .push(StageItem::ProductUrls(product_urls));
                                                }

                                                if enable_dedupe {
                                                    info!(
                                                        "✅ Extracted page {} URLs: original={} after_dedupe={} skipped={} recent_cache_size={} recent_set_size={}",
                                                        page_number,
                                                        original_count,
                                                        after_count,
                                                        original_count.saturating_sub(after_count),
                                                        self.recent_product_urls.len(),
                                                        self.recent_product_set.len()
                                                    );
                                                } else {
                                                    info!(
                                                        "✅ Extracted {} ProductURLs from page {} (dedupe disabled)",
                                                        original_count, page_number
                                                    );
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            warn!(
                                                "⚠️  Failed to parse collected data for page {}: {}",
                                                page_number, e
                                            );
                                            warn!(
                                                "⚠️  Raw collected data: {}",
                                                collected_data_json
                                            );
                                        }
                                    }
                                } else {
                                    warn!(
                                        "⚠️  Page {} succeeded but no collected data available",
                                        page_number
                                    );
                                }
                            } else {
                                warn!(
                                    "⚠️  Page {} failed in ListPageCrawling stage, skipping URL extraction",
                                    page_number
                                );
                            }
                        } else {
                            warn!(
                                "⚠️  No stage result found for page {} (item index {})",
                                page_number, item_index
                            );
                        }
                    }
                }

                if enable_dedupe {
                    info!(
                        "✅ Transformed {} Page items → {} ProductUrls items (total_urls_collected={} after_dedupe={} duplicates_skipped={} recent_cache_size={})",
                        input_items.len(),
                        transformed_items.len(),
                        total_urls_collected,
                        total_urls_after_dedupe,
                        total_duplicates_skipped,
                        self.recent_product_urls.len()
                    );
                } else {
                    info!(
                        "✅ Transformed {} Page items to {} ProductUrls items ({} total URLs)",
                        input_items.len(),
                        transformed_items.len(),
                        total_urls_collected
                    );
                }

                if transformed_items.is_empty() {
                    warn!(
                        "⚠️  No ProductURLs were extracted - all pages may have failed or returned no data"
                    );
                }
                // 누적 합산 후 저장
                self.duplicates_skipped = self
                    .duplicates_skipped
                    .saturating_add(total_duplicates_skipped);

                Ok(transformed_items)
            }
            StageType::ProductDetailCrawling => {
                // ProductDetailCrawling → DataValidation: collected_data에서 ProductDetails 추출
                info!(
                    "🔄 ProductDetailCrawling → DataValidation: extracting ProductDetails from collected data"
                );

                let mut transformed_items = Vec::new();
                let mut total_products_collected = 0;

                for (item_index, item) in input_items.iter().enumerate() {
                    let item_type_name = match item {
                        StageItem::Page(page) => format!("Page({page})"),
                        StageItem::Url(url) => format!("Url({url})"),
                        StageItem::Product(_) => "Product".to_string(),
                        StageItem::ValidationTarget(_) => "ValidationTarget".to_string(),
                        StageItem::ProductList(_) => "ProductList".to_string(),
                        StageItem::ProductUrls(urls) => {
                            format!("ProductUrls({} URLs)", urls.urls.len())
                        }
                        StageItem::ProductDetails(details) => {
                            format!("ProductDetails({} products)", details.products.len())
                        }
                        StageItem::ValidatedProducts(_) => "ValidatedProducts".to_string(),
                    };
                    info!(
                        "🔍 Checking item {} of type: {}",
                        item_index, item_type_name
                    );

                    if let StageItem::ProductUrls(_product_urls) = item {
                        // stage_result에서 해당 아이템의 실행 결과 확인
                        info!("🔍 Looking for stage result at index {}", item_index);
                        if let Some(stage_item_result) = stage_result.details.get(item_index) {
                            info!(
                                "🔍 Found stage result: success={}, collected_data_present={}",
                                stage_item_result.success,
                                stage_item_result.collected_data.is_some()
                            );
                            if stage_item_result.success {
                                // 실제 수집된 ProductDetails 데이터가 있는지 확인
                                if let Some(collected_data_json) = &stage_item_result.collected_data
                                {
                                    info!(
                                        "🔄 Attempting to parse ProductDetails JSON: {} chars",
                                        collected_data_json.len()
                                    );
                                    // JSON에서 ProductDetails를 파싱
                                    match serde_json::from_str::<
                                        crate::crawl_engine::channels::types::ProductDetails,
                                    >(collected_data_json)
                                    {
                                        Ok(product_details_wrapper) => {
                                            if product_details_wrapper.products.is_empty() {
                                                warn!(
                                                    "⚠️  ProductDetailCrawling succeeded but no ProductDetails were collected"
                                                );
                                            } else {
                                                let product_count =
                                                    product_details_wrapper.products.len();
                                                total_products_collected += product_count;
                                                transformed_items.push(StageItem::ProductDetails(
                                                    product_details_wrapper,
                                                ));
                                                info!(
                                                    "✅ Extracted {} ProductDetails from ProductUrls",
                                                    product_count
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            error!(
                                                "❌ Failed to parse ProductDetails from collected data: {}",
                                                e
                                            );
                                            error!(
                                                "📄 Raw collected data preview: {}",
                                                &collected_data_json
                                                    [..collected_data_json.len().min(200)]
                                            );
                                        }
                                    }
                                } else {
                                    warn!(
                                        "⚠️  ProductDetailCrawling succeeded but no collected data available for item {} -> synthesizing minimal ProductDetails (dev mode)",
                                        item_index
                                    );
                                    // Synthesize minimal ProductDetails wrapper using the original ProductUrls if accessible
                                    if let StageItem::ProductUrls(urls_wrapper) = item {
                                        if !urls_wrapper.urls.is_empty() {
                                            // Use current domain::product::ProductDetail definition
                                            let synth_count = 3.min(urls_wrapper.urls.len());
                                            let now = chrono::Utc::now();
                                            let synth_products: Vec<
                                                crate::domain::product::ProductDetail,
                                            > = urls_wrapper
                                                .urls
                                                .iter()
                                                .take(synth_count)
                                                .enumerate()
                                                .map(|(i, u)| {
                                                    crate::domain::product::ProductDetail {
                                                        url: u.url.clone(),
                                                        page_id: Some(u.page_id),
                                                        index_in_page: Some(u.index_in_page),
                                                        id: None,
                                                        manufacturer: Some(
                                                            "SynthManufacturer".into(),
                                                        ),
                                                        model: Some(format!("Model{i}")),
                                                        device_type: None,
                                                        certificate_id: None,
                                                        certification_date: None,
                                                        software_version: None,
                                                        hardware_version: None,
                                                        vid: None,
                                                        pid: None,
                                                        family_sku: None,
                                                        family_variant_sku: None,
                                                        firmware_version: None,
                                                        family_id: None,
                                                        tis_trp_tested: None,
                                                        specification_version: None,
                                                        transport_interface: None,
                                                        primary_device_type_id: None,
                                                        application_categories: None,
                                                        description: Some(
                                                            "Synthetic placeholder detail".into(),
                                                        ),
                                                        compliance_document_url: None,
                                                        program_type: Some("Synthetic".into()),
                                                        created_at: now,
                                                        updated_at: now,
                                                    }
                                                })
                                                .collect();
                                            let wrapper = crate::crawl_engine::channels::types::ProductDetails {
                                                products: synth_products,
                                                source_urls: urls_wrapper.urls.clone(),
                                                extraction_stats: crate::crawl_engine::channels::types::ExtractionStats {
                                                    attempted: urls_wrapper.urls.len() as u32,
                                                    successful: synth_count as u32,
                                                    failed: 0,
                                                    empty_responses: 0,
                                                },
                                            };
                                            total_products_collected += wrapper.products.len();
                                            transformed_items
                                                .push(StageItem::ProductDetails(wrapper));
                                            info!(
                                                "🧪 Synthesized {} ProductDetails (dev fallback)",
                                                synth_count
                                            );
                                        }
                                    }
                                }
                            } else {
                                warn!(
                                    "⚠️  ProductUrls failed in ProductDetailCrawling stage, skipping item {}",
                                    item_index
                                );
                            }
                        } else {
                            warn!(
                                "⚠️  No stage result found for ProductUrls item (item index {})",
                                item_index
                            );
                        }
                    } else {
                        info!("🔍 Skipping non-ProductUrls item at index {}", item_index);
                    }
                }

                info!(
                    "✅ Transformed {} ProductUrls items to {} ProductDetails items ({} total products)",
                    input_items.len(),
                    transformed_items.len(),
                    total_products_collected
                );

                if transformed_items.is_empty() {
                    warn!("⚠️  No ProductDetails were extracted - all ProductUrls may have failed");
                }

                Ok(transformed_items)
            }
            StageType::DataValidation => {
                // DataValidation → DataSaving: ProductDetails 아이템 그대로 전달
                let item_count = input_items.len();
                info!(
                    "🔄 DataValidation → DataSaving: passing {} ProductDetails items",
                    item_count
                );
                Ok(input_items)
            }
            StageType::DataSaving => {
                // DataSaving은 마지막 단계이므로 변환 불필요
                info!("🔄 DataSaving completed - pipeline finished");
                Ok(input_items)
            }
        }
    }
}

#[cfg(test)]
mod batch_actor_metrics_tests {
    use super::*;
    use crate::crawl_engine::actors::{StageItemResult, StageItemType, StageResult};

    fn mk_item(duration_ms: u64, retry_count: u32, success: bool) -> StageItemResult {
        StageItemResult {
            item_id: "t".into(),
            item_type: StageItemType::SiteCheck,
            success,
            error: None,
            duration_ms,
            retry_count,
            collected_data: None,
        }
    }

    fn mk_result(items: &[(u64, u32, bool)]) -> StageResult {
        StageResult {
            processed_items: items.len() as u32,
            successful_items: items.iter().filter(|(_, _, s)| *s).count() as u32,
            failed_items: items.iter().filter(|(_, _, s)| !*s).count() as u32,
            duration_ms: items.iter().map(|(d, _, _)| *d).sum(),
            details: items.iter().map(|(d, r, s)| mk_item(*d, *r, *s)).collect(),
        }
    }

    #[test]
    fn test_compute_stage_duration_sums_no_defer() {
        let list_res = mk_result(&[(10, 1, true), (20, 0, false)]);
        let det_res = mk_result(&[(5, 2, true), (7, 0, true)]);
        let (s2, s3) = BatchActor::compute_stage_duration_sums(&list_res, Some(&det_res), false);
        assert_eq!(s2, 30);
        assert_eq!(s3, 12);
    }

    #[test]
    fn test_compute_stage_duration_sums_defer() {
        let list_res = mk_result(&[(10, 1, true), (20, 0, false)]);
        let (s2, s3) = BatchActor::compute_stage_duration_sums(&list_res, None, true);
        assert_eq!(s2, 30);
        assert_eq!(s3, 0);
    }

    #[test]
    fn test_compute_retries_used_no_defer() {
        let list_res = mk_result(&[(10, 1, true), (20, 0, false)]); // sum=1
        let det_res = mk_result(&[(5, 2, true), (7, 3, true)]); // sum=5
        let retries = BatchActor::compute_retries_used(&list_res, Some(&det_res), false);
        assert_eq!(retries, 6);
    }

    #[test]
    fn test_compute_retries_used_defer() {
        let list_res = mk_result(&[(10, 1, true), (20, 2, false)]); // sum=3
        let retries = BatchActor::compute_retries_used(&list_res, None, true);
        assert_eq!(retries, 3);
    }
}
