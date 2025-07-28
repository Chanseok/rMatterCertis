use std::sync::Arc;
use anyhow::{Result, anyhow};
use tracing::{info, warn};

use crate::domain::services::{
    StatusChecker, DatabaseAnalyzer, SiteStatus, DatabaseAnalysis, ProcessingStrategy
};
use crate::domain::services::crawling_services::CrawlingRangeRecommendation;
use crate::new_architecture::config::SystemConfig;

/// 크롤링 계획 수립 서비스
///
/// 여러 서비스(StatusChecker, DatabaseAnalyzer)의 분석 결과를 종합하여
/// 최적의 크롤링 실행 계획(어떤 페이지를, 어떤 방식으로)을 수립합니다.
/// 
/// **설계 원칙**: 모든 하드코딩 값을 제거하고 설정 파일 기반으로 지능형 최적화
pub struct CrawlingPlanner {
    status_checker: Arc<dyn StatusChecker>,
    db_analyzer: Arc<dyn DatabaseAnalyzer>,
    config: Arc<SystemConfig>,
}

impl CrawlingPlanner {
    /// CrawlingPlanner를 생성합니다.
    /// 
    /// # Arguments
    /// * `status_checker` - 사이트 상태 분석 서비스
    /// * `db_analyzer` - 데이터베이스 분석 서비스  
    /// * `config` - 시스템 설정 (모든 하드코딩 값을 대체)
    pub fn new(
        status_checker: Arc<dyn StatusChecker>,
        db_analyzer: Arc<dyn DatabaseAnalyzer>,
        config: Arc<SystemConfig>,
    ) -> Self {
        info!("🏗️ [CrawlingPlanner] Initializing with config-based intelligent system");
        Self {
            status_checker,
            db_analyzer,
            config,
        }
    }

    /// 시스템의 현재 상태를 종합적으로 분석합니다.
    ///
    /// # Arguments
    /// * `cached_site_status` - 이미 확인된 사이트 상태 (중복 호출 방지)
    ///
    /// # Returns
    /// (SiteStatus, DatabaseAnalysis) 튜플
    pub async fn analyze_system_state_with_cache(&self, cached_site_status: SiteStatus) -> Result<(SiteStatus, DatabaseAnalysis)> {
        info!("🧠 [CrawlingPlanner] Starting intelligent system state analysis with cached site status...");

        // 1. 캐시된 사이트 상태 사용 (중복 호출 방지)
        info!("✅ [CrawlingPlanner] Using cached site status: {} pages found", cached_site_status.total_pages);

        // 2. 데이터베이스 상태 분석
        let db_analysis = self.db_analyzer.analyze_current_state().await?;
        info!("✅ [CrawlingPlanner] Database analysis complete: {} products in DB", db_analysis.total_products);

        Ok((cached_site_status, db_analysis))
    }

    /// 시스템의 현재 상태를 종합적으로 분석합니다. (레거시 호환용)
    ///
    /// # Returns
    /// (SiteStatus, DatabaseAnalysis) 튜플
    pub async fn analyze_system_state(&self) -> Result<(SiteStatus, DatabaseAnalysis)> {
        warn!("⚠️ [CrawlingPlanner] Using legacy analyze_system_state - consider using analyze_system_state_with_cache for better performance");

        // 1. 사이트 상태 분석
        let site_status = self.status_checker.check_site_status().await?;
        info!("✅ [CrawlingPlanner] Site status analysis complete: {} pages found", site_status.total_pages);

        // 2. 데이터베이스 상태 분석
        let db_analysis = self.db_analyzer.analyze_current_state().await?;
        info!("✅ [CrawlingPlanner] Database analysis complete: {} products in DB", db_analysis.total_products);

        Ok((site_status, db_analysis))
    }

    /// 분석 결과를 바탕으로 크롤링 전략을 결정합니다.
    /// 
    /// **설계 원칙**: 하드코딩 값 완전 제거, 설정 파일 기반 지능형 최적화
    ///
    /// # Arguments
    /// * `site_status` - 사이트 현재 상태
    /// * `db_analysis` - 데이터베이스 분석 결과
    ///
    /// # Returns
    /// (CrawlingRangeRecommendation, ProcessingStrategy) 튜플
    pub async fn determine_crawling_strategy(
        &self,
        site_status: &SiteStatus,
        db_analysis: &DatabaseAnalysis,
    ) -> Result<(CrawlingRangeRecommendation, ProcessingStrategy)> {
        info!("🎯 [CrawlingPlanner] Determining config-based intelligent crawling strategy...");

        // 1. 크롤링 범위 추천 가져오기 (지능형 분석)
        let range_recommendation = self.status_checker
            .calculate_crawling_range_recommendation(site_status, db_analysis)
            .await?;

        // 2. 설정 기반 처리 전략 결정 (하드코딩 값 완전 제거)
        let processing_strategy = self.determine_processing_strategy_from_config(
            site_status,
            db_analysis,
        ).await?;

        info!(
            "📋 [CrawlingPlanner] Strategy determined: {:?}, batch_size: {}, concurrency: {}",
            range_recommendation, 
            processing_strategy.recommended_batch_size,
            processing_strategy.recommended_concurrency
        );

        Ok((range_recommendation, processing_strategy))
    }

