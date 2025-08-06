//! CrawlingPlanner - 지능형 크롤링 계획 수립 시스템
//! 
//! Actor 기반 아키텍처에서 크롤링 전략을 수립하고 
//! 최적화된 실행 계획을 생성하는 모듈입니다.

use std::sync::Arc;
use serde::{Serialize, Deserialize};
use ts_rs::TS;
use tracing::info;

use crate::domain::services::{StatusChecker, DatabaseAnalyzer};
use crate::domain::services::crawling_services::{
    DatabaseAnalysis, ProcessingStrategy, DuplicateAnalysis, 
    FieldAnalysis, CrawlingRangeRecommendation
};
use super::super::{
    SystemConfig,
    actors::types::{CrawlingConfig, BatchConfig, ActorError}
};

/// 지능형 크롤링 계획 수립자
/// 
/// 사이트 상태와 데이터베이스 분석을 기반으로 
/// 최적화된 크롤링 전략을 수립합니다.
pub struct CrawlingPlanner {
    /// 상태 확인기
    status_checker: Arc<dyn StatusChecker>,
    
    /// 데이터베이스 분석기
    database_analyzer: Arc<dyn DatabaseAnalyzer>,
    
    /// 시스템 설정
    config: Arc<SystemConfig>,
}

impl CrawlingPlanner {
    /// 새로운 CrawlingPlanner 인스턴스를 생성합니다.
    /// 
    /// # Arguments
    /// * `status_checker` - 사이트 상태 확인기
    /// * `database_analyzer` - 데이터베이스 분석기
    /// * `config` - 시스템 설정
    #[must_use]
    pub fn new(
        status_checker: Arc<dyn StatusChecker>,
        database_analyzer: Arc<dyn DatabaseAnalyzer>,
        config: Arc<SystemConfig>,
    ) -> Self {
        Self {
            status_checker,
            database_analyzer,
            config,
        }
    }
    
    /// 크롤링 계획을 수립합니다.
    /// 
    /// # Arguments
    /// * `crawling_config` - 기본 크롤링 설정
    /// 
    /// # Returns
    /// * `Ok(CrawlingPlan)` - 수립된 크롤링 계획
    /// * `Err(ActorError)` - 계획 수립 실패
    pub async fn create_crawling_plan(
        &self,
        crawling_config: &CrawlingConfig,
    ) -> Result<CrawlingPlan, ActorError> {
        // 1. 사이트 상태 확인
        let site_status = self.status_checker
            .check_site_status()
            .await
            .map_err(|e| ActorError::CommandProcessingFailed(format!("Site status check failed: {e}")))?;
        
        // 2. 데이터베이스 분석
        let db_analysis = self.database_analyzer
            .analyze_current_state()
            .await
            .map_err(|e| ActorError::CommandProcessingFailed(format!("Database analysis failed: {e}")))?;
        
        // 3. 최적화된 계획 수립
        let plan = self.optimize_crawling_strategy(
            crawling_config,
            Box::new(site_status),
            Box::new(db_analysis),
        ).await?;
        
        Ok(plan)
    }
    
    /// 시스템 상태를 분석합니다.
    /// 
    /// # Returns
    /// * `Ok((SiteStatus, DatabaseAnalysis))` - 분석된 시스템 상태
    /// * `Err(ActorError)` - 분석 실패
    pub async fn analyze_system_state(&self) -> Result<(crate::domain::services::SiteStatus, DatabaseAnalysis), ActorError> {
        // 사이트 상태 확인
        let site_status = self.status_checker
            .check_site_status()
            .await
            .map_err(|e| ActorError::CommandProcessingFailed(format!("Site status check failed: {e}")))?;
        
        // 데이터베이스 분석
        let db_analysis = self.database_analyzer
            .analyze_current_state()
            .await
            .map_err(|e| ActorError::CommandProcessingFailed(format!("Database analysis failed: {e}")))?;
        
        Ok((site_status, db_analysis))
    }
    
