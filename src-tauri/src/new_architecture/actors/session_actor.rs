//! SessionActor: 크롤링 세션 관리 Actor
//! 
//! Phase 3: Actor 구현 - 세션 레벨 제어 및 모니터링
//! Modern Rust 2024 준수: 함수형 원칙, 명시적 의존성, 상태 최소화

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, broadcast};
use tokio::time::timeout;
use tracing::{info, warn, error, debug};
use uuid::Uuid;
use chrono::{DateTime, Utc};

use super::traits::{Actor, ActorHealth, ActorStatus, ActorType};
use super::types::{ActorCommand, AppEvent, CrawlingConfig, SessionSummary, ActorError};
use crate::new_architecture::context::{AppContext, EventEmitter};
use crate::new_architecture::migration::ServiceMigrationBridge;

/// SessionActor: 크롤링 세션의 전체 생명주기 관리
/// 
/// 책임:
/// - 세션 시작/일시정지/재개/종료 제어
/// - 배치 Actor들의 조정 및 모니터링
/// - 세션 레벨 이벤트 발행
/// - 전체 세션 상태 추적
#[derive(Debug)]
pub struct SessionActor {
    /// Actor 고유 식별자
    actor_id: String,
    /// 현재 관리 중인 세션 ID
    session_id: Option<String>,
    /// 세션 상태
    state: SessionState,
    /// 세션 시작 시간
    start_time: Option<Instant>,
    /// 처리된 배치 수
    processed_batches: u32,
    /// 총 성공 아이템 수
    total_success_count: u32,
    /// ServiceBased 로직 브릿지 (Phase 2 호환성)
    migration_bridge: Option<Arc<ServiceMigrationBridge>>,
}

/// 세션 상태 열거형
#[derive(Debug, Clone, PartialEq)]
pub enum SessionState {
    Idle,
    Starting,
    Running,
    Paused { reason: String },
    Completing,
    Completed,
    Failed { error: String },
}

/// 세션 관련 에러 타입
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("Session initialization failed: {0}")]
    InitializationFailed(String),
    
    #[error("Session already running: {0}")]
    AlreadyRunning(String),
    
    #[error("Session not found: {0}")]
    SessionNotFound(String),
    
    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidStateTransition { from: SessionState, to: SessionState },
    
    #[error("Context communication error: {0}")]
    ContextError(String),
    
    #[error("Migration bridge error: {0}")]
    MigrationError(String),
}

