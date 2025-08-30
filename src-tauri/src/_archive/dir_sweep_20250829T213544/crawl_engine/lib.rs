//! rMatterCertis 새로운 아키텍처 모듈
//! Modern Rust 2024 완전 준수: mod.rs 금지, 설정 기반 시스템
//! 삼중 채널 시스템과 회복탄력성을 갖춘 크롤링 엔진

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![deny(clippy::unimplemented, clippy::todo)]

// 하위 모듈들 (mod.rs 대신 직접 선언)
pub mod config;
pub mod channels;
pub mod results;
pub mod context;
pub mod actor_system;
pub mod channel_types;
pub mod events;

// 주요 타입들 재익스포트
pub use config::{SystemConfig, RetryPolicy, ConfigError};
pub use channels::{ChannelFactory, ActorCommand, AppEvent, StageType, types::*};
pub use channels::types::{StageItem, BatchConfig};
pub use results::{StageResult, StageError, ErrorClassifier, RetryCalculator, FailedItem};
pub use context::{IntegratedContext, ContextBuilder, LogLevel};
pub use events::*;

/// 새 아키텍처의 메인 결과 타입
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// 시스템 초기화 헬퍼
pub async fn initialize_system(config_path: Option<&str>) -> Result<SystemConfig> {
    let config = match config_path {
        Some(path) => SystemConfig::from_file(path)?,
        None => {
            // 환경 변수에서 환경 감지
            let env = std::env::var("RMATTERCERTIS_ENV").unwrap_or_else(|_| "development".to_string());
            SystemConfig::for_environment(&env)?
        }
    };
    
    // 설정 검증
    config.validate()?;
    
    // 로그 시스템 초기화 (설정 기반)
    initialize_logging(&config).await?;
    
    Ok(config)
}

/// 설정 기반 로깅 초기화
async fn initialize_logging(config: &SystemConfig) -> Result<()> {
    // 실제 로깅 시스템 초기화 로직은 나중에 구현
    println!("Logging initialized with level: {}", config.monitoring.log_level);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_system_initialization() {
        // 기본 설정으로 시스템 초기화
        let result = initialize_system(None).await;
        assert!(result.is_ok());
        
        let config = result.unwrap();
        assert!(config.system.max_concurrent_sessions > 0);
        assert!(config.retry_policies.list_collection.max_attempts > 0);
    }
    
    #[test]
    fn test_module_exports() {
        // 주요 타입들이 올바르게 익스포트되는지 확인
        let _config = SystemConfig::default();
        let _error = StageError::NetworkTimeout { 
            message: "test".to_string() 
        };
        let _command = ActorCommand::CancelSession {
            session_id: "test".to_string(),
            reason: "test".to_string(),
        };
    }
}