    /// 캐시된 사이트 상태로 시스템 상태를 분석합니다.
    /// 
    /// # Arguments
    /// * `cached_site_status` - 캐시된 사이트 상태
    /// 
    /// # Returns
    /// * `Ok((SiteStatus, DatabaseAnalysis))` - 분석된 시스템 상태
    /// * `Err(ActorError)` - 분석 실패
    pub async fn analyze_system_state_with_cache(&self, cached_site_status: Option<crate::domain::services::SiteStatus>) -> Result<(crate::domain::services::SiteStatus, DatabaseAnalysis), ActorError> {
        // 캐시된 상태가 있으면 사용, 없으면 새로 확인
        let site_status = if let Some(cached) = cached_site_status {
            cached
        } else {
            self.status_checker
                .check_site_status()
                .await
                .map_err(|e| ActorError::CommandProcessingFailed(format!("Site status check failed: {e}")))?
        };
        
        // 데이터베이스 분석
        let db_analysis = self.database_analyzer
            .analyze_current_state()
            .await
            .map_err(|e| ActorError::CommandProcessingFailed(format!("Database analysis failed: {e}")))?;
        
        Ok((site_status, db_analysis))
    }
    
    /// 크롤링 전략을 결정합니다.
    /// 
    /// # Arguments
    /// * `site_status` - 사이트 상태
    /// * `db_analysis` - 데이터베이스 분석 결과
    /// 
    /// # Returns
    /// * `Ok((CrawlingRangeRecommendation, ProcessingStrategy))` - 결정된 전략
    /// * `Err(ActorError)` - 전략 결정 실패
    pub async fn determine_crawling_strategy(
        &self,
        site_status: &crate::domain::services::SiteStatus,
        db_analysis: &DatabaseAnalysis,
    ) -> Result<(CrawlingRangeRecommendation, ProcessingStrategy), ActorError> {
        // 사이트 상태와 DB 분석을 기반으로 크롤링 범위 추천
        let is_site_healthy = site_status.is_accessible && site_status.health_score > 0.7;
        let range_recommendation = if is_site_healthy {
            if db_analysis.total_products > 5000 {
                CrawlingRangeRecommendation::Partial(50) // 부분 크롤링
            } else {
                CrawlingRangeRecommendation::Full // 전체 크롤링
            }
        } else {
            CrawlingRangeRecommendation::Partial(20) // 사이트 상태가 좋지 않으면 최소한의 크롤링
        };
        
        // 처리 전략 결정
        let processing_strategy = ProcessingStrategy {
            recommended_batch_size: self.calculate_optimal_batch_size(100),
            recommended_concurrency: self.calculate_optimal_concurrency(),
            should_skip_duplicates: db_analysis.missing_products_count > 100,
            should_update_existing: db_analysis.data_quality_score < 0.8,
            priority_urls: vec![],
        };
        
        Ok((range_recommendation, processing_strategy))
    }
    
    /// 배치 설정을 최적화합니다.
    /// 
    /// # Arguments
    /// * `base_config` - 기본 배치 설정
    /// * `total_pages` - 총 페이지 수
    /// 
    /// # Returns
    /// * `BatchConfig` - 최적화된 배치 설정
    #[must_use]
    pub fn optimize_batch_config(
        &self,
        base_config: &BatchConfig,
        total_pages: u32,
    ) -> BatchConfig {
        let optimal_batch_size = self.calculate_optimal_batch_size(total_pages);
        let optimal_concurrency = self.calculate_optimal_concurrency();
        
        BatchConfig {
            batch_size: optimal_batch_size.min(base_config.batch_size),
            concurrency_limit: optimal_concurrency.min(base_config.concurrency_limit),
            batch_delay_ms: self.calculate_optimal_delay(),
            retry_on_failure: base_config.retry_on_failure,
        }
    }
    
