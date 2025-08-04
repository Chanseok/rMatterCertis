//! BatchActor: 배치 단위 크롤링 처리 Actor
//! 
//! Phase 3: Actor 구현 - 배치 레벨 작업 관리 및 실행
//! Modern Rust 2024 준수: 함수형 원칙, 명시적 의존성, 상태 최소화

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Semaphore};
use tracing::{info, warn, error, debug};
use uuid::Uuid;
use chrono::{DateTime, Utc};

use super::traits::{Actor, ActorHealth, ActorStatus, ActorType};
use super::types::{ActorCommand, BatchConfig, StageType, StageItem, ActorError};
use crate::new_architecture::channels::types::AppEvent;
use crate::new_architecture::context::{AppContext, EventEmitter};
use crate::new_architecture::migration::ServiceMigrationBridge;

/// BatchActor: 배치 단위의 크롤링 작업 관리
/// 
/// 책임:
/// - 배치 내 페이지들의 병렬 처리 관리
/// - StageActor들의 조정 및 스케줄링
/// - 배치 레벨 이벤트 발행
/// - 동시성 제어 및 리소스 관리
#[derive(Debug)]
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
    /// ServiceBased 로직 브릿지 (Phase 2 호환성)
    migration_bridge: Option<Arc<ServiceMigrationBridge>>,
    /// 설정 (OneShot 호환성)
    pub config: Option<Arc<crate::new_architecture::config::SystemConfig>>,
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
    
    #[error("Migration bridge error: {0}")]
    MigrationError(String),
}

impl BatchActor {
    /// 새로운 BatchActor 인스턴스 생성
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
            migration_bridge: None,
            config: None,
        }
    }
    
    /// ServiceMigrationBridge 설정 (Phase 2 호환성)
    /// 
    /// # Arguments
    /// * `bridge` - 마이그레이션 브릿지
    pub fn with_migration_bridge(mut self, bridge: Arc<ServiceMigrationBridge>) -> Self {
        self.migration_bridge = Some(bridge);
        self
    }
    
    /// 배치 처리 시작
    /// 
    /// # Arguments
    /// * `batch_id` - 배치 ID
    /// * `pages` - 처리할 페이지 번호 리스트
    /// * `config` - 배치 설정
    /// * `context` - Actor 컨텍스트
    async fn handle_process_batch(
        &mut self,
        batch_id: String,
        pages: Vec<u32>,
        config: BatchConfig,
        batch_size: u32,
        concurrency_limit: u32,
        total_pages: u32,
        products_on_last_page: u32,
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
        
        // Phase 2 호환성: ServiceMigrationBridge 사용
        if let Some(bridge) = &self.migration_bridge {
            match bridge.execute_batch_crawling(pages, config).await {
                Ok(result) => {
                    self.success_count = result.processed_items;
                    self.completed_pages = self.total_pages;
                    self.state = BatchState::Completed;
                    
                    // 완료 이벤트 발행
                    let completion_event = AppEvent::BatchCompleted {
                        batch_id: batch_id.clone(),
                        session_id: context.session_id.clone(),
                        success_count: self.success_count,
                        failed_count: self.failure_count,
                        duration: result.duration_ms,
                        timestamp: Utc::now(),
                    };
                    
                    context.emit_event(completion_event).await
                        .map_err(|e| BatchError::ContextError(e.to_string()))?;
                    
                    info!("✅ Batch {} completed successfully with {} items", 
                          batch_id, self.success_count);
                }
                Err(e) => {
                    let error_msg = format!("Migration bridge error: {}", e);
                    self.state = BatchState::Failed { error: error_msg.clone() };
                    
                    // 실패 이벤트 발행
                    let failure_event = AppEvent::BatchFailed {
                        batch_id: batch_id.clone(),
                        session_id: context.session_id.clone(),
                        error: error_msg.clone(),
                        final_failure: true,
                        timestamp: Utc::now(),
                    };
                    
                    context.emit_event(failure_event).await
                        .map_err(|e| BatchError::ContextError(e.to_string()))?;
                    
                    return Err(BatchError::MigrationError(error_msg));
                }
            }
        } else {
            // TODO: Phase 3에서 실제 StageActor 기반 처리 구현
            warn!("No migration bridge available, using placeholder logic");
            
            // 임시: 간단한 처리 시뮬레이션
            self.success_count = pages.len() as u32;
            self.completed_pages = self.total_pages;
            self.state = BatchState::Completed;
            
            let completion_event = AppEvent::BatchCompleted {
                batch_id: batch_id.clone(),
                session_id: context.session_id.clone(),
                success_count: self.success_count,
                failed_count: 0,
                duration: 1000, // Placeholder: 1 second
                timestamp: Utc::now(),
            };
            
            context.emit_event(completion_event).await
                .map_err(|e| BatchError::ContextError(e.to_string()))?;
        }
        
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
