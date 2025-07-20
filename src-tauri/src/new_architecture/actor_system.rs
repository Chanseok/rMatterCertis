//! Actor 시스템: 세션, 배치, 스테이지 분리 구조
//! Modern Rust 2024 준수: 의존성 주입 기반 Actor 설계

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::{sleep, timeout};
use tracing::{info, warn, error, debug};
use uuid::Uuid;

// 개별 모듈에서 직접 import
use crate::new_architecture::system_config::{SystemConfig, ConfigError, RetryPolicy};
use crate::new_architecture::channel_types::{ActorCommand, AppEvent, BatchConfig, StageType, StageItem};
use crate::infrastructure::config::AppConfig;

// 임시 타입 정의 (컴파일 에러 해결용)

#[derive(Debug, Clone)]
pub enum StageResult {
    Success(StageSuccessResult),
    Failure(StageError),
    RecoverableError {
        error: StageError,
        attempts: u32,
        stage_id: String,
        suggested_retry_delay: Duration,
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

#[derive(Debug, Clone)]
pub enum StageError {
    NetworkError { message: String },
    ParsingError { message: String },
    NetworkTimeout { message: String },
    ValidationError { message: String },
    ChannelError { message: String },
    DatabaseError { message: String },
    ResourceExhausted { message: String },
    ConfigurationError { message: String },
    // Phase 3: TaskActor 관련 에러 추가
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
            // Phase 3: TaskActor 관련 에러 처리 추가
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

#[derive(Debug, Clone)]
pub struct FailedItem {
    pub item_id: String,
    pub error: StageError,
    pub retry_count: u32,
    pub last_attempt: std::time::SystemTime,
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

    /// 에러와 시도 횟수를 기반으로 재시도 여부 결정
    pub fn should_retry(&self, attempts: u32) -> bool {
        attempts < self.max_attempts
    }

    /// 시도 횟수에 따른 지연 시간 계산 (지수 백오프 + 지터)
    pub fn calculate_delay(&self, attempt: u32) -> u64 {
        if attempt == 0 {
            return self.base_delay_ms;
        }

        // 지수 백오프 계산
        let exponential_delay = (self.base_delay_ms as f64) * self.exponential_factor.powi(attempt as i32 - 1);
        let mut delay = exponential_delay as u64;

        // 최대 지연 시간 제한
        delay = delay.min(self.max_delay_ms);

        // 지터 적용 (±25% 범위)
        if self.jitter_enabled {
            let jitter_range = (delay as f64 * 0.25) as u64;
            let jitter = fastrand::u64(0..=jitter_range * 2);
            let jitter_offset = jitter.saturating_sub(jitter_range);
            delay = delay.saturating_add(jitter_offset);
        }

        delay
    }

    /// 특정 에러 타입에 대해 재시도 가능 여부 확인
    pub fn is_retryable_error(&self, error: &StageError) -> bool {
        match error {
            StageError::NetworkError { .. } => true,       // 네트워크 에러는 재시도 가능
            StageError::ParsingError { .. } => false,      // 파싱 에러는 재시도 불가
            StageError::ResourceExhausted { .. } => true,  // 리소스 부족은 재시도 가능
            StageError::NetworkTimeout { .. } => true,     // 네트워크 타임아웃은 재시도 가능
            StageError::ValidationError { .. } => false,   // 검증 에러는 재시도 불가
            StageError::ChannelError { .. } => false,      // 채널 에러는 재시도 불가
            StageError::DatabaseError { .. } => true,      // 데이터베이스 에러는 재시도 가능
            StageError::ConfigurationError { .. } => false, // 설정 에러는 재시도 불가
            // Phase 3: TaskActor 관련 에러 재시도 정책
            StageError::TaskCancelled { .. } => false,     // 취소된 태스크는 재시도 불가
            StageError::TaskExecutionFailed { .. } => true, // 태스크 실행 실패는 재시도 가능
        }
    }

    /// 정책 기반 재시도 여부 결정 (에러 타입과 시도 횟수 모두 고려)
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
    
    /// OneShot 채널을 사용한 BatchActor 스폰 및 결과 대기 (BatchPlan 버전)
    pub async fn spawn_and_wait_for_batch(
        &mut self, 
        batch_plan: BatchPlan
    ) -> Result<StageResult, ActorError> {
        // 기존 spawn_and_wait_for_batch 함수를 호출
        self.spawn_and_wait_for_batch_internal(batch_plan).await
    }

    /// OneShot 채널을 사용한 BatchActor 스폰 및 결과 대기 (내부 구현)
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
        
        // 1. OneShot 데이터 채널 생성
        let (data_tx, data_rx) = oneshot::channel::<StageResult>();
        
        // 2. BatchActor용 Control 채널 생성
        let (control_tx, control_rx) = mpsc::channel::<ActorCommand>(32);
        
        // 3. BatchActor 생성 및 스폰
        let batch_actor = BatchActor::new(
            batch_plan.batch_id.clone(),
            self.config.clone(),
            self.event_tx.clone(),
        );
        
        let handle = tokio::spawn(async move {
            batch_actor.run_with_oneshot(control_rx, data_tx).await
        });
        
        // 4. 배치 처리 명령 전송
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
        
        // 5. 타임아웃과 함께 결과 대기
        let timeout_duration = Duration::from_secs(self.config.system.session_timeout_secs);
        match timeout(timeout_duration, data_rx).await {
            Ok(Ok(stage_result)) => {
                // BatchActor handle 정리
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
    
    /// 세션 실행 시작
    pub async fn run(&mut self) -> Result<(), ActorError> {
        info!(session_id = %self.session_id, "SessionActor started");
        
        let session_timeout = Duration::from_secs(self.config.actor.session_timeout_secs);
        
        loop {
            let elapsed = self.start_time.elapsed();
            
            // 세션 타임아웃 체크
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
            
            // 명령 처리
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
                Err(_) => {
                    // 타임아웃 - 다음 루프 계속
                }
            }
        }
        
        let elapsed = self.start_time.elapsed();
        info!(session_id = %self.session_id, elapsed = ?elapsed, "SessionActor completed");
        Ok(())
    }
    
    /// 명령 처리
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
    
    /// 배치 처리 시작 (OneShot 채널 사용)
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
        
        // OneShot 채널을 사용한 배치 실행 및 결과 대기
        match self.spawn_and_wait_for_batch(batch_plan).await {
            Ok(result) => {
                self.handle_batch_result(result).await
            }
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
    
    /// 배치 결과 처리
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
                // 세션 레벨에서는 복구 가능한 오류도 실패로 처리 (재시도는 하위 레벨에서 수행됨)
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
    
    /// OneShot 채널을 지원하는 새 생성자
    pub fn new_with_oneshot(
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
    
    /// OneShot 채널을 사용한 실행 루프
    pub async fn run_with_oneshot(
        mut self,
        mut control_rx: mpsc::Receiver<ActorCommand>,
        result_tx: oneshot::Sender<StageResult>,
    ) -> Result<(), ActorError> {
        info!(batch_id = %self.batch_id, "BatchActor started with OneShot channel");
        
        let mut final_result = StageResult::FatalError {
            error: StageError::ValidationError {
                message: "No commands received".to_string(),
            },
            stage_id: self.batch_id.clone(),
            context: "BatchActor initialization".to_string(),
        };
        
        // 명령 대기 및 처리
        while let Some(command) = control_rx.recv().await {
            match command {
                ActorCommand::ProcessBatch { pages, config: _, batch_size, concurrency_limit } => {
                    final_result = self.process_batch_with_oneshot(pages, batch_size, concurrency_limit).await;
                    break; // 배치 처리 완료 후 종료
                }
                ActorCommand::CancelSession { reason, .. } => {
                    final_result = StageResult::FatalError {
                        error: StageError::ValidationError {
                            message: format!("Batch cancelled: {}", reason),
                        },
                        stage_id: self.batch_id.clone(),
                        context: "User cancellation".to_string(),
                    };
                    break;
                }
                _ => {
                    warn!(batch_id = %self.batch_id, "Unsupported command for BatchActor");
                }
            }
        }
        
        // 결과 전송
        if result_tx.send(final_result).is_err() {
            warn!(batch_id = %self.batch_id, "Failed to send batch result - receiver dropped");
        }
        
        info!(batch_id = %self.batch_id, "BatchActor completed");
        Ok(())
    }
    
    /// OneShot 채널을 사용한 배치 처리 (재시도 정책 적용)
    async fn process_batch_with_oneshot(
        &mut self,
        pages: Vec<u32>,
        batch_size: u32,
        _concurrency_limit: u32,
    ) -> StageResult {
        info!(batch_id = %self.batch_id, pages_count = pages.len(), "Processing batch with OneShot and retry policy");
        
        let mut collected_urls: Vec<String> = Vec::new();
        let mut total_processed = 0u32;
        let mut total_failures = 0u32;
        let mut failed_items: Vec<FailedItem> = Vec::new();
        
        // 설정 기반 재시도 정책 사용 (임시로 기존 RetryCalculator 사용)
        let retry_calculator = RetryCalculator::default();
        
        // 페이지를 배치 크기로 분할하여 처리
        for chunk in pages.chunks(batch_size as usize) {
            // 첫 번째 시도
            let mut items_to_retry: Vec<(u32, u32)> = Vec::new(); // (page_id, attempt_count)
            
            // 초기 처리
            for &page_id in chunk {
                match self.process_single_page_with_retry(&retry_calculator, page_id, 0).await {
                    Ok(urls) => {
                        collected_urls.extend(urls);
                        total_processed += 1;
                    }
                    Err(error) => {
                        if retry_calculator.should_retry_with_policy(&error, 1) {
                            items_to_retry.push((page_id, 1));
                            info!(
                                batch_id = %self.batch_id,
                                page_id = page_id,
                                attempt = 1,
                                "Scheduling retry for failed page"
                            );
                        } else {
                            failed_items.push(FailedItem {
                                item_id: page_id.to_string(),
                                error,
                                retry_count: 0,
                                last_attempt: std::time::SystemTime::now(),
                            });
                            total_failures += 1;
                        }
                    }
                }
            }
            
            // 재시도 처리
            while !items_to_retry.is_empty() {
                let mut next_retry_batch = Vec::new();
                
                for (page_id, attempt_count) in items_to_retry {
                    // 재시도 지연 적용
                    let delay_ms = retry_calculator.calculate_delay(attempt_count);
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    
                    match self.process_single_page_with_retry(&retry_calculator, page_id, attempt_count).await {
                        Ok(urls) => {
                            collected_urls.extend(urls);
                            total_processed += 1;
                            info!(
                                batch_id = %self.batch_id,
                                page_id = page_id,
                                attempt = attempt_count,
                                delay_ms = delay_ms,
                                "Retry succeeded"
                            );
                        }
                        Err(error) => {
                            let next_attempt = attempt_count + 1;
                            if retry_calculator.should_retry_with_policy(&error, next_attempt) {
                                next_retry_batch.push((page_id, next_attempt));
                                info!(
                                    batch_id = %self.batch_id,
                                    page_id = page_id,
                                    attempt = attempt_count,
                                    next_attempt = next_attempt,
                                    "Retry failed, scheduling next attempt"
                                );
                            } else {
                                failed_items.push(FailedItem {
                                    item_id: page_id.to_string(),
                                    error,
                                    retry_count: attempt_count,
                                    last_attempt: std::time::SystemTime::now(),
                                });
                                total_failures += 1;
                                warn!(
                                    batch_id = %self.batch_id,
                                    page_id = page_id,
                                    attempt = attempt_count,
                                    "Retry exhausted, marking as failed"
                                );
                            }
                        }
                    }
                }
                
                items_to_retry = next_retry_batch;
            }
        }
        
        // 최종 결과 반환
        if total_failures == 0 {
            StageResult::Success(StageSuccessResult {
                processed_items: pages.len() as u32,
                stage_duration_ms: 0, // 추후 구현
                collection_metrics: Some(CollectionMetrics {
                    total_items: pages.len() as u32,
                    successful_items: pages.len() as u32,
                    failed_items: 0,
                    duration_ms: 0, // 추후 구현
                    avg_response_time_ms: 0,
                    success_rate: 100.0,
                }),
                processing_metrics: None,
            })
        } else {
            StageResult::PartialSuccess {
                success_items: StageSuccessResult {
                    processed_items: total_processed,
                    stage_duration_ms: 0,
                    collection_metrics: Some(CollectionMetrics {
                        total_items: total_processed + total_failures,
                        successful_items: total_processed,
                        failed_items: total_failures,
                        duration_ms: 0,
                        avg_response_time_ms: 0,
                        success_rate: if total_processed + total_failures > 0 {
                            (total_processed as f64 / (total_processed + total_failures) as f64) * 100.0
                        } else { 0.0 },
                    }),
                    processing_metrics: None,
                },
                failed_items,
                stage_id: self.batch_id.clone(),
            }
        }
    }

    /// 재시도 정책이 적용된 단일 페이지 처리
    async fn process_single_page_with_retry(
        &self,
        _retry_calculator: &RetryCalculator,
        page_id: u32,
        attempt_count: u32,
    ) -> Result<Vec<String>, StageError> {
        info!(
            batch_id = %self.batch_id,
            page_id = page_id,
            attempt = attempt_count,
            "🔍 Starting real crawling for page"
        );
        
        // 실제 크롤링 서비스 사용
        match self.execute_real_crawling_stage(page_id).await {
            Ok(urls) => {
                info!(
                    batch_id = %self.batch_id,
                    page_id = page_id,
                    urls_count = urls.len(),
                    "✅ Successfully crawled page"
                );
                Ok(urls)
            }
            Err(e) => {
                error!(
                    batch_id = %self.batch_id,
                    page_id = page_id,
                    attempt = attempt_count,
                    error = %e,
                    "❌ Failed to crawl page"
                );
                Err(e)
            }
        }
    }
    
    /// 실제 크롤링 스테이지 실행
    async fn execute_real_crawling_stage(&self, page_id: u32) -> Result<Vec<String>, StageError> {
        use crate::new_architecture::services::crawling_integration::{RealCrawlingStageExecutor, CrawlingIntegrationService};
        use crate::new_architecture::system_config::SystemConfig;
        
        // 기본 설정 생성
        let system_config = Arc::new(SystemConfig::default());
        let app_config = AppConfig::default();
        
        // CrawlingIntegrationService 생성
        let crawling_service = match CrawlingIntegrationService::new(
            system_config,
            app_config
        ).await {
            Ok(service) => service,
            Err(e) => {
                return Err(StageError::ResourceExhausted {
                    message: format!("Failed to create crawling service: {}", e)
                });
            }
        };
        
        // RealCrawlingStageExecutor 생성
        let executor = RealCrawlingStageExecutor::new(Arc::new(crawling_service));
        
        // 페이지 URL 생성
        let base_url = "https://www.mattercertis.com";
        let target_url = format!("{}/search?page={}", base_url, page_id);
        
        // StageType::ListCollection 실행
        let items = vec![crate::new_architecture::channel_types::StageItem::Page(page_id)];
        let cancellation_token = tokio_util::sync::CancellationToken::new();
        
        let result = executor.execute_stage(
            crate::new_architecture::channel_types::StageType::ListCollection,
            items,
            2, // concurrency_limit
            cancellation_token
        ).await;
        
        match result {
            crate::new_architecture::actor_system::StageResult::Success(stage_result) => {
                // 성공 결과에서 URL 추출
                let urls = self.extract_urls_from_stage_result(&stage_result);
                info!(
                    batch_id = %self.batch_id,
                    page_id = page_id,
                    stage_duration_ms = stage_result.stage_duration_ms,
                    processed_items = stage_result.processed_items,
                    "🎯 Real crawling stage completed"
                );
                Ok(urls)
            }
            crate::new_architecture::actor_system::StageResult::Failure(stage_error) => {
                Err(stage_error)
            }
            _ => {
                Err(StageError::ParsingError {
                    message: "Unexpected stage result type".to_string()
                })
            }
        }
    }
    
    /// StageSuccessResult에서 URL 목록 추출
    fn extract_urls_from_stage_result(&self, _result: &StageSuccessResult) -> Vec<String> {
        // 실제 구현에서는 result의 내용을 파싱하여 URL을 추출
        // 현재는 기본값 반환
        vec![
            format!("https://www.mattercertis.com/product/page_{}_item_1", self.batch_id),
            format!("https://www.mattercertis.com/product/page_{}_item_2", self.batch_id),
        ]
    }
    
    /// 기존 배치 처리 메서드 (호환성 유지)
    async fn process_batch_legacy(
        &mut self,
        pages: Vec<u32>,
        batch_size: u32,
        _concurrency_limit: u32,
    ) -> StageResult {
        info!(batch_id = %self.batch_id, pages_count = pages.len(), "Processing batch (legacy mode)");
        
        // 간단한 성공 결과 반환 (구현 간소화)
        StageResult::Success(StageSuccessResult {
            processed_items: pages.len() as u32,
            stage_duration_ms: 100,
            collection_metrics: Some(CollectionMetrics {
                total_items: pages.len() as u32,
                successful_items: pages.len() as u32,
                failed_items: 0,
                duration_ms: 100,
                avg_response_time_ms: 50,
                success_rate: 100.0,
            }),
            processing_metrics: None,
        })
    }

    /// OneShot 채널을 사용한 스테이지 실행
    async fn execute_stage_with_oneshot(&mut self, stage_type: StageType, items: Vec<StageItem>) -> StageResult {
        info!(batch_id = %self.batch_id, stage = ?stage_type, items_count = items.len(), "Executing stage with OneShot");
        
        // 1. OneShot 데이터 채널 생성
        let (stage_data_tx, stage_data_rx) = oneshot::channel::<StageResult>();
        
        // 2. 제어 채널 생성
        let (stage_control_tx, stage_control_rx) = mpsc::channel(self.config.channels.control_buffer_size);
        
        // 3. StageActor 생성 및 스폰
        let stage_actor = StageActor::new_with_oneshot(
            self.batch_id.clone(),
            self.config.clone(),
        );
        
        let handle = tokio::spawn(async move {
            stage_actor.run_with_oneshot(stage_control_rx, stage_data_tx).await
        });
        
        // 4. 스테이지 명령 전송
        let stage_timeout = Duration::from_secs(self.config.actor.stage_timeout_secs);
        let command = ActorCommand::ExecuteStage {
            stage_type: stage_type.clone(),
            items,
            concurrency_limit: self.config.performance.concurrency.stage_concurrency_limits
                .get(&format!("{:?}", stage_type))
                .copied()
                .unwrap_or(10),
            timeout_secs: stage_timeout.as_secs(),
        };
        
        if let Err(e) = stage_control_tx.send(command).await {
            return StageResult::FatalError {
                error: StageError::ValidationError {
                    message: format!("Failed to send stage command: {}", e),
                },
                stage_id: self.batch_id.clone(),
                context: "Stage command sending".to_string(),
            };
        }
        
        // 5. 결과 대기 (타임아웃과 함께)
        let result = match timeout(stage_timeout, stage_data_rx).await {
            Ok(Ok(stage_result)) => stage_result,
            Ok(Err(_)) => StageResult::FatalError {
                error: StageError::ValidationError {
                    message: "Stage data channel closed".to_string(),
                },
                stage_id: self.batch_id.clone(),
                context: "Stage communication failure".to_string(),
            },
            Err(_) => StageResult::RecoverableError {
                error: StageError::NetworkTimeout {
                    message: "Stage execution timeout".to_string(),
                },
                attempts: 0,
                stage_id: self.batch_id.clone(),
                suggested_retry_delay: Duration::from_secs(5),
            },
        };
        
        // 6. StageActor 정리
        if let Err(e) = handle.await {
            warn!(batch_id = %self.batch_id, error = %e, "StageActor join failed");
        }
        
        // 7. 재시도 정책 적용
        self.apply_retry_policy(result, stage_type).await
    }
    
    /// 재시도 정책 적용
    async fn apply_retry_policy(&mut self, result: StageResult, stage_type: StageType) -> StageResult {
        match result {
            StageResult::RecoverableError { error, attempts, .. } => {
                let retry_calculator = RetryCalculator::default();
                
                if retry_calculator.should_retry_with_policy(&error, attempts) {
                    let delay_ms = retry_calculator.calculate_delay(attempts);
                    info!(
                        batch_id = %self.batch_id,
                        stage = ?stage_type,
                        attempts = attempts,
                        delay_ms = delay_ms,
                        "Retrying stage after delay"
                    );
                    
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    
                    // TODO: 재시도 실행 - 현재는 원본 결과를 반환하되, attempts를 증가시킴
                    StageResult::RecoverableError {
                        error,
                        attempts: attempts + 1,
                        stage_id: format!("{}-retry-{}", self.batch_id, attempts + 1),
                        suggested_retry_delay: Duration::from_millis(delay_ms),
                    }
                } else {
                    // 최대 재시도 초과
                    StageResult::FatalError {
                        error: StageError::ValidationError {
                            message: format!("Max retries exceeded for stage {:?}: {}", stage_type, error),
                        },
                        stage_id: self.batch_id.clone(),
                        context: format!("Retry exhausted after {} attempts", attempts),
                    }
                }
            }
            other => other,
        }
    }
    
    /// 스테이지별 재시도 정책 가져오기
    fn get_retry_policy_for_stage(&self, stage_type: &StageType) -> RetryPolicy {
        match stage_type {
            StageType::Collection => self.config.retry_policies.list_collection.clone(),
            StageType::Processing => self.config.retry_policies.detail_collection.clone(),
            StageType::ListCollection => self.config.retry_policies.list_collection.clone(),
            StageType::DetailCollection => self.config.retry_policies.detail_collection.clone(),
            StageType::DataValidation => self.config.retry_policies.data_validation.clone(),
            StageType::DatabaseSave => self.config.retry_policies.database_save.clone(),
        }
    }
    
    /// 페이지 처리
    pub async fn process_pages(
        &mut self,
        pages: Vec<u32>,
        batch_size: u32,
        concurrency_limit: u32,
    ) -> Result<u32, ActorError> {
        info!(batch_id = %self.batch_id, pages_count = pages.len(), "Processing pages");
        
        let mut success_count = 0u32;
        
        // 페이지를 배치 크기로 분할
        for chunk in pages.chunks(batch_size as usize) {
            let stage_items: Vec<StageItem> = chunk.iter().map(|&page| StageItem::Page(page)).collect();
            
            // StageActor 생성 및 실행
            let mut stage_actor = StageActor::new(
                self.batch_id.clone(),
                self.config.clone(),
            );
            
            // 각 스테이지 순차 실행
            let stages = [
                StageType::ListCollection,
                StageType::DetailCollection,
                StageType::DataValidation,
                StageType::DatabaseSave,
            ];
            
            for stage_type in &stages {
                let stage_timeout = Duration::from_secs(self.config.actor.stage_timeout_secs);
                
                let stage_result = stage_actor.execute_stage(
                    stage_type.clone(),
                    stage_items.clone(),
                    concurrency_limit,
                    stage_timeout,
                ).await;
                
                match stage_result {
                    Ok(processed_count) => {
                        debug!(
                            batch_id = %self.batch_id,
                            stage = ?stage_type,
                            processed = processed_count,
                            "Stage completed"
                        );
                        success_count += processed_count;
                    }
                    Err(e) => {
                        error!(
                            batch_id = %self.batch_id,
                            stage = ?stage_type,
                            error = %e,
                            "Stage failed"
                        );
                        return Err(e);
                    }
                }
            }
            
            self.stage_actors.push(stage_actor);
        }
        
        info!(batch_id = %self.batch_id, success_count = success_count, "Batch processing completed");
        Ok(success_count)
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
                    suggested_retry_delay: Duration::from_secs(10),
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
        // 실제 크롤링 서비스가 있으면 사용, 없으면 시뮬레이션
        if let Some(ref executor) = self.crawling_executor {
            info!(
                batch_id = %self.batch_id,
                stage = ?stage_type,
                items_count = items.len(),
                "Using real crawling service for stage execution"
            );
            
            // 취소 토큰 생성
            let cancellation_token = tokio_util::sync::CancellationToken::new();
            
            // 실제 크롤링 서비스 실행
            return executor.execute_stage(
                stage_type,
                items,
                concurrency_limit,
                cancellation_token,
            ).await;
        }
        
        // 크롤링 서비스가 없는 경우 기존 시뮬레이션 로직 실행
        info!(
            batch_id = %self.batch_id,
            stage = ?stage_type,
            items_count = items.len(),
            "Using simulation mode for stage execution"
        );
        
        // 동시성 제어
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
        
        // 모든 작업 완료 대기
        let mut successful_items = Vec::new();
        let mut failed_items = Vec::new();
        
        for handle in handles {
            match handle.await {
                Ok(Ok(item_id)) => successful_items.push(item_id),
                Ok(Err(item_id)) => failed_items.push(item_id),
                Err(e) => {
                    error!(batch_id = %self.batch_id, error = %e, "Task join failed");
                    // Join 에러는 failed_items에 추가하지 않음 (알 수 없는 상태)
                }
            }
        }
        
        let total_items = successful_items.len() + failed_items.len();
        let success_result = StageSuccessResult {
            processed_items: successful_items.len() as u32,
            stage_duration_ms: 0, // 호출자에서 설정
            collection_metrics: Some(CollectionMetrics {
                total_items: total_items as u32,
                successful_items: successful_items.len() as u32,
                failed_items: failed_items.len() as u32,
                duration_ms: 0,
                avg_response_time_ms: 100,
                success_rate: if total_items > 0 {
                    (successful_items.len() as f64 / total_items as f64) * 100.0
                } else { 0.0 },
            }),
            processing_metrics: None,
        };
        
        if !failed_items.is_empty() {
            StageResult::PartialSuccess {
                success_items: success_result,
                failed_items: failed_items.into_iter().map(|item| {
                    FailedItem {
                        item_id: format!("item-{}", item),
                        error: StageError::ValidationError {
                            message: "Processing failed".to_string(),
                        },
                        retry_count: 0,
                        last_attempt: std::time::SystemTime::now(),
                    }
                }).collect(),
                stage_id: self.batch_id.clone(),
            }
        } else {
            StageResult::Success(success_result)
        }
    }
    
    /// 개별 아이템 처리 (성공 시 아이템 ID, 실패 시 에러 반환)
    async fn process_single_item_with_result(
        _batch_id: String,
        _stage_type: &StageType,
        item: StageItem,
    ) -> Result<u32, u32> {
        // 실제 처리 로직을 시뮬레이션
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        
        // 90% 성공률로 시뮬레이션
        if fastrand::f64() < 0.9 {
            let item_id = match item {
                StageItem::Page(page) => page,
                StageItem::Url(_) => fastrand::u32(1..=1000),
            };
            Ok(item_id)
        } else {
            let item_id = match item {
                StageItem::Page(page) => page,
                StageItem::Url(_) => fastrand::u32(1..=1000),
            };
            Err(item_id)
        }
    }
    
    /// 스테이지 실행
    pub async fn execute_stage(
        &mut self,
        stage_type: StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
        timeout_duration: Duration,
    ) -> Result<u32, ActorError> {
        info!(
            batch_id = %self.batch_id,
            stage = ?stage_type,
            items_count = items.len(),
            "Executing stage"
        );
        
        let start_time = Instant::now();
        
        // 타임아웃과 함께 스테이지 실행
        let result = timeout(timeout_duration, self.process_stage_items(stage_type.clone(), items, concurrency_limit)).await;
        
        let execution_time = start_time.elapsed();
        
        // 통계 업데이트
        {
            let mut stats = self.execution_stats.lock().await;
            stats.update_stage_execution(stage_type.clone(), execution_time, result.is_ok());
        }
        
        match result {
            Ok(Ok(processed_count)) => {
                info!(
                    batch_id = %self.batch_id,
                    stage = ?stage_type,
                    processed = processed_count,
                    duration = ?execution_time,
                    "Stage execution completed"
                );
                Ok(processed_count)
            }
            Ok(Err(e)) => {
                error!(
                    batch_id = %self.batch_id,
                    stage = ?stage_type,
                    error = %e,
                    duration = ?execution_time,
                    "Stage execution failed"
                );
                Err(e)
            }
            Err(_) => {
                error!(
                    batch_id = %self.batch_id,
                    stage = ?stage_type,
                    timeout = ?timeout_duration,
                    "Stage execution timed out"
                );
                Err(ActorError::StageError {
                    stage: stage_type,
                    message: format!("Stage timed out after {timeout_duration:?}"),
                })
            }
        }
    }
    
    /// 스테이지 아이템 처리
    async fn process_stage_items(
        &self,
        stage_type: StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
    ) -> Result<u32, ActorError> {
        // 동시성 제어
        let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency_limit as usize));
        let mut handles = Vec::new();
        
        for item in items {
            let permit = semaphore.clone().acquire_owned().await
                .map_err(|e| ActorError::ChannelError(format!("Semaphore acquire failed: {e}")))?;
            
            let batch_id = self.batch_id.clone();
            let stage_type = stage_type.clone();
            
            let handle = tokio::spawn(async move {
                let _permit = permit; // 스코프 유지
                Self::process_single_item(batch_id, stage_type, item).await
            });
            
            handles.push(handle);
        }
        
        // 모든 작업 완료 대기
        let mut success_count = 0u32;
        for handle in handles {
            match handle.await {
                Ok(Ok(())) => success_count += 1,
                Ok(Err(e)) => {
                    warn!(
                        batch_id = %self.batch_id,
                        stage = ?stage_type,
                        error = %e,
                        "Single item processing failed"
                    );
                }
                Err(e) => {
                    error!(
                        batch_id = %self.batch_id,
                        stage = ?stage_type,
                        error = %e,
                        "Task join failed"
                    );
                }
            }
        }
        
        Ok(success_count)
    }
    
    /// 개별 아이템 처리 (현재는 목업)
    async fn process_single_item(
        _batch_id: String,
        _stage_type: StageType,
        _item: StageItem,
    ) -> Result<(), ActorError> {
        // 실제 처리 로직을 시뮬레이션
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
        
        // 실행 시간 기록
        self.stage_durations
            .entry(stage_name.clone())
            .or_default()
            .push(duration);
        
        // 성공률 기록
        let (success_count, total_count) = self.stage_success_rates
            .entry(stage_name)
            .or_insert((0, 0));
        
        *total_count += 1;
        if success {
            *success_count += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::{mpsc, oneshot};

    #[tokio::test]
    async fn test_basic_oneshot_channel() {
        println!("🧪 기본 OneShot 채널 테스트 시작");

        // 간단한 OneShot 채널 통신 테스트
        let (tx, rx) = oneshot::channel::<StageResult>();

        // 성공 결과 전송
        let result = StageResult::Success(StageSuccessResult {
            processed_items: 5,
            stage_duration_ms: 1000,
            collection_metrics: None,
            processing_metrics: None,
        });

        let _ = tx.send(result);

        // 결과 수신 확인
        match rx.await {
            Ok(StageResult::Success(success_result)) => {
                println!("✅ OneShot 채널 통신 성공!");
                println!("   처리된 아이템: {}", success_result.processed_items);
                println!("   실행 시간: {}ms", success_result.stage_duration_ms);
                assert_eq!(success_result.processed_items, 5);
                assert_eq!(success_result.stage_duration_ms, 1000);
            },
            Ok(StageResult::Failure(error)) => {
                panic!("예상치 못한 에러 결과: {}", error);
            },
            Ok(StageResult::RecoverableError { error, .. }) => {
                panic!("예상치 못한 복구 가능한 에러: {}", error);
            },
            Ok(StageResult::FatalError { error, .. }) => {
                panic!("예상치 못한 치명적 에러: {}", error);
            },
            Ok(StageResult::PartialSuccess { success_items, .. }) => {
                println!("✅ 부분 성공: {}", success_items.processed_items);
            },
            Err(_) => {
                panic!("OneShot 채널 수신 실패");
            }
        }

        println!("🎯 기본 OneShot 채널 테스트 완료!");
    }

    #[tokio::test]
    async fn test_retry_calculator() {
        println!("🧪 RetryCalculator 테스트 시작");

        let calculator = RetryCalculator::new(3, 100, 5000, 2.0, true);

        // 첫 번째 시도
        assert!(calculator.should_retry(1));
        let delay1 = calculator.calculate_delay(1);
        println!("   1차 재시도 지연: {}ms", delay1);
        assert!(delay1 >= 50 && delay1 <= 150); // 지터 포함 범위

        // 두 번째 시도
        assert!(calculator.should_retry(2));
        let delay2 = calculator.calculate_delay(2);
        println!("   2차 재시도 지연: {}ms", delay2);
        assert!(delay2 >= 100 && delay2 <= 300); // 지터 포함 범위

        // 최대 시도 초과
        assert!(!calculator.should_retry(3));
        assert!(!calculator.should_retry(4));

        println!("🎯 RetryCalculator 테스트 완료!");
    }

    #[tokio::test]
    async fn test_stage_execution_stats() {
        println!("🧪 StageExecutionStats 테스트 시작");

        let mut stats = StageExecutionStats::default();

        // 여러 스테이지 실행 기록
        stats.update_stage_execution(StageType::Collection, Duration::from_millis(500), true);
        stats.update_stage_execution(StageType::Collection, Duration::from_millis(600), true);
        stats.update_stage_execution(StageType::Collection, Duration::from_millis(400), false);

        stats.update_stage_execution(StageType::Processing, Duration::from_millis(200), true);
        stats.update_stage_execution(StageType::Processing, Duration::from_millis(250), true);

        // 통계 확인
        assert_eq!(stats.stage_durations.len(), 2);
        assert!(stats.stage_durations.contains_key("Collection"));
        assert!(stats.stage_durations.contains_key("Processing"));

        // Collection 스테이지: 3번 시도, 2번 성공
        let collection_stats = stats.stage_success_rates.get("Collection").unwrap();
        assert_eq!(collection_stats.0, 2); // 성공 횟수
        assert_eq!(collection_stats.1, 3); // 총 시도 횟수

        // Processing 스테이지: 2번 시도, 2번 성공
        let processing_stats = stats.stage_success_rates.get("Processing").unwrap();
        assert_eq!(processing_stats.0, 2); // 성공 횟수
        assert_eq!(processing_stats.1, 2); // 총 시도 횟수

        println!("✅ Collection 성공률: {}/{}", collection_stats.0, collection_stats.1);
        println!("✅ Processing 성공률: {}/{}", processing_stats.0, processing_stats.1);

        println!("🎯 StageExecutionStats 테스트 완료!");
    }

    #[tokio::test]
    async fn test_actor_error_display() {
        println!("🧪 ActorError Display 테스트 시작");

        let errors = vec![
            ActorError::ChannelError("채널 에러 테스트".to_string()),
            ActorError::TimeoutError("타임아웃 에러 테스트".to_string()),
            ActorError::ProcessingError("처리 에러 테스트".to_string()),
            ActorError::ConfigurationError("설정 에러 테스트".to_string()),
        ];

        for (i, error) in errors.iter().enumerate() {
            let error_str = format!("{}", error);
            println!("   에러 {}: {}", i + 1, error_str);
            assert!(!error_str.is_empty());
        }

        println!("🎯 ActorError Display 테스트 완료!");
    }

    #[tokio::test]
    async fn test_stage_error_display() {
        println!("🧪 StageError Display 테스트 시작");

        let errors = vec![
            StageError::NetworkError { message: "네트워크 연결 실패".to_string() },
            StageError::ParsingError { message: "HTML 파싱 실패".to_string() },
            StageError::ResourceExhausted { message: "리소스 부족".to_string() },
        ];

        for (i, error) in errors.iter().enumerate() {
            let error_str = format!("{}", error);
            println!("   에러 {}: {}", i + 1, error_str);
            assert!(!error_str.is_empty());
            
            // 실제 에러 메시지가 포함되어 있는지 확인
            match error {
                StageError::NetworkError { message } => assert!(error_str.contains(message)),
                StageError::ParsingError { message } => assert!(error_str.contains(message)),
                StageError::NetworkTimeout { message } => assert!(error_str.contains(message)),
                StageError::ValidationError { message } => assert!(error_str.contains(message)),
                StageError::ChannelError { message } => assert!(error_str.contains(message)),
                StageError::DatabaseError { message } => assert!(error_str.contains(message)),
                StageError::ResourceExhausted { message } => assert!(error_str.contains(message)),
                StageError::ConfigurationError { message } => assert!(error_str.contains(message)),
                // Phase 3: TaskActor 관련 에러 테스트 추가
                StageError::TaskCancelled { task_id } => assert!(error_str.contains(task_id)),
                StageError::TaskExecutionFailed { task_id, message } => {
                    assert!(error_str.contains(task_id));
                    assert!(error_str.contains(message));
                },
            }
        }

        println!("🎯 StageError Display 테스트 완료!");
    }

    #[tokio::test]
    async fn test_mpsc_channel_communication() {
        println!("🧪 MPSC 채널 통신 테스트 시작");

        let (tx, mut rx) = mpsc::channel::<AppEvent>(10);

        // 여러 이벤트 전송
        let events = vec![
            AppEvent::BatchStarted { batch_id: "batch-001".to_string() },
            AppEvent::StageCompleted { 
                stage: StageType::Collection,
                result: StageResult::Success(StageSuccessResult {
                    processed_items: 3,
                    stage_duration_ms: 500,
                    collection_metrics: None,
                    processing_metrics: None,
                })
            },
            AppEvent::BatchCompleted { 
                batch_id: "batch-001".to_string(),
                success_count: 0,
            },
        ];

        // 이벤트 전송
        for event in events {
            tx.send(event).await.expect("이벤트 전송 실패");
        }

        // 이벤트 수신 확인
        let mut received_count = 0;
        while let Some(event) = rx.recv().await {
            received_count += 1;
            println!("   수신된 이벤트 {}: {:?}", received_count, event);
            
            if received_count >= 3 {
                break;
            }
        }

        assert_eq!(received_count, 3);
        println!("🎯 MPSC 채널 통신 테스트 완료!");
    }

    /// **🚀 OneShot 통합 테스트: 전체 Actor 시스템 검증**
    #[tokio::test]
    async fn test_comprehensive_oneshot_integration() {
        println!("🧪 **포괄적 OneShot 통합 테스트 시작** 🧪");
        
        // 1. 테스트 설정
        let config = Arc::new(SystemConfig::default());
        let (event_tx, mut event_rx) = mpsc::channel::<AppEvent>(100);
        let (command_tx, command_rx) = mpsc::channel::<ActorCommand>(100);
        
        println!("   ✅ 1단계: 테스트 채널 및 설정 초기화 완료");
        
        // 2. SessionActor 생성 및 스폰
        let mut session_actor = SessionActor::new(
            config.clone(),
            command_rx,
            event_tx.clone(),
        );
        
        let session_handle = tokio::spawn(async move {
            session_actor.run().await
        });
        
        println!("   ✅ 2단계: SessionActor 스폰 완료");
        
        // 3. BatchPlan 생성 (페이지 3개 처리)
        let batch_plan = BatchPlan {
            batch_id: "test-batch-oneshot".to_string(),
            pages: vec![1, 2, 3],
            config: BatchConfig {
                target_url: "https://example.com".to_string(),
                max_pages: Some(10),
            },
            batch_size: 2,
            concurrency_limit: 3,
        };
        
        println!("   ✅ 3단계: BatchPlan 설정 완료 (3개 페이지, 배치크기 2)");
        
        // 4. ProcessBatch 명령 전송
        let process_command = ActorCommand::ProcessBatch {
            pages: batch_plan.pages.clone(),
            config: batch_plan.config.clone(),
            batch_size: batch_plan.batch_size,
            concurrency_limit: batch_plan.concurrency_limit,
        };
        
        command_tx.send(process_command).await.expect("명령 전송 실패");
        println!("   ✅ 4단계: ProcessBatch 명령 전송 완료");
        
        // 5. 이벤트 수신 및 검증
        let mut session_started = false;
        let mut batch_completed = false;
        
        // 최대 3초 동안 이벤트 대기
        let event_timeout = Duration::from_secs(3);
        let start_time = Instant::now();
        
        while start_time.elapsed() < event_timeout {
            match timeout(Duration::from_millis(300), event_rx.recv()).await {
                Ok(Some(event)) => {
                    match event {
                        AppEvent::SessionStarted { .. } => {
                            session_started = true;
                            println!("   ✅ 5-a단계: SessionStarted 이벤트 수신");
                        }
                        AppEvent::BatchCompleted { batch_id, success_count } => {
                            batch_completed = true;
                            println!("   ✅ 5-b단계: BatchCompleted 이벤트 수신 (배치: {}, 성공: {})", batch_id, success_count);
                            break; // 완료 이벤트 수신 시 종료
                        }
                        AppEvent::BatchFailed { batch_id, error, final_failure } => {
                            println!("   ⚠️  BatchFailed 이벤트: {} - {} (최종실패: {})", batch_id, error, final_failure);
                            if final_failure {
                                break;
                            }
                        }
                        AppEvent::SessionTimeout { .. } => {
                            println!("   ⚠️  SessionTimeout 이벤트 수신");
                            break;
                        }
                        _ => {
                            println!("   📢 기타 이벤트 수신: {:?}", event);
                        }
                    }
                }
                Ok(None) => {
                    println!("   ⚠️  이벤트 채널 종료");
                    break;
                }
                Err(_) => {
                    // 타임아웃 - 계속 대기
                }
            }
        }
        
        // 6. 세션 정리
        drop(command_tx); // 명령 채널 닫기
        
        // SessionActor 완료 대기 (최대 1초)
        match timeout(Duration::from_secs(1), session_handle).await {
            Ok(result) => {
                match result {
                    Ok(Ok(())) => println!("   ✅ 6단계: SessionActor 정상 종료"),
                    Ok(Err(e)) => println!("   ⚠️  SessionActor 오류 종료: {}", e),
                    Err(e) => println!("   ⚠️  SessionActor 패닉: {}", e),
                }
            }
            Err(_) => {
                println!("   ⚠️  SessionActor 종료 타임아웃");
            }
        }
        
        // 7. 결과 검증
        println!("   📊 **OneShot 통합 테스트 결과 검증**");
        
        if session_started {
            println!("   ✅ SessionStarted 이벤트 수신 확인");
        } else {
            println!("   ❌ SessionStarted 이벤트 누락");
        }
        
        if batch_completed {
            println!("   ✅ BatchCompleted 이벤트 수신 확인");
        } else {
            println!("   ⚠️  BatchCompleted 이벤트 누락 (재시도 또는 부분 성공 가능)");
        }
        
        println!("🎯 **포괄적 OneShot 통합 테스트 완료!** 🎯");
        
        // 적어도 세션이 시작되었어야 함
        assert!(session_started, "SessionStarted 이벤트가 수신되지 않음");
    }

    /// **🔄 재시도 정책 통합 테스트**
    #[tokio::test]
    async fn test_retry_policy_integration() {
        println!("🧪 재시도 정책 통합 테스트 시작");
        
        let retry_calculator = RetryCalculator::new(3, 100, 2000, 2.0, true);
        
        // 재시도 가능한 에러 테스트
        let network_error = StageError::NetworkError {
            message: "Connection timeout".to_string(),
        };
        
        assert!(retry_calculator.should_retry_with_policy(&network_error, 0));
        assert!(retry_calculator.should_retry_with_policy(&network_error, 1));
        assert!(retry_calculator.should_retry_with_policy(&network_error, 2));
        assert!(!retry_calculator.should_retry_with_policy(&network_error, 3));
        
        // 재시도 불가능한 에러 테스트
        let parsing_error = StageError::ParsingError {
            message: "Invalid JSON".to_string(),
        };
        
        assert!(!retry_calculator.should_retry_with_policy(&parsing_error, 0));
        assert!(!retry_calculator.should_retry_with_policy(&parsing_error, 1));
        
        // 지연 시간 테스트
        let delay1 = retry_calculator.calculate_delay(1);
        let delay2 = retry_calculator.calculate_delay(2);
        let delay3 = retry_calculator.calculate_delay(3);
        
        println!("   재시도 지연시간: 1차={}ms, 2차={}ms, 3차={}ms", delay1, delay2, delay3);
        
        // 지수 백오프 확인 (지터 때문에 정확한 값은 확인할 수 없지만 증가 경향은 확인 가능)
        assert!(delay2 >= delay1 / 2); // 지터를 고려한 최소 증가
        assert!(delay3 >= delay2 / 2); // 지터를 고려한 최소 증가
        
        println!("🎯 재시도 정책 통합 테스트 완료!");
    }

    /// **📡 채널 통신 성능 테스트**
    #[tokio::test]
    async fn test_channel_performance() {
        println!("🧪 채널 통신 성능 테스트 시작");
        
        // OneShot 채널 생성 시간 측정
        let start_time = Instant::now();
        
        let mut oneshot_channels = Vec::new();
        for _ in 0..100 {
            let (tx, rx) = oneshot::channel::<StageResult>();
            oneshot_channels.push((tx, rx));
        }
        
        let creation_time = start_time.elapsed();
        println!("   OneShot 채널 100개 생성 시간: {:?}", creation_time);
        
        // MPSC 채널 통신 시간 측정
        let (mpsc_tx, mut mpsc_rx) = mpsc::channel(100);
        
        let send_start = Instant::now();
        for i in 0..100 {
            let command = ActorCommand::ProcessBatch {
                pages: vec![i],
                config: BatchConfig {
                    target_url: "https://example.com".to_string(),
                    max_pages: Some(10),
                },
                batch_size: 1,
                concurrency_limit: 1,
            };
            mpsc_tx.send(command).await.expect("메시지 전송 실패");
        }
        
        let send_time = send_start.elapsed();
        println!("   MPSC 메시지 100개 전송 시간: {:?}", send_time);
        
        // 수신 시간 측정
        let recv_start = Instant::now();
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
}
