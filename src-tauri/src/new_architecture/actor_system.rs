//! Actor 시스템: 세션, 배치, 스테이지 분리 구조
//! Modern Rust 2024 준수: 의존성 주입 기반 Actor 설계

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex};
use tokio::time::{sleep, timeout};
use tracing::{info, warn, error, debug};
use uuid::Uuid;

use crate::new_architecture::{
    system_config::{SystemConfig, ConfigError},
    channel_types::{ActorCommand, AppEvent, BatchConfig, StageType, StageItem},
};

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
                    continue;
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
    
    /// 배치 처리 시작
    async fn process_batch(
        &mut self,
        pages: Vec<u32>,
        config: BatchConfig,
        batch_size: u32,
        concurrency_limit: u32,
    ) -> Result<(), ActorError> {
        let batch_id = Uuid::new_v4().to_string();
        info!(session_id = %self.session_id, batch_id = %batch_id, "Starting batch processing");
        
        let event = AppEvent::SessionStarted {
            session_id: self.session_id.clone(),
            config: config.clone(),
        };
        
        if let Err(e) = self.event_tx.send(event).await {
            return Err(ActorError::ChannelError(format!("Failed to send session start event: {e}")));
        }
        
        // BatchActor 생성 및 실행
        let mut batch_actor = BatchActor::new(
            batch_id.clone(),
            self.config.clone(),
            self.event_tx.clone(),
        );
        
        let result = batch_actor.process_pages(pages, batch_size, concurrency_limit).await;
        
        match result {
            Ok(success_count) => {
                let event = AppEvent::BatchCompleted {
                    batch_id,
                    success_count,
                };
                
                if let Err(e) = self.event_tx.send(event).await {
                    warn!(session_id = %self.session_id, error = %e, "Failed to send completion event");
                }
            }
            Err(e) => {
                let event = AppEvent::BatchFailed {
                    batch_id,
                    error: e.to_string(),
                    final_failure: true,
                };
                
                if let Err(send_err) = self.event_tx.send(event).await {
                    error!(session_id = %self.session_id, error = %send_err, "Failed to send failure event");
                }
                
                return Err(e);
            }
        }
        
        self.batch_actors.push(batch_actor);
        Ok(())
    }
}

/// BatchActor - 배치 단위 처리 관리
pub struct BatchActor {
    batch_id: String,
    config: Arc<SystemConfig>,
    event_tx: mpsc::Sender<AppEvent>,
    stage_actors: Vec<StageActor>,
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
    batch_id: String,
    config: Arc<SystemConfig>,
    execution_stats: Arc<Mutex<StageExecutionStats>>,
}

impl StageActor {
    pub fn new(batch_id: String, config: Arc<SystemConfig>) -> Self {
        Self {
            batch_id,
            config,
            execution_stats: Arc::new(Mutex::new(StageExecutionStats::default())),
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
