//! 전체 시스템 설정 통합 관리
//! Modern Rust 2024: serde, config crate 활용한 설정 시스템
//! 모든 하드코딩 값을 제거하고 설정 파일 기반으로 완전히 구성 가능한 시스템

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![deny(clippy::unimplemented, clippy::todo)]

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
    
    #[error("Environment variable error: {message}")]
    Environment { message: String },
}

/// 전체 시스템 설정 - 모든 하드코딩 값을 설정 파일로 이전
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    /// 전체 시스템 설정
    pub system: SystemSettings,
    
    /// 재시도 정책들
    pub retry_policies: RetryPolicies,
    
    /// 성능 튜닝 설정
    pub performance: PerformanceSettings,
    
    /// 모니터링 설정
    pub monitoring: MonitoringSettings,
    
    /// 채널 크기 설정
    pub channels: ChannelSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSettings {
    /// 최대 동시 세션 수
    pub max_concurrent_sessions: u32,
    
    /// 세션 타임아웃 (초)
    pub session_timeout_secs: u64,
    
    /// 스테이지 타임아웃 (초)
    pub stage_timeout_secs: Option<u64>,
    
    /// 전역 취소 타임아웃 (초)
    pub cancellation_timeout_secs: u64,
    
    /// 메모리 사용량 제한 (MB)
    pub memory_limit_mb: u64,
    
    /// 데이터베이스 오류 시 세션 중단 여부
    pub abort_on_database_error: Option<bool>,
    
    /// 검증 오류 시 세션 중단 여부
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
    /// 배치 크기 설정
    pub batch_sizes: BatchSizeSettings,
    
    /// 동시성 제어
    pub concurrency: ConcurrencySettings,
    
    /// 버퍼 크기 설정
    pub buffers: BufferSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSizeSettings {
    /// 초기 배치 크기
    pub initial_size: u32,
    
    /// 최소 배치 크기
    pub min_size: u32,
    
    /// 최대 배치 크기
    pub max_size: u32,
    
    /// 자동 조정 임계값 (성공률 %)
    pub auto_adjust_threshold: f64,
    
    /// 크기 조정 배수
    pub adjust_multiplier: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrencySettings {
    /// 최대 동시 작업 수
    pub max_concurrent_tasks: u32,
    
    /// StageActor별 동시성 제한
    pub stage_concurrency_limits: HashMap<String, u32>,
    
    /// 작업 큐 크기
    pub task_queue_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferSettings {
    /// 요청 버퍼 크기
    pub request_buffer_size: usize,
    
    /// 응답 버퍼 크기
    pub response_buffer_size: usize,
    
    /// 임시 저장소 제한 (MB)
    pub temp_storage_limit_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelSettings {
    /// 제어 채널 버퍼 크기
    pub control_buffer_size: usize,
    
    /// 이벤트 채널 버퍼 크기
    pub event_buffer_size: usize,
    
    /// 백프레셔 임계값
    pub backpressure_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringSettings {
    /// 메트릭 수집 간격 (초)
    pub metrics_interval_secs: u64,
    
    /// 로그 레벨 설정
    pub log_level: String,
    
    /// 성능 프로파일링 활성화
    pub enable_profiling: bool,
    
    /// 이벤트 저장 기간 (일)
    pub event_retention_days: u32,
}

impl RetryPolicy {
    /// Duration 변환 헬퍼 메서드들
    pub fn base_delay(&self) -> Duration {
        Duration::from_millis(self.base_delay_ms)
    }
    
    pub fn max_delay(&self) -> Duration {
        Duration::from_millis(self.max_delay_ms)
    }
    
    pub fn jitter_range(&self) -> Duration {
        Duration::from_millis(self.jitter_range_ms)
    }
    
    /// 에러 타입이 재시도 대상인지 확인
    pub fn should_retry(&self, error_type: &RetryableErrorType) -> bool {
        self.retry_on_errors.iter().any(|retry_type| {
            match (retry_type, error_type) {
                (RetryableErrorType::NetworkTimeout, RetryableErrorType::NetworkTimeout) => true,
                (RetryableErrorType::RateLimit, RetryableErrorType::RateLimit) => true,
                (RetryableErrorType::ParseError, RetryableErrorType::ParseError) => true,
                (
                    RetryableErrorType::ServerError { status_range: (min1, max1) },
                    RetryableErrorType::ServerError { status_range: (min2, max2) }
                ) => min1 <= min2 && max2 <= max1,
                _ => false,
            }
        })
    }
}

impl SystemConfig {
    /// 설정 파일에서 로드 - Modern Rust 2024 패턴
    pub fn from_file(path: &str) -> Result<Self, ConfigError> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name(path))
            .add_source(config::Environment::with_prefix("RMATTERCERTIS"))
            .build()?;
        
        let config: Self = settings.try_deserialize()?;
        config.validate()?;
        Ok(config)
    }
    
    /// 환경별 설정 로드
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
    
    /// 설정값 유효성 검증
    pub fn validate(&self) -> Result<(), ConfigError> {
        // 배치 크기 검증
        if self.performance.batch_sizes.min_size > self.performance.batch_sizes.max_size {
            return Err(ConfigError::Validation {
                message: "min_size cannot be greater than max_size".to_string(),
            });
        }
        
        // 타임아웃 검증
        if self.system.session_timeout_secs == 0 {
            return Err(ConfigError::Validation {
                message: "session_timeout_secs must be greater than 0".to_string(),
            });
        }
        
        // 재시도 정책 검증
        for (name, policy) in [
            ("list_collection", &self.retry_policies.list_collection),
            ("detail_collection", &self.retry_policies.detail_collection),
            ("data_validation", &self.retry_policies.data_validation),
            ("database_save", &self.retry_policies.database_save),
        ] {
            if policy.max_attempts == 0 {
                return Err(ConfigError::Validation {
                    message: format!("{} max_attempts must be greater than 0", name),
                });
            }
            
            if policy.base_delay_ms > policy.max_delay_ms {
                return Err(ConfigError::Validation {
                    message: format!("{} base_delay_ms cannot be greater than max_delay_ms", name),
                });
            }
        }
        
        // 채널 크기 검증
        if self.channels.control_buffer_size == 0 || self.channels.event_buffer_size == 0 {
            return Err(ConfigError::Validation {
                message: "Channel buffer sizes must be greater than 0".to_string(),
            });
        }
        
        Ok(())
    }
    
    /// 기본 설정 생성 (하드코딩 제거를 위한 합리적 기본값들)
    pub fn default() -> Self {
        Self {
            system: SystemSettings {
                max_concurrent_sessions: 10,
                session_timeout_secs: 3600, // 1시간
                stage_timeout_secs: Some(300), // 5분
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
                    retry_on_errors: vec![
                        RetryableErrorType::ValidationTimeout,
                    ],
                },
                database_save: RetryPolicy {
                    max_attempts: 10,
                    base_delay_ms: 200,
                    max_delay_ms: 30000,
                    backoff_multiplier: 1.8,
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
                    initial_size: 10,
                    min_size: 1,
                    max_size: 100,
                    auto_adjust_threshold: 0.8,
                    adjust_multiplier: 1.5,
                },
                concurrency: ConcurrencySettings {
                    max_concurrent_tasks: 50,
                    stage_concurrency_limits: HashMap::from([
                        ("list_collection".to_string(), 5),
                        ("detail_collection".to_string(), 20),
                        ("data_validation".to_string(), 10),
                        ("database_save".to_string(), 3),
                        ("batch_processing".to_string(), 10),
                    ]),
                    task_queue_size: 1000,
                },
                buffers: BufferSettings {
                    request_buffer_size: 10000,
                    response_buffer_size: 10000,
                    temp_storage_limit_mb: 500,
                },
            },
            channels: ChannelSettings {
                control_buffer_size: 100,
                event_buffer_size: 1000,
                backpressure_threshold: 0.8,
            },
            monitoring: MonitoringSettings {
                metrics_interval_secs: 30,
                log_level: "INFO".to_string(),
                enable_profiling: false,
                event_retention_days: 7,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_valid() {
        let config = SystemConfig::default();
        assert!(config.validate().is_ok());
    }
    
    #[test]
    fn test_retry_policy_duration_helpers() {
        let policy = RetryPolicy {
            base_delay_ms: 1000,
            max_delay_ms: 30000,
            jitter_range_ms: 500,
            max_attempts: 3,
            backoff_multiplier: 2.0,
            retry_on_errors: vec![],
        };
        
        assert_eq!(policy.base_delay(), Duration::from_millis(1000));
        assert_eq!(policy.max_delay(), Duration::from_millis(30000));
        assert_eq!(policy.jitter_range(), Duration::from_millis(500));
    }
    
    #[test]
    fn test_config_validation() {
        let mut config = SystemConfig::default();
        
        // 유효한 설정
        assert!(config.validate().is_ok());
        
        // 잘못된 배치 크기
        config.performance.batch_sizes.min_size = 100;
        config.performance.batch_sizes.max_size = 50;
        assert!(config.validate().is_err());
        
        // 설정 복원
        config.performance.batch_sizes.min_size = 1;
        config.performance.batch_sizes.max_size = 100;
        
        // 잘못된 타임아웃
        config.system.session_timeout_secs = 0;
        assert!(config.validate().is_err());
    }
}
