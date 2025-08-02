use std::sync::Arc;
use anyhow::{Result, anyhow};
use tracing::{info, warn};

use crate::domain::services::{
    StatusChecker, DatabaseAnalyzer, ProcessingStrategy
};
use crate::domain::services::crawling_services::{SiteStatus, DatabaseAnalysis, CrawlingRangeRecommendation};
use crate::infrastructure::config::AppConfig;
use crate::infrastructure::crawling_service_impls::CrawlingRangeCalculator;
use crate::infrastructure::IntegratedProductRepository;

/// 크롤링 계획 수립 서비스
///
/// 여러 서비스(StatusChecker, DatabaseAnalyzer)의 분석 결과를 종합하여
/// 최적의 크롤링 전략과 처리 방식을 결정하는 역할을 담당합니다.
/// 
/// **핵심 원칙**: ServiceBasedBatchCrawlingEngine의 검증된 크롤링 범위 계산 로직을 재사용
pub struct CrawlingPlanner {
    status_checker: Arc<dyn StatusChecker>,
    database_analyzer: Arc<dyn DatabaseAnalyzer>,
    config: Arc<AppConfig>,
    range_calculator: CrawlingRangeCalculator,
}

impl CrawlingPlanner {
    /// 새로운 CrawlingPlanner 인스턴스를 생성합니다.
    /// 
    /// **중요**: ServiceBasedBatchCrawlingEngine의 검증된 CrawlingRangeCalculator를 통합
    pub fn new(
        status_checker: Arc<dyn StatusChecker>,
        database_analyzer: Arc<dyn DatabaseAnalyzer>,
        product_repo: Arc<IntegratedProductRepository>,
        config: Arc<AppConfig>,
    ) -> Self {
        let range_calculator = CrawlingRangeCalculator::new(
            product_repo,
            (*config).clone(),
        );

        Self {
            status_checker,
            database_analyzer,
            config,
            range_calculator,
        }
    }

    /// 사이트 분석 결과를 기반으로 크롤링 계획을 수립합니다.
    /// 
    /// 반환값:
    /// - SiteStatus: 사이트 기본 정보
    /// - DatabaseAnalysis: DB 분석 결과  
    /// - ProcessingStrategy: 처리 전략
    pub async fn create_crawling_plan(&self) -> Result<(SiteStatus, DatabaseAnalysis, ProcessingStrategy)> {
        info!("🎯 [CrawlingPlanner] Creating comprehensive crawling plan...");

        // 1. 기본 분석
        let site_status = self.status_checker.check_site_status().await?;
        let db_analysis = self.database_analyzer.analyze_current_state().await?;

        info!("📊 [CrawlingPlanner] Site analysis: {} total pages, {} products in DB", 
              site_status.total_pages, db_analysis.total_products);

        // 2. 크롤링 전략 결정
        let (crawling_recommendation, processing_strategy) = self.determine_crawling_strategy(
            &site_status,
            &db_analysis,
        ).await?;

        info!("✅ [CrawlingPlanner] Plan created successfully");

        Ok((site_status, db_analysis, processing_strategy))
    }

