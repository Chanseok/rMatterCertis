//! 회복탄력성 결과 시스템 - Stage 실행 결과와 복구 전략
//! Modern Rust 2024 준수: thiserror와 구조화된 에러 처리

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashMap;
use std::time::Duration;
use serde::{Serialize, Deserialize};
use thiserror::Error;

use crate::new_architecture::{
    actor_system::{StageSuccessResult, StageError},
    channel_types::StageType,
};

/// 회복탄력성 Stage 결과 시스템
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResilienceStageResult {
    /// 완전 성공
    Success {
        result: StageSuccessResult,
        stage_type: StageType,
        resilience_metrics: ResilienceMetrics,
    },
    /// 복구 가능한 에러 (재시도 권장)
    RecoverableError {
        error: ResilienceError,
        attempts: u32,
        stage_id: String,
        suggested_retry_delay: Duration,
        recovery_strategy: RecoveryStrategy,
    },
    /// 치명적 에러 (복구 불가능)
    FatalError {
        error: ResilienceError,
        stage_id: String,
        context: String,
        failure_analysis: FailureAnalysis,
    },
    /// 부분 성공 (일부 항목만 성공)
    PartialSuccess {
        success_items: StageSuccessResult,
        failed_items: Vec<FailedItem>,
        stage_id: String,
        recovery_recommendations: Vec<RecoveryRecommendation>,
    },
}

/// 복구 가능한 에러 타입 (확장된 StageError)
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum ResilienceError {
    #[error("Network timeout: {message} (retry recommended after {suggested_delay:?})")]
    NetworkTimeout { 
        message: String,
        suggested_delay: Duration,
    },
    
    #[error("Rate limit exceeded: retry after {retry_after:?}")]
    RateLimit { 
        retry_after: Option<Duration>,
        current_rate: f64,
        limit_threshold: f64,
    },
    
    #[error("Resource temporarily unavailable: {resource_type}")]
    ResourceExhausted { 
        resource_type: String,
        current_usage: f64,
        max_capacity: f64,
        recovery_time_estimate: Duration,
    },
    
    #[error("Parsing error: {message} (data integrity issue)")]
    DataIntegrityError { 
        message: String,
        corrupted_field: String,
        suggested_fallback: Option<String>,
    },
    
    #[error("Database connection error: {message}")]
    DatabaseConnectionError {
        message: String,
        connection_pool_status: String,
        retry_possible: bool,
    },
    
    #[error("Authentication failed: {reason}")]
    AuthenticationError {
        reason: String,
        auth_method: String,
        retry_with_refresh: bool,
    },
    
    #[error("Task cancelled: {task_id}")]
    TaskCancelled { task_id: String },
    
    #[error("Task execution failed: {task_id} - {message}")]
    TaskExecutionFailed { task_id: String, message: String },
    
    #[error("Configuration error: {message} (requires manual intervention)")]
    ConfigurationError { message: String },
}

/// 복구 전략
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryStrategy {
    /// 즉시 재시도
    ImmediateRetry { max_attempts: u32 },
    /// 지연 후 재시도
    DelayedRetry { 
        delay: Duration, 
        max_attempts: u32,
        backoff_multiplier: f64,
    },
    /// 대체 방법 시도
    AlternativeMethod { 
        alternative: String,
        fallback_timeout: Duration,
    },
    /// 부분 복구 (가능한 부분만 처리)
    PartialRecovery { 
        partial_threshold: f64,
        continue_with_partial: bool,
    },
    /// 수동 개입 필요
    ManualIntervention { 
        intervention_type: String,
        estimated_resolution_time: Duration,
    },
    /// 복구 불가능 (중단)
    NoRecovery { reason: String },
}

