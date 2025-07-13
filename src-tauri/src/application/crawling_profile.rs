use serde::{Deserialize, Serialize};

/// 크롤링 프로필 - 크롤링 모드와 설정을 담는 구조체
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CrawlingProfile {
    /// 크롤링 모드: "intelligent", "manual", "verification"
    pub mode: String,
    /// 수동 모드에서 사용할 페이지 범위 (start_page, end_page)
    pub override_range: Option<(u32, u32)>,
    /// 설정 제한값을 무시할지 여부 (intelligent 모드 전용)
    pub override_config_limit: Option<bool>,
    /// 검증 모드에서 확인할 특정 페이지들
    pub verification_pages: Option<Vec<u32>>,
    /// 크롤링 속도 조절 (밀리초 단위 딜레이)
    pub crawling_delay_ms: Option<u64>,
}

impl CrawlingProfile {
    /// 지능형 크롤링 프로필 생성
    pub fn intelligent() -> Self {
        Self {
            mode: "intelligent".to_string(),
            override_range: None,
            override_config_limit: Some(true),
            verification_pages: None,
            crawling_delay_ms: None,
        }
    }
    
    /// 수동 크롤링 프로필 생성
    pub fn manual(start_page: u32, end_page: u32) -> Self {
        Self {
            mode: "manual".to_string(),
            override_range: Some((start_page, end_page)),
            override_config_limit: None,
            verification_pages: None,
            crawling_delay_ms: None,
        }
    }
    
    /// 검증 크롤링 프로필 생성
    pub fn verification(pages: Vec<u32>) -> Self {
        Self {
            mode: "verification".to_string(),
            override_range: None,
            override_config_limit: None,
            verification_pages: Some(pages),
            crawling_delay_ms: None,
        }
    }
    
    /// 프로필 유효성 검증
    pub fn validate(&self) -> Result<(), String> {
        match self.mode.as_str() {
            "intelligent" => {
                if self.override_range.is_some() {
                    return Err("Intelligent mode should not have override_range".to_string());
                }
                Ok(())
            }
            "manual" => {
                if let Some((start, end)) = self.override_range {
                    if start == 0 || end == 0 {
                        return Err("Manual mode requires valid page numbers (> 0)".to_string());
                    }
                    Ok(())
                } else {
                    Err("Manual mode requires override_range".to_string())
                }
            }
            "verification" => {
                if let Some(ref pages) = self.verification_pages {
                    if pages.is_empty() {
                        return Err("Verification mode requires at least one page".to_string());
                    }
                    if pages.iter().any(|&p| p == 0) {
                        return Err("Verification pages must be valid (> 0)".to_string());
                    }
                    Ok(())
                } else {
                    Err("Verification mode requires verification_pages".to_string())
                }
            }
            _ => Err(format!("Unknown crawling mode: {}", self.mode))
        }
    }
    
    /// 프로필에서 페이지 범위 추출 (intelligent 모드에서는 None 반환)
    pub fn get_page_range(&self) -> Option<(u32, u32)> {
        match self.mode.as_str() {
            "manual" => self.override_range,
            "verification" => {
                if let Some(ref pages) = self.verification_pages {
                    let min_page = *pages.iter().min()?;
                    let max_page = *pages.iter().max()?;
                    Some((min_page, max_page))
                } else {
                    None
                }
            }
            _ => None
        }
    }
    
    /// 설정 제한을 무시할지 여부 확인
    pub fn should_override_config_limit(&self) -> bool {
        self.override_config_limit.unwrap_or(false)
    }
}

/// 크롤링 요청 구조체 - 프로필과 추가 메타데이터
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CrawlingRequest {
    /// 크롤링 프로필
    pub profile: CrawlingProfile,
    /// 요청 ID (추적용)
    pub request_id: Option<String>,
    /// 우선순위 (1-10, 높을수록 우선)
    pub priority: Option<u8>,
    /// 재시도 횟수
    pub max_retries: Option<u32>,
    /// 타임아웃 (초 단위)
    pub timeout_seconds: Option<u64>,
}

impl CrawlingRequest {
    /// 새 크롤링 요청 생성
    pub fn new(profile: CrawlingProfile) -> Self {
        Self {
            profile,
            request_id: None,
            priority: Some(5), // 기본 우선순위
            max_retries: Some(3), // 기본 재시도 횟수
            timeout_seconds: Some(300), // 기본 타임아웃 5분
        }
    }
    
    /// 요청 ID 설정
    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }
    
    /// 우선순위 설정
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = Some(priority.min(10).max(1));
        self
    }
    
    /// 재시도 횟수 설정
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = Some(max_retries);
        self
    }
    
    /// 타임아웃 설정
    pub fn with_timeout(mut self, timeout_seconds: u64) -> Self {
        self.timeout_seconds = Some(timeout_seconds);
        self
    }
    
    /// 요청 유효성 검증
    pub fn validate(&self) -> Result<(), String> {
        self.profile.validate()?;
        
        if let Some(priority) = self.priority {
            if priority < 1 || priority > 10 {
                return Err("Priority must be between 1 and 10".to_string());
            }
        }
        
        if let Some(timeout) = self.timeout_seconds {
            if timeout == 0 {
                return Err("Timeout must be greater than 0".to_string());
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_crawling_profile_intelligent() {
        let profile = CrawlingProfile::intelligent();
        assert_eq!(profile.mode, "intelligent");
        assert!(profile.override_range.is_none());
        assert_eq!(profile.override_config_limit, Some(true));
        assert!(profile.validate().is_ok());
    }
    
    #[test]
    fn test_crawling_profile_manual() {
        let profile = CrawlingProfile::manual(1, 100);
        assert_eq!(profile.mode, "manual");
        assert_eq!(profile.override_range, Some((1, 100)));
        assert!(profile.validate().is_ok());
        
        // 유효하지 않은 페이지 번호 테스트
        let invalid_profile = CrawlingProfile::manual(0, 100);
        assert!(invalid_profile.validate().is_err());
    }
    
    #[test]
    fn test_crawling_profile_verification() {
        let profile = CrawlingProfile::verification(vec![1, 5, 10]);
        assert_eq!(profile.mode, "verification");
        assert_eq!(profile.verification_pages, Some(vec![1, 5, 10]));
        assert!(profile.validate().is_ok());
        
        // 빈 페이지 리스트 테스트
        let invalid_profile = CrawlingProfile::verification(vec![]);
        assert!(invalid_profile.validate().is_err());
    }
    
    #[test]
    fn test_crawling_request() {
        let profile = CrawlingProfile::intelligent();
        let request = CrawlingRequest::new(profile)
            .with_request_id("test-123".to_string())
            .with_priority(8)
            .with_max_retries(5);
            
        assert_eq!(request.request_id, Some("test-123".to_string()));
        assert_eq!(request.priority, Some(8));
        assert_eq!(request.max_retries, Some(5));
        assert!(request.validate().is_ok());
    }
}
