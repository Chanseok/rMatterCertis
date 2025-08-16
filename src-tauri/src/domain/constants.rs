//! 사이트 특성 및 도메인 상수들
//!
//! matters.town 사이트의 고유한 특성들과 비즈니스 도메인 상수들을 정의합니다.
//! Modern Rust 2024 스타일로 구성되어 있습니다.

/// matters.town 사이트 특성 상수들
pub mod site {
    /// 사이트의 기본 페이지당 제품 수 (마지막 페이지 제외)
    ///
    /// 이 값은 matters.town 사이트의 고유한 특성으로,
    /// 모든 페이지(마지막 페이지 제외)에는 정확히 12개의 제품이 있습니다.
    pub const PRODUCTS_PER_PAGE: i32 = 12;

    /// 사이트 기본 URL
    pub const BASE_URL: &str = "https://matters.town";

    /// 제품 목록 페이지 URL 패턴 (페이지 번호 placeholder: {})
    pub const LIST_PAGE_URL_PATTERN: &str = "https://matters.town/products?page={}";

    /// 제품 상세 페이지 URL 패턴 (제품 ID placeholder: {})
    pub const DETAIL_PAGE_URL_PATTERN: &str = "https://matters.town/product/{}";

    /// 사이트 페이지 번호는 1-based 인덱싱 사용
    pub const PAGE_NUMBERING_BASE: u32 = 1;
}

/// 크롤링 관련 기본 제한값들
pub mod crawling {

    /// 기본 요청 간 딜레이 (밀리초)
    pub const DEFAULT_REQUEST_DELAY_MS: u64 = 1000;

    /// 기본 요청 타임아웃 (밀리초)  
    pub const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 30000;

    /// 기본 동시 요청 수
    pub const DEFAULT_MAX_CONCURRENT_REQUESTS: u32 = 3;

    /// 기본 페이지 범위 제한
    pub const DEFAULT_PAGE_RANGE_LIMIT: u32 = 50;

    /// 기본 배치 크기
    pub const DEFAULT_BATCH_SIZE: u32 = 10;

    /// 기본 재시도 횟수
    pub const DEFAULT_MAX_RETRY_ATTEMPTS: u32 = 3;

    /// TTL 관련 기본값들
    pub mod ttl {

        /// 사이트 분석 결과 TTL (분)
        pub const SITE_ANALYSIS_TTL_MINUTES: u64 = 5;

        /// DB 분석 결과 TTL (분)
        pub const DB_ANALYSIS_TTL_MINUTES: u64 = 3;

        /// 계산된 범위 TTL (분)
        pub const CALCULATED_RANGE_TTL_MINUTES: u64 = 2;

        /// 설정 캐시 TTL (분)
        pub const CONFIG_CACHE_TTL_MINUTES: u64 = 10;
    }
}

/// 데이터베이스 관련 상수들
pub mod database {
    /// 기본 연결 풀 크기
    pub const DEFAULT_CONNECTION_POOL_SIZE: u32 = 10;

    /// 기본 쿼리 타임아웃 (초)
    pub const DEFAULT_QUERY_TIMEOUT_SECONDS: u64 = 30;

    /// 배치 삽입 크기
    pub const DEFAULT_BATCH_INSERT_SIZE: usize = 100;

    /// pageId는 0-based 인덱싱 사용 (DB 내부)
    pub const PAGE_ID_INDEXING_BASE: i32 = 0;

    /// indexInPage는 0-based 인덱싱 사용 (DB 내부)
    pub const INDEX_IN_PAGE_BASE: i32 = 0;
}

/// 유효성 검증 관련 상수들
pub mod validation {
    /// 최소 동시 요청 수
    pub const MIN_CONCURRENT_REQUESTS: u32 = 1;

    /// 최대 동시 요청 수
    pub const MAX_CONCURRENT_REQUESTS: u32 = 50;

    /// 최소 페이지 범위
    pub const MIN_PAGE_RANGE: u32 = 1;

    /// 최대 페이지 범위
    pub const MAX_PAGE_RANGE: u32 = 500;

    /// 최소 배치 크기
    pub const MIN_BATCH_SIZE: u32 = 1;

    /// 최대 배치 크기
    pub const MAX_BATCH_SIZE: u32 = 1000;

    /// 최소 요청 딜레이 (밀리초)
    pub const MIN_REQUEST_DELAY_MS: u64 = 100;

    /// 최대 요청 딜레이 (밀리초)
    pub const MAX_REQUEST_DELAY_MS: u64 = 10000;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_site_constants() {
        assert_eq!(site::PRODUCTS_PER_PAGE, 12);
        assert_eq!(site::PAGE_NUMBERING_BASE, 1);
        assert!(site::BASE_URL.starts_with("https://"));
    }

    #[test]
    fn test_validation_ranges() {
        assert!(validation::MIN_CONCURRENT_REQUESTS <= validation::MAX_CONCURRENT_REQUESTS);
        assert!(validation::MIN_PAGE_RANGE <= validation::MAX_PAGE_RANGE);
        assert!(validation::MIN_BATCH_SIZE <= validation::MAX_BATCH_SIZE);
        assert!(validation::MIN_REQUEST_DELAY_MS <= validation::MAX_REQUEST_DELAY_MS);
    }

    #[test]
    fn test_ttl_values() {
        use crawling::ttl::*;

        // TTL 값들이 합리적인 범위에 있는지 확인
        assert!(SITE_ANALYSIS_TTL_MINUTES >= 1 && SITE_ANALYSIS_TTL_MINUTES <= 60);
        assert!(DB_ANALYSIS_TTL_MINUTES >= 1 && DB_ANALYSIS_TTL_MINUTES <= 30);
        assert!(CALCULATED_RANGE_TTL_MINUTES >= 1 && CALCULATED_RANGE_TTL_MINUTES <= 10);
    }
}
