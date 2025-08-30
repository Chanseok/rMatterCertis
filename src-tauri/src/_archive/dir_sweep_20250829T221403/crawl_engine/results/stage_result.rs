//! 모든 단계의 실행 결과를 처리하는 회복탄력성 시스템
//! Modern Rust 2024 준수: mod.rs 금지, thiserror 사용, 구체적 Error 타입
//! 설정 기반 재시도 정책과 완전히 통합된 결과 처리

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::Duration;
use thiserror::Error;
use serde::{Deserialize, Serialize};
use crate::crawl_engine::{
    config::system_config::{RetryPolicy, RetryableErrorType},
    channels::types::{StageSuccessResult, CollectionMetrics, ProcessingMetrics, ProductInfo},
};

/// 모든 Stage의 실행 결과를 담는 통합 열거형
/// Modern Rust 2024: Error는 thiserror로 구체적 타입 정의
#[derive(Debug, Clone)]
pub enum StageResult {
    /// 성공 결과들
    Success(StageSuccessResult),
    
    /// 복구 가능한 오류 (재시도 대상)
    RecoverableError {
        error: StageError,
        attempts: u32,
        stage_id: String,
        suggested_retry_delay: Duration,
    },
    
    /// 복구 불가능한 오류 (즉시 실패 처리)
    FatalError {
        error: StageError,
        stage_id: String,
        context: String,
    },
    
    /// 부분 성공 (일부 항목 성공, 일부 실패)
    PartialSuccess {
        success_items: StageSuccessResult,
        failed_items: Vec<FailedItem>,
        stage_id: String,
    },
}

/// Modern Rust 2024: thiserror 사용한 구체적 Error 타입 정의
#[derive(Error, Debug, Clone)]
pub enum StageError {
    #[error("Network timeout: {message}")]
    NetworkTimeout { message: String },
    
    #[error("Server error {status}: {message}")]
    ServerError { status: u16, message: String },
    
    #[error("Rate limit exceeded: {retry_after:?}")]
    RateLimit { retry_after: Option<Duration> },
    
    #[error("Parse error: {message}")]
    ParseError { message: String },
    
    #[error("Database error: {message}")]
    DatabaseError { message: String },
    
    #[error("Validation error: {message}")]
    ValidationError { message: String },
    
    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },
    
    #[error("Resource exhausted: {resource}")]
    ResourceExhausted { resource: String },
    
    #[error("Authentication failed: {message}")]
    AuthenticationError { message: String },
    
    #[error("Permission denied: {message}")]
    PermissionError { message: String },
}

/// 실패한 항목 정보
#[derive(Debug, Clone)]
pub struct FailedItem {
    pub item_id: String,
    pub error: StageError,
    pub retry_count: u32,
    pub last_attempt: std::time::SystemTime,
}

/// 검증된 제품 정보
#[derive(Debug, Clone)]
pub struct ValidatedProduct {
    pub product: ProductInfo,
    pub validation_score: f64,
    pub validation_timestamp: std::time::SystemTime,
}

/// 검증 에러
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
    pub severity: ValidationSeverity,
}

#[derive(Debug, Clone)]
pub enum ValidationSeverity {
    Warning,
    Error,
    Critical,
}

/// 검증 메트릭
#[derive(Debug, Clone)]
pub struct ValidationMetrics {
    pub total_validated: u64,
    pub passed_validation: u64,
    pub failed_validation: u64,
    pub average_score: f64,
    pub duration_ms: u64,
}

/// 저장 메트릭
#[derive(Debug, Clone)]
pub struct SaveMetrics {
    pub total_attempted: u64,
    pub successfully_saved: u64,
    pub failed_saves: u64,
    pub average_save_time_ms: u64,
    pub duration_ms: u64,
}

/// 에러 타입 분류기
pub struct ErrorClassifier;

