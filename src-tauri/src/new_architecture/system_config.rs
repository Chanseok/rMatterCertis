//! 전체 시스템 설정 통합 관리
//! Modern Rust 2024: serde, config crate 활용한 설정 시스템

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to load config from file: {source}")]
    FileLoad {
        #[from]
        source: config::ConfigError,
    },
    
    #[error("Configuration validation failed: {message}")]
    Validation { message: String },
}

/// 전체 시스템 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub system: SystemSettings,
    pub retry_policies: RetryPolicies,
    pub performance: PerformanceSettings,
    pub monitoring: MonitoringSettings,
    pub channels: ChannelSettings,
    pub actor: ActorSettings,
    
    /// 호환성 필드들 (레거시 지원)
    pub control_buffer_size: Option<usize>,
    pub event_buffer_size: Option<usize>,
    pub crawling: Option<CrawlingSettings>,
    pub legacy_actor: Option<LegacyActorSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSettings {
    pub max_concurrent_sessions: u32,
    pub session_timeout_secs: u64,
    pub stage_timeout_secs: Option<u64>,
    pub cancellation_timeout_secs: u64,
    pub memory_limit_mb: u64,
    pub abort_on_database_error: Option<bool>,
    pub abort_on_validation_error: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicies {
    pub list_collection: RetryPolicy,
    pub detail_collection: RetryPolicy,
    pub data_validation: RetryPolicy,
    pub database_save: RetryPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_multiplier: f64,
    pub jitter_range_ms: u64,
    pub retry_on_errors: Vec<RetryableErrorType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RetryableErrorType {
    NetworkTimeout,
    ServerError { status_range: (u16, u16) },
    RateLimit,
    ParseError,
    ValidationTimeout,
    DatabaseConnection,
    DatabaseTimeout,
    DatabaseLock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSettings {
    pub batch_sizes: BatchSizeSettings,
    pub concurrency: ConcurrencySettings,
    pub buffers: BufferSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSizeSettings {
    pub initial_size: u32,
    pub min_size: u32,
    pub max_size: u32,
    pub auto_adjust_threshold: f64,
    pub adjust_multiplier: f64,
    pub small_db_multiplier: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrencySettings {
    pub max_concurrent_tasks: u32,
    pub stage_concurrency_limits: HashMap<String, u32>,
    pub task_queue_size: u32,
    pub min_concurrent_batches: u32,
    pub max_concurrent_batches: u32,
    pub high_load_multiplier: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferSettings {
    pub request_buffer_size: usize,
    pub response_buffer_size: usize,
    pub temp_storage_limit_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelSettings {
    pub control_buffer_size: usize,
    pub event_buffer_size: usize,
    pub backpressure_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringSettings {
    pub metrics_interval_secs: u64,
    pub log_level: String,
    pub enable_profiling: bool,
    pub event_retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorSettings {
    pub session_timeout_secs: u64,
    pub stage_timeout_secs: u64,
    pub batch_timeout_secs: u64,
    pub max_concurrent_sessions: u32,
    pub max_concurrent_batches: u32,
}

impl RetryPolicy {
    pub fn base_delay(&self) -> Duration {
        Duration::from_millis(self.base_delay_ms)
    }
    
    pub fn max_delay(&self) -> Duration {
        Duration::from_millis(self.max_delay_ms)
    }
    
    pub fn jitter_range(&self) -> Duration {
        Duration::from_millis(self.jitter_range_ms)
    }
}

impl SystemConfig {
    pub fn from_file(path: &str) -> Result<Self, ConfigError> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name(path))
            .add_source(config::Environment::with_prefix("RMATTERCERTIS"))
            .build()?;
        
        let config: Self = settings.try_deserialize()?;
        config.validate()?;
        Ok(config)
    }
    
    pub fn for_environment(env: &str) -> Result<Self, ConfigError> {
        let base_path = "config/default";
        let env_path = &format!("config/{}", env);
        
        let settings = config::Config::builder()
            .add_source(config::File::with_name(base_path))
            .add_source(config::File::with_name(env_path).required(false))
            .add_source(config::Environment::with_prefix("RMATTERCERTIS"))
            .build()?;
        
        let config: Self = settings.try_deserialize()?;
        config.validate()?;
        Ok(config)
    }
    
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.performance.batch_sizes.min_size > self.performance.batch_sizes.max_size {
            return Err(ConfigError::Validation {
                message: "min_size cannot be greater than max_size".to_string(),
            });
        }
        
        if self.system.session_timeout_secs == 0 {
            return Err(ConfigError::Validation {
                message: "session_timeout_secs must be greater than 0".to_string(),
            });
        }
        
        Ok(())
    }
    
    pub fn default() -> Self {
        use std::collections::HashMap;
        
        let mut stage_limits = HashMap::new();
        stage_limits.insert("list_collection".to_string(), 5);
        stage_limits.insert("detail_collection".to_string(), 10);
        stage_limits.insert("validation".to_string(), 3);

        SystemConfig {
            system: SystemSettings {
                max_concurrent_sessions: 10,
                session_timeout_secs: 3600,
                stage_timeout_secs: Some(300),
                cancellation_timeout_secs: 30,
                memory_limit_mb: 2048,
                abort_on_database_error: Some(false),
                abort_on_validation_error: Some(false),
            },
            retry_policies: RetryPolicies {
                list_collection: RetryPolicy {
                    max_attempts: 3,
                    base_delay_ms: 1000,
                    max_delay_ms: 30000,
                    backoff_multiplier: 2.0,
                    jitter_range_ms: 500,
                    retry_on_errors: vec![
                        RetryableErrorType::NetworkTimeout,
                        RetryableErrorType::ServerError { status_range: (500, 599) },
                        RetryableErrorType::RateLimit,
                    ],
                },
                detail_collection: RetryPolicy {
                    max_attempts: 5,
                    base_delay_ms: 500,
                    max_delay_ms: 60000,
                    backoff_multiplier: 1.5,
                    jitter_range_ms: 200,
                    retry_on_errors: vec![
                        RetryableErrorType::NetworkTimeout,
                        RetryableErrorType::ServerError { status_range: (500, 599) },
                        RetryableErrorType::ParseError,
                    ],
                },
                data_validation: RetryPolicy {
                    max_attempts: 2,
                    base_delay_ms: 100,
                    max_delay_ms: 5000,
                    backoff_multiplier: 1.2,
                    jitter_range_ms: 50,
                    retry_on_errors: vec![RetryableErrorType::ValidationTimeout],
                },
                database_save: RetryPolicy {
                    max_attempts: 10,
                    base_delay_ms: 200,
                    max_delay_ms: 30000,
                    backoff_multiplier: 2.0,
                    jitter_range_ms: 100,
                    retry_on_errors: vec![
                        RetryableErrorType::DatabaseConnection,
                        RetryableErrorType::DatabaseTimeout,
                        RetryableErrorType::DatabaseLock,
                    ],
                },
            },
            performance: PerformanceSettings {
                batch_sizes: BatchSizeSettings {
                    initial_size: 100,
                    min_size: 10,
                    max_size: 1000,
                    auto_adjust_threshold: 0.8,
                    adjust_multiplier: 1.5,
                    small_db_multiplier: 1.0,
                },
                concurrency: ConcurrencySettings {
                    max_concurrent_tasks: 50,
                    stage_concurrency_limits: stage_limits,
                    task_queue_size: 1000,
                    min_concurrent_batches: 1,
                    max_concurrent_batches: 10,
                    high_load_multiplier: 1.2,
                },
                buffers: BufferSettings {
                    request_buffer_size: 8192,
                    response_buffer_size: 16384,
                    temp_storage_limit_mb: 256,
                },
            },
            monitoring: MonitoringSettings {
                metrics_interval_secs: 30,
                log_level: "INFO".to_string(),
                enable_profiling: false,
                event_retention_days: 7,
            },
            channels: ChannelSettings {
                control_buffer_size: 100,
                event_buffer_size: 500,
                backpressure_threshold: 0.8,
            },
            actor: ActorSettings {
                session_timeout_secs: 300,
                stage_timeout_secs: 120,
                batch_timeout_secs: 600,
                max_concurrent_sessions: 10,
                max_concurrent_batches: 3,
            },
            
            // Phase 3: 통합 컨텍스트 기본값
            // 호환성 필드들
            control_buffer_size: Some(100),
            event_buffer_size: Some(1000),
            crawling: Some(CrawlingSettings {
                max_concurrent_requests: Some(10),
                timeout_seconds: Some(30),
                default_concurrency_limit: Some(5),
                request_delay_ms: Some(1000),
            }),
            legacy_actor: Some(LegacyActorSettings {
                max_actors: Some(100),
                restart_policy: Some("always".to_string()),
            }),
        }
    }

    /// 테스트용 설정 로드
    pub fn load_for_test() -> Result<Self, ConfigError> {
        // 테스트에서는 기본 설정 사용
        Ok(Self::default())
    }
}

/// 크롤링 설정 (호환성용)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingSettings {
    pub max_concurrent_requests: Option<u32>,
    pub timeout_seconds: Option<u64>,
    pub default_concurrency_limit: Option<u32>,
    pub request_delay_ms: Option<u64>,
}

/// Actor 설정 (호환성용)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyActorSettings {
    pub max_actors: Option<u32>,
    pub restart_policy: Option<String>,
}
