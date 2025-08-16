//! Actor trait 정의
//!
//! 모든 Actor가 구현해야 하는 기본 인터페이스를 정의합니다.
//! Modern Rust 2024 원칙을 따라 async trait과 강타입 시스템을 활용합니다.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use ts_rs::TS;

use super::super::context::AppContext;
use super::types::ActorError;

/// 모든 Actor가 구현해야 하는 기본 trait
///
/// 이 trait은 Actor 시스템의 핵심 인터페이스를 정의합니다.
/// 모든 Actor는 이 trait을 구현하여 시스템에 참여할 수 있습니다.
#[async_trait]
pub trait Actor: Send + Sync + 'static {
    /// Actor가 처리할 수 있는 명령 타입
    type Command: Send + Sync + 'static;

    /// Actor에서 발생할 수 있는 에러 타입
    type Error: std::error::Error + Send + Sync + 'static;

    /// Actor의 고유 식별자를 반환합니다.
    ///
    /// # Returns
    /// * Actor의 고유 ID 문자열
    fn actor_id(&self) -> &str;

    /// Actor의 타입을 반환합니다.
    ///
    /// # Returns
    /// * Actor 타입 열거형
    fn actor_type(&self) -> ActorType;

    /// Actor의 메인 실행 루프입니다.
    ///
    /// 이 메서드는 Actor의 생명주기 동안 계속 실행되며,
    /// 명령을 수신하고 처리하는 핵심 로직을 담고 있습니다.
    ///
    /// # Arguments
    /// * `context` - 공유 애플리케이션 컨텍스트
    /// * `command_rx` - 명령 수신용 채널
    ///
    /// # Returns
    /// * `Ok(())` - 정상 종료
    /// * `Err(Self::Error)` - 실행 중 오류 발생
    async fn run(
        &mut self,
        context: AppContext,
        mut command_rx: mpsc::Receiver<Self::Command>,
    ) -> Result<(), Self::Error>;

    /// Actor의 현재 상태를 확인합니다.
    ///
    /// # Returns
    /// * `Ok(ActorHealth)` - 상태 정보
    /// * `Err(Self::Error)` - 상태 확인 실패
    async fn health_check(&self) -> Result<ActorHealth, Self::Error>;

    /// Actor를 우아하게 종료합니다.
    ///
    /// 진행 중인 작업을 완료하고 리소스를 정리합니다.
    ///
    /// # Returns
    /// * `Ok(())` - 정상 종료
    /// * `Err(Self::Error)` - 종료 중 오류 발생
    async fn shutdown(&mut self) -> Result<(), Self::Error>;

    /// Actor 초기화를 수행합니다.
    ///
    /// Actor가 시작되기 전에 필요한 초기 설정을 수행합니다.
    ///
    /// # Arguments
    /// * `context` - 공유 애플리케이션 컨텍스트
    ///
    /// # Returns
    /// * `Ok(())` - 초기화 성공
    /// * `Err(Self::Error)` - 초기화 실패
    async fn initialize(&mut self, context: &AppContext) -> Result<(), Self::Error> {
        // 기본 구현: 아무것도 하지 않음
        let _ = context; // 경고 제거
        Ok(())
    }

    /// Actor가 명령을 처리할 수 있는지 확인합니다.
    ///
    /// # Arguments
    /// * `command` - 확인할 명령
    ///
    /// # Returns
    /// * `true` - 처리 가능
    /// * `false` - 처리 불가능
    fn can_handle(&self, command: &Self::Command) -> bool {
        // 기본 구현: 모든 명령 처리 가능
        let _ = command; // 경고 제거
        true
    }
}

/// Actor 타입 열거형
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ActorType {
    /// 세션 관리 Actor
    Session,

    /// 배치 처리 Actor
    Batch,

    /// 스테이지 실행 Actor
    Stage,

    /// 모니터링 Actor
    Monitor,

    /// 메시지 라우터 Actor
    Router,

    /// 사용자 정의 Actor
    Custom(String),
}

/// Actor 상태 정보
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ActorHealth {
    /// Actor ID
    pub actor_id: String,

    /// Actor 타입
    pub actor_type: ActorType,

    /// 현재 상태
    pub status: ActorStatus,

    /// 마지막 활동 시간
    pub last_activity: DateTime<Utc>,

    /// 메모리 사용량 (MB)
    pub memory_usage_mb: u64,

    /// 활성 작업 수
    pub active_tasks: u32,

    /// 처리된 명령 수
    pub commands_processed: u64,

    /// 발생한 에러 수
    pub errors_count: u64,

    /// 평균 명령 처리 시간 (밀리초)
    pub avg_command_processing_time_ms: f64,

    /// 추가 메타데이터 (JSON 문자열)
    pub metadata: String,
}

/// Actor 상태 열거형
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ActorStatus {
    /// 정상 동작 중
    Healthy,

    /// 성능 저하
    Degraded {
        reason: String,
        since: DateTime<Utc>,
    },

    /// 비정상 상태
    Unhealthy { error: String, since: DateTime<Utc> },

    /// 시작 중
    Starting,

    /// 종료 중
    Stopping,

    /// 중지됨
    Stopped,
}

/// Actor 관리를 위한 헬퍼 trait
#[async_trait]
pub trait ActorManager: Send + Sync {
    /// Actor를 등록합니다.
    ///
    /// # Arguments
    /// * `actor` - 등록할 Actor
    ///
    /// # Returns
    /// * `Ok(())` - 등록 성공
    /// * `Err(ActorError)` - 등록 실패
    async fn register_actor<A>(&mut self, actor: Arc<A>) -> Result<(), ActorError>
    where
        A: Actor + 'static;

