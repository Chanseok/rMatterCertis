//! 크롤링 서비스 레이어 트레이트 정의
//! 
//! 이 모듈은 BatchCrawlingEngine의 각 단계를 담당하는 서비스들의 
//! 인터페이스를 정의합니다.

use async_trait::async_trait;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::domain::product::Product;

/// 사이트 상태 체크 서비스
#[async_trait]
pub trait StatusChecker: Send + Sync {
    /// 사이트 상태 확인
    async fn check_site_status(&self) -> Result<SiteStatus>;
    
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
    /// 모든 페이지에서 제품 URL 수집
    async fn collect_all_pages(&self, total_pages: u32) -> Result<Vec<String>>;
    
    /// 단일 페이지에서 제품 URL 수집
    async fn collect_single_page(&self, page: u32) -> Result<Vec<String>>;
    
    /// 배치별 페이지 수집
    async fn collect_page_batch(&self, pages: &[u32]) -> Result<Vec<String>>;
}

/// 제품 상세정보 수집 서비스
#[async_trait]
pub trait ProductDetailCollector: Send + Sync {
    /// 여러 제품의 상세정보 수집
    async fn collect_details(&self, urls: &[String]) -> Result<Vec<Product>>;
    
    /// 단일 제품 상세정보 수집
    async fn collect_single_product(&self, url: &str) -> Result<Product>;
    
    /// 배치별 제품 수집
    async fn collect_product_batch(&self, urls: &[String]) -> Result<Vec<Product>>;
}

/// 사이트 상태 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteStatus {
    pub is_accessible: bool,
    pub response_time_ms: u64,
    pub total_pages: u32,
    pub estimated_products: u32,
    pub last_check_time: chrono::DateTime<chrono::Utc>,
    pub health_score: f64, // 0.0 ~ 1.0
}

/// 데이터베이스 분석 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseAnalysis {
    pub total_products: u32,
    pub unique_products: u32,
    pub duplicate_count: u32,
    pub last_update: Option<chrono::DateTime<chrono::Utc>>,
    pub missing_fields_analysis: FieldAnalysis,
    pub data_quality_score: f64, // 0.0 ~ 1.0
}

/// 필드별 분석 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldAnalysis {
    pub missing_company: u32,
    pub missing_model: u32,
    pub missing_matter_version: u32,
    pub missing_connectivity: u32,
    pub missing_certification_date: u32,
}

/// 중복 데이터 분석
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateAnalysis {
    pub total_duplicates: u32,
    pub duplicate_groups: Vec<DuplicateGroup>,
    pub duplicate_percentage: f64,
}

/// 중복 그룹 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub product_ids: Vec<i32>,
    pub duplicate_type: DuplicateType,
    pub confidence: f64,
}

/// 중복 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DuplicateType {
    ExactMatch,
    SimilarModel,
    SameCompanyModel,
    SimilarCertificationId,
}

/// 처리 전략
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingStrategy {
    pub recommended_batch_size: u32,
    pub recommended_concurrency: u32,
    pub should_skip_duplicates: bool,
    pub should_update_existing: bool,
    pub priority_urls: Vec<String>,
}