    /// 크롤링 전략을 최적화합니다.
    async fn optimize_crawling_strategy(
        &self,
        config: &CrawlingConfig,
        _site_status: Box<dyn std::any::Any + Send>, // SiteStatus trait object workaround
        _db_analysis: Box<dyn std::any::Any + Send>, // DatabaseAnalysis trait object workaround
    ) -> Result<CrawlingPlan, ActorError> {
        // Mock 구현 - 실제로는 site_status와 db_analysis를 기반으로 최적화
        let total_pages = if config.start_page >= config.end_page {
            config.start_page - config.end_page + 1  // 역순 크롤링
        } else {
            config.end_page - config.start_page + 1  // 정순 크롤링
        };
        
        // 🔧 역순 크롤링 지원: start_page >= end_page인 경우 범위를 뒤집어서 생성
        let page_range: Vec<u32> = if config.start_page >= config.end_page {
            // 역순: 299, 298, 297, 296, 295
            (config.end_page..=config.start_page).rev().collect()
        } else {
            // 정순: start_page..=end_page
            (config.start_page..=config.end_page).collect()
        };
        
        info!("🔧 CrawlingPlanner page range generation: start={}, end={}, reverse={}, pages={:?}", 
              config.start_page, config.end_page, config.start_page >= config.end_page, page_range);
        
        // 🔧 batch_size에 따른 배치 분할 로직 구현
        let batch_size = config.batch_size as usize;
        let batched_pages = if batch_size > 0 && page_range.len() > batch_size {
            page_range.chunks(batch_size).map(|chunk| chunk.to_vec()).collect::<Vec<_>>()
        } else {
            vec![page_range.clone()] // 작은 범위는 하나의 배치로
        };
        
        info!("📋 배치 계획 수립: 총 {}페이지를 {}개 배치로 분할 (batch_size={})", 
              page_range.len(), batched_pages.len(), batch_size);
        
        // 🎯 각 배치별로 ListPageCrawling phase 생성
        let mut phases = vec![
            CrawlingPhase {
                phase_type: PhaseType::StatusCheck,
                estimated_duration_secs: 30,
                priority: 1,
                pages: vec![], // 상태 확인은 페이지별 처리 없음
            },
        ];
        
        // 배치별 ListPageCrawling phases 추가
        for (batch_idx, batch_pages) in batched_pages.iter().enumerate() {
            phases.push(CrawlingPhase {
                phase_type: PhaseType::ListPageCrawling,
                estimated_duration_secs: (batch_pages.len() * 2) as u64, // 페이지당 2초 추정
                priority: 2 + batch_idx as u32, // 배치별 우선순위
                pages: batch_pages.clone(),
            });
        }
        
        // 나머지 phases 추가
        phases.extend(vec![
            CrawlingPhase {
                phase_type: PhaseType::ProductDetailCrawling,
                estimated_duration_secs: (total_pages * 10) as u64, // 페이지당 10초 추정 (상품 상세)
                priority: 100, // 높은 우선순위로 마지막에 실행
                pages: page_range.clone(),
            },
            CrawlingPhase {
                phase_type: PhaseType::DataValidation,
                estimated_duration_secs: (total_pages / 2) as u64, // 검증은 빠름
                priority: 101,
                pages: vec![],
            },
        ]);
        
        let total_estimated_duration_secs = phases.iter().map(|p| p.estimated_duration_secs).sum();
        
        Ok(CrawlingPlan {
            session_id: format!("crawling_{}", uuid::Uuid::new_v4()),
            phases,
            total_estimated_duration_secs,
            optimization_strategy: OptimizationStrategy::Balanced,
            created_at: chrono::Utc::now(),
        })
    }
    
    /// 최적 배치 크기를 계산합니다.
    fn calculate_optimal_batch_size(&self, total_pages: u32) -> u32 {
        // 총 페이지 수에 따른 적응적 배치 크기
        match total_pages {
            1..=50 => 10,
            51..=200 => 20,
            201..=1000 => 50,
            _ => 100,
        }
    }
    
    /// 최적 동시성 수준을 계산합니다.
    fn calculate_optimal_concurrency(&self) -> u32 {
        // 시스템 설정 기반 동시성 계산
        self.config.crawling
            .as_ref()
            .and_then(|c| c.default_concurrency_limit)
            .unwrap_or(5)
            .min(10)
    }
    
    /// 최적 지연 시간을 계산합니다.
    fn calculate_optimal_delay(&self) -> u64 {
        // 설정된 지연 시간 사용
        self.config.crawling
            .as_ref()
            .and_then(|c| c.request_delay_ms)
            .unwrap_or(1000)
    }
}