    /// Actor를 시작합니다.
    ///
    /// # Arguments
    /// * `actor_id` - 시작할 Actor ID
    ///
    /// # Returns
    /// * `Ok(())` - 시작 성공
    /// * `Err(ActorError)` - 시작 실패
    async fn start_actor(&mut self, actor_id: &str) -> Result<(), ActorError>;

    /// Actor를 중지합니다.
    ///
    /// # Arguments
    /// * `actor_id` - 중지할 Actor ID
    ///
    /// # Returns
    /// * `Ok(())` - 중지 성공
    /// * `Err(ActorError)` - 중지 실패
    async fn stop_actor(&mut self, actor_id: &str) -> Result<(), ActorError>;

    /// 모든 Actor의 상태를 확인합니다.
    ///
    /// # Returns
    /// * `Vec<ActorHealth>` - Actor 상태 목록
    async fn get_all_actor_health(&self) -> Vec<ActorHealth>;

    /// 특정 Actor에게 명령을 전송합니다.
    ///
    /// # Arguments
    /// * `actor_id` - 대상 Actor ID
    /// * `command` - 전송할 명령
    ///
    /// # Returns
    /// * `Ok(())` - 전송 성공
    /// * `Err(ActorError)` - 전송 실패
    async fn send_command(
        &self,
        actor_id: &str,
        command: serde_json::Value,
    ) -> Result<(), ActorError>;
}

/// Actor 생명주기 이벤트
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ActorLifecycleEvent {
    /// Actor 시작됨
    Started {
        actor_id: String,
        actor_type: ActorType,
        timestamp: DateTime<Utc>,
    },

    /// Actor 중지됨
    Stopped {
        actor_id: String,
        actor_type: ActorType,
        timestamp: DateTime<Utc>,
        reason: String,
    },

    /// Actor에서 에러 발생
    Error {
        actor_id: String,
        actor_type: ActorType,
        error: String,
        timestamp: DateTime<Utc>,
    },

    /// Actor 상태 변화
    StatusChanged {
        actor_id: String,
        actor_type: ActorType,
        old_status: ActorStatus,
        new_status: ActorStatus,
        timestamp: DateTime<Utc>,
    },
}

/// Actor 메트릭을 수집하는 trait
#[async_trait]
pub trait ActorMetrics: Send + Sync {
    /// 명령 처리 시작을 기록합니다.
    async fn record_command_start(&self, actor_id: &str, command_type: &str);

    /// 명령 처리 완료를 기록합니다.
    async fn record_command_complete(&self, actor_id: &str, command_type: &str, duration_ms: u64);

    /// 에러 발생을 기록합니다.
    async fn record_error(&self, actor_id: &str, error_type: &str);

    /// 메모리 사용량을 기록합니다.
    async fn record_memory_usage(&self, actor_id: &str, memory_mb: u64);

    /// Actor 메트릭을 가져옵니다.
    async fn get_actor_metrics(&self, actor_id: &str) -> Option<ActorHealth>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_actor_health_creation() {
        let health = ActorHealth {
            actor_id: "test-actor".to_string(),
            actor_type: ActorType::Session,
            status: ActorStatus::Healthy,
            last_activity: Utc::now(),
            memory_usage_mb: 100,
            active_tasks: 5,
            commands_processed: 1000,
            errors_count: 2,
            avg_command_processing_time_ms: 15.5,
            metadata: serde_json::json!({"version": "1.0"}).to_string(),
        };

        assert_eq!(health.actor_id, "test-actor");
        assert_eq!(health.memory_usage_mb, 100);
        assert_eq!(health.active_tasks, 5);
        assert_eq!(health.commands_processed, 1000);
        assert_eq!(health.errors_count, 2);
    }

    #[test]
    fn test_actor_status_serialization() {
        let status = ActorStatus::Degraded {
            reason: "High memory usage".to_string(),
            since: Utc::now(),
        };

        let serialized = serde_json::to_string(&status).unwrap();
        let deserialized: ActorStatus = serde_json::from_str(&serialized).unwrap();

        match deserialized {
            ActorStatus::Degraded { reason, .. } => {
                assert_eq!(reason, "High memory usage");
            }
            _ => panic!("Unexpected status type"),
        }
    }

    #[test]
    fn test_actor_type() {
        let types = vec![
            ActorType::Session,
            ActorType::Batch,
            ActorType::Stage,
            ActorType::Monitor,
            ActorType::Router,
            ActorType::Custom("CustomType".to_string()),
        ];

        for actor_type in types {
            let serialized = serde_json::to_string(&actor_type).unwrap();
            let deserialized: ActorType = serde_json::from_str(&serialized).unwrap();

            match (&actor_type, &deserialized) {
                (ActorType::Custom(a), ActorType::Custom(b)) => assert_eq!(a, b),
                _ => assert_eq!(
                    std::mem::discriminant(&actor_type),
                    std::mem::discriminant(&deserialized)
                ),
            }
        }
    }

    #[test]
    fn test_lifecycle_event() {
        let event = ActorLifecycleEvent::Started {
            actor_id: "test-actor".to_string(),
            actor_type: ActorType::Session,
            timestamp: Utc::now(),
        };

        let serialized = serde_json::to_string(&event).unwrap();
        let deserialized: ActorLifecycleEvent = serde_json::from_str(&serialized).unwrap();

        match deserialized {
            ActorLifecycleEvent::Started {
                actor_id,
                actor_type,
                ..
            } => {
                assert_eq!(actor_id, "test-actor");
                assert!(matches!(actor_type, ActorType::Session));
            }
            _ => panic!("Unexpected event type"),
        }
    }
}
