//! 설정 주입 통합 컨텍스트
//! Modern Rust 2024: 모든 컴포넌트가 설정 기반으로 동작하도록 지원
//! Arc를 통한 효율적인 설정 공유 및 계층적 컨텍스트 구성

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use tokio::sync::watch;
use crate::new_architecture::{
    SystemConfig,
    config::RetryPolicy,
    channels::types::{ControlChannel, EventChannel, ActorCommand, AppEvent},
};

/// 통합 컨텍스트: 모든 채널과 설정을 포함
#[derive(Clone)]
pub struct IntegratedContext {
    pub session_id: String,
    pub batch_id: Option<String>,
    pub stage_id: Option<String>,
    pub task_id: Option<String>,
    
    /// 설정 (Arc를 통한 효율적 공유)
    pub config: Arc<SystemConfig>,
    
    /// 채널들
    pub control_tx: ControlChannel<ActorCommand>,
    pub event_tx: EventChannel<AppEvent>,
    pub cancellation_rx: watch::Receiver<bool>,
}

impl IntegratedContext {
    /// 새 컨텍스트 생성
    pub fn new(
        session_id: String,
        config: Arc<SystemConfig>,
        control_tx: ControlChannel<ActorCommand>,
        event_tx: EventChannel<AppEvent>,
        cancellation_rx: watch::Receiver<bool>,
    ) -> Self {
        Self {
            session_id,
            batch_id: None,
            stage_id: None,
            task_id: None,
            config,
            control_tx,
            event_tx,
            cancellation_rx,
        }
    }
    
    /// 하위 컨텍스트 생성 메서드들
    pub fn with_batch(&self, batch_id: String) -> Self {
        Self {
            batch_id: Some(batch_id),
            ..self.clone()
        }
    }
    
    pub fn with_stage(&self, stage_id: String) -> Self {
        Self {
            stage_id: Some(stage_id),
            ..self.clone()
        }
    }
    
    pub fn with_task(&self, task_id: String) -> Self {
        Self {
            task_id: Some(task_id),
            ..self.clone()
        }
    }
    
    /// 설정 기반 유틸리티 메서드들
    
    /// 현재 컨텍스트의 재시도 정책 가져오기
    pub fn get_retry_policy(&self, stage_type: &str) -> &RetryPolicy {
        match stage_type {
            "list_collection" => &self.config.retry_policies.list_collection,
            "detail_collection" => &self.config.retry_policies.detail_collection,
            "data_validation" => &self.config.retry_policies.data_validation,
            "database_save" => &self.config.retry_policies.database_save,
            _ => &self.config.retry_policies.list_collection, // 기본값
        }
    }
    
    /// 스테이지별 동시성 제한 가져오기
    pub fn get_concurrency_limit(&self, stage_type: &str) -> u32 {
        self.config.performance.concurrency
            .stage_concurrency_limits
            .get(stage_type)
            .copied()
            .unwrap_or(self.config.performance.concurrency.max_concurrent_tasks / 4)
    }
    
    /// 현재 배치 크기 설정 가져오기
    pub fn get_current_batch_size(&self) -> u32 {
        self.config.performance.batch_sizes.initial_size
    }
    
    /// 세션 타임아웃 가져오기
    pub fn get_session_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.config.system.session_timeout_secs)
    }
    
    /// 스테이지 타임아웃 가져오기
    pub fn get_stage_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(
            self.config.system.stage_timeout_secs.unwrap_or(300)
        )
    }
    
    /// 취소 신호 확인
    pub fn is_cancelled(&self) -> bool {
        *self.cancellation_rx.borrow()
    }
    
    /// 이벤트 발행 (에러 처리 포함)
    pub async fn emit_event(&self, event: AppEvent) -> Result<(), EventEmissionError> {
        self.event_tx.send(event)
            .map_err(|_| EventEmissionError::ChannelClosed)?;
        Ok(())
    }
    
    /// 명령 전송 (에러 처리 포함)
    pub async fn send_command(&self, command: ActorCommand) -> Result<(), CommandSendError> {
        self.control_tx.send(command).await
            .map_err(|_| CommandSendError::ChannelClosed)?;
        Ok(())
    }
    
    /// 컨텍스트 식별자 생성
    pub fn get_context_id(&self) -> String {
        let mut parts = vec![self.session_id.clone()];
        
        if let Some(batch_id) = &self.batch_id {
            parts.push(batch_id.clone());
        }
        
        if let Some(stage_id) = &self.stage_id {
            parts.push(stage_id.clone());
        }
        
        if let Some(task_id) = &self.task_id {
            parts.push(task_id.clone());
        }
        
        parts.join("/")
    }
    
    /// 로그 레벨 확인
    pub fn should_log(&self, level: &LogLevel) -> bool {
        let config_level = LogLevel::from_string(&self.config.monitoring.log_level);
        level.is_enabled_for(&config_level)
    }
    
    /// 프로파일링 활성화 여부
    pub fn is_profiling_enabled(&self) -> bool {
        self.config.monitoring.enable_profiling
    }
}

/// 컨텍스트 빌더 - 설정 기반 컨텍스트 구성
pub struct ContextBuilder {
    config: Arc<SystemConfig>,
}

