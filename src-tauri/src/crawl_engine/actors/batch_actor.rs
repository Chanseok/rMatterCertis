// Removed an unused pipeline-oriented execute_stage variant in BatchActor; the active flow uses StageActor::execute_stage.

use chrono::Utc;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Semaphore};
use tracing::{debug, error, info, warn};

use crate::crawl_engine::actors::traits::{Actor, ActorHealth, ActorStatus, ActorType};
use crate::crawl_engine::actors::types::{ActorCommand, ActorError, StageResult, StageType};
use crate::crawl_engine::channels::types::{ProductUrls, StageItem};
use crate::crawl_engine::integrated_context::AppContext;
use crate::infrastructure::config::AppConfig;
use crate::infrastructure::{HttpClient, IntegratedProductRepository, MatterDataExtractor};
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
#[deprecated(since = "0.2.0", note = "BatchActor will be phased out; prefer SessionActor + StageActor with StageBatcher policy helper.")]
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

    // Behavior flags
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
    /// Configure whether to skip duplicate product URLs within this batch.
    pub const fn set_skip_duplicate_urls(&mut self, flag: bool) {
        self.skip_duplicate_urls = flag;
    }

    /// 내부 보조: Stage 2/3 per-item duration 합계 산출
    #[cfg(test)]
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
    #[cfg(test)]
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
            config: Some(Arc::new(
                crate::crawl_engine::config::SystemConfig::default(),
            )),
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
            defer_detail_crawling: std::env::var("MC_UNIFIED_DETAIL")
                .map(|v| {
                    let t = v.trim();
                    !(t.eq("0") || t.eq_ignore_ascii_case("false"))
                })
                .unwrap_or(false),
        }
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
                                // Inline minimal batch processing flow using StageActor helpers
                                let list_items: Vec<StageItem> = pages.into_iter().map(StageItem::Page).collect();
                                // StatusCheck stage
                                let status_items = list_items.clone();
                                let status_res = self.execute_stage_with_actor(
                                    StageType::StatusCheck,
                                    status_items,
                                    concurrency_limit,
                                    &context,
                                ).await.map_err(|e| ActorError::CommandProcessingFailed(format!("status stage failed: {e}")))?;

                                // Transform and run ListPageCrawling with hints
                                let list_stage_input = self.transform_stage_output(StageType::StatusCheck, list_items.clone(), &status_res)
                                    .map_err(|e| ActorError::CommandProcessingFailed(format!("transform after status failed: {e}")))?;
                                let list_input_for_exec = list_stage_input.clone();
                                let list_res = self.execute_stage_with_actor_with_hints(
                                    StageType::ListPageCrawling,
                                    list_input_for_exec,
                                    concurrency_limit,
                                    &context,
                                    Some(total_pages),
                                    Some(products_on_last_page),
                                ).await.map_err(|e| ActorError::CommandProcessingFailed(format!("list page stage failed: {e}")))?;

                                // Transform to ProductDetailCrawling input
                                let detail_input = self.transform_stage_output(StageType::ListPageCrawling, list_stage_input.clone(), &list_res)
                                    .map_err(|e| ActorError::CommandProcessingFailed(format!("transform after list failed: {e}")))?;
                                let detail_input_for_exec = detail_input.clone();
                                // Optionally defer detail crawling based on flag
                                if !self.defer_detail_crawling {
                                    let detail_res = self.execute_stage_with_actor(
                                        StageType::ProductDetailCrawling,
                                        detail_input_for_exec,
                                        concurrency_limit,
                                        &context,
                                    ).await.map_err(|e| ActorError::CommandProcessingFailed(format!("detail stage failed: {e}")))?;

                                    // Transform to DataValidation input
                                    let validation_input = self.transform_stage_output(StageType::ProductDetailCrawling, detail_input.clone(), &detail_res)
                                        .map_err(|e| ActorError::CommandProcessingFailed(format!("transform after detail failed: {e}")))?;
                                    let validation_input_for_exec = validation_input.clone();
                                    // DataValidation
                                    let validation_res = self.execute_stage_with_actor(
                                        StageType::DataValidation,
                                        validation_input_for_exec,
                                        concurrency_limit,
                                        &context,
                                    ).await.map_err(|e| ActorError::CommandProcessingFailed(format!("validation stage failed: {e}")))?;

                                    // Transform to DataSaving input
                                    let saving_input = self.transform_stage_output(StageType::DataValidation, validation_input.clone(), &validation_res)
                                        .map_err(|e| ActorError::CommandProcessingFailed(format!("transform after validation failed: {e}")))?;

                                    // DataSaving
                                    let _saving_res = self.execute_stage_with_actor(
                                        StageType::DataSaving,
                                        saving_input,
                                        concurrency_limit,
                                        &context,
                                    ).await.map_err(|e| ActorError::CommandProcessingFailed(format!("saving stage failed: {e}")))?;
                                }

                                // Done with inline processing
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
        // 설정 기반 정책 배처 주입 (현재는 pass-through)
        {
            use crate::crawl_engine::actors::stage_batcher::ConfigurableStageBatcher;
            let cfg = context.config.performance.stage_batcher.clone();
            let batcher = std::sync::Arc::new(ConfigurableStageBatcher::from_settings(cfg));
            stage_actor.set_batcher(batcher);
        }

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
        // 설정 기반 정책 배처 주입 (현재는 pass-through)
        {
            use crate::crawl_engine::actors::stage_batcher::ConfigurableStageBatcher;
            let cfg = context.config.performance.stage_batcher.clone();
            let batcher = std::sync::Arc::new(ConfigurableStageBatcher::from_settings(cfg));
            stage_actor.set_batcher(batcher);
        }

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
                                if let Some(collected) = &stage_item_result.collected_data {
                                    // Typed path: extract directly from StageResultData
                                    let parsed_urls: Option<Vec<crate::domain::product_url::ProductUrl>> = match collected {
                                        crate::crawl_engine::actors::types::StageResultData::ProductUrls { urls, .. } => Some(urls.clone()),
                                        _ => None,
                                    };

                                    if let Some(product_urls_vec) = parsed_urls {
                                        if product_urls_vec.is_empty() {
                                            warn!(
                                                "⚠️  Page {} crawling succeeded but no ProductURLs were collected",
                                                page_number
                                            );
                                        } else {
                                            let original_count = product_urls_vec.len();
                                            total_urls_collected += original_count;
                                            let filtered_vec = if enable_dedupe {
                                                let mut filtered = Vec::with_capacity(original_count);
                                                for pu in product_urls_vec {
                                                    let key = pu.url.clone();
                                                    if self.recent_product_set.contains(&key) {
                                                        total_duplicates_skipped += 1;
                                                    } else {
                                                        // LRU eviction if needed
                                                        if self.recent_product_urls.len() >= self.recent_capacity {
                                                            if let Some(old) = self.recent_product_urls.pop_front() {
                                                                self.recent_product_set.remove(&old);
                                                            }
                                                        }
                                                        self.recent_product_set.insert(key.clone());
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
                                                let product_urls = ProductUrls { urls: filtered_vec, batch_id: Some(self.actor_id.clone()) };
                                                transformed_items.push(StageItem::ProductUrls(product_urls));
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
                                    } else {
                                        warn!(
                                            "⚠️  Could not use collected_data for page {} as ProductUrls (unexpected variant)",
                                            page_number
                                        );
                                        warn!("⚠️  Raw collected data: {:?}", collected);
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
                                if let Some(collected) = &stage_item_result.collected_data {
                                    match collected {
                                        crate::crawl_engine::actors::types::StageResultData::ProductDetails { details, successful_count, failed_count: _ } => {
                                            if details.is_empty() {
                                                warn!("⚠️  ProductDetailCrawling succeeded but no typed ProductDetails were collected");
                                            } else {
                                                let product_count = details.len();
                                                total_products_collected += product_count;
                                                let wrapper = crate::crawl_engine::channels::types::ProductDetails {
                                                products: details.clone(),
                                                    // typed 경로에서는 source_urls를 보존하지 못할 수 있음
                                                    source_urls: vec![],
                                                    extraction_stats: crate::crawl_engine::channels::types::ExtractionStats {
                                                        attempted: *successful_count, // best-effort
                                                        successful: *successful_count,
                                                        failed: 0,
                                                        empty_responses: 0,
                                                    },
                                                };
                                                transformed_items.push(StageItem::ProductDetails(wrapper));
                                                info!("✅ Parsed typed ProductDetails: {} products", product_count);
                                            }
                                        }
                                        other => {
                                            warn!(
                                                "⚠️  Unexpected collected_data variant for ProductDetailCrawling item {}: {:?}",
                                                item_index, other
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
            collected_data: Some(crate::crawl_engine::actors::types::StageResultData::Empty),
        }
    }

    fn mk_result(items: &[(u64, u32, bool)]) -> StageResult {
        StageResult {
            processed_items: items.len() as u32,
            successful_items: items.iter().filter(|(_, _, s)| *s).count() as u32,
            failed_items: items.iter().filter(|(_, _, s)| !*s).count() as u32,
            duration_ms: items.iter().map(|(d, _, _)| *d).sum(),
            // StageResult.details is typed in Phase 2
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
