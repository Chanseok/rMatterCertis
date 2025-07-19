//! rMatterCertis 새로운 아키텍처 모듈
//! Modern Rust 2024 완전 준수: mod.rs 금지, 설정 기반 시스템

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

// 개별 모듈들 (Modern Rust 2024: mod.rs 대신)
pub mod system_config;
pub mod channel_types;
pub mod actor_system;

#[cfg(test)]
mod integration_tests;

// 주요 타입들 재익스포트
pub use system_config::{SystemConfig, RetryPolicy, ConfigError};
pub use channel_types::{ChannelFactory, ActorCommand, AppEvent, StageType};
pub use actor_system::{SessionActor, BatchActor, StageActor, ActorError};

/// 새 아키텍처의 메인 결과 타입
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// 시스템 초기화 헬퍼
pub async fn initialize_system(config_path: Option<&str>) -> Result<SystemConfig> {
    let config = match config_path {
        Some(path) => SystemConfig::from_file(path)?,
        None => {
            let env = std::env::var("RMATTERCERTIS_ENV").unwrap_or_else(|_| "development".to_string());
            SystemConfig::for_environment(&env)?
        }
    };
    
    config.validate()?;
    initialize_logging(&config).await?;
    
    Ok(config)
}

/// 설정 기반 로깅 초기화
async fn initialize_logging(config: &SystemConfig) -> Result<()> {
    println!("🚀 New Architecture initialized with level: {}", config.monitoring.log_level);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_system_initialization() {
        let result = initialize_system(None).await;
        assert!(result.is_ok());
        
        let config = result.unwrap();
        assert!(config.system.max_concurrent_sessions > 0);
        assert!(config.retry_policies.list_collection.max_attempts > 0);
    }
    
    #[test]
    fn test_module_exports() {
        let _config = SystemConfig::default();
        let _command = ActorCommand::CancelSession {
            session_id: "test".to_string(),
            reason: "test".to_string(),
        };
    }
}