impl ContextBuilder {
    pub fn new(config: Arc<SystemConfig>) -> Self {
        Self { config }
    }
    
    /// 완전한 컨텍스트 구축
    pub fn build(
        self,
        session_id: String,
        control_tx: ControlChannel<ActorCommand>,
        event_tx: EventChannel<AppEvent>,
        cancellation_rx: watch::Receiver<bool>,
    ) -> IntegratedContext {
        IntegratedContext::new(
            session_id,
            self.config,
            control_tx,
            event_tx,
            cancellation_rx,
        )
    }
    
    /// 설정 검증
    pub fn validate_config(&self) -> Result<(), crate::new_architecture::config::ConfigError> {
        self.config.validate()
    }
}

/// 로그 레벨 정의
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn from_string(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "TRACE" => LogLevel::Trace,
            "DEBUG" => LogLevel::Debug,
            "INFO" => LogLevel::Info,
            "WARN" => LogLevel::Warn,
            "ERROR" => LogLevel::Error,
            _ => LogLevel::Info, // 기본값
        }
    }
    
    pub fn is_enabled_for(&self, config_level: &LogLevel) -> bool {
        self >= config_level
    }
}

/// 이벤트 발행 에러
#[derive(thiserror::Error, Debug)]
pub enum EventEmissionError {
    #[error("Event channel is closed")]
    ChannelClosed,
}

/// 명령 전송 에러  
#[derive(thiserror::Error, Debug)]
pub enum CommandSendError {
    #[error("Command channel is closed")]
    ChannelClosed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::new_architecture::{
        system_config::SystemConfig,
        channels::types::ChannelFactory,
    };
    // Removed unused watch import

    #[test]
    fn test_context_hierarchy() {
        let config = Arc::new(SystemConfig::default());
        let factory = ChannelFactory::new(config.clone());
        
        let (control_tx, _) = factory.create_control_channel();
        let event_tx = factory.create_event_channel();
        let (_, cancellation_rx) = factory.create_cancellation_channel();
        
        let base_context = IntegratedContext::new(
            "session-1".to_string(),
            config,
            control_tx,
            event_tx,
            cancellation_rx,
        );
        
        // 배치 컨텍스트 생성
        let batch_context = base_context.with_batch("batch-1".to_string());
        assert_eq!(batch_context.batch_id, Some("batch-1".to_string()));
        assert_eq!(batch_context.session_id, "session-1");
        
        // 스테이지 컨텍스트 생성
        let stage_context = batch_context.with_stage("stage-1".to_string());
        assert_eq!(stage_context.stage_id, Some("stage-1".to_string()));
        assert_eq!(stage_context.batch_id, Some("batch-1".to_string()));
        
        // 컨텍스트 ID 확인
        assert_eq!(stage_context.get_context_id(), "session-1/batch-1/stage-1");
    }
    
    #[test]
    fn test_retry_policy_access() {
        let config = Arc::new(SystemConfig::default());
        let factory = ChannelFactory::new(config.clone());
        
        let (control_tx, _) = factory.create_control_channel();
        let event_tx = factory.create_event_channel();
        let (_, cancellation_rx) = factory.create_cancellation_channel();
        
        let context = IntegratedContext::new(
            "session-1".to_string(),
            config,
            control_tx,
            event_tx,
            cancellation_rx,
        );
        
        // 재시도 정책 접근
        let list_policy = context.get_retry_policy("list_collection");
        assert_eq!(list_policy.max_attempts, 3);
        
        let detail_policy = context.get_retry_policy("detail_collection");
        assert_eq!(detail_policy.max_attempts, 5);
    }
    
    #[test]
    fn test_concurrency_limits() {
        let config = Arc::new(SystemConfig::default());
        let factory = ChannelFactory::new(config.clone());
        
        let (control_tx, _) = factory.create_control_channel();
        let event_tx = factory.create_event_channel();
        let (_, cancellation_rx) = factory.create_cancellation_channel();
        
        let context = IntegratedContext::new(
            "session-1".to_string(),
            config,
            control_tx,
            event_tx,
            cancellation_rx,
        );
        
        // 동시성 제한 확인
        let list_limit = context.get_concurrency_limit("list_collection");
        assert_eq!(list_limit, 5);
        
        let detail_limit = context.get_concurrency_limit("detail_collection");
        assert_eq!(detail_limit, 20);
        
        // 정의되지 않은 스테이지의 기본값
        let unknown_limit = context.get_concurrency_limit("unknown_stage");
        assert!(unknown_limit > 0);
    }
    
    #[test]
    fn test_log_level_comparison() {
        assert!(LogLevel::Error > LogLevel::Info);
        assert!(LogLevel::Info > LogLevel::Debug);
        assert!(LogLevel::Debug > LogLevel::Trace);
        
        let info_level = LogLevel::Info;
        assert!(info_level.is_enabled_for(&LogLevel::Debug));
        assert!(!LogLevel::Debug.is_enabled_for(&LogLevel::Info));
    }
    
    #[test]
    fn test_context_builder() {
        let config = Arc::new(SystemConfig::default());
        let builder = ContextBuilder::new(config.clone());
        
        // 설정 검증
        assert!(builder.validate_config().is_ok());
        
        // 컨텍스트 구축 테스트는 채널 생성이 필요하므로 별도 테스트에서
    }
}