impl ErrorClassifier {
    /// StageError를 RetryableErrorType으로 변환
    pub fn classify_error(error: &StageError) -> Option<RetryableErrorType> {
        match error {
            StageError::NetworkTimeout { .. } => Some(RetryableErrorType::NetworkTimeout),
            
            StageError::ServerError { status, .. } => {
                Some(RetryableErrorType::ServerError { 
                    status_range: (*status, *status) 
                })
            }
            
            StageError::RateLimit { .. } => Some(RetryableErrorType::RateLimit),
            
            StageError::ParseError { .. } => Some(RetryableErrorType::ParseError),
            
            StageError::ValidationError { .. } => Some(RetryableErrorType::ValidationTimeout),
            
            StageError::DatabaseError { message } => {
                if message.contains("connection") {
                    Some(RetryableErrorType::DatabaseConnection)
                } else if message.contains("timeout") {
                    Some(RetryableErrorType::DatabaseTimeout)
                } else if message.contains("lock") {
                    Some(RetryableErrorType::DatabaseLock)
                } else {
                    None // 일반적인 DB 에러는 재시도하지 않음
                }
            }
            
            // 이 에러들은 재시도해도 의미 없음
            StageError::ConfigurationError { .. } |
            StageError::AuthenticationError { .. } |
            StageError::PermissionError { .. } => None,
            
            StageError::ResourceExhausted { .. } => {
                // 리소스 고갈은 상황에 따라 재시도 가능
                Some(RetryableErrorType::NetworkTimeout) // 임시로 네트워크 타임아웃으로 분류
            }
        }
    }
    
    /// 에러가 치명적인지 판단
    pub fn is_fatal_error(error: &StageError) -> bool {
        match error {
            StageError::ConfigurationError { .. } |
            StageError::AuthenticationError { .. } |
            StageError::PermissionError { .. } => true,
            
            StageError::DatabaseError { message } => {
                // 특정 DB 에러는 치명적
                message.contains("schema") || 
                message.contains("constraint") ||
                message.contains("corruption")
            }
            
            _ => false,
        }
    }
}

/// 재시도 계산기 - 설정 기반 재시도 로직
pub struct RetryCalculator;

impl RetryCalculator {
    /// 설정 기반 재시도 지연 계산 (Exponential Backoff + Jitter)
    pub fn calculate_delay(policy: &RetryPolicy, attempt: u32) -> Duration {
        let base_delay = policy.base_delay();
        let exponential_delay = Duration::from_millis(
            (base_delay.as_millis() as f64 * policy.backoff_multiplier.powi(attempt as i32 - 1)) as u64
        );
        
        let capped_delay = std::cmp::min(exponential_delay, policy.max_delay());
        
        // 설정된 범위에서 Jitter 추가 (fastrand 사용)
        let jitter = Duration::from_millis(
            fastrand::u64(0..=policy.jitter_range_ms)
        );
        
        capped_delay + jitter
    }
    
    /// 재시도 가능 여부 판단
    pub fn should_retry(
        policy: &RetryPolicy,
        error: &StageError,
        current_attempts: u32,
    ) -> bool {
        // 최대 시도 횟수 확인
        if current_attempts >= policy.max_attempts {
            return false;
        }
        
        // 치명적 오류는 재시도 안함
        if ErrorClassifier::is_fatal_error(error) {
            return false;
        }
        
        // 재시도 가능한 에러 타입인지 확인
        if let Some(error_type) = ErrorClassifier::classify_error(error) {
            policy.should_retry(&error_type)
        } else {
            false
        }
    }
}

impl StageResult {
    /// 성공 여부 확인
    pub fn is_success(&self) -> bool {
        matches!(self, StageResult::Success(_) | StageResult::PartialSuccess { .. })
    }
    
    /// 재시도 가능 여부 확인
    pub fn is_recoverable(&self) -> bool {
        matches!(self, StageResult::RecoverableError { .. })
    }
    
    /// 치명적 오류 여부 확인
    pub fn is_fatal(&self) -> bool {
        matches!(self, StageResult::FatalError { .. })
    }
    
