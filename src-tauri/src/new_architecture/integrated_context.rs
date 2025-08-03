//! 통합 채널 컨텍스트 - 삼중 채널 시스템의 중앙 관리
//! Modern Rust 2024 준수: 설정 기반 채널 통합 관리

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use tokio::sync::{mpsc, broadcast};
use crate::new_architecture::{
    channels::types::{ActorCommand, AppEvent, ControlChannel, EventChannel},
    system_config::SystemConfig,
};

/// 취소 채널 타입 정의
pub type CancellationReceiver = tokio::sync::watch::Receiver<bool>;
pub type CancellationSender = tokio::sync::watch::Sender<bool>;

/// 통합 채널 컨텍스트 - 모든 Actor가 공유하는 통신 인프라
#[derive(Clone)]
pub struct IntegratedContext {
    /// 세션 식별자
    pub session_id: String,
    /// 배치 식별자 (선택적)
    pub batch_id: Option<String>,
    /// 스테이지 식별자 (선택적)
    pub stage_id: Option<String>,
    /// 태스크 식별자 (선택적)
    pub task_id: Option<String>,

    // 삼중 채널 시스템
    /// 제어 채널: 명령 하향 전달 (MPSC)
    pub control_tx: ControlChannel<ActorCommand>,
    /// 이벤트 채널: 독립적 상태 발행 (Broadcast)
    pub event_tx: EventChannel<AppEvent>,
    /// 취소 신호 수신 채널
    pub cancellation_rx: CancellationReceiver,

    /// 시스템 설정
    pub config: Arc<SystemConfig>,
}

impl IntegratedContext {
    /// 새로운 통합 컨텍스트 생성
    pub fn new(
        session_id: String,
        control_tx: ControlChannel<ActorCommand>,
        event_tx: EventChannel<AppEvent>,
        cancellation_rx: CancellationReceiver,
        config: Arc<SystemConfig>,
    ) -> Self {
        Self {
            session_id,
            batch_id: None,
            stage_id: None,
            task_id: None,
            control_tx,
            event_tx,
            cancellation_rx,
            config,
        }
    }

    /// 배치 컨텍스트로 확장
    pub fn with_batch(&self, batch_id: String) -> Self {
        let mut context = self.clone();
        context.batch_id = Some(batch_id);
        context
    }

    /// 스테이지 컨텍스트로 확장
    pub fn with_stage(&self, stage_id: String) -> Self {
        let mut context = self.clone();
        context.stage_id = Some(stage_id);
        context
    }

    /// 태스크 컨텍스트로 확장
    pub fn with_task(&self, task_id: String) -> Self {
        let mut context = self.clone();
        context.task_id = Some(task_id);
        context
    }

    /// 제어 명령 전송
    pub async fn send_control_command(&self, cmd: ActorCommand) -> Result<(), ContextError> {
        self.control_tx
            .send(cmd)
            .await
            .map_err(|e| ContextError::ControlChannelSend {
                message: e.to_string(),
            })
    }

    /// 이벤트 발행
    pub fn emit_event(&self, event: AppEvent) -> Result<usize, ContextError> {
        self.event_tx
            .send(event)
            .map_err(|_| ContextError::EventBroadcastFailed)
    }

    /// 취소 신호 확인
    pub fn is_cancelled(&self) -> bool {
        *self.cancellation_rx.borrow()
    }

    /// 현재 컨텍스트 경로 문자열 생성
    pub fn context_path(&self) -> String {
        let mut path = format!("session:{}", self.session_id);
        
        if let Some(batch_id) = &self.batch_id {
            path.push_str(&format!("/batch:{}", batch_id));
        }
        
        if let Some(stage_id) = &self.stage_id {
            path.push_str(&format!("/stage:{}", stage_id));
        }
        
        if let Some(task_id) = &self.task_id {
            path.push_str(&format!("/task:{}", task_id));
        }
        
        path
    }

    /// 설정에서 채널 버퍼 크기 가져오기
    pub fn get_control_buffer_size(&self) -> usize {
        self.config.control_buffer_size.unwrap_or(100)
    }

