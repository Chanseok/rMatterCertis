use crate::infrastructure::config::AppConfig;
use std::time::Duration;

/// 검증되고 적용 가능한 크롤링 설정 구조체
/// 
/// 사용자 설정을 검증하고 하드코딩된 값들을 제거하기 위한 구조체입니다.
/// 모든 설정값은 실제 사용 가능한 범위로 검증되며, 이 값들이 실제 크롤링에서 사용됩니다.
#[derive(Debug, Clone)]
pub struct ValidatedCrawlingConfig {
    /// 검증된 최대 동시 요청 수 (하드코딩 제거)
    pub max_concurrent: u32,
    /// 검증된 페이지 범위 제한 (하드코딩 제거) 
    pub page_range_limit: u32,
    /// 검증된 배치 크기
    pub batch_size: u32,
    /// 세마포어 허용 수 (max_concurrent와 동일)
    pub semaphore_permits: usize,
    /// 요청 간 지연 시간
    pub request_delay_ms: u64,
    /// 재시도 횟수
    pub max_retries: u32,
    /// 타임아웃 설정
    pub request_timeout_seconds: u32,
}

impl ValidatedCrawlingConfig {
    /// 사용자 설정에서 검증된 크롤링 설정 생성
    /// 
    /// 모든 설정값을 검증하고 안전한 범위로 제한합니다.
    pub fn from_user_config(config: &AppConfig) -> Self {
        let user_config = &config.user;
        let crawling_config = &user_config.crawling;
        let workers_config = &crawling_config.workers;
        
        // 동시성 설정 검증 (1-50 범위)
        let max_concurrent = workers_config.list_page_max_concurrent
            .max(1)   // 최소 1개
            .min(50); // 최대 50개
            
        // 페이지 범위 제한 검증 (1-500 범위)  
        let page_range_limit = crawling_config.page_range_limit
            .max(1)    // 최소 1페이지
            .min(500); // 최대 500페이지
            
        // 배치 크기 검증 (1-200 범위)
        let batch_size = workers_config.db_batch_size
            .max(1)    // 최소 1개
            .min(200); // 최대 200개
            
        // 요청 지연 시간 검증 (100ms-10000ms 범위)
        let request_delay_ms = user_config.request_delay_ms
            .max(100)   // 최소 100ms
            .min(10000); // 최대 10초
            
        // 재시도 횟수 검증 (1-10 범위)
        let max_retries = workers_config.max_retries
            .max(1)   // 최소 1회
            .min(10); // 최대 10회
            
        // 타임아웃 검증 (10-120초 범위)
        let request_timeout_seconds = workers_config.request_timeout_seconds
            .max(10)  // 최소 10초
            .min(120); // 최대 2분
            
        Self {
            max_concurrent,
            page_range_limit,
            batch_size,
            semaphore_permits: max_concurrent as usize,
            request_delay_ms,
            max_retries,
            request_timeout_seconds,
        }
    }
    
    /// 설정값 로그 출력 (디버깅용)
    pub fn log_config(&self) {
        log::info!("🔧 ValidatedCrawlingConfig applied:");
        log::info!("   max_concurrent: {} (semaphore_permits: {})", 
                   self.max_concurrent, self.semaphore_permits);
        log::info!("   page_range_limit: {}", self.page_range_limit);
        log::info!("   batch_size: {}", self.batch_size);
        log::info!("   request_delay_ms: {}", self.request_delay_ms);
        log::info!("   max_retries: {}", self.max_retries);
        log::info!("   request_timeout_seconds: {}", self.request_timeout_seconds);
    }
    
    /// 동시성 제어용 세마포어 크기 반환
    pub fn get_semaphore_permits(&self) -> usize {
        self.semaphore_permits
    }
    
    /// 요청 지연 Duration 반환
    pub fn get_request_delay(&self) -> Duration {
        Duration::from_millis(self.request_delay_ms)
    }
    
    /// 요청 타임아웃 Duration 반환
    pub fn get_request_timeout(&self) -> Duration {
        Duration::from_secs(self.request_timeout_seconds as u64)
    }
}

impl Default for ValidatedCrawlingConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 3,
            page_range_limit: 10,
            batch_size: 50,
            semaphore_permits: 3,
            request_delay_ms: 1000,
            max_retries: 3,
            request_timeout_seconds: 30,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::config::defaults;

    #[test]
    fn test_config_validation_ranges() {
        let mut config = AppConfig::default();
        
        // 극단적인 값들 설정
        config.user.crawling.workers.list_page_max_concurrent = 0; // 너무 작음
        config.user.crawling.page_range_limit = 1000; // 너무 큼
        config.user.request_delay_ms = 50; // 너무 작음
        
        let validated = ValidatedCrawlingConfig::from_user_config(&config);
        
        // 검증된 범위 확인
        assert_eq!(validated.max_concurrent, 1); // 최소값으로 보정
        assert_eq!(validated.page_range_limit, 500); // 최대값으로 보정
        assert_eq!(validated.request_delay_ms, 100); // 최소값으로 보정
        assert_eq!(validated.semaphore_permits, 1); // max_concurrent와 동일
    }
    
    #[test]
    fn test_config_normal_values() {
        let mut config = AppConfig::default();
        
        // 정상적인 값들 설정
        config.user.crawling.workers.list_page_max_concurrent = 24;
        config.user.crawling.page_range_limit = 20;
        config.user.request_delay_ms = 500;
        
        let validated = ValidatedCrawlingConfig::from_user_config(&config);
        
        // 설정값이 그대로 유지되는지 확인
        assert_eq!(validated.max_concurrent, 24);
        assert_eq!(validated.page_range_limit, 20);
        assert_eq!(validated.request_delay_ms, 500);
        assert_eq!(validated.semaphore_permits, 24);
    }
}
