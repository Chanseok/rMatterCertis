//! Actor 시스템 핵심 인프라: AppContext 및 EventEmitter
//! 
//! 모든 Actor가 공유하는 컨텍스트와 이벤트 발행 능력을 제공합니다.
//! Modern Rust 2024 원칙을 준수하며, 함수형 프로그래밍 접근을 사용합니다.

use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use serde::{Serialize, Deserialize};
use ts_rs::TS;

use super::actors::types::{AppEvent, ActorError};

/// 모든 Actor가 공유하는 애플리케이션 컨텍스트
/// 
/// 이 구조체는 Actor 간의 통신과 상태 공유를 위한 핵심 인프라입니다.
/// Clone이 가능하여 Actor 간에 안전하게 공유될 수 있습니다.
#[derive(Debug, Clone)]
pub struct AppContext {
    /// 현재 세션의 고유 식별자
    pub session_id: String,
    
    /// 시스템 전체 설정 (불변)
    pub config: Arc<SystemConfig>,
    
    /// 이벤트 브로드캐스트를 위한 채널 송신단
    pub event_tx: broadcast::Sender<AppEvent>,
    
    /// 시스템 전체 취소 신호를 위한 토큰
    pub cancellation_token: CancellationToken,
}

impl AppContext {
    /// 새로운 AppContext 인스턴스를 생성합니다.
    /// 
    /// # Arguments
    /// * `session_id` - 현재 세션의 고유 식별자
    /// * `config` - 시스템 설정 (Arc로 래핑됨)
    /// * `event_tx` - 이벤트 브로드캐스트 채널
    /// * `cancellation_token` - 취소 신호 토큰
    #[must_use]
    pub fn new(
        session_id: String,
        config: Arc<SystemConfig>,
        event_tx: broadcast::Sender<AppEvent>,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            session_id,
            config,
            event_tx,
            cancellation_token,
        }
    }
    
    /// 현재 세션 ID를 반환합니다.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
    
    /// 시스템 설정에 대한 참조를 반환합니다.
    #[must_use]
    pub fn config(&self) -> &SystemConfig {
        &self.config
    }
    
    /// 이벤트를 시스템 전체에 브로드캐스트합니다.
    pub async fn emit_event(&self, event: AppEvent) -> Result<(), ActorError> {
        self.event_tx
            .send(event)
            .map_err(|e| ActorError::EventBroadcastFailed(e.to_string()))?;
        Ok(())
    }
    
    /// 현재 취소 신호가 발생했는지 확인합니다.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }
}

/// 이벤트 발행 능력을 가진 Actor를 위한 trait
/// 
/// 이 trait을 구현하는 Actor는 시스템 전체에 이벤트를 브로드캐스트할 수 있습니다.
/// async_trait을 사용하여 async 메서드를 지원합니다.
#[async_trait::async_trait]
pub trait EventEmitter {
    /// 이벤트를 시스템 전체에 브로드캐스트합니다.
    /// 
    /// # Arguments
    /// * `event` - 발행할 이벤트
    /// 
    /// # Returns
    /// * `Ok(())` - 이벤트가 성공적으로 발행됨
    /// * `Err(ActorError)` - 이벤트 발행 실패
    async fn emit_event(&self, event: AppEvent) -> Result<(), ActorError>;
    
    /// 현재 취소 신호가 발생했는지 확인합니다.
    /// 
    /// # Returns
    /// * `true` - 취소 신호가 발생함
    /// * `false` - 정상 동작 중
    fn is_cancelled(&self) -> bool;
    
    /// 취소 신호 토큰을 반환합니다.
    /// 
    /// # Returns
    /// * `CancellationToken` - 현재 취소 토큰
    fn cancellation_token(&self) -> &CancellationToken;
}

#[async_trait::async_trait]
impl EventEmitter for AppContext {
    async fn emit_event(&self, event: AppEvent) -> Result<(), ActorError> {
        self.event_tx
            .send(event)
            .map_err(|e| ActorError::EventBroadcastFailed(e.to_string()))?;
        Ok(())
    }
    
    fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }
    
    fn cancellation_token(&self) -> &CancellationToken {
        &self.cancellation_token
    }
}

/// 시스템 전체 설정
/// 
/// 이 구조체는 Actor 시스템 전반에 걸쳐 사용되는 설정을 담고 있습니다.
/// 불변 데이터로 Arc로 래핑되어 공유됩니다.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SystemConfig {
    /// 크롤링 관련 설정
    pub crawling: CrawlingConfig,
    
    /// 성능 관련 설정
    pub performance: PerformanceConfig,
    
    /// 데이터베이스 관련 설정
    pub database: DatabaseConfig,
    
    /// 로깅 관련 설정
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CrawlingConfig {
    /// 기본 동시 실행 제한
    pub default_concurrency_limit: u32,
    
    /// 요청 간 지연 시간 (밀리초)
    pub request_delay_ms: u64,
    
    /// 요청 타임아웃 (초)
    pub request_timeout_secs: u64,
    
    /// 재시도 최대 횟수
    pub max_retries: u32,
    
    /// 배치 크기
    pub default_batch_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PerformanceConfig {
    /// 메모리 사전 할당 크기
    pub memory_preallocation_size: usize,
    
    /// 채널 버퍼 크기
    pub channel_buffer_size: usize,
    
    /// Actor 헬스 체크 간격 (초)
    pub health_check_interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DatabaseConfig {
    /// 데이터베이스 연결 URL
    pub connection_url: String,
    
    /// 최대 연결 수
    pub max_connections: u32,
    
    /// 연결 타임아웃 (초)
    pub connection_timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LoggingConfig {
    /// 로그 레벨
    pub level: String,
    
    /// 성능 로깅 활성화
    pub enable_performance_logging: bool,
    
    /// 상세 타이밍 로깅 활성화
    pub enable_detailed_timing: bool,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            crawling: CrawlingConfig {
                default_concurrency_limit: 10,
                request_delay_ms: 1000,
                request_timeout_secs: 30,
                max_retries: 3,
                default_batch_size: 50,
            },
            performance: PerformanceConfig {
                memory_preallocation_size: 1000,
                channel_buffer_size: 100,
                health_check_interval_secs: 30,
            },
            database: DatabaseConfig {
                connection_url: "sqlite://./data.db".to_string(),
                max_connections: 10,
                connection_timeout_secs: 30,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                enable_performance_logging: true,
                enable_detailed_timing: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast;

    #[test]
    fn test_system_config_default() {
        let config = SystemConfig::default();
        assert_eq!(config.crawling.default_concurrency_limit, 10);
        assert_eq!(config.performance.channel_buffer_size, 100);
        assert!(config.logging.enable_performance_logging);
    }

    #[tokio::test]
    async fn test_app_context_creation() {
        let config = Arc::new(SystemConfig::default());
        let (event_tx, _) = broadcast::channel(100);
        let cancellation_token = CancellationToken::new();
        
        let context = AppContext::new(
            "test-session".to_string(),
            config,
            event_tx,
            cancellation_token,
        );
        
        assert_eq!(context.session_id(), "test-session");
        assert!(!context.is_cancelled());
    }

    #[tokio::test]
    async fn test_event_emission() {
        let config = Arc::new(SystemConfig::default());
        let (event_tx, mut event_rx) = broadcast::channel(100);
        let cancellation_token = CancellationToken::new();
        
        let context = AppContext::new(
            "test-session".to_string(),
            config,
            event_tx,
            cancellation_token,
        );
        
        let test_event = AppEvent::SessionStarted {
            session_id: "test-session".to_string(),
            config: super::super::actors::types::CrawlingConfig::default(),
            timestamp: chrono::Utc::now(),
        };
        
        // 이벤트 발행
        context.emit_event(test_event.clone()).await.unwrap();
        
        // 이벤트 수신 확인
        let received_event = event_rx.recv().await.unwrap();
        match received_event {
            AppEvent::SessionStarted { session_id, .. } => {
                assert_eq!(session_id, "test-session");
            }
            _ => panic!("Unexpected event type"),
        }
    }
}
