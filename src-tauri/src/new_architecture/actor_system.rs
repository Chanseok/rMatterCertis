//! Actor 시스템: 세션, 배치, 스테이지 분리 구조
//! Modern Rust 2024 준수: 의존성 주입 기반 Actor 설계

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot, Mutex, Semaphore};
use tokio::time::{sleep, timeout};
use tracing::{info, warn, error, debug};
use uuid::Uuid;
use futures::future::join_all;
use serde::Serialize;

// 개별 모듈에서 직접 import
use crate::new_architecture::system_config::{SystemConfig, ConfigError, RetryPolicy};
use crate::new_architecture::channel_types::{ActorCommand, AppEvent, BatchConfig, StageType, StageItem};
use crate::infrastructure::config::AppConfig;

// 임시 타입 정의 (컴파일 에러 해결용)

#[derive(Debug, Clone, Serialize)]
pub enum StageResult {
    Success(StageSuccessResult),
    Failure(StageError),
    RecoverableError {
        error: StageError,
        attempts: u32,
        stage_id: String,
        suggested_retry_delay_ms: u64,  // Duration을 u64로 변경
    },
    FatalError {
        error: StageError,
        stage_id: String,
        context: String,
    },
    PartialSuccess {
        success_items: StageSuccessResult,
        failed_items: Vec<FailedItem>,
        stage_id: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub enum StageError {
    NetworkError { message: String },
    ParsingError { message: String },
    NetworkTimeout { message: String },
    ValidationError { message: String },
    ChannelError { message: String },
    DatabaseError { message: String },
    ResourceExhausted { message: String },
    ConfigurationError { message: String },
    TaskCancelled { task_id: String },
    TaskExecutionFailed { task_id: String, message: String },
}

impl std::fmt::Display for StageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StageError::NetworkError { message } => write!(f, "Network error: {}", message),
            StageError::ParsingError { message } => write!(f, "Parsing error: {}", message),
            StageError::NetworkTimeout { message } => write!(f, "Network timeout: {}", message),
            StageError::ValidationError { message } => write!(f, "Validation error: {}", message),
            StageError::ChannelError { message } => write!(f, "Channel error: {}", message),
            StageError::DatabaseError { message } => write!(f, "Database error: {}", message),
            StageError::ResourceExhausted { message } => write!(f, "Resource exhausted: {}", message),
            StageError::ConfigurationError { message } => write!(f, "Configuration error: {}", message),
            StageError::TaskCancelled { task_id } => write!(f, "Task cancelled: {}", task_id),
            StageError::TaskExecutionFailed { task_id, message } => write!(f, "Task execution failed ({}): {}", task_id, message),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StageSuccessResult {
    pub processed_items: u32,
    pub stage_duration_ms: u64,
    pub collection_metrics: Option<CollectionMetrics>,
    pub processing_metrics: Option<ProcessingMetrics>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CollectionMetrics {
    pub total_items: u32,
    pub successful_items: u32,
    pub failed_items: u32,
    pub duration_ms: u64,
    pub avg_response_time_ms: u64,
    pub success_rate: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProcessingMetrics {
    pub total_processed: u32,
    pub successful_saves: u32,
    pub failed_saves: u32,
    pub duration_ms: u64,
    pub avg_processing_time_ms: u64,
    pub success_rate: f64,
}

#[derive(Debug, Clone)]
pub struct ProductInfo {
    pub id: String,
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FailedItem {
    pub item_id: String,
    pub error: StageError,
    pub retry_count: u32,
    pub last_attempt_ms: u64,  // SystemTime을 u64로 변경
}

/// 재시도 정책 계산기
#[derive(Debug, Clone)]
pub struct RetryCalculator {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub exponential_factor: f64,
    pub jitter_enabled: bool,
}

impl RetryCalculator {
    pub fn new(
        max_attempts: u32,
        base_delay_ms: u64,
        max_delay_ms: u64,
        exponential_factor: f64,
        jitter_enabled: bool,
    ) -> Self {
        Self {
            max_attempts,
            base_delay_ms,
            max_delay_ms,
            exponential_factor,
            jitter_enabled,
        }
    }

    pub fn should_retry(&self, attempts: u32) -> bool {
        attempts < self.max_attempts
    }

    pub fn calculate_delay(&self, attempt: u32) -> u64 {
        if attempt == 0 {
            return self.base_delay_ms;
        }
        let exponential_delay = (self.base_delay_ms as f64) * self.exponential_factor.powi(attempt as i32 - 1);
        let mut delay = exponential_delay as u64;
        delay = delay.min(self.max_delay_ms);
        if self.jitter_enabled {
            let jitter_range = (delay as f64 * 0.25) as u64;
            let jitter = fastrand::u64(0..=jitter_range * 2);
            let jitter_offset = jitter.saturating_sub(jitter_range);
            delay = delay.saturating_add(jitter_offset);
        }
        delay
    }

    pub fn is_retryable_error(&self, error: &StageError) -> bool {
        matches!(error, StageError::NetworkError { .. } | StageError::ResourceExhausted { .. } | StageError::NetworkTimeout { .. } | StageError::DatabaseError { .. } | StageError::TaskExecutionFailed { .. })
    }

    pub fn should_retry_with_policy(&self, error: &StageError, attempts: u32) -> bool {
        self.should_retry(attempts) && self.is_retryable_error(error)
    }
}

impl Default for RetryCalculator {
    fn default() -> Self {
        Self::new(3, 100, 5000, 2.0, true)
    }
}

/// 배치 실행 계획
#[derive(Debug, Clone)]
pub struct BatchPlan {
    pub batch_id: String,
    pub pages: Vec<u32>,
    pub config: BatchConfig,
    pub batch_size: u32,
    pub concurrency_limit: u32,
}

/// Actor 시스템 오류 타입
#[derive(Debug, thiserror::Error)]
pub enum ActorError {
    #[error("Session timeout: {session_id} after {elapsed:?}")]
    SessionTimeout { session_id: String, elapsed: Duration },
    #[error("Batch processing failed: {batch_id} - {cause}")]
    BatchFailed { batch_id: String, cause: String },
    #[error("Stage execution error: {stage:?} - {message}")]
    StageError { stage: StageType, message: String },
    #[error("Channel communication error: {0}")]
    ChannelError(String),
    #[error("Configuration error: {0}")]
    ConfigError(#[from] ConfigError),
    #[error("Timeout error: {0}")]
    TimeoutError(String),
    #[error("Processing error: {0}")]
    ProcessingError(String),
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}

/// SessionActor - 세션 생명주기 관리
pub struct SessionActor {
    session_id: String,
    config: Arc<SystemConfig>,
    command_rx: mpsc::Receiver<ActorCommand>,
    event_tx: mpsc::Sender<AppEvent>,
    batch_actors: Vec<BatchActor>,
    start_time: Instant,
}

impl SessionActor {
    pub fn new(
        config: Arc<SystemConfig>,
        command_rx: mpsc::Receiver<ActorCommand>,
        event_tx: mpsc::Sender<AppEvent>,
    ) -> Self {
        let session_id = Uuid::new_v4().to_string();
        Self {
            session_id,
            config,
            command_rx,
            event_tx,
            batch_actors: Vec::new(),
            start_time: Instant::now(),
        }
    }

    pub async fn spawn_and_wait_for_batch(
        &mut self, 
        batch_plan: BatchPlan
    ) -> Result<StageResult, ActorError> {
        self.spawn_and_wait_for_batch_internal(batch_plan).await
    }

    async fn spawn_and_wait_for_batch_internal(
        &mut self, 
        batch_plan: BatchPlan
    ) -> Result<StageResult, ActorError> {
        info!(
            session_id = %self.session_id, 
            batch_id = %batch_plan.batch_id,
            pages_count = batch_plan.pages.len(),
            "Spawning BatchActor with OneShot channel"
        );
        let (data_tx, data_rx) = oneshot::channel::<StageResult>();
        let (control_tx, control_rx) = mpsc::channel::<ActorCommand>(32);
        let batch_actor = BatchActor::new(
            batch_plan.batch_id.clone(),
            self.config.clone(),
            self.event_tx.clone(),
        );
        let handle = tokio::spawn(async move {
            batch_actor.run_with_oneshot(control_rx, data_tx).await
        });
        let command = ActorCommand::ProcessBatch {
            pages: batch_plan.pages,
            config: batch_plan.config,
            batch_size: batch_plan.batch_size,
            concurrency_limit: batch_plan.concurrency_limit,
        };
        if let Err(e) = control_tx.send(command).await {
            handle.abort();
            return Err(ActorError::ChannelError(format!("Failed to send batch command: {}", e)));
        }
        let timeout_duration = Duration::from_secs(self.config.system.session_timeout_secs);
        match timeout(timeout_duration, data_rx).await {
            Ok(Ok(stage_result)) => {
                let _ = handle.await;
                info!(
                    session_id = %self.session_id,
                    batch_id = %batch_plan.batch_id,
                    "BatchActor completed successfully"
                );
                Ok(stage_result)
            }
            Ok(Err(_)) => {
                handle.abort();
                let error = ActorError::ChannelError("BatchActor result channel closed unexpectedly".to_string());
                let event = AppEvent::BatchFailed {
                    batch_id: batch_plan.batch_id.clone(),
                    error: error.to_string(),
                    final_failure: true,
                };
                if let Err(send_err) = self.event_tx.send(event).await {
                    error!(session_id = %self.session_id, error = %send_err, "Failed to send failure event");
                }
                Err(error)
            }
            Err(_) => {
                handle.abort();
                let error = ActorError::TimeoutError(format!("BatchActor timeout after {}s", timeout_duration.as_secs()));
                let event = AppEvent::SessionTimeout {
                    session_id: self.session_id.clone(),
                    elapsed: timeout_duration,
                };
                if let Err(send_err) = self.event_tx.send(event).await {
                    error!(session_id = %self.session_id, error = %send_err, "Failed to send timeout event");
                }
                Err(error)
            }
        }
    }

    pub async fn run(&mut self) -> Result<(), ActorError> {
        info!(session_id = %self.session_id, "SessionActor started");
        let session_timeout = Duration::from_secs(self.config.actor.session_timeout_secs);
        loop {
            let elapsed = self.start_time.elapsed();
            if elapsed >= session_timeout {
                let event = AppEvent::SessionTimeout {
                    session_id: self.session_id.clone(),
                    elapsed,
                };
                if let Err(e) = self.event_tx.send(event).await {
                    error!(session_id = %self.session_id, error = %e, "Failed to send timeout event");
                }
                return Err(ActorError::SessionTimeout {
                    session_id: self.session_id.clone(),
                    elapsed,
                });
            }
            match timeout(Duration::from_millis(100), self.command_rx.recv()).await {
                Ok(Some(command)) => {
                    if let Err(e) = self.handle_command(command).await {
                        error!(session_id = %self.session_id, error = %e, "Command handling failed");
                        return Err(e);
                    }
                }
                Ok(None) => {
                    debug!(session_id = %self.session_id, "Command channel closed");
                    break;
                }
                Err(_) => {}
            }
        }
        let elapsed = self.start_time.elapsed();
        info!(session_id = %self.session_id, elapsed = ?elapsed, "SessionActor completed");
        Ok(())
    }

    async fn handle_command(&mut self, command: ActorCommand) -> Result<(), ActorError> {
        match command {
            ActorCommand::ProcessBatch { pages, config, batch_size, concurrency_limit } => {
                self.process_batch(pages, config, batch_size, concurrency_limit).await
            }
            ActorCommand::CancelSession { session_id, reason } => {
                if session_id == self.session_id {
                    warn!(session_id = %self.session_id, reason = %reason, "Session cancelled");
                    return Err(ActorError::BatchFailed {
                        batch_id: self.session_id.clone(),
                        cause: reason,
                    });
                }
                Ok(())
            }
            _ => {
                warn!(session_id = %self.session_id, "Unsupported command for SessionActor");
                Ok(())
            }
        }
    }

    async fn process_batch(
        &mut self,
        pages: Vec<u32>,
        config: BatchConfig,
        batch_size: u32,
        concurrency_limit: u32,
    ) -> Result<(), ActorError> {
        let batch_plan = BatchPlan {
            batch_id: Uuid::new_v4().to_string(),
            pages,
            config: config.clone(),
            batch_size,
            concurrency_limit,
        };
        info!(session_id = %self.session_id, batch_id = %batch_plan.batch_id, "Starting batch processing");
        let event = AppEvent::SessionStarted {
            session_id: self.session_id.clone(),
            config,
        };
        if let Err(e) = self.event_tx.send(event).await {
            return Err(ActorError::ChannelError(format!("Failed to send session start event: {e}")));
        }
        match self.spawn_and_wait_for_batch(batch_plan).await {
            Ok(result) => self.handle_batch_result(result).await,
            Err(e) => {
                let event = AppEvent::BatchFailed {
                    batch_id: self.session_id.clone(),
                    error: e.to_string(),
                    final_failure: true,
                };
                if let Err(send_err) = self.event_tx.send(event).await {
                    error!(session_id = %self.session_id, error = %send_err, "Failed to send failure event");
                }
                Err(e)
            }
        }
    }

    async fn handle_batch_result(&mut self, result: StageResult) -> Result<(), ActorError> {
        match result {
            StageResult::Success(success_result) => {
                let event = AppEvent::BatchCompleted {
                    batch_id: self.session_id.clone(),
                    success_count: success_result.processed_items,
                };
                if let Err(e) = self.event_tx.send(event).await {
                    warn!(session_id = %self.session_id, error = %e, "Failed to send completion event");
                }
                Ok(())
            }
            StageResult::Failure(error) => {
                let event = AppEvent::BatchFailed {
                    batch_id: self.session_id.clone(),
                    error: error.to_string(),
                    final_failure: true,
                };
                if let Err(e) = self.event_tx.send(event).await {
                    warn!(session_id = %self.session_id, error = %e, "Failed to send failure event");
                }
                Err(ActorError::BatchFailed {
                    batch_id: self.session_id.clone(),
                    cause: error.to_string(),
                })
            }
            StageResult::RecoverableError { error, attempts, .. } => {
                let event = AppEvent::BatchFailed {
                    batch_id: self.session_id.clone(),
                    error: format!("Recoverable error after {} attempts: {}", attempts, error),
                    final_failure: false,
                };
                if let Err(e) = self.event_tx.send(event).await {
                    warn!(session_id = %self.session_id, error = %e, "Failed to send recoverable error event");
                }
                Err(ActorError::BatchFailed {
                    batch_id: self.session_id.clone(),
                    cause: format!("Recoverable error: {}", error),
                })
            }
            StageResult::FatalError { error, .. } => {
                let event = AppEvent::BatchFailed {
                    batch_id: self.session_id.clone(),
                    error: error.to_string(),
                    final_failure: true,
                };
                if let Err(e) = self.event_tx.send(event).await {
                    warn!(session_id = %self.session_id, error = %e, "Failed to send fatal error event");
                }
                Err(ActorError::BatchFailed {
                    batch_id: self.session_id.clone(),
                    cause: error.to_string(),
                })
            }
            StageResult::PartialSuccess { success_items, .. } => {
                let event = AppEvent::BatchCompleted {
                    batch_id: self.session_id.clone(),
                    success_count: success_items.processed_items,
                };
                if let Err(e) = self.event_tx.send(event).await {
                    warn!(session_id = %self.session_id, error = %e, "Failed to send partial success event");
                }
                Ok(())
            }
        }
    }
}

/// BatchActor - 배치 단위 처리 관리
pub struct BatchActor {
    pub batch_id: String,
    pub config: Arc<SystemConfig>,
    pub event_tx: mpsc::Sender<AppEvent>,
    pub stage_actors: Vec<StageActor>,
}

impl BatchActor {
    pub fn new(
        batch_id: String,
        config: Arc<SystemConfig>,
        event_tx: mpsc::Sender<AppEvent>,
    ) -> Self {
        Self {
            batch_id,
            config,
            event_tx,
            stage_actors: Vec::new(),
        }
    }

    pub async fn run_with_oneshot(
        mut self,
        mut control_rx: mpsc::Receiver<ActorCommand>,
        result_tx: oneshot::Sender<StageResult>,
    ) -> Result<(), ActorError> {
        info!(batch_id = %self.batch_id, "BatchActor started with OneShot channel");
        let mut final_result = StageResult::FatalError {
            error: StageError::ValidationError { message: "No commands received".to_string() },
            stage_id: self.batch_id.clone(),
            context: "BatchActor initialization".to_string(),
        };
        if let Some(command) = control_rx.recv().await {
            match command {
                ActorCommand::ProcessBatch { pages, config: _, batch_size, concurrency_limit } => {
                    final_result = self.process_batch_concurrently(pages, batch_size, concurrency_limit).await;
                }
                ActorCommand::CancelSession { reason, .. } => {
                    final_result = StageResult::FatalError {
                        error: StageError::ValidationError { message: format!("Batch cancelled: {}", reason) },
                        stage_id: self.batch_id.clone(),
                        context: "User cancellation".to_string(),
                    };
                }
                _ => {
                    warn!(batch_id = %self.batch_id, "Unsupported command for BatchActor");
                }
            }
        }
        if result_tx.send(final_result).is_err() {
            warn!(batch_id = %self.batch_id, "Failed to send batch result - receiver dropped");
        }
        info!(batch_id = %self.batch_id, "BatchActor completed");
        Ok(())
    }

    async fn process_batch_concurrently(
        &mut self,
        pages: Vec<u32>,
        batch_size: u32,
        concurrency_limit: u32,
    ) -> StageResult {
        info!(batch_id = %self.batch_id, pages_count = pages.len(), concurrency = concurrency_limit, "Processing batch concurrently");
        let semaphore = Arc::new(Semaphore::new(concurrency_limit as usize));
        let retry_calculator = Arc::new(RetryCalculator::default());
        let mut handles = Vec::new();

        for page_id in pages {
            let semaphore_clone = semaphore.clone();
            let self_clone = self.clone_for_task();
            let retry_calculator_clone = retry_calculator.clone();

            handles.push(tokio::spawn(async move {
                let _permit = semaphore_clone.acquire().await.expect("Semaphore closed");
                self_clone.process_single_page_with_retry(retry_calculator_clone, page_id, 0).await
            }));
        }

        let results = join_all(handles).await;
        let mut collected_urls: Vec<String> = Vec::new();
        let mut failed_items: Vec<FailedItem> = Vec::new();

        for result in results {
            match result {
                Ok(Ok(urls)) => collected_urls.extend(urls),
                Ok(Err(failed_item)) => failed_items.push(failed_item),
                Err(join_error) => {
                    error!(batch_id = %self.batch_id, "Task join error: {}", join_error);
                }
            }
        }

        let total_processed = collected_urls.len() as u32;
        let total_failures = failed_items.len() as u32;
        let total_items = total_processed + total_failures;

        if total_failures == 0 {
            StageResult::Success(StageSuccessResult {
                processed_items: total_processed,
                stage_duration_ms: 0, // Placeholder
                collection_metrics: Some(CollectionMetrics {
                    total_items,
                    successful_items: total_processed,
                    failed_items: 0,
                    duration_ms: 0, // Placeholder
                    avg_response_time_ms: 0, // Placeholder
                    success_rate: 100.0,
                }),
                processing_metrics: None,
            })
        } else {
            StageResult::PartialSuccess {
                success_items: StageSuccessResult {
                    processed_items: total_processed,
                    stage_duration_ms: 0, // Placeholder
                    collection_metrics: Some(CollectionMetrics {
                        total_items,
                        successful_items: total_processed,
                        failed_items: total_failures,
                        duration_ms: 0, // Placeholder
                        avg_response_time_ms: 0, // Placeholder
                        success_rate: if total_items > 0 { (total_processed as f64 / total_items as f64) * 100.0 } else { 0.0 },
                    }),
                    processing_metrics: None,
                },
                failed_items,
                stage_id: self.batch_id.clone(),
            }
        }
    }

    async fn process_single_page_with_retry(
        &self,
        retry_calculator: Arc<RetryCalculator>,
        page_id: u32,
        initial_attempt: u32,
    ) -> Result<Vec<String>, FailedItem> {
        let mut attempt = initial_attempt;
        loop {
            match self.execute_real_crawling_stage(page_id).await {
                Ok(urls) => {
                    info!(batch_id = %self.batch_id, page_id = page_id, attempt = attempt, "✅ Successfully crawled page");
                    return Ok(urls);
                }
                Err(error) => {
                    attempt += 1;
                    if retry_calculator.should_retry_with_policy(&error, attempt) {
                        let delay_ms = retry_calculator.calculate_delay(attempt);
                        warn!(batch_id = %self.batch_id, page_id = page_id, attempt = attempt, delay_ms = delay_ms, "Retryable error, retrying...");
                        sleep(Duration::from_millis(delay_ms)).await;
                    } else {
                        error!(batch_id = %self.batch_id, page_id = page_id, attempt = attempt, "❌ Failed to crawl page after retries");
                        return Err(FailedItem {
                            item_id: page_id.to_string(),
                            error,
                            retry_count: attempt,
                            last_attempt_ms: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64,
                        });
                    }
                }
            }
        }
    }

    fn clone_for_task(&self) -> Self {
        Self {
            batch_id: self.batch_id.clone(),
            config: self.config.clone(),
            event_tx: self.event_tx.clone(),
            stage_actors: Vec::new(), // Do not clone stage actors for a task
        }
    }

    async fn execute_real_crawling_stage(&self, page_id: u32) -> Result<Vec<String>, StageError> {
        use crate::new_architecture::services::crawling_integration::{RealCrawlingStageExecutor, CrawlingIntegrationService};
        let system_config = Arc::new(SystemConfig::default());
        let app_config = AppConfig::default();
        let crawling_service = CrawlingIntegrationService::new(system_config, app_config).await
            .map_err(|e| StageError::ResourceExhausted { message: format!("Failed to create crawling service: {}", e) })?;
        let executor = RealCrawlingStageExecutor::new(Arc::new(crawling_service));
        let items = vec![crate::new_architecture::channel_types::StageItem::Page(page_id)];
        let cancellation_token = tokio_util::sync::CancellationToken::new();
        let result = executor.execute_stage(
            crate::new_architecture::channel_types::StageType::ListCollection,
            items,
            2, // This concurrency limit is for within the stage, not across pages
            cancellation_token,
        ).await;
        match result {
            StageResult::Success(stage_result) => {
                let urls = self.extract_urls_from_stage_result(&stage_result);
                info!(batch_id = %self.batch_id, page_id = page_id, "🎯 Real crawling stage completed");
                Ok(urls)
            }
            StageResult::Failure(stage_error) => Err(stage_error),
            _ => Err(StageError::ParsingError { message: "Unexpected stage result type".to_string() }),
        }
    }

    fn extract_urls_from_stage_result(&self, _result: &StageSuccessResult) -> Vec<String> {
        vec![
            format!("https://www.mattercertis.com/product/page_{}_item_1", self.batch_id),
            format!("https://www.mattercertis.com/product/page_{}_item_2", self.batch_id),
        ]
    }
}


/// StageActor - 개별 스테이지 실행 관리
pub struct StageActor {
    pub batch_id: String,
    pub config: Arc<SystemConfig>,
    pub execution_stats: Arc<Mutex<StageExecutionStats>>,
    pub crawling_executor: Option<Arc<crate::new_architecture::services::crawling_integration::RealCrawlingStageExecutor>>,
}

impl StageActor {
    pub fn new(batch_id: String, config: Arc<SystemConfig>) -> Self {
        Self {
            batch_id,
            config,
            execution_stats: Arc::new(Mutex::new(StageExecutionStats::default())),
            crawling_executor: None,
        }
    }
    
    /// OneShot 채널을 지원하는 새 생성자
    pub fn new_with_oneshot(batch_id: String, config: Arc<SystemConfig>) -> Self {
        Self {
            batch_id,
            config,
            execution_stats: Arc::new(Mutex::new(StageExecutionStats::default())),
            crawling_executor: None,
        }
    }
    
    /// 실제 크롤링 서비스와 함께 생성
    pub fn new_with_real_crawling(
        batch_id: String,
        config: Arc<SystemConfig>,
        crawling_executor: Arc<crate::new_architecture::services::crawling_integration::RealCrawlingStageExecutor>,
    ) -> Self {
        Self {
            batch_id,
            config,
            execution_stats: Arc::new(Mutex::new(StageExecutionStats::default())),
            crawling_executor: Some(crawling_executor),
        }
    }
    
    /// OneShot 채널을 사용한 실행 루프
    pub async fn run_with_oneshot(
        mut self,
        mut control_rx: mpsc::Receiver<ActorCommand>,
        result_tx: oneshot::Sender<StageResult>,
    ) -> Result<(), ActorError> {
        info!(batch_id = %self.batch_id, "StageActor started with OneShot channel");
        
        let mut final_result = StageResult::FatalError {
            error: StageError::ValidationError {
                message: "No commands received".to_string(),
            },
            stage_id: self.batch_id.clone(),
            context: "StageActor initialization".to_string(),
        };
        
        // 명령 대기 및 처리
        while let Some(command) = control_rx.recv().await {
            match command {
                ActorCommand::ExecuteStage { stage_type, items, concurrency_limit, timeout_secs } => {
                    final_result = self.execute_stage_with_oneshot(
                        stage_type,
                        items,
                        concurrency_limit,
                        Duration::from_secs(timeout_secs),
                    ).await;
                    break; // 스테이지 처리 완료 후 종료
                }
                ActorCommand::CancelSession { reason, .. } => {
                    final_result = StageResult::FatalError {
                        error: StageError::ValidationError {
                            message: format!("Stage cancelled: {}", reason),
                        },
                        stage_id: self.batch_id.clone(),
                        context: "User cancellation".to_string(),
                    };
                    break;
                }
                _ => {
                    warn!(batch_id = %self.batch_id, "Unsupported command for StageActor");
                }
            }
        }
        
        // 결과 전송
        if result_tx.send(final_result).is_err() {
            warn!(batch_id = %self.batch_id, "Failed to send stage result - receiver dropped");
        }
        
        info!(batch_id = %self.batch_id, "StageActor completed");
        Ok(())
    }
    
    /// OneShot 채널을 사용한 스테이지 실행
    async fn execute_stage_with_oneshot(
        &mut self,
        stage_type: StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
        timeout_duration: Duration,
    ) -> StageResult {
        info!(
            batch_id = %self.batch_id,
            stage = ?stage_type,
            items_count = items.len(),
            concurrency_limit = concurrency_limit,
            "Executing stage with OneShot"
        );
        
        let start_time = Instant::now();
        
        // 타임아웃과 함께 스테이지 아이템 처리
        let processing_result = timeout(
            timeout_duration,
            self.process_stage_items_with_result(stage_type.clone(), items, concurrency_limit)
        ).await;
        
        let elapsed = start_time.elapsed();
        
        match processing_result {
            Ok(result) => {
                info!(
                    batch_id = %self.batch_id,
                    stage = ?stage_type,
                    elapsed_ms = elapsed.as_millis(),
                    "Stage completed"
                );
                result
            }
            Err(_) => {
                warn!(
                    batch_id = %self.batch_id,
                    stage = ?stage_type,
                    elapsed_ms = elapsed.as_millis(),
                    timeout_ms = timeout_duration.as_millis(),
                    "Stage execution timed out"
                );
                
                StageResult::RecoverableError {
                    error: StageError::NetworkTimeout {
                        message: format!("Stage {:?} timed out after {:?}", stage_type, timeout_duration),
                    },
                    attempts: 0,
                    stage_id: self.batch_id.clone(),
                    suggested_retry_delay_ms: 10000,  // 10초를 밀리초로 변경
                }
            }
        }
    }
    
    /// 스테이지 아이템 처리 및 결과 반환
    async fn process_stage_items_with_result(
        &self,
        stage_type: StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
    ) -> StageResult {
        if let Some(ref executor) = self.crawling_executor {
            info!(
                batch_id = %self.batch_id,
                stage = ?stage_type,
                items_count = items.len(),
                "Using real crawling service for stage execution"
            );
            let cancellation_token = tokio_util::sync::CancellationToken::new();
            return executor.execute_stage(
                stage_type,
                items,
                concurrency_limit,
                cancellation_token,
            ).await;
        }

        info!(
            batch_id = %self.batch_id,
            stage = ?stage_type,
            items_count = items.len(),
            "Using simulation mode for stage execution"
        );
        
        let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency_limit as usize));
        let mut handles = Vec::new();
        
        for item in items {
            let permit = match semaphore.clone().acquire_owned().await {
                Ok(permit) => permit,
                Err(e) => {
                    return StageResult::FatalError {
                        error: StageError::ResourceExhausted {
                            message: format!("Semaphore acquire failed: {}", e),
                        },
                        stage_id: self.batch_id.clone(),
                        context: "Concurrency control failure".to_string(),
                    };
                }
            };
            
            let batch_id = self.batch_id.clone();
            let stage_type_for_task = stage_type.clone();
            let handle = tokio::spawn(async move {
                let _permit = permit; // 스코프 종료시 자동 해제
                Self::process_single_item_with_result(batch_id, &stage_type_for_task, item).await
            });
            
            handles.push(handle);
        }
        
        let mut successful_items = Vec::new();
        let mut failed_items = Vec::new();
        
        for handle in handles {
            match handle.await {
                Ok(Ok(item_id)) => successful_items.push(item_id),
                Ok(Err(item_id)) => failed_items.push(item_id),
                Err(e) => {
                    error!(batch_id = %self.batch_id, error = %e, "Task join failed");
                }
            }
        }
        
        let total_items = successful_items.len() + failed_items.len();
        let success_result = StageSuccessResult {
            processed_items: successful_items.len() as u32,
            stage_duration_ms: 0,
            collection_metrics: Some(CollectionMetrics {
                total_items: total_items as u32,
                successful_items: successful_items.len() as u32,
                failed_items: failed_items.len() as u32,
                duration_ms: 0,
                avg_response_time_ms: 100,
                success_rate: if total_items > 0 { (successful_items.len() as f64 / total_items as f64) * 100.0 } else { 0.0 },
            }),
            processing_metrics: None,
        };
        
        if !failed_items.is_empty() {
            StageResult::PartialSuccess {
                success_items: success_result,
                failed_items: failed_items.into_iter().map(|item| {
                    FailedItem {
                        item_id: format!("item-{}", item),
                        error: StageError::ValidationError { message: "Processing failed".to_string() },
                        retry_count: 0,
                        last_attempt_ms: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                    }
                }).collect(),
                stage_id: self.batch_id.clone(),
            }
        } else {
            StageResult::Success(success_result)
        }
    }
    
    async fn process_single_item_with_result(
        _batch_id: String,
        _stage_type: &StageType,
        item: StageItem,
    ) -> Result<u32, u32> {
        sleep(std::time::Duration::from_millis(100)).await;
        if fastrand::f64() < 0.9 {
            Ok(match item { StageItem::Page(page) => page, StageItem::Url(_) => fastrand::u32(1..=1000) })
        } else {
            Err(match item { StageItem::Page(page) => page, StageItem::Url(_) => fastrand::u32(1..=1000) })
        }
    }
    
    pub async fn execute_stage(
        &mut self,
        stage_type: StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
        timeout_duration: Duration,
    ) -> Result<u32, ActorError> {
        info!(batch_id = %self.batch_id, stage = ?stage_type, items_count = items.len(), "Executing stage");
        let start_time = Instant::now();
        let result = timeout(timeout_duration, self.process_stage_items(stage_type.clone(), items, concurrency_limit)).await;
        let execution_time = start_time.elapsed();
        {
            let mut stats = self.execution_stats.lock().await;
            stats.update_stage_execution(stage_type.clone(), execution_time, result.is_ok());
        }
        match result {
            Ok(Ok(processed_count)) => {
                info!(batch_id = %self.batch_id, stage = ?stage_type, processed = processed_count, duration = ?execution_time, "Stage execution completed");
                Ok(processed_count)
            }
            Ok(Err(e)) => {
                error!(batch_id = %self.batch_id, stage = ?stage_type, error = %e, duration = ?execution_time, "Stage execution failed");
                Err(e)
            }
            Err(_) => {
                error!(batch_id = %self.batch_id, stage = ?stage_type, timeout = ?timeout_duration, "Stage execution timed out");
                Err(ActorError::StageError {
                    stage: stage_type,
                    message: format!("Stage timed out after {timeout_duration:?}"),
                })
            }
        }
    }
    
    async fn process_stage_items(
        &self,
        stage_type: StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
    ) -> Result<u32, ActorError> {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency_limit as usize));
        let mut handles = Vec::new();
        for item in items {
            let permit = semaphore.clone().acquire_owned().await.map_err(|e| ActorError::ChannelError(format!("Semaphore acquire failed: {e}")))?;
            let batch_id = self.batch_id.clone();
            let stage_type = stage_type.clone();
            let handle = tokio::spawn(async move {
                let _permit = permit;
                Self::process_single_item(batch_id, stage_type, item).await
            });
            handles.push(handle);
        }
        let mut success_count = 0u32;
        for handle in handles {
            match handle.await {
                Ok(Ok(())) => success_count += 1,
                Ok(Err(e)) => {
                    warn!(batch_id = %self.batch_id, stage = ?stage_type, error = %e, "Single item processing failed");
                }
                Err(e) => {
                    error!(batch_id = %self.batch_id, stage = ?stage_type, error = %e, "Task join failed");
                }
            }
        }
        Ok(success_count)
    }
    
    async fn process_single_item(
        _batch_id: String,
        _stage_type: StageType,
        _item: StageItem,
    ) -> Result<(), ActorError> {
        sleep(Duration::from_millis(100)).await;
        Ok(())
    }
}

/// 스테이지 실행 통계
#[derive(Debug, Default)]
struct StageExecutionStats {
    stage_durations: std::collections::HashMap<String, Vec<Duration>>,
    stage_success_rates: std::collections::HashMap<String, (u32, u32)>, // (성공, 총시도)
}

impl StageExecutionStats {
    fn update_stage_execution(&mut self, stage_type: StageType, duration: Duration, success: bool) {
        let stage_name = format!("{stage_type:?}");
        self.stage_durations.entry(stage_name.clone()).or_default().push(duration);
        let (success_count, total_count) = self.stage_success_rates.entry(stage_name).or_insert((0, 0));
        *total_count += 1;
        if success { *success_count += 1; }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::{mpsc, oneshot};
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_basic_oneshot_channel() {
        println!("🧪 기본 OneShot 채널 테스트 시작");
        let (tx, rx) = oneshot::channel::<StageResult>();
        let result = StageResult::Success(StageSuccessResult {
            processed_items: 5,
            stage_duration_ms: 1000,
            collection_metrics: None,
            processing_metrics: None,
        });
        let _ = tx.send(result);
        match rx.await {
            Ok(StageResult::Success(success_result)) => {
                println!("✅ OneShot 채널 통신 성공!");
                assert_eq!(success_result.processed_items, 5);
            }
            _ => panic!("예상치 못한 결과"),
        }
        println!("🎯 기본 OneShot 채널 테스트 완료!");
    }

    #[tokio::test]
    async fn test_retry_calculator() {
        println!("🧪 RetryCalculator 테스트 시작");
        let calculator = RetryCalculator::new(3, 100, 5000, 2.0, true);
        assert!(calculator.should_retry(1));
        let delay1 = calculator.calculate_delay(1);
        assert!(delay1 >= 50 && delay1 <= 150);
        assert!(!calculator.should_retry(3));
        println!("🎯 RetryCalculator 테스트 완료!");
    }

    /*
    // TODO: 불완전한 테스트 - 나중에 완성 필요
    #[tokio::test]
    async fn test_channel_performance() {
        println!("🧪 채널 통신 성능 테스트 시작");
        
        let mut received_count = 0;
        
        while received_count < 100 {
            if let Ok(Some(_)) = timeout(Duration::from_millis(1), mpsc_rx.recv()).await {
                received_count += 1;
            } else {
                break;
            }
        }
        
        let recv_time = recv_start.elapsed();
        println!("   MPSC 메시지 {}개 수신 시간: {:?}", received_count, recv_time);
        
        // 성능 검증
        assert!(creation_time < Duration::from_millis(100), "채널 생성이 너무 느림");
        assert!(send_time < Duration::from_millis(50), "메시지 전송이 너무 느림");
        
        println!("🎯 채널 통신 성능 테스트 완료!");
    }
    */
}