/// 복구 권장사항
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryRecommendation {
    pub priority: RecoveryPriority,
    pub action: String,
    pub estimated_impact: String,
    pub estimated_duration: Duration,
    pub required_resources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryPriority {
    Critical,
    High,
    Medium,
    Low,
}

/// 실패 분석
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureAnalysis {
    pub root_cause: String,
    pub contributing_factors: Vec<String>,
    pub impact_assessment: ImpactAssessment,
    pub prevention_suggestions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAssessment {
    pub affected_items: u32,
    pub data_loss_risk: RiskLevel,
    pub performance_impact: RiskLevel,
    pub user_experience_impact: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    None,
    Low,
    Medium,
    High,
    Critical,
}

/// 회복탄력성 메트릭스
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResilienceMetrics {
    pub recovery_time: Duration,
    pub retry_attempts: u32,
    pub success_rate: f64,
    pub resilience_score: f64,
    pub stability_indicators: HashMap<String, f64>,
}

/// 실패한 항목
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedItem {
    pub item_id: String,
    pub error: ResilienceError,
    pub retry_count: u32,
    pub last_attempt: chrono::DateTime<chrono::Utc>,
    pub recoverable: bool,
}

/// 회복탄력성 분석기
pub struct ResilienceAnalyzer {
    config: ResilienceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResilienceConfig {
    pub max_retry_attempts: u32,
    pub base_retry_delay: Duration,
    pub max_retry_delay: Duration,
    pub backoff_multiplier: f64,
    pub partial_success_threshold: f64,
    pub failure_rate_threshold: f64,
}

impl Default for ResilienceConfig {
    fn default() -> Self {
        Self {
            max_retry_attempts: 3,
            base_retry_delay: Duration::from_secs(1),
            max_retry_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            partial_success_threshold: 0.5,
            failure_rate_threshold: 0.1,
        }
    }
}

impl ResilienceAnalyzer {
    /// 새로운 회복탄력성 분석기 생성
    pub fn new(config: ResilienceConfig) -> Self {
        Self { config }
    }

    /// 기본 설정으로 분석기 생성
    pub fn default() -> Self {
        Self {
            config: ResilienceConfig::default(),
        }
    }

    /// Stage 에러 분석 (테스트용 메서드)
    pub fn analyze_stage_error(
        &self,
        error: &StageError,
        attempt_count: u32,
        stage_id: String,
    ) -> ResilienceStageResult {
        self.create_recoverable_error(error.clone(), attempt_count, stage_id)
    }

    /// Stage 에러 분석 및 복구 전략 결정
    pub fn create_recoverable_error(
        &self,
        error: StageError,
        attempt_count: u32,
        stage_id: String,
    ) -> ResilienceStageResult {
        let resilience_error = self.convert_to_resilience_error(&error);
        let recovery_strategy = self.determine_recovery_strategy(&resilience_error, attempt_count);

        match recovery_strategy {
            RecoveryStrategy::NoRecovery { reason } => {
                ResilienceStageResult::FatalError {
                    error: resilience_error.clone(),
                    stage_id,
                    context: reason.clone(),
                    failure_analysis: self.analyze_failure(&resilience_error, &reason),
                }
            }
            strategy => {
                let retry_delay = self.calculate_retry_delay(attempt_count);
                ResilienceStageResult::RecoverableError {
                    error: resilience_error,
                    attempts: attempt_count,
                    stage_id,
                    suggested_retry_delay: retry_delay,
                    recovery_strategy: strategy,
                }
            }
        }
    }

    /// 부분 성공 결과 분석
    pub fn analyze_partial_success(
        &self,
        success_items: StageSuccessResult,
        failed_items: Vec<FailedItem>,
        stage_id: String,
    ) -> ResilienceStageResult {
        let recovery_recommendations = self.generate_recovery_recommendations(&failed_items);
        
        ResilienceStageResult::PartialSuccess {
            success_items,
            failed_items,
            stage_id,
            recovery_recommendations,
        }
    }

    /// 성공 결과에 회복탄력성 메트릭스 추가
    pub fn enhance_success_result(
        &self,
        result: StageSuccessResult,
        stage_type: StageType,
        recovery_time: Duration,
        retry_attempts: u32,
    ) -> ResilienceStageResult {
        let resilience_metrics = ResilienceMetrics {
            recovery_time,
            retry_attempts,
            success_rate: 1.0,
            resilience_score: self.calculate_resilience_score(retry_attempts, recovery_time),
            stability_indicators: self.calculate_stability_indicators(&result),
        };

        ResilienceStageResult::Success {
            result,
            stage_type,
            resilience_metrics,
        }
    }

    /// StageError를 ResilienceError로 변환
    fn convert_to_resilience_error(&self, error: &StageError) -> ResilienceError {
        match error {
            StageError::NetworkError { message } => ResilienceError::NetworkTimeout {
                message: message.clone(),
                suggested_delay: self.config.base_retry_delay,
            },
            StageError::NetworkTimeout { message } => ResilienceError::NetworkTimeout {
                message: message.clone(),
                suggested_delay: self.config.base_retry_delay * 2,
            },
            StageError::ParsingError { message } => ResilienceError::DataIntegrityError {
                message: message.clone(),
                corrupted_field: "unknown".to_string(),
                suggested_fallback: None,
            },
            StageError::DatabaseError { message } => ResilienceError::DatabaseConnectionError {
                message: message.clone(),
                connection_pool_status: "unknown".to_string(),
                retry_possible: true,
            },
            StageError::ResourceExhausted { message } => ResilienceError::ResourceExhausted {
                resource_type: "system".to_string(),
                current_usage: 0.9,
                max_capacity: 1.0,
                recovery_time_estimate: self.config.base_retry_delay * 3,
            },
            StageError::TaskCancelled { task_id } => ResilienceError::TaskCancelled {
                task_id: task_id.clone(),
            },
            StageError::TaskExecutionFailed { task_id, message } => ResilienceError::TaskExecutionFailed {
                task_id: task_id.clone(),
                message: message.clone(),
            },
            StageError::ConfigurationError { message } => ResilienceError::ConfigurationError {
                message: message.clone(),
            },
            _ => ResilienceError::ConfigurationError {
                message: format!("Unknown error: {}", error),
            },
        }
    }

    /// 복구 전략 결정
    fn determine_recovery_strategy(
        &self,
        error: &ResilienceError,
        attempt_count: u32,
    ) -> RecoveryStrategy {
        if attempt_count >= self.config.max_retry_attempts {
            return RecoveryStrategy::NoRecovery {
                reason: format!("Maximum retry attempts ({}) exceeded", self.config.max_retry_attempts),
            };
        }

        match error {
            ResilienceError::NetworkTimeout { .. } => RecoveryStrategy::DelayedRetry {
                delay: self.calculate_retry_delay(attempt_count),
                max_attempts: self.config.max_retry_attempts,
                backoff_multiplier: self.config.backoff_multiplier,
            },
            ResilienceError::RateLimit { retry_after, .. } => RecoveryStrategy::DelayedRetry {
                delay: retry_after.unwrap_or(self.config.base_retry_delay * 5),
                max_attempts: self.config.max_retry_attempts,
                backoff_multiplier: 1.0, // Rate limit에서는 백오프 사용하지 않음
            },
            ResilienceError::ResourceExhausted { recovery_time_estimate, .. } => {
                RecoveryStrategy::DelayedRetry {
                    delay: *recovery_time_estimate,
                    max_attempts: self.config.max_retry_attempts - 1,
                    backoff_multiplier: self.config.backoff_multiplier,
                }
            },
            ResilienceError::AuthenticationError { retry_with_refresh, .. } => {
                if *retry_with_refresh {
                    RecoveryStrategy::AlternativeMethod {
                        alternative: "refresh_token".to_string(),
                        fallback_timeout: Duration::from_secs(30),
                    }
                } else {
                    RecoveryStrategy::NoRecovery {
                        reason: "Authentication cannot be refreshed".to_string(),
                    }
                }
            },
            ResilienceError::TaskCancelled { .. } => RecoveryStrategy::NoRecovery {
                reason: "Task was explicitly cancelled".to_string(),
            },
            ResilienceError::ConfigurationError { .. } => RecoveryStrategy::ManualIntervention {
                intervention_type: "Configuration fix required".to_string(),
                estimated_resolution_time: Duration::from_secs(300),
            },
            _ => RecoveryStrategy::ImmediateRetry {
                max_attempts: self.config.max_retry_attempts,
            },
        }
    }

    /// 재시도 지연 시간 계산 (Exponential Backoff)
    fn calculate_retry_delay(&self, attempt_count: u32) -> Duration {
        let delay_secs = self.config.base_retry_delay.as_secs_f64()
            * self.config.backoff_multiplier.powi(attempt_count as i32);
        
        let max_delay_secs = self.config.max_retry_delay.as_secs_f64();
        Duration::from_secs_f64(delay_secs.min(max_delay_secs))
    }

    /// 실패 분석 수행
    fn analyze_failure(&self, error: &ResilienceError, context: &str) -> FailureAnalysis {
        FailureAnalysis {
            root_cause: format!("Error type: {}", error),
            contributing_factors: vec![
                "Network instability".to_string(),
                "System resource constraints".to_string(),
                context.to_string(),
            ],
            impact_assessment: ImpactAssessment {
                affected_items: 1,
                data_loss_risk: RiskLevel::Low,
                performance_impact: RiskLevel::Medium,
                user_experience_impact: RiskLevel::Low,
            },
            prevention_suggestions: vec![
                "Implement better error handling".to_string(),
                "Add circuit breaker pattern".to_string(),
                "Increase timeout values".to_string(),
            ],
        }
    }

    /// 복구 권장사항 생성
    fn generate_recovery_recommendations(&self, failed_items: &[FailedItem]) -> Vec<RecoveryRecommendation> {
        let mut recommendations = Vec::new();

        for item in failed_items {
            if item.recoverable && item.retry_count < self.config.max_retry_attempts {
                recommendations.push(RecoveryRecommendation {
                    priority: RecoveryPriority::High,
                    action: format!("Retry failed item: {}", item.item_id),
                    estimated_impact: "Resume processing of failed item".to_string(),
                    estimated_duration: self.calculate_retry_delay(item.retry_count),
                    required_resources: vec!["Network".to_string(), "CPU".to_string()],
                });
            }
        }

        recommendations
    }

    /// 회복탄력성 점수 계산
    fn calculate_resilience_score(&self, retry_attempts: u32, recovery_time: Duration) -> f64 {
        let attempt_penalty = retry_attempts as f64 * 0.1;
        let time_penalty = recovery_time.as_secs_f64() / 60.0 * 0.05; // 분당 5% 감점
        
        (1.0 - attempt_penalty - time_penalty).max(0.0).min(1.0)
    }

    /// 안정성 지표 계산
    fn calculate_stability_indicators(&self, result: &StageSuccessResult) -> HashMap<String, f64> {
        let mut indicators = HashMap::new();
        
        indicators.insert("processing_speed".to_string(), 
            1000.0 / result.stage_duration_ms as f64);
        indicators.insert("success_ratio".to_string(), 
            result.processed_items as f64 / result.processed_items.max(1) as f64);
        indicators.insert("efficiency_score".to_string(), 0.85);
        
        indicators
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::new_architecture::actor_system::StageError;

    #[test]
    fn test_resilience_analyzer_creation() {
        let analyzer = ResilienceAnalyzer::default();
        assert_eq!(analyzer.config.max_retry_attempts, 3);
    }

    #[test]
    fn test_network_error_analysis() {
        let analyzer = ResilienceAnalyzer::default();
        let error = StageError::NetworkError { 
            message: "Connection timeout".to_string() 
        };
        
        let result = analyzer.analyze_stage_error(&error, 1, "test-stage".to_string());
        
        match result {
            ResilienceStageResult::RecoverableError { recovery_strategy, .. } => {
                match recovery_strategy {
                    RecoveryStrategy::DelayedRetry { .. } => {
                        // Expected behavior
                    },
                    _ => panic!("Expected DelayedRetry strategy"),
                }
            },
            _ => panic!("Expected RecoverableError result"),
        }
    }

    #[test]
    fn test_max_retry_attempts_exceeded() {
        let analyzer = ResilienceAnalyzer::default();
        let error = StageError::NetworkError { 
            message: "Connection timeout".to_string() 
        };
        
        let result = analyzer.analyze_stage_error(&error, 5, "test-stage".to_string());
        
        match result {
            ResilienceStageResult::FatalError { .. } => {
                // Expected behavior when max attempts exceeded
            },
            _ => panic!("Expected FatalError result when max attempts exceeded"),
        }
    }

    #[test]
    fn test_retry_delay_calculation() {
        let analyzer = ResilienceAnalyzer::default();
        
        let delay1 = analyzer.calculate_retry_delay(1);
        let delay2 = analyzer.calculate_retry_delay(2);
        
        assert!(delay2 > delay1, "Delay should increase with attempt count");
    }
}