    /// 사이트 상태와 DB 분석 결과를 바탕으로 최적의 크롤링 전략을 결정합니다.
    /// 
    /// **핵심**: ServiceBasedBatchCrawlingEngine의 검증된 범위 계산 로직을 재사용
    /// 
    /// 반환값:
    /// (CrawlingRangeRecommendation, ProcessingStrategy) 튜플
    async fn determine_crawling_strategy(
        &self,
        site_status: &SiteStatus,
        db_analysis: &DatabaseAnalysis,
    ) -> Result<(CrawlingRangeRecommendation, ProcessingStrategy)> {
        info!("🧮 [CrawlingPlanner] Using ServiceBasedBatchCrawlingEngine's proven range calculation logic...");

        // 1. ServiceBasedBatchCrawlingEngine의 검증된 범위 계산 로직 사용
        let optimal_range = self.range_calculator.calculate_next_crawling_range(
            site_status.total_pages,
            site_status.products_on_last_page,
        ).await?;

        // 2. 범위 추천 결과를 CrawlingRangeRecommendation 형식으로 변환
        let range_recommendation = match optimal_range {
            Some((start_page, end_page)) => {
                info!("🎯 [PROPER ACTOR] CrawlingPlanner range: {} to {} (reverse crawling)", 
                      start_page, end_page);
                
                // 전체 페이지 크롤링인지 확인
                if start_page == site_status.total_pages && end_page == 1 {
                    CrawlingRangeRecommendation::Full
                } else {
                    // 역순 크롤링이므로 실제 페이지 수는 start_page - end_page + 1
                    let pages_to_crawl = start_page - end_page + 1;
                    CrawlingRangeRecommendation::Partial(pages_to_crawl)
                }
            },
            None => {
                warn!("⚠️ [CrawlingPlanner] No optimal range calculated, using fallback");
                CrawlingRangeRecommendation::None
            }
        };

        // 3. ProcessingStrategy 결정
        let processing_strategy = self.determine_processing_strategy_from_config(
            site_status,
            db_analysis,
        ).await?;

        // 4. 범위 정보 로깅 (range_recommendation에 따라 다르게 표시)
        match &range_recommendation {
            CrawlingRangeRecommendation::Full => {
                info!(
                    "📋 [CrawlingPlanner] Strategy determined: FULL crawling (1→{}), batch_size: {}, concurrency: {}",
                    site_status.total_pages,
                    processing_strategy.recommended_batch_size,
                    processing_strategy.recommended_concurrency
                );
            },
            CrawlingRangeRecommendation::Partial(pages) => {
                if let Some((start_page, end_page)) = optimal_range {
                    info!(
                        "📋 [CrawlingPlanner] Strategy determined: PARTIAL crawling ({}→{}, {} pages), batch_size: {}, concurrency: {}",
                        start_page, end_page, pages,
                        processing_strategy.recommended_batch_size,
                        processing_strategy.recommended_concurrency
                    );
                }
            },
            CrawlingRangeRecommendation::None => {
                info!(
                    "📋 [CrawlingPlanner] Strategy determined: NO crawling needed, batch_size: {}, concurrency: {}",
                    processing_strategy.recommended_batch_size,
                    processing_strategy.recommended_concurrency
                );
            }
        }

        Ok((range_recommendation, processing_strategy))
    }

    /// 사이트 상태를 기반으로 실제 크롤링 범위를 계산합니다.
    /// CrawlingRangeCalculator를 직접 사용하여 정확한 범위를 반환합니다.
    pub async fn calculate_actual_crawling_range(&self, site_status: &SiteStatus) -> Result<Option<(u32, u32)>> {
        self.range_calculator.calculate_next_crawling_range(
            site_status.total_pages,
            site_status.products_on_last_page,
        ).await
    }

    /// 설정 파일 기반으로 최적의 처리 전략을 결정합니다.
    /// 
    /// **핵심**: 모든 값을 설정에서 읽되, 현재 상황에 맞게 지능형 조정
    async fn determine_processing_strategy_from_config(
        &self,
        _site_status: &SiteStatus,
        _db_analysis: &DatabaseAnalysis,
    ) -> Result<ProcessingStrategy> {
        info!("⚙️ [CrawlingPlanner] Using user configuration values directly...");
        
        // ✅ 사용자 설정값을 그대로 사용 (임의 변경 금지)
        let batch_size = self.config.user.batch.batch_size;
        let concurrency = self.config.user.max_concurrent_requests;
        
        info!("📊 [CrawlingPlanner] Using user settings: batch_size={}, concurrency={}", 
              batch_size, concurrency);
        
        // 크롤링 전략 설정 (중복 처리는 DB 레벨에서 자동 처리됨)
        let should_skip_duplicates = false; // URL이 Primary Key이므로 DB가 자동으로 중복 처리
        
        Ok(ProcessingStrategy {
            recommended_batch_size: batch_size,
            recommended_concurrency: concurrency,
            should_skip_duplicates,
            should_update_existing: true, // 기존 제품 정보 업데이트 허용
            priority_urls: vec![], // 향후 우선순위 URL 기능 구현 시 사용
        })
    }
}