    /// 설정에서 이벤트 채널 크기 가져오기
    pub fn get_event_buffer_size(&self) -> usize {
        self.config.event_buffer_size.unwrap_or(1000)
    }
}

/// 컨텍스트 관련 에러
#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("Failed to send control command: {message}")]
    ControlChannelSend { message: String },

    #[error("Failed to broadcast event")]
    EventBroadcastFailed,

    #[error("Context operation cancelled")]
    Cancelled,

    #[error("Invalid context state: {message}")]
    InvalidState { message: String },
}

/// 통합 컨텍스트 팩토리
pub struct IntegratedContextFactory {
    config: Arc<SystemConfig>,
}

impl IntegratedContextFactory {
    /// 새로운 팩토리 생성
    pub fn new(config: Arc<SystemConfig>) -> Self {
        Self { config }
    }

    /// 새로운 세션용 통합 컨텍스트 생성
    pub fn create_session_context(&self, session_id: String) -> Result<(IntegratedContext, ContextChannels), ContextError> {
        // 제어 채널 생성
        let control_buffer_size = self.config.control_buffer_size.unwrap_or(100);
        let (control_tx, control_rx) = mpsc::channel(control_buffer_size);

        // 이벤트 채널 생성
        let event_buffer_size = self.config.event_buffer_size.unwrap_or(1000);
        let (event_tx, _) = broadcast::channel(event_buffer_size);

        // 취소 채널 생성
        let (cancellation_tx, cancellation_rx) = tokio::sync::watch::channel(false);

        let context = IntegratedContext::new(
            session_id,
            control_tx,
            event_tx.clone(),
            cancellation_rx,
            self.config.clone(),
        );

        let channels = ContextChannels {
            control_rx,
            event_tx,
            cancellation_tx,
        };

        Ok((context, channels))
    }
}

/// 컨텍스트 생성 시 반환되는 채널들
pub struct ContextChannels {
    pub control_rx: mpsc::Receiver<ActorCommand>,
    pub event_tx: EventChannel<AppEvent>,
    pub cancellation_tx: CancellationSender,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::new_architecture::system_config::SystemConfig;

    #[tokio::test]
    async fn test_integrated_context_creation() {
        let config = Arc::new(SystemConfig::default());
        let factory = IntegratedContextFactory::new(config);
        
        let (context, _channels) = factory
            .create_session_context("test-session".to_string())
            .expect("Should create context");

        assert_eq!(context.session_id, "test-session");
        assert!(context.batch_id.is_none());
        assert!(!context.is_cancelled());
    }

    #[tokio::test]
    async fn test_context_hierarchy() {
        let config = Arc::new(SystemConfig::default());
        let factory = IntegratedContextFactory::new(config);
        
        let (session_context, _) = factory
            .create_session_context("test-session".to_string())
            .expect("Should create context");

        let batch_context = session_context.with_batch("batch-1".to_string());
        let stage_context = batch_context.with_stage("stage-1".to_string());
        let task_context = stage_context.with_task("task-1".to_string());

        assert_eq!(
            task_context.context_path(),
            "session:test-session/batch:batch-1/stage:stage-1/task:task-1"
        );
    }

    #[tokio::test]
    async fn test_event_emission() {
        let config = Arc::new(SystemConfig::default());
        let factory = IntegratedContextFactory::new(config);
        
        let (context, channels) = factory
            .create_session_context("test-session".to_string())
            .expect("Should create context");

        let mut event_rx = channels.event_tx.subscribe();

        let event = AppEvent::SessionStarted {
            session_id: "test-session".to_string(),
            config: crate::new_architecture::channels::types::BatchConfig {
                target_url: "https://test.com".to_string(),
                max_pages: Some(10),
            },
        };

        context.emit_event(event.clone()).expect("Should emit event");

        let received = event_rx.recv().await.expect("Should receive event");
        match received {
            AppEvent::SessionStarted { session_id, .. } => {
                assert_eq!(session_id, "test-session");
            }
            _ => panic!("Wrong event type"),
        }
    }
}
