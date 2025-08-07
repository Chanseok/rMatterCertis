//! BatchActor: 배치 단위 크롤링 처리 Actor
//! 
//! Phase 3: Actor 구현 - 배치 레벨 작업 관리 및 실행
//! Modern Rust 2024 준수: 함수형 원칙, 명시적 의존성, 상태 최소화

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Semaphore};
use tracing::{info, warn, error, debug};
use chrono::Utc;

use super::traits::{Actor, ActorHealth, ActorStatus, ActorType};
use super::types::{ActorCommand, BatchConfig, StageType, StageResult, ActorError};
use crate::new_architecture::channels::types::{AppEvent, StageItem, ProductUrls};  // enum 버전의 StageItem과 ProductUrls 사용
use crate::new_architecture::context::AppContext;
use crate::new_architecture::actors::StageActor;

// 실제 서비스 imports 추가
use crate::infrastructure::{HttpClient, MatterDataExtractor, IntegratedProductRepository};
use crate::infrastructure::config::AppConfig;

/// BatchActor: 배치 단위의 크롤링 작업 관리
/// 
/// 책임:
/// - 배치 내 페이지들의 병렬 처리 관리
/// - StageActor들의 조정 및 스케줄링
/// - 배치 레벨 이벤트 발행
/// - 동시성 제어 및 리소스 관리
pub struct BatchActor {
    /// Actor 고유 식별자
    actor_id: String,
    /// 현재 처리 중인 배치 ID (OneShot 호환성)
    pub batch_id: Option<String>,
    /// 배치 상태
    state: BatchState,
    /// 배치 시작 시간
    start_time: Option<Instant>,
    /// 총 페이지 수
    total_pages: u32,
    /// 처리 완료된 페이지 수
    completed_pages: u32,
    /// 성공한 아이템 수
    success_count: u32,
    /// 실패한 아이템 수
    failure_count: u32,
    /// 동시성 제어용 세마포어
    concurrency_limiter: Option<Arc<Semaphore>>,
    /// 설정 (OneShot 호환성)
    pub config: Option<Arc<crate::new_architecture::config::SystemConfig>>,
    
    // 🔥 Phase 1: 실제 서비스 의존성 추가
    /// HTTP 클라이언트
    http_client: Option<Arc<HttpClient>>,
    /// 데이터 추출기
    data_extractor: Option<Arc<MatterDataExtractor>>,
    /// 제품 레포지토리
    product_repo: Option<Arc<IntegratedProductRepository>>,
    /// 앱 설정
    app_config: Option<AppConfig>,
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
            .finish()
    }
}