impl SessionActor {
    /// 새로운 SessionActor 인스턴스 생성
    /// 
    /// # Arguments
    /// * `actor_id` - Actor 고유 식별자
    /// 
    /// # Returns
    /// * `Self` - 새로운 SessionActor 인스턴스
    pub fn new(actor_id: String) -> Self {
        Self {
            actor_id,
            session_id: None,
            state: SessionState::Idle,
            start_time: None,
            processed_batches: 0,
            total_success_count: 0,
            migration_bridge: None,
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
    
    /// 세션 시작 처리
    /// 
    /// # Arguments
    /// * `session_id` - 시작할 세션 ID
    /// * `config` - 크롤링 설정
    /// * `context` - Actor 컨텍스트
    /// 
    /// # Returns
    /// * `Result<(), SessionError>` - 성공 시 (), 실패 시 에러
    async fn handle_start_crawling(
        &mut self,
        session_id: String,
        config: CrawlingConfig,
        context: &AppContext,
    ) -> Result<(), SessionError> {
        // 상태 검증
        if !matches!(self.state, SessionState::Idle) {
            return Err(SessionError::AlreadyRunning(session_id));
        }
        
        info!("🚀 SessionActor {} starting session {}", self.actor_id, session_id);
        
        // 상태 업데이트
        self.session_id = Some(session_id.clone());
        self.state = SessionState::Starting;
        self.start_time = Some(Instant::now());
        self.processed_batches = 0;
        self.total_success_count = 0;
        
        // 세션 시작 이벤트 발행
        let start_event = AppEvent::SessionStarted {
            session_id: session_id.clone(),
            config: config.clone(),
            timestamp: Utc::now(),
        };
        
        context.emit_event(start_event).await
            .map_err(|e| SessionError::ContextError(e.to_string()))?;
        
        // 상태를 Running으로 전환
        self.state = SessionState::Running;
        
        info!("✅ Session {} started successfully", session_id);
        Ok(())
    }
    
    /// 세션 일시정지 처리
    /// 
    /// # Arguments
    /// * `session_id` - 일시정지할 세션 ID
    /// * `reason` - 일시정지 이유
    /// * `context` - Actor 컨텍스트
    async fn handle_pause_session(
        &mut self,
        session_id: String,
        reason: String,
        context: &AppContext,
    ) -> Result<(), SessionError> {
        // 세션 검증
        self.validate_session(&session_id)?;
        
        if !matches!(self.state, SessionState::Running) {
            return Err(SessionError::InvalidStateTransition {
                from: self.state.clone(),
                to: SessionState::Paused { reason: reason.clone() },
            });
        }
        
        warn!("⏸️ SessionActor {} pausing session {}: {}", self.actor_id, session_id, reason);
        
        // 상태 업데이트
        self.state = SessionState::Paused { reason: reason.clone() };
        
        // 일시정지 이벤트 발행
        let pause_event = AppEvent::SessionPaused {
            session_id,
            reason,
            timestamp: Utc::now(),
        };
        
        context.emit_event(pause_event).await
            .map_err(|e| SessionError::ContextError(e.to_string()))?;
        
        Ok(())
    }
    
    /// 세션 재개 처리
    /// 
    /// # Arguments
    /// * `session_id` - 재개할 세션 ID
    /// * `context` - Actor 컨텍스트
    async fn handle_resume_session(
        &mut self,
        session_id: String,
        context: &AppContext,
    ) -> Result<(), SessionError> {
        // 세션 검증
        self.validate_session(&session_id)?;
        
        if !matches!(self.state, SessionState::Paused { .. }) {
            return Err(SessionError::InvalidStateTransition {
                from: self.state.clone(),
                to: SessionState::Running,
            });
        }
        
        info!("▶️ SessionActor {} resuming session {}", self.actor_id, session_id);
        
        // 상태 업데이트
        self.state = SessionState::Running;
        
        // 재개 이벤트 발행
        let resume_event = AppEvent::SessionResumed {
            session_id,
            timestamp: Utc::now(),
        };
        
        context.emit_event(resume_event).await
            .map_err(|e| SessionError::ContextError(e.to_string()))?;
        
        Ok(())
    }
    
    /// 세션 취소 처리
    /// 
    /// # Arguments
    /// * `session_id` - 취소할 세션 ID
    /// * `reason` - 취소 이유
    /// * `context` - Actor 컨텍스트
    async fn handle_cancel_session(
        &mut self,
        session_id: String,
        reason: String,
        context: &AppContext,
    ) -> Result<(), SessionError> {
        // 세션 검증
        self.validate_session(&session_id)?;
        
        error!("❌ SessionActor {} cancelling session {}: {}", self.actor_id, session_id, reason);
        
        // 상태 업데이트
        self.state = SessionState::Failed { error: reason.clone() };
        
        // 취소 이벤트 발행
        let cancel_event = AppEvent::SessionFailed {
            session_id,
            error: reason,
            final_failure: true,
            timestamp: Utc::now(),
        };
        
        context.emit_event(cancel_event).await
            .map_err(|e| SessionError::ContextError(e.to_string()))?;
        
        // 세션 정리
        self.cleanup_session();
        
        Ok(())
    }
    
    /// 세션 ID 검증
    /// 
    /// # Arguments
    /// * `session_id` - 검증할 세션 ID
    fn validate_session(&self, session_id: &str) -> Result<(), SessionError> {
        match &self.session_id {
            Some(current_id) if current_id == session_id => Ok(()),
            Some(current_id) => Err(SessionError::SessionNotFound(format!(
                "Expected {}, got {}", current_id, session_id
            ))),
            None => Err(SessionError::SessionNotFound("No active session".to_string())),
        }
    }
    
    /// 세션 정리
    fn cleanup_session(&mut self) {
        self.session_id = None;
        self.state = SessionState::Idle;
        self.start_time = None;
        self.processed_batches = 0;
        self.total_success_count = 0;
    }
    
    /// 현재 세션 요약 생성
    /// 
    /// # Returns
    /// * `Option<SessionSummary>` - 세션이 활성화된 경우 요약, 그렇지 않으면 None
    fn create_session_summary(&self) -> Option<SessionSummary> {
        self.session_id.as_ref().map(|session_id| {
            let duration = self.start_time
                .map(|start| start.elapsed())
                .unwrap_or(Duration::ZERO);
            
            SessionSummary {
                session_id: session_id.clone(),
                total_duration_ms: duration.as_millis() as u64,
                total_pages_processed: 0, // TODO: 실제 처리된 페이지 수 계산
                total_products_processed: 0, // TODO: 실제 처리된 상품 수 계산
                success_rate: if self.processed_batches > 0 { 
                    self.total_success_count as f64 / self.processed_batches as f64 
                } else { 
                    0.0 
                },
                avg_page_processing_time: if self.processed_batches > 0 { 
                    duration.as_millis() as u64 / self.processed_batches as u64 
                } else { 
                    0 
                },
                error_summary: vec![], // TODO: 실제 에러 요약 구현
                processed_batches: self.processed_batches,
                total_success_count: self.total_success_count,
                final_state: format!("{:?}", self.state),
                timestamp: Utc::now(),
            }
        })
    }
}

#[async_trait::async_trait]
impl Actor for SessionActor {
    type Command = ActorCommand;
    type Error = ActorError;

    fn actor_id(&self) -> &str {
        &self.actor_id
    }

    fn actor_type(&self) -> ActorType {
        ActorType::Session
    }    async fn run(
        &mut self,
        context: AppContext,
        mut command_rx: mpsc::Receiver<Self::Command>,
    ) -> Result<(), Self::Error> {
        info!("🎬 SessionActor {} starting execution loop", self.actor_id);
        
        loop {
            tokio::select! {
                // 명령 처리
                command = command_rx.recv() => {
                    match command {
                        Some(cmd) => {
                            debug!("📨 SessionActor {} received command: {:?}", self.actor_id, cmd);
                            
                            match cmd {
                                ActorCommand::StartCrawling { session_id, config } => {
                                    if let Err(e) = self.handle_start_crawling(session_id, config, &context).await {
                                        error!("Failed to start crawling: {}", e);
                                    }
                                }
                                
                                ActorCommand::PauseSession { session_id, reason } => {
                                    if let Err(e) = self.handle_pause_session(session_id, reason, &context).await {
                                        error!("Failed to pause session: {}", e);
                                    }
                                }
                                
                                ActorCommand::ResumeSession { session_id } => {
                                    if let Err(e) = self.handle_resume_session(session_id, &context).await {
                                        error!("Failed to resume session: {}", e);
                                    }
                                }
                                
                                ActorCommand::CancelSession { session_id, reason } => {
                                    if let Err(e) = self.handle_cancel_session(session_id, reason, &context).await {
                                        error!("Failed to cancel session: {}", e);
                                    }
                                }
                                
                                ActorCommand::Shutdown => {
                                    info!("🛑 SessionActor {} received shutdown command", self.actor_id);
                                    break;
                                }
                                
                                _ => {
                                    debug!("SessionActor {} ignoring non-session command", self.actor_id);
                                }
                            }
                        }
                        None => {
                            warn!("📪 SessionActor {} command channel closed", self.actor_id);
                            break;
                        }
                    }
                }
                
                // 취소 신호 확인
                _ = context.cancellation_token.cancelled() => {
                    warn!("🚫 SessionActor {} received cancellation signal", self.actor_id);
                    break;
                }
            }
        }
        
        // 정리 작업
        if let Some(summary) = self.create_session_summary() {
            let completion_event = AppEvent::SessionCompleted {
                session_id: summary.session_id.clone(),
                summary,
                timestamp: Utc::now(),
            };
            
            let _ = context.emit_event(completion_event).await;
        }
        
        info!("🏁 SessionActor {} execution loop ended", self.actor_id);
        Ok(())
    }
    
    async fn health_check(&self) -> Result<ActorHealth, Self::Error> {
        let status = match &self.state {
            SessionState::Idle | SessionState::Running => ActorStatus::Healthy,
            SessionState::Paused { reason } => ActorStatus::Degraded { 
                reason: format!("Paused: {}", reason),
                since: Utc::now(),
            },
            SessionState::Failed { error } => ActorStatus::Unhealthy { 
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
            actor_type: ActorType::Session,
            status,
            last_activity: Utc::now(),
            memory_usage_mb: 0, // TODO: 실제 메모리 사용량 계산
            active_tasks: if matches!(self.state, SessionState::Running) { 1 } else { 0 },
            commands_processed: 0, // TODO: 실제 처리된 명령 수 계산
            errors_count: 0, // TODO: 실제 에러 수 계산
            avg_command_processing_time_ms: 0.0, // TODO: 실제 평균 처리 시간 계산
            metadata: serde_json::json!({
                "session_id": self.session_id,
                "state": format!("{:?}", self.state),
                "processed_batches": self.processed_batches,
                "total_success_count": self.total_success_count
            }).to_string(),
        })
    }
    
    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        info!("🔌 SessionActor {} shutting down", self.actor_id);
        
        // 활성 세션이 있다면 정리
        if self.session_id.is_some() {
            warn!("Cleaning up active session during shutdown");
            self.cleanup_session();
        }
        
        Ok(())
    }
}
