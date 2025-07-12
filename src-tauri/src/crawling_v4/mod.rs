//! # Modern Crawling Module v4.0
//!
//! Clean Architecture 기반 크롤링 도메인 모듈
//! - 의존성 역전 원칙 준수
//! - 테스트 가능한 구조
//! - Modern Rust 2024 패턴 적용

pub mod domain;
pub mod application;
pub mod infrastructure;

// Re-export core types
pub use domain::*;
pub use application::*;

/// 크롤링 도메인의 핵심 추상화
pub trait CrawlingUseCase {
    type Error;
    
    /// 크롤링 시작
    async fn start_crawling(&self, request: StartCrawlingRequest) -> Result<CrawlingSession, Self::Error>;
    
    /// 크롤링 중지
    async fn stop_crawling(&self, session_id: SessionId) -> Result<(), Self::Error>;
    
    /// 크롤링 상태 조회
    async fn get_crawling_stats(&self, session_id: SessionId) -> Result<CrawlingStats, Self::Error>;
}

/// 크롤링 리포지토리 추상화
pub trait CrawlingRepository {
    type Error;
    
    /// 크롤링 세션 저장
    async fn save_session(&self, session: &CrawlingSession) -> Result<(), Self::Error>;
    
    /// 크롤링 결과 저장
    async fn save_products(&self, products: &[ProductData]) -> Result<(), Self::Error>;
    
    /// 크롤링 통계 조회
    async fn get_stats(&self, session_id: SessionId) -> Result<CrawlingStats, Self::Error>;
}

/// 웹 크롤러 추상화
pub trait WebCrawler {
    type Error;
    
    /// 페이지 가져오기
    async fn fetch_page(&self, url: &str) -> Result<String, Self::Error>;
    
    /// 페이지 파싱
    async fn parse_page(&self, html: &str) -> Result<Vec<ProductData>, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_crawling_abstractions() {
        // 추상화 인터페이스 테스트
        // Mock 구현체로 단위 테스트 가능
    }
}