    /// 설정 파일 기반으로 최적의 처리 전략을 결정합니다.
    /// 
    /// **핵심**: 모든 값을 설정에서 읽되, 현재 상황에 맞게 지능형 조정
    async fn determine_processing_strategy_from_config(
        &self,
        site_status: &SiteStatus,
        db_analysis: &DatabaseAnalysis,
    ) -> Result<ProcessingStrategy> {
        info!("⚙️ [CrawlingPlanner] Determining processing strategy from config...");
        
        // 설정에서 기본값 읽기
        let base_batch_size = self.config.performance.batch_sizes.initial_size;
        let base_concurrency = self.config.performance.concurrency.max_concurrent_batches;
        
        info!("📊 [CrawlingPlanner] Base config values: batch_size={}, concurrency={}", 
              base_batch_size, base_concurrency);
        
        // 현재 상황에 맞는 지능형 조정
        let adjusted_batch_size = self.calculate_optimal_batch_size(
            base_batch_size,
            db_analysis.total_products.into(),
            site_status.total_pages,
        );
        
        let adjusted_concurrency = self.calculate_optimal_concurrency(
            base_concurrency,
            site_status.total_pages,
            db_analysis.total_products.into(),
        );
        
        info!("🎯 [CrawlingPlanner] Intelligent adjustments: batch_size: {} -> {}, concurrency: {} -> {}", 
              base_batch_size, adjusted_batch_size, base_concurrency, adjusted_concurrency);
        
        // 중복 처리 전략 (설정 + 분석 기반)
        let should_skip_duplicates = self.should_skip_duplicates_based_on_config(db_analysis);
        
        Ok(ProcessingStrategy {
            recommended_batch_size: adjusted_batch_size,
            recommended_concurrency: adjusted_concurrency,
            should_skip_duplicates,
            should_update_existing: self.config.system.update_existing_items,
            priority_urls: vec![], // 향후 설정에서 우선순위 URL 목록 읽기
        })
    }

    /// 설정 기반 최적 배치 크기 계산
    fn calculate_optimal_batch_size(
        &self,
        base_batch_size: u32,
        total_products_in_db: u64,
        total_pages: u32,
    ) -> u32 {
        // 설정에서 최소/최대값 읽기
        let min_batch = self.config.performance.batch_sizes.min_size;
        let max_batch = self.config.performance.batch_sizes.max_size;
        
        // 지능형 조정: DB 크기와 페이지 수를 고려한 최적화
        let adjusted_size = if total_products_in_db < 1000 {
            // 작은 DB: 더 큰 배치로 효율성 증대
            (base_batch_size as f32 * self.config.performance.batch_sizes.small_db_multiplier) as u32
        } else if total_pages > 100 {
            // 많은 페이지: 작은 배치로 안정성 확보
            (base_batch_size as f32 * self.config.performance.batch_sizes.large_site_multiplier) as u32
        } else {
            base_batch_size
        };
        
        // 설정된 범위 내로 제한
        adjusted_size.clamp(min_batch, max_batch)
    }

    /// 설정 기반 최적 동시성 계산
    fn calculate_optimal_concurrency(
        &self,
        base_concurrency: u32,
        total_pages: u32,
        total_products_in_db: u64,
    ) -> u32 {
        let min_concurrency = self.config.performance.concurrency.min_concurrent_batches;
        let max_concurrency = self.config.performance.concurrency.max_concurrent_batches;
        
        // 지능형 조정: 사이트 크기와 시스템 상태를 고려
        let adjusted_concurrency = if total_pages > 100 && total_products_in_db < 10000 {
            // 큰 사이트 + 작은 DB: 높은 동시성 가능
            (base_concurrency as f32 * self.config.performance.concurrency.high_load_multiplier) as u32
        } else if total_products_in_db > 50000 {
            // 큰 DB: 동시성 제한으로 안정성 확보
            (base_concurrency as f32 * self.config.performance.concurrency.stable_load_multiplier) as u32
        } else {
            base_concurrency
        };
        
        adjusted_concurrency.clamp(min_concurrency, max_concurrency)
    }

    /// 설정과 분석을 기반으로 중복 스킵 여부 결정
    fn should_skip_duplicates_based_on_config(&self, db_analysis: &DatabaseAnalysis) -> bool {
        // 설정에서 중복 처리 임계값 읽기
        let duplicate_threshold = self.config.performance.deduplication.skip_threshold_percentage;
        
        // 현재 중복률 계산
        let current_duplicate_rate = if db_analysis.total_products > 0 {
            (db_analysis.duplicate_count as f64 / db_analysis.total_products as f64) * 100.0
        } else {
            0.0
        };
        
        info!("🔍 [CrawlingPlanner] Duplicate analysis: rate={:.2}%, threshold={:.2}%, total={}, duplicates={}", 
              current_duplicate_rate, duplicate_threshold, db_analysis.total_products, db_analysis.duplicate_count);
        
        // 설정된 임계값과 비교하여 결정
        let should_skip = current_duplicate_rate > duplicate_threshold;
        info!("🎯 [CrawlingPlanner] Skip duplicates decision: {}", should_skip);
        
        should_skip
    }
}