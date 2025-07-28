//! 크롤링 서비스 레이어 트레이트 정의
//! 
//! 이 모듈은 BatchCrawlingEngine의 각 단계를 담당하는 서비스들의 
//! 인터페이스를 정의합니다.

use async_trait::async_trait;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use std::time::Duration;
use std::any::Any;
use tokio_util::sync::CancellationToken;

use crate::domain::product::ProductDetail;
use crate::domain::product_url::ProductUrl;

/// 사이트 상태 체크 서비스
#[async_trait]
pub trait StatusChecker: Send + Sync {
    /// 사이트 상태 확인
    async fn check_site_status(&self) -> Result<SiteStatus>;
    
    /// 크롤링 범위 추천 계산
    async fn calculate_crawling_range_recommendation(
        &self, 
        site_status: &SiteStatus, 
        db_analysis: &DatabaseAnalysis
    ) -> Result<CrawlingRangeRecommendation>;
    
    /// 크롤링 예상 시간 계산
    async fn estimate_crawling_time(&self, pages: u32) -> Duration;
    
    /// 사이트 접근 가능 여부 확인
    async fn verify_site_accessibility(&self) -> Result<bool>;
}

/// 데이터베이스 분석 서비스
#[async_trait]
pub trait DatabaseAnalyzer: Send + Sync {
    /// 현재 데이터베이스 상태 분석
    async fn analyze_current_state(&self) -> Result<DatabaseAnalysis>;
    
    /// 처리 전략 추천
    async fn recommend_processing_strategy(&self) -> Result<ProcessingStrategy>;
    
    /// 중복 데이터 분석
    async fn analyze_duplicates(&self) -> Result<DuplicateAnalysis>;
}

/// 제품 목록 수집 서비스
#[async_trait]
pub trait ProductListCollector: Send + Sync {
    /// 모든 페이지에서 제품 URL 수집 (메타데이터 포함)
    /// 
    /// # Parameters  
    /// * `total_pages` - 사이트의 총 페이지 수 (사전 계산된 값)
    /// * `products_on_last_page` - 마지막 페이지의 제품 수 (사전 계산된 값)
    async fn collect_all_pages(
        &self, 
        total_pages: u32,
        products_on_last_page: u32
    ) -> Result<Vec<ProductUrl>>;
    
    /// 페이지 범위에서 제품 URL 수집 (start_page부터 end_page까지, 메타데이터 포함)
    /// 
    /// # Parameters
    /// * `start_page` - 시작 페이지 번호
    /// * `end_page` - 종료 페이지 번호
    /// * `total_pages` - 사이트의 총 페이지 수 (사전 계산된 값)
    /// * `products_on_last_page` - 마지막 페이지의 제품 수 (사전 계산된 값)
    async fn collect_page_range(
        &self, 
        start_page: u32, 
        end_page: u32,
        total_pages: u32,
        products_on_last_page: u32
    ) -> Result<Vec<ProductUrl>>;
    
    /// 페이지 범위에서 제품 URL 수집 (취소 토큰 지원, 메타데이터 포함)
    /// 
    /// # Parameters
    /// * `start_page` - 시작 페이지 번호
    /// * `end_page` - 종료 페이지 번호
    /// * `total_pages` - 사이트의 총 페이지 수 (사전 계산된 값)
    /// * `products_on_last_page` - 마지막 페이지의 제품 수 (사전 계산된 값)
    /// * `cancellation_token` - 취소 토큰
    async fn collect_page_range_with_cancellation(
        &self, 
        start_page: u32, 
        end_page: u32,
        total_pages: u32,
        products_on_last_page: u32,
        cancellation_token: CancellationToken
    ) -> Result<Vec<ProductUrl>>;
    
    /// 단일 페이지에서 제품 URL 수집 (메타데이터 포함)
    /// 
    /// # Parameters
    /// * `page` - 페이지 번호
    /// * `total_pages` - 사이트의 총 페이지 수 (사전 계산된 값)
    /// * `products_on_last_page` - 마지막 페이지의 제품 수 (사전 계산된 값)
    async fn collect_single_page(
        &self, 
        page: u32,
        total_pages: u32,
        products_on_last_page: u32
    ) -> Result<Vec<ProductUrl>>;
    
    /// 배치별 페이지 수집 (메타데이터 포함)
    /// 
    /// # Parameters
    /// * `pages` - 수집할 페이지 번호 배열
    /// * `total_pages` - 사이트의 총 페이지 수 (사전 계산된 값)
    /// * `products_on_last_page` - 마지막 페이지의 제품 수 (사전 계산된 값)
    async fn collect_page_batch(
        &self, 
        pages: &[u32],
        total_pages: u32,
        products_on_last_page: u32
    ) -> Result<Vec<ProductUrl>>;
    
    /// Any 트레이트 지원을 위한 다운캐스팅
    fn as_any(&self) -> &dyn Any;
}

/// 제품 상세정보 수집 서비스
#[async_trait]
pub trait ProductDetailCollector: Send + Sync {
    /// 여러 제품의 상세정보 수집 (메타데이터 포함)
    async fn collect_details(&self, product_urls: &[ProductUrl]) -> Result<Vec<ProductDetail>>;
    
