//! Advanced Crawling Engine을 위한 TypeScript 연동 타입
//! ts-rs를 사용하여 Rust 타입을 TypeScript로 자동 변환

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Advanced Crawling Engine 설정
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AdvancedCrawlingConfig {
    /// 시작 페이지
    pub start_page: u32,
    /// 종료 페이지
    pub end_page: u32,
    /// 배치 크기
    pub batch_size: u32,
    /// 동시 실행 수
    pub concurrency: u32,
    /// 요청 간 딜레이 (ms)
    pub delay_ms: u64,
    /// 최대 재시도 횟수
    pub retry_max: u32,
}

/// 크롤링 진행 상황
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CrawlingProgressInfo {
    /// 현재 단계 (0-5)
    pub stage: u32,
    /// 단계 이름
    pub stage_name: String,
    /// 진행률 (0-100)
    pub progress_percentage: f64,
    /// 처리된 항목 수
    pub items_processed: u32,
    /// 현재 메시지
    pub current_message: String,
    /// 예상 남은 시간 (초)
    pub estimated_remaining_time: Option<u32>,
    /// 세션 ID
    pub session_id: String,
    /// 타임스탬프
    pub timestamp: DateTime<Utc>,
}

/// 사이트 상태 정보
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SiteStatusInfo {
    /// 사이트 접근 가능 여부
    pub is_accessible: bool,
    /// 전체 페이지 수
    pub total_pages: u32,
    /// 건강 점수 (0.0-1.0)
    pub health_score: f64,
    /// 응답 시간 (ms)
    pub response_time_ms: u64,
    /// 마지막 페이지 제품 수
    pub products_on_last_page: u32,
    /// 예상 총 제품 수
    pub estimated_total_products: u32,
}

/// 제품 정보
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProductInfo {
    /// 제품 ID
    pub id: String,
    /// 제품 URL
    pub url: String,
    /// 제품명
    pub name: String,
    /// 회사명
    pub company: String,
    /// 인증 번호
    pub certification_number: String,
    /// 제품 설명
    pub description: Option<String>,
    /// 생성 시간
    pub created_at: DateTime<Utc>,
    /// 업데이트 시간
    pub updated_at: Option<DateTime<Utc>>,
}

/// 크롤링 세션 정보
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CrawlingSession {
    /// 세션 ID
    pub session_id: String,
    /// 시작 시간
    pub started_at: DateTime<Utc>,
    /// 설정
    pub config: AdvancedCrawlingConfig,
    /// 상태
    pub status: SessionStatus,
    /// 총 처리된 제품 수
    pub total_products_processed: u32,
    /// 성공률
    pub success_rate: f64,
}

/// 세션 상태
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum SessionStatus {
    /// 준비 중
    Preparing,
    /// 실행 중
    Running,
    /// 완료됨
    Completed,
    /// 실패함
    Failed,
    /// 중단됨
    Cancelled,
}

/// 크롤링 시작 요청
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StartCrawlingRequest {
    /// 설정
    pub config: AdvancedCrawlingConfig,
}

/// 크롤링 시작 응답
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StartCrawlingResponse {
    /// 성공 여부
    pub success: bool,
    /// 세션 정보
    pub session: Option<CrawlingSession>,
    /// 에러 메시지
    pub error: Option<String>,
}

/// 페이지네이션된 제품 목록
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProductPage {
    /// 제품 목록
    pub products: Vec<ProductInfo>,
    /// 현재 페이지
    pub current_page: u32,
    /// 페이지당 항목 수
    pub page_size: u32,
    /// 총 항목 수
    pub total_items: u32,
    /// 총 페이지 수
    pub total_pages: u32,
}

/// 데이터베이스 통계
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DatabaseStats {
    /// 총 제품 수
    pub total_products: u32,
    /// 오늘 추가된 제품 수
    pub products_added_today: u32,
    /// 마지막 업데이트 시간
    pub last_updated: Option<DateTime<Utc>>,
    /// 데이터베이스 크기 (bytes)
    pub database_size_bytes: u64,
}

/// 에러 정보
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ErrorInfo {
    /// 에러 코드
    pub code: String,
    /// 에러 메시지
    pub message: String,
    /// 세부 정보
    pub details: Option<String>,
    /// 복구 가능 여부
    pub recoverable: bool,
}

/// API 응답 래퍼
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ApiResponse<T> {
    /// 성공 여부
    pub success: bool,
    /// 데이터
    pub data: Option<T>,
    /// 에러 정보
    pub error: Option<ErrorInfo>,
    /// 타임스탬프
    pub timestamp: DateTime<Utc>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: Utc::now(),
        }
    }

    pub fn error(code: String, message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(ErrorInfo {
                code,
                message,
                details: None,
                recoverable: false,
            }),
            timestamp: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typescript_exports() {
        // TypeScript 타입 파일 생성 테스트
        AdvancedCrawlingConfig::export().unwrap();
        CrawlingProgressInfo::export().unwrap();
        SiteStatusInfo::export().unwrap();
        ProductInfo::export().unwrap();
        CrawlingSession::export().unwrap();
    }
}