/// 크롤링 계획
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CrawlingPlan {
    /// 세션 ID
    pub session_id: String,
    
    /// 크롤링 단계들
    pub phases: Vec<CrawlingPhase>,
    
    /// 총 예상 실행 시간 (초)
    pub total_estimated_duration_secs: u64,
    
    /// 최적화 전략
    pub optimization_strategy: OptimizationStrategy,
    
    /// 계획 생성 시간
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 크롤링 단계
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CrawlingPhase {
    /// 단계 타입
    pub phase_type: PhaseType,
    
    /// 예상 실행 시간 (초)
    pub estimated_duration_secs: u64,
    
    /// 우선순위 (낮을수록 먼저 실행)
    pub priority: u32,
    
    /// 처리할 페이지 목록
    pub pages: Vec<u32>,
}

/// 단계 타입
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum PhaseType {
    /// 상태 확인
    StatusCheck,
    
    /// 리스트 페이지 크롤링
    ListPageCrawling,
    
    /// 상품 상세 크롤링
    ProductDetailCrawling,
    
    /// 데이터 검증
    DataValidation,
    
    /// 데이터 저장
    DataSaving,
}

/// 최적화 전략
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum OptimizationStrategy {
    /// 속도 우선
    Speed,
    
    /// 안정성 우선
    Stability,
    
    /// 균형
    Balanced,
    
    /// 리소스 절약
    ResourceEfficient,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    
    // Mock implementations for testing
    struct MockStatusChecker;
    struct MockDatabaseAnalyzer;
    
    #[async_trait::async_trait]
    impl StatusChecker for MockStatusChecker {
        async fn check_site_status(&self, _url: &str) -> Result<Box<dyn std::any::Any>, Box<dyn std::error::Error + Send + Sync>> {
            Ok(Box::new("mock_status"))
        }
    }
    
    #[async_trait::async_trait]
    impl DatabaseAnalyzer for MockDatabaseAnalyzer {
        async fn analyze_current_state(&self) -> anyhow::Result<DatabaseAnalysis> {
            Ok(DatabaseAnalysis {
                total_products: 0,
                unique_products: 0,
                missing_products_count: 0,
                last_update: Some(chrono::Utc::now()),
                missing_fields_analysis: FieldAnalysis {
                    missing_company: 0,
                    missing_model: 0,
                    missing_matter_version: 0,
                    missing_connectivity: 0,
                    missing_certification_date: 0,
                },
                data_quality_score: 1.0,
            })
        }
        
        async fn recommend_processing_strategy(&self) -> anyhow::Result<ProcessingStrategy> {
            Ok(ProcessingStrategy::default())
        }
        
        async fn analyze_duplicates(&self) -> anyhow::Result<DuplicateAnalysis> {
            Ok(DuplicateAnalysis {
                duplicate_pairs: vec![],
                total_duplicates: 0,
                confidence_scores: vec![],
            })
        }
    }
    
    #[tokio::test]
    async fn test_crawling_planner_creation() {
        let status_checker = Arc::new(MockStatusChecker) as Arc<dyn StatusChecker>;
        let database_analyzer = Arc::new(MockDatabaseAnalyzer) as Arc<dyn DatabaseAnalyzer>;
        let config = Arc::new(SystemConfig::default());
        
        let planner = CrawlingPlanner::new(status_checker, database_analyzer, config);
        
        // 플래너가 생성되었는지 확인
        assert_eq!(planner.config.crawling.default_concurrency_limit, 10);
    }
    
    #[test]
    fn test_batch_config_optimization() {
        let status_checker = Arc::new(MockStatusChecker) as Arc<dyn StatusChecker>;
        let database_analyzer = Arc::new(MockDatabaseAnalyzer) as Arc<dyn DatabaseAnalyzer>;
        let config = Arc::new(SystemConfig::default());
        
        let planner = CrawlingPlanner::new(status_checker, database_analyzer, config);
        
        let base_config = BatchConfig {
            batch_size: 100,
            concurrency_limit: 20,
            batch_delay_ms: 1000,
            retry_on_failure: true,
        };
        
        let optimized = planner.optimize_batch_config(&base_config, 150);
        
        // 최적화된 설정이 기본값보다 작거나 같은지 확인
        assert!(optimized.batch_size <= base_config.batch_size);
        assert!(optimized.concurrency_limit <= base_config.concurrency_limit);
    }
    
    #[test]
    fn test_optimal_batch_size_calculation() {
        let status_checker = Arc::new(MockStatusChecker) as Arc<dyn StatusChecker>;
        let database_analyzer = Arc::new(MockDatabaseAnalyzer) as Arc<dyn DatabaseAnalyzer>;
        let config = Arc::new(SystemConfig::default());
        
        let planner = CrawlingPlanner::new(status_checker, database_analyzer, config);
        
        assert_eq!(planner.calculate_optimal_batch_size(30), 10);
        assert_eq!(planner.calculate_optimal_batch_size(100), 20);
        assert_eq!(planner.calculate_optimal_batch_size(500), 50);
        assert_eq!(planner.calculate_optimal_batch_size(2000), 100);
    }
}
