//! 통합 채널 컨텍스트 - 삼중 채널 시스템의 중앙 관리
//! Modern Rust 2024 준수: 설정 기반 채널 통합 관리

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::application::events::EventEmitter as ApplicationEventEmitter;
use crate::crawl_engine::{
    channels::types::{ActorCommand, AppEvent, ControlChannel, EventChannel},
    system_config::SystemConfig,
};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

/// 취소 채널 타입 정의
pub type CancellationReceiver = tokio::sync::watch::Receiver<bool>;
pub type CancellationSender = tokio::sync::watch::Sender<bool>;

/// 애플리케이션 컨텍스트 (AppContext는 IntegratedContext의 별칭)
pub type AppContext = IntegratedContext;

/// 이벤트 에미터 (EventEmitter는 ApplicationEventEmitter의 별칭)
pub type EventEmitter = ApplicationEventEmitter;

/// 통합 채널 컨텍스트 - 모든 Actor가 공유하는 통신 인프라
#[derive(Clone, Debug)]
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
    /// 취소 토큰 (호환성을 위해 추가)
    pub cancellation_token: CancellationReceiver,

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
            cancellation_rx: cancellation_rx.clone(),
            cancellation_token: cancellation_rx,
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

    /// 이벤트 발행 - Actor 시스템에서 프론트엔드로 직접 전달
    ///
    /// 설계 의도: 각 Actor, Task 레벨에서 독립적으로 이벤트 발행이 가능하게 하여
    /// 낮은 복잡성의 구현으로도 모든 경우를 다 커버할 수 있도록 함
    pub fn emit_event(&self, event: AppEvent) -> Result<usize, ContextError> {
        // 내부 브로드캐스트 채널로 발행 (다른 컴포넌트들이 구독할 수 있도록)
        let subscriber_count = self
            .event_tx
            .send(event)
            .map_err(|_| ContextError::EventBroadcastFailed)?;

        // TODO: ActorEventBridge가 이 이벤트를 받아서 프론트엔드로 전달
        // 현재는 브로드캐스트 채널에만 발행하고,
        // ActorEventBridge가 이를 수신하여 Tauri emit으로 프론트엔드에 전달

        Ok(subscriber_count)
    }

    /// 취소 신호 확인
    pub fn is_cancelled(&self) -> bool {
        *self.cancellation_rx.borrow()
    }

    /// 세션 ID 접근자 메서드
    pub fn session_id(&self) -> &str {
        &self.session_id
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

    /// 이벤트 채널 구독자 생성 (Broadcast Receiver 반환)
    ///
    /// SessionActor 등이 실시간으로 BatchReport 등 AppEvent 를 수신하여
    /// 누적 지표(예: duplicates_skipped)를 집계하기 위한 표준 인터페이스.
    /// 호출 시마다 새로운 receiver 가 생성되며 call site 에서 select! 에 통합 가능.
    pub fn subscribe_events(&self) -> broadcast::Receiver<AppEvent> {
        self.event_tx.subscribe()
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

impl From<ContextError> for crate::crawl_engine::actors::types::ActorError {
    fn from(err: ContextError) -> Self {
        use crate::crawl_engine::actors::types::ActorError;
        match err {
            ContextError::ControlChannelSend { message } => ActorError::ChannelError(message),
            ContextError::EventBroadcastFailed => {
                ActorError::EventBroadcastFailed("Event broadcast failed".to_string())
            }
            ContextError::Cancelled => ActorError::Cancelled("Operation cancelled".to_string()),
            ContextError::InvalidState { message } => ActorError::ConfigurationError(message),
        }
    }
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
    pub fn create_session_context(
        &self,
        session_id: String,
    ) -> Result<(IntegratedContext, ContextChannels), ContextError> {
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
    use crate::crawl_engine::system_config::SystemConfig;

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
            config: crate::crawl_engine::actors::types::CrawlingConfig {
                site_url: "https://test.com".to_string(),
                start_page: 1,
                end_page: 10,
                concurrency_limit: 1,
                batch_size: 5,
                request_delay_ms: 0,
                timeout_secs: 30,
                max_retries: 0,
                strategy: crate::crawl_engine::actors::types::CrawlingStrategy::NewestFirst,
            },
            timestamp: chrono::Utc::now(),
        };

        // emit_event is synchronous
        context
            .emit_event(event.clone())
            .expect("Should emit event");

        let received = event_rx.recv().await.expect("Should receive event");
        match received {
            AppEvent::SessionStarted { session_id, .. } => {
                assert_eq!(session_id, "test-session");
            }
            _ => panic!("Wrong event type"),
        }
    }
}