    /// 여러 제품의 상세정보 수집 (cancellation token 지원, 메타데이터 포함)
    async fn collect_details_with_cancellation(&self, product_urls: &[ProductUrl], cancellation_token: CancellationToken) -> Result<Vec<ProductDetail>>;
    
    /// 단일 제품 상세정보 수집 (메타데이터 포함)
    async fn collect_single_product(&self, product_url: &ProductUrl) -> Result<ProductDetail>;
    
    /// 배치별 제품 수집 (메타데이터 포함)
    async fn collect_product_batch(&self, product_urls: &[ProductUrl]) -> Result<Vec<ProductDetail>>;
    
    /// Any 타입으로 다운캐스트하기 위한 메서드
    fn as_any(&self) -> &dyn std::any::Any;
}

/// 크롤링 범위 권장 사항
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export)]
pub enum CrawlingRangeRecommendation {
    /// 전체 크롤링 권장
    Full,
    /// 최근 추가된 일부만 크롤링 권장 (페이지 수)
    Partial(u32),
    /// 크롤링 필요 없음
    None,
}

impl CrawlingRangeRecommendation {
    /// 범위 권장사항을 (시작 페이지, 끝 페이지) 튜플로 변환
    /// 전체 크롤링의 경우 total_pages가 필요하므로 파라미터로 받음
    pub fn to_page_range(&self, total_pages: u32) -> Option<(u32, u32)> {
        match self {
            CrawlingRangeRecommendation::Full => Some((1, total_pages)),
            CrawlingRangeRecommendation::Partial(pages) => Some((1, *pages)),
            CrawlingRangeRecommendation::None => None,
        }
    }
}

/// 사이트 상태 정보
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SiteStatus {
    pub is_accessible: bool,
    pub response_time_ms: u64,
    pub total_pages: u32,
    pub estimated_products: u32,
    pub products_on_last_page: u32,
    pub last_check_time: chrono::DateTime<chrono::Utc>,
    pub health_score: f64, // 0.0 ~ 1.0
    pub data_change_status: SiteDataChangeStatus,
    pub decrease_recommendation: Option<DataDecreaseRecommendation>,
    pub crawling_range_recommendation: CrawlingRangeRecommendation,
}

/// 데이터베이스 분석 결과
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DatabaseAnalysis {
    pub total_products: u32,
    pub unique_products: u32,
    pub duplicate_count: u32,
    pub last_update: Option<chrono::DateTime<chrono::Utc>>,
    pub missing_fields_analysis: FieldAnalysis,
    pub data_quality_score: f64, // 0.0 ~ 1.0
}

/// 필드별 분석 정보
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FieldAnalysis {
    pub missing_company: u32,
    pub missing_model: u32,
    pub missing_matter_version: u32,
    pub missing_connectivity: u32,
    pub missing_certification_date: u32,
}

/// 중복 데이터 분석
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DuplicateAnalysis {
    pub total_duplicates: u32,
    pub duplicate_groups: Vec<DuplicateGroup>,
    pub duplicate_percentage: f64,
}

/// 중복 그룹 정보
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DuplicateGroup {
    pub product_ids: Vec<i32>,
    pub duplicate_type: DuplicateType,
    pub confidence: f64,
}

/// 중복 타입
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum DuplicateType {
    ExactMatch,
    SimilarModel,
    SameCompanyModel,
    SimilarCertificationId,
}

/// 처리 전략
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProcessingStrategy {
    pub recommended_batch_size: u32,
    pub recommended_concurrency: u32,
    pub should_skip_duplicates: bool,
    pub should_update_existing: bool,
    pub priority_urls: Vec<String>,
}

/// 사이트 데이터 변화 상태
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export)]
pub enum SiteDataChangeStatus {
    /// 데이터 증가 - 새로운 제품이 추가됨
    Increased { new_count: u32, previous_count: u32 },
    /// 데이터 안정 - 변화 없음
    Stable { count: u32 },
    /// 데이터 감소 - 제품이 삭제됨 (주의 필요)
    Decreased { current_count: u32, previous_count: u32, decrease_amount: u32 },
    /// 초기 상태 - 처음 체크
    Initial { count: u32 },
    /// 사이트 접근 불가
    Inaccessible,
}

/// 데이터 감소 시 권장 조치사항
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DataDecreaseRecommendation {
    /// 권장 조치 유형
    pub action_type: RecommendedAction,
    /// 상세 설명
    pub description: String,
    /// 영향도 수준
    pub severity: SeverityLevel,
    /// 구체적인 단계별 안내
    pub action_steps: Vec<String>,
}

/// 권장 조치 유형
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum RecommendedAction {
    /// 잠시 대기 후 재시도
    WaitAndRetry,
    /// 데이터베이스 백업 후 전체 재크롤링
    BackupAndRecrawl,
    /// 수동 확인 필요
    ManualVerification,
    /// 부분적 재크롤링
    PartialRecrawl,
}

/// 심각도 수준
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum SeverityLevel {
    Low,     // 10% 미만 감소
    Medium,  // 10-30% 감소
    High,    // 30-50% 감소
    Critical, // 50% 이상 감소
}