    /// 성공 결과 추출
    pub fn success_result(&self) -> Option<&StageSuccessResult> {
        match self {
            StageResult::Success(result) => Some(result),
            StageResult::PartialSuccess { success_items, .. } => Some(success_items),
            _ => None,
        }
    }
    
    /// 에러 추출
    pub fn error(&self) -> Option<&StageError> {
        match self {
            StageResult::RecoverableError { error, .. } |
            StageResult::FatalError { error, .. } => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crawl_engine::config::system_config::{RetryPolicy, RetryableErrorType};

    #[test]
    fn test_error_classification() {
        let network_error = StageError::NetworkTimeout {
            message: "Connection timed out".to_string(),
        };
        
        let classified = ErrorClassifier::classify_error(&network_error);
        assert!(matches!(classified, Some(RetryableErrorType::NetworkTimeout)));
        
        let config_error = StageError::ConfigurationError {
            message: "Invalid config".to_string(),
        };
        
        let classified = ErrorClassifier::classify_error(&config_error);
        assert!(classified.is_none());
    }
    
    #[test]
    fn test_fatal_error_detection() {
        let auth_error = StageError::AuthenticationError {
            message: "Invalid credentials".to_string(),
        };
        assert!(ErrorClassifier::is_fatal_error(&auth_error));
        
        let network_error = StageError::NetworkTimeout {
            message: "Timeout".to_string(),
        };
        assert!(!ErrorClassifier::is_fatal_error(&network_error));
    }
    
    #[test]
    fn test_retry_calculation() {
        let policy = RetryPolicy {
            max_attempts: 3,
            base_delay_ms: 1000,
            max_delay_ms: 10000,
            backoff_multiplier: 2.0,
            jitter_range_ms: 100,
            retry_on_errors: vec![RetryableErrorType::NetworkTimeout],
        };
        
        // 첫 번째 재시도 (attempt = 1)
        let delay1 = RetryCalculator::calculate_delay(&policy, 1);
        assert!(delay1 >= Duration::from_millis(1000));
        assert!(delay1 <= Duration::from_millis(1100));
        
        // 두 번째 재시도 (attempt = 2)
        let delay2 = RetryCalculator::calculate_delay(&policy, 2);
        assert!(delay2 >= Duration::from_millis(2000));
        assert!(delay2 <= Duration::from_millis(2100));
    }
    
    #[test]
    fn test_should_retry_logic() {
        let policy = RetryPolicy {
            max_attempts: 3,
            base_delay_ms: 1000,
            max_delay_ms: 10000,
            backoff_multiplier: 2.0,
            jitter_range_ms: 100,
            retry_on_errors: vec![RetryableErrorType::NetworkTimeout],
        };
        
        let network_error = StageError::NetworkTimeout {
            message: "Timeout".to_string(),
        };
        
        // 첫 번째 시도 실패 - 재시도 가능
        assert!(RetryCalculator::should_retry(&policy, &network_error, 1));
        
        // 최대 시도 횟수 도달 - 재시도 불가
        assert!(!RetryCalculator::should_retry(&policy, &network_error, 3));
        
        // 치명적 오류 - 재시도 불가
        let fatal_error = StageError::AuthenticationError {
            message: "Invalid auth".to_string(),
        };
        assert!(!RetryCalculator::should_retry(&policy, &fatal_error, 1));
    }
    
    #[test]
    fn test_stage_result_methods() {
        let success_result = StageResult::Success(
            StageSuccessResult::ListCollection {
                collected_urls: vec!["http://example.com".to_string()],
                total_pages: 1,
                successful_pages: vec![1],
                failed_pages: vec![],
                collection_metrics: CollectionMetrics {
                    duration_ms: 1000,
                    avg_response_time_ms: 200,
                    success_rate: 1.0,
                },
            }
        );
        
        assert!(success_result.is_success());
        assert!(!success_result.is_recoverable());
        assert!(!success_result.is_fatal());
        assert!(success_result.success_result().is_some());
    }
}
