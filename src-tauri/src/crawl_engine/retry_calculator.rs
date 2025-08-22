//! 재시도 계산기 모듈
//! OneShot Actor 시스템에서 사용하는 재시도 로직 구현

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::Duration;
use serde::{Deserialize, Serialize};

/// 재시도 정책 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// 최대 재시도 횟수
    pub max_attempts: u32,
    /// 기본 지연 시간 (밀리초)
    pub base_delay_ms: u64,
    /// 최대 지연 시간 (밀리초)
    pub max_delay_ms: u64,
    /// 백오프 승수 (예: 2.0 = 배수 증가)
    pub backoff_multiplier: f64,
    /// 지터 범위 (밀리초)
    pub jitter_range_ms: u64,
    /// 재시도 대상 오류 타입들
    pub retry_on_errors: Vec<RetryableErrorType>,
}

/// 재시도 가능한 오류 타입
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetryableErrorType {
    NetworkTimeout,
    NetworkConnectionError,
    ResourceExhausted,
    TemporaryFailure,
    RateLimited,
}

/// 재시도 계산기
pub struct RetryCalculator {
    max_retries: u32,
    base_delay_ms: u64,
    max_delay_ms: u64,
    backoff_multiplier: f64,
    enable_jitter: bool,
}

impl RetryCalculator {
    /// 새로운 재시도 계산기 생성
    pub fn new(
        max_retries: u32,
        base_delay_ms: u64,
        max_delay_ms: u64,
        backoff_multiplier: f64,
        enable_jitter: bool,
    ) -> Self {
        Self {
            max_retries,
            base_delay_ms,
            max_delay_ms,
            backoff_multiplier,
            enable_jitter,
        }
    }
    
    /// 기본 재시도 계산기 생성
    pub fn default() -> Self {
        Self::new(3, 1000, 30000, 2.0, true)
    }
    
    /// 재시도 가능 여부 확인
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }
    
    /// 정책 기반 재시도 가능 여부 확인
    pub fn should_retry_with_policy(
        policy: &RetryPolicy,
        error_type: &RetryableErrorType,
        current_attempts: u32,
    ) -> bool {
        // 최대 시도 횟수 확인
        if current_attempts >= policy.max_attempts {
            return false;
        }
        
        // 재시도 가능한 에러 타입인지 확인
        policy.retry_on_errors.contains(error_type)
    }
    
    /// 지연 시간 계산 (Exponential Backoff)
    pub fn calculate_delay(&self, attempt: u32) -> u64 {
        let exponential_delay = (self.base_delay_ms as f64 
            * self.backoff_multiplier.powi(attempt as i32 - 1)) as u64;
        
        let capped_delay = std::cmp::min(exponential_delay, self.max_delay_ms);
        
        if self.enable_jitter {
            // 50%-150% 범위의 지터 추가
            let jitter_factor = 0.5 + (fastrand::f64() * 1.0); // 0.5 ~ 1.5
            (capped_delay as f64 * jitter_factor) as u64
        } else {
            capped_delay
        }
    }
    
    /// 정책 기반 지연 시간 계산
    pub fn calculate_delay_with_policy(
        policy: &RetryPolicy, 
        attempt: u32
    ) -> Duration {
        let base_delay = Duration::from_millis(policy.base_delay_ms);
        let exponential_delay = Duration::from_millis(
            (base_delay.as_millis() as f64 * policy.backoff_multiplier.powi(attempt as i32 - 1)) as u64
        );
        
        let capped_delay = std::cmp::min(
            exponential_delay, 
            Duration::from_millis(policy.max_delay_ms)
        );
        
        // 설정된 범위에서 Jitter 추가
        let jitter = Duration::from_millis(
            fastrand::u64(0..=policy.jitter_range_ms)
        );
        
        capped_delay + jitter
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 1000,
            max_delay_ms: 30000,
            backoff_multiplier: 2.0,
            jitter_range_ms: 500,
            retry_on_errors: vec![
                RetryableErrorType::NetworkTimeout,
                RetryableErrorType::NetworkConnectionError,
                RetryableErrorType::TemporaryFailure,
            ],
        }
    }
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
        self.retry_on_errors.contains(error_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_calculator_basic() {
        println!("🧪 RetryCalculator 기본 테스트 시작");

        let calculator = RetryCalculator::new(3, 100, 5000, 2.0, true);

        // 첫 번째 시도
        assert!(calculator.should_retry(1));
        let delay1 = calculator.calculate_delay(1);
        println!("   1차 재시도 지연: {}ms", delay1);
        assert!(delay1 >= 50 && delay1 <= 150); // 지터 포함 범위

        // 두 번째 시도
        assert!(calculator.should_retry(2));
        let delay2 = calculator.calculate_delay(2);
        println!("   2차 재시도 지연: {}ms", delay2);
        assert!(delay2 >= 100 && delay2 <= 300); // 지터 포함 범위

        // 최대 시도 초과
        assert!(!calculator.should_retry(3));
        assert!(!calculator.should_retry(4));

        println!("🎯 RetryCalculator 기본 테스트 완료!");
    }

    #[test]
    fn test_retry_policy() {
        println!("🧪 RetryPolicy 테스트 시작");

        let policy = RetryPolicy::default();

        // 재시도 가능한 오류
        assert!(RetryCalculator::should_retry_with_policy(
            &policy,
            &RetryableErrorType::NetworkTimeout,
            1
        ));

        // 재시도 불가능한 오류 (정책에 없음)
        assert!(!RetryCalculator::should_retry_with_policy(
            &policy,
            &RetryableErrorType::RateLimited,
            1
        ));

        // 최대 시도 횟수 도달
        assert!(!RetryCalculator::should_retry_with_policy(
            &policy,
            &RetryableErrorType::NetworkTimeout,
            3
        ));

        println!("🎯 RetryPolicy 테스트 완료!");
    }

    #[test]
    fn test_delay_calculation_with_policy() {
        println!("🧪 정책 기반 지연 계산 테스트 시작");

        let policy = RetryPolicy {
            max_attempts: 3,
            base_delay_ms: 1000,
            max_delay_ms: 10000,
            backoff_multiplier: 2.0,
            jitter_range_ms: 100,
            retry_on_errors: vec![RetryableErrorType::NetworkTimeout],
        };

        // 첫 번째 재시도 (attempt = 1)
        let delay1 = RetryCalculator::calculate_delay_with_policy(&policy, 1);
        println!("   1차 재시도 지연: {:?}", delay1);
        assert!(delay1 >= Duration::from_millis(1000));
        assert!(delay1 <= Duration::from_millis(1100));

        // 두 번째 재시도 (attempt = 2)
        let delay2 = RetryCalculator::calculate_delay_with_policy(&policy, 2);
        println!("   2차 재시도 지연: {:?}", delay2);
        assert!(delay2 >= Duration::from_millis(2000));
        assert!(delay2 <= Duration::from_millis(2100));

        println!("🎯 정책 기반 지연 계산 테스트 완료!");
    }
}