/// 배치 상태 열거형
#[derive(Debug, Clone, PartialEq)]
pub enum BatchState {
    Idle,
    Starting,
    Processing,
    Paused,
    Completing,
    Completed,
    Failed { error: String },
}

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
    /// 새로운 BatchActor 인스턴스 생성 (기본)
    /// 
    /// # Arguments
    /// * `actor_id` - Actor 고유 식별자
    /// 
    /// # Returns
    /// * `Self` - 새로운 BatchActor 인스턴스
    pub fn new(actor_id: String) -> Self {
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
            // 새로 추가된 필드들 초기화
            http_client: None,
            data_extractor: None,
            product_repo: None,
            app_config: None,
        }
    }
    
    /// 🔥 Phase 1: 실제 서비스들과 함께 BatchActor 생성
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
    /// * `Self` - 서비스가 주입된 BatchActor 인스턴스
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
            config: None,
            // 실제 서비스 의존성 주입
            http_client: Some(http_client),
            data_extractor: Some(data_extractor),
            product_repo: Some(product_repo),
            app_config: Some(app_config),
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
        stage_type: StageType,
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
        
        info!("🔄 BatchActor {} starting batch {} with {} pages", 
              self.actor_id, batch_id, pages.len());
        
        // 상태 초기화
        self.batch_id = Some(batch_id.clone());
        self.state = BatchState::Starting;
        self.start_time = Some(Instant::now());
        self.total_pages = pages.len() as u32;
        self.completed_pages = 0;
        self.success_count = 0;
        self.failure_count = 0;
        
        // 동시성 제어 설정
        self.concurrency_limiter = Some(Arc::new(Semaphore::new(concurrency_limit as usize)));
        
        // 배치 시작 이벤트 발행
        let start_event = AppEvent::BatchStarted {
            batch_id: batch_id.clone(),
            session_id: context.session_id.clone(),
            pages_count: pages.len() as u32,
            timestamp: Utc::now(),
        };
        
        context.emit_event(start_event).await
            .map_err(|e| BatchError::ContextError(e.to_string()))?;
        
        // 상태를 Processing으로 전환
        self.state = BatchState::Processing;
        
        // 실제 StageActor 기반 처리 구현
        info!("🎭 Using real StageActor-based processing for batch {}", batch_id);
        
        // 초기 Stage Items 생성 - 페이지 기반 아이템들
        let initial_items: Vec<StageItem> = pages.iter().map(|&page_number| {
            StageItem::Page(page_number)
        }).collect();

        // Stage 1: StatusCheck - 사이트 상태 확인
        info!("🔍 Starting Stage 1: StatusCheck");
        // StatusCheck는 사이트 전체 상태를 확인하므로 특별한 URL 아이템으로 처리
        let status_check_items = vec![StageItem::Url("https://csa-iot.org/csa-iot_products/".to_string())];
        
        let status_check_result = self.execute_stage_with_actor(
            StageType::StatusCheck, 
            status_check_items, 
            concurrency_limit, 
            context
        ).await?;
        
        info!("✅ Stage 1 (StatusCheck) completed: {} success, {} failed", 
              status_check_result.successful_items, status_check_result.failed_items);

        // StatusCheck 스테이지는 사이트 접근성 확인이므로 특별 처리
        // 성공적으로 완료되었다면 (처리된 아이템이 있다면) 다음 단계로 진행
        if status_check_result.processed_items == 0 {
            error!("❌ Stage 1 (StatusCheck) failed completely - no status check performed");
            self.state = BatchState::Failed { error: "StatusCheck stage failed - no status check performed".to_string() };
            return Err(BatchError::StageExecutionFailed("StatusCheck stage failed - no status check performed".to_string()));
        }
        
        // StatusCheck에서 사이트 접근 불가능한 경우에만 중단
        if status_check_result.failed_items > 0 && status_check_result.successful_items == 0 {
            error!("❌ Stage 1 (StatusCheck) failed completely - site is not accessible");
            self.state = BatchState::Failed { error: "StatusCheck stage failed - site is not accessible".to_string() };
            return Err(BatchError::StageExecutionFailed("StatusCheck stage failed - site is not accessible".to_string()));
        }

        // Stage 2: ListPageCrawling - ProductURL 수집
        info!("🔍 Starting Stage 2: ListPageCrawling");
        let list_page_result = self.execute_stage_with_actor(
            StageType::ListPageCrawling, 
            initial_items.clone(), 
            concurrency_limit, 
            context
        ).await?;
        
        info!("✅ Stage 2 (ListPageCrawling) completed: {} success, {} failed", 
              list_page_result.successful_items, list_page_result.failed_items);

        // Stage 실패 시 파이프라인 중단 검증
        if list_page_result.successful_items == 0 {
            error!("❌ Stage 2 (ListPageCrawling) failed completely - aborting pipeline");
            self.state = BatchState::Failed { error: "ListPageCrawling stage failed completely".to_string() };
            return Err(BatchError::StageExecutionFailed("ListPageCrawling stage failed completely".to_string()));
        }

        // Stage 2 결과를 Stage 3 입력으로 변환
        let product_detail_items = self.transform_stage_output(
            StageType::ListPageCrawling,
            initial_items.clone(),
            &list_page_result
        ).await?;

        // Stage 3: ProductDetailCrawling - 상품 상세 정보 수집
        info!("🔍 Starting Stage 3: ProductDetailCrawling");
        let detail_result = self.execute_stage_with_actor(
            StageType::ProductDetailCrawling, 
            product_detail_items, 
            concurrency_limit, 
            context
        ).await?;
        
        info!("✅ Stage 3 (ProductDetailCrawling) completed: {} success, {} failed", 
              detail_result.successful_items, detail_result.failed_items);

        // Stage 3 결과를 Stage 4 입력으로 변환 
        let data_validation_items = self.transform_stage_output(
            StageType::ProductDetailCrawling,
            initial_items.clone(),
            &detail_result
        ).await?;

        // Stage 4: DataValidation - 데이터 품질 분석
        info!("🔍 Starting Stage 4: DataValidation");
        let validation_result = self.execute_stage_with_actor(
            StageType::DataValidation, 
            data_validation_items.clone(), 
            concurrency_limit, 
            context
        ).await?;
        
        info!("✅ Stage 4 (DataValidation) completed: {} success, {} failed", 
              validation_result.successful_items, validation_result.failed_items);

        // Stage 4 결과를 Stage 5 입력으로 변환 
        let data_saving_items = self.transform_stage_output(
            StageType::DataValidation,
            data_validation_items,
            &validation_result
        ).await?;

        // Stage 5: DataSaving - 데이터 저장
        info!("🔍 Starting Stage 5: DataSaving");
        let saving_result = self.execute_stage_with_actor(
            StageType::DataSaving, 
            data_saving_items, 
            concurrency_limit, 
            context
        ).await?;
        
        info!("✅ Stage 5 (DataSaving) completed: {} success, {} failed", 
              saving_result.successful_items, saving_result.failed_items);

        // 배치 결과 집계
        self.success_count = saving_result.successful_items;
        self.completed_pages = pages.len() as u32;
        self.state = BatchState::Completed;
        
        let completion_event = AppEvent::BatchCompleted {
            batch_id: batch_id.clone(),
            session_id: context.session_id.clone(),
            success_count: self.success_count,
            failed_count: saving_result.failed_items,
            duration: self.start_time.map(|s| s.elapsed().as_millis() as u64).unwrap_or(0),
            timestamp: Utc::now(),
        };
        
        context.emit_event(completion_event).await
            .map_err(|e| BatchError::ContextError(e.to_string()))?;
        
        Ok(())
    }
    
    /// 배치 설정 검증
    /// 
    /// # Arguments
    /// * `config` - 배치 설정
    /// * `concurrency_limit` - 동시성 제한
    fn validate_batch_config(&self, config: &BatchConfig, concurrency_limit: u32) -> Result<(), BatchError> {
        if config.batch_size == 0 {
            return Err(BatchError::InvalidConfiguration("Batch size cannot be zero".to_string()));
        }
        
        if concurrency_limit == 0 {
            return Err(BatchError::InvalidConfiguration("Concurrency limit cannot be zero".to_string()));
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
    
    /// 배치 ID 검증
    /// 
    /// # Arguments
    /// * `batch_id` - 검증할 배치 ID
    #[allow(dead_code)]
    fn validate_batch(&self, batch_id: &str) -> Result<(), BatchError> {
        match &self.batch_id {
            Some(current_id) if current_id == batch_id => Ok(()),
            Some(current_id) => Err(BatchError::BatchNotFound(format!(
                "Expected {}, got {}", current_id, batch_id
            ))),
            None => Err(BatchError::BatchNotFound("No active batch".to_string())),
        }
    }
    
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
    }    async fn run(
        &mut self,
        mut context: AppContext,
        mut command_rx: mpsc::Receiver<Self::Command>,
    ) -> Result<(), Self::Error> {
        info!("🔄 BatchActor {} starting execution loop", self.actor_id);
        
        loop {
            tokio::select! {
                // 명령 처리
                command = command_rx.recv() => {
                    match command {
                        Some(cmd) => {
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
                                    if let Err(e) = self.handle_process_batch(
                                        batch_id, 
                                        pages, 
                                        config, 
                                        batch_size, 
                                        concurrency_limit, 
                                        total_pages, 
                                        products_on_last_page, 
                                        &context
                                    ).await {
                                        error!("Failed to process batch: {}", e);
                                    }
                                }
                                
                                ActorCommand::Shutdown => {
                                    info!("🛑 BatchActor {} received shutdown command", self.actor_id);
                                    break;
                                }
                                
                                _ => {
                                    debug!("BatchActor {} ignoring non-batch command", self.actor_id);
                                }
                            }
                        }
                        None => {
                            warn!("📪 BatchActor {} command channel closed", self.actor_id);
                            break;
                        }
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
            errors_count: 0, // TODO: 실제 에러 수 계산
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
            }).to_string(),
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
    /// 개별 Stage를 StageActor로 실행
    /// 
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
        let http_client = self.http_client.as_ref()
            .ok_or_else(|| BatchError::ServiceNotAvailable("HttpClient not initialized".to_string()))?;
        let data_extractor = self.data_extractor.as_ref()
            .ok_or_else(|| BatchError::ServiceNotAvailable("MatterDataExtractor not initialized".to_string()))?;
        let product_repo = self.product_repo.as_ref()
            .ok_or_else(|| BatchError::ServiceNotAvailable("IntegratedProductRepository not initialized".to_string()))?;
        let app_config = self.app_config.as_ref()
            .ok_or_else(|| BatchError::ServiceNotAvailable("AppConfig not initialized".to_string()))?;

        // StageActor 생성 (실제 서비스들과 함께)
        let mut stage_actor = StageActor::new_with_services(
            format!("stage_{}_{}", stage_type.as_str(), self.actor_id),
            self.batch_id.clone().unwrap_or_default(),
            Arc::clone(http_client),
            Arc::clone(data_extractor),
            Arc::clone(product_repo),
            app_config.clone(),
        );

        // StageActor로 Stage 실행 (실제 items 전달)
        let stage_result = stage_actor.execute_stage(
            stage_type,
            items,
            concurrency_limit,
            30, // timeout_secs - 30초 타임아웃
            context,
        ).await
        .map_err(|e| BatchError::StageExecutionFailed(format!("Stage execution failed: {}", e)))?;

        Ok(stage_result)
    }

    /// Stage 파이프라인 실행 - Stage 간 데이터 전달 구현
    /// 
    /// # Arguments
    /// * `stage_type` - 실행할 스테이지 타입 (현재는 사용하지 않음 - 순차 실행)
    /// * `pages` - 처리할 페이지들
    /// * `context` - Actor 컨텍스트
    #[allow(dead_code)]
    async fn execute_stage(
        &mut self,
        _stage_type: StageType, // 파이프라인에서는 모든 Stage 순차 실행
        pages: Vec<u32>,
        context: &AppContext,
    ) -> Result<StageResult, BatchError> {
        use crate::new_architecture::actors::StageActor;
        use crate::new_architecture::channels::types::StageItem;
        
        info!("� Starting Stage pipeline processing for {} pages", pages.len());
        
        // Stage 실행 순서 정의
        let stages = vec![
            StageType::StatusCheck,
            StageType::ListPageCrawling, 
            StageType::ProductDetailCrawling,
            StageType::DataSaving,
        ];
        
        // 초기 입력: 페이지들을 StageItem으로 변환
        let mut current_items: Vec<StageItem> = pages.into_iter().map(|page| {
            StageItem::Page(page)
        }).collect();
        
        let mut final_result = StageResult {
            processed_items: 0,
            successful_items: 0, 
            failed_items: 0,
            duration_ms: 0,
            details: vec![],
        };
        
        // Stage 파이프라인 실행
        for (stage_idx, stage_type) in stages.iter().enumerate() {
            info!("🎯 Executing stage {} for {} items", stage_type.as_str(), current_items.len());
            
            // 🔥 Phase 1: 실제 서비스와 함께 StageActor 생성
            let mut stage_actor = if let (Some(http_client), Some(data_extractor), Some(product_repo), Some(app_config)) = 
                (&self.http_client, &self.data_extractor, &self.product_repo, &self.app_config) {
                
                info!("✅ Creating StageActor with real services");
                StageActor::new_with_services(
                    format!("stage_{}_{}", stage_type.as_str().to_lowercase(), self.actor_id),
                    self.batch_id.clone().unwrap_or_default(),
                    Arc::clone(http_client),
                    Arc::clone(data_extractor),
                    Arc::clone(product_repo),
                    app_config.clone(),
                )
            } else {
                warn!("⚠️  Creating StageActor without services - falling back to basic initialization");
                let mut stage_actor = StageActor::new(
                    format!("stage_{}_{}", stage_type.as_str().to_lowercase(), self.actor_id),
                );
                
                // 기존 방식으로 서비스 초기화 시도
                stage_actor.initialize_real_services(context).await
                    .map_err(|e| BatchError::StageProcessingFailed { 
                        stage: stage_type.as_str().to_string(), 
                        error: format!("Failed to initialize real services: {}", e) 
                    })?;
                    
                stage_actor
            };
            
            // Stage 실행 (실제 current_items 전달)
            let concurrency_limit = 5; // TODO: 설정 파일에서 가져와야 함
            let timeout_secs = 300;
            
            let stage_result = stage_actor.execute_stage(
                stage_type.clone(),
                current_items.clone(),
                concurrency_limit,
                timeout_secs,
                context,
            ).await.map_err(|e| BatchError::StageProcessingFailed { 
                stage: stage_type.as_str().to_string(),
                error: format!("Stage execution failed: {:?}", e)
            })?;
            
            info!("✅ Stage {} ({}) completed: {} success, {} failed", 
                  stage_idx + 1, stage_type.as_str(), stage_result.successful_items, stage_result.failed_items);
            
            // 최종 결과 누적
            final_result.processed_items += stage_result.processed_items;
            final_result.successful_items += stage_result.successful_items;
            final_result.failed_items += stage_result.failed_items;
            final_result.duration_ms += stage_result.duration_ms;
            
            // 다음 Stage를 위한 입력 데이터 변환
            current_items = self.transform_stage_output(stage_type.clone(), current_items, &stage_result).await?;
        }
        
        info!("✅ All stages completed in pipeline");
        Ok(final_result)
    }
    
    /// Stage 출력을 다음 Stage 입력으로 변환
    async fn transform_stage_output(
        &self, 
        completed_stage: StageType, 
        input_items: Vec<StageItem>,
        stage_result: &StageResult
    ) -> Result<Vec<StageItem>, BatchError> {
        match completed_stage {
            StageType::StatusCheck => {
                // StatusCheck → ListPageCrawling: Page 아이템 그대로 전달
                info!("🔄 StatusCheck → ListPageCrawling: passing {} Page items", input_items.len());
                Ok(input_items)
            }
            StageType::ListPageCrawling => {
                // ListPageCrawling → ProductDetailCrawling: 실제 수집된 ProductUrls 사용
                info!("🔄 ListPageCrawling → ProductDetailCrawling: extracting ProductUrls from collected data");
                
                let mut transformed_items = Vec::new();
                let mut total_urls_collected = 0;
                
                for (item_index, item) in input_items.iter().enumerate() {
                    if let StageItem::Page(page_number) = item {
                        // stage_result에서 해당 페이지의 실행 결과 확인
                        if let Some(stage_item_result) = stage_result.details.get(item_index) {
                            if stage_item_result.success {
                                // 실제 수집된 데이터가 있는지 확인
                                if let Some(collected_data_json) = &stage_item_result.collected_data {
                                    // JSON에서 ProductURL들을 파싱
                                    match serde_json::from_str::<Vec<crate::domain::product_url::ProductUrl>>(collected_data_json) {
                                        Ok(product_urls) => {
                                            // ProductURL들을 String으로 변환
                                            let url_strings: Vec<String> = product_urls.iter()
                                                .map(|product_url| product_url.url.clone())
                                                .collect();
                                            
                                            if !url_strings.is_empty() {
                                                total_urls_collected += url_strings.len();
                                                
                                                let product_urls = ProductUrls {
                                                    urls: url_strings.clone(),
                                                    batch_id: Some(self.actor_id.clone()),
                                                };
                                                
                                                transformed_items.push(StageItem::ProductUrls(product_urls));
                                                
                                                info!("✅ Extracted {} ProductURLs from page {}", url_strings.len(), page_number);
                                            } else {
                                                warn!("⚠️  Page {} crawling succeeded but no ProductURLs were collected", page_number);
                                            }
                                        }
                                        Err(e) => {
                                            warn!("⚠️  Failed to parse collected data for page {}: {}", page_number, e);
                                            warn!("⚠️  Raw collected data: {}", collected_data_json);
                                        }
                                    }
                                } else {
                                    warn!("⚠️  Page {} succeeded but no collected data available", page_number);
                                }
                            } else {
                                warn!("⚠️  Page {} failed in ListPageCrawling stage, skipping URL extraction", page_number);
                            }
                        } else {
                            warn!("⚠️  No stage result found for page {} (item index {})", page_number, item_index);
                        }
                    }
                }
                
                info!("✅ Transformed {} Page items to {} ProductUrls items ({} total URLs)", 
                      input_items.len(), transformed_items.len(), total_urls_collected);
                
                if transformed_items.is_empty() {
                    warn!("⚠️  No ProductURLs were extracted - all pages may have failed or returned no data");
                }
                
                Ok(transformed_items)
            }
            StageType::ProductDetailCrawling => {
                // ProductDetailCrawling → DataSaving: ProductUrls 아이템 그대로 전달
                // (실제로는 ProductDetail 데이터로 변환되어야 하지만, 현재는 간소화)
                let item_count = input_items.len();
                info!("🔄 ProductDetailCrawling → DataSaving: passing {} ProductUrls items", item_count);
                Ok(input_items)
            }
            StageType::DataSaving => {
                // DataSaving은 마지막 단계이므로 변환 불필요
                info!("🔄 DataSaving completed - pipeline finished");
                Ok(input_items)
            }
            _ => Ok(input_items)
        }
    }
}
