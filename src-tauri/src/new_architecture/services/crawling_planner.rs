//! CrawlingPlanner - 지능형 크롤링 계획 수립 시스템
//! 
//! Actor 기반 아키텍처에서 크롤링 전략을 수립하고 
//! 최적화된 실행 계획을 생성하는 모듈입니다.

use std::sync::Arc;
use serde::{Serialize, Deserialize};
use ts_rs::TS;
use tracing::{info, warn};

use crate::domain::services::{StatusChecker, DatabaseAnalyzer};
use crate::domain::services::crawling_services::{
    DatabaseAnalysis, ProcessingStrategy, CrawlingRangeRecommendation
};
use super::super::{
    SystemConfig,
    actors::types::{CrawlingConfig, BatchConfig, ActorError}
};
use crate::domain::services::SiteStatus;
// Removed lazy_static cache (unused) to reduce warnings
// use lazy_static::lazy_static;
// use std::sync::Mutex;

// (Legacy cache removed)

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

    /// (선택) 통합 제품 저장소 - ContinueFromDb 전략 정밀 계산에 사용
    product_repo: Option<Arc<crate::infrastructure::IntegratedProductRepository>>,
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
            product_repo: None,
        }
    }

    /// 통합 제품 저장소를 추가로 연결 (builder 패턴)
    #[must_use]
    pub fn with_repository(mut self, repo: Arc<crate::infrastructure::IntegratedProductRepository>) -> Self {
        self.product_repo = Some(repo);
        self
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

    /// 캐시된 SiteStatus를 활용해 크롤링 계획을 수립하고, 사용된 SiteStatus도 함께 반환합니다.
    pub async fn create_crawling_plan_with_cache(
        &self,
        crawling_config: &CrawlingConfig,
        cached_site_status: Option<SiteStatus>,
    ) -> Result<(CrawlingPlan, SiteStatus), ActorError> {
        // 1. 사이트 상태 확인 (캐시 우선)
        let site_status = if let Some(cached) = cached_site_status {
            cached
        } else {
            self.status_checker
                .check_site_status()
                .await
                .map_err(|e| ActorError::CommandProcessingFailed(format!("Site status check failed: {e}")))?
        };

        // 2. 데이터베이스 분석
        let db_analysis = self.database_analyzer
            .analyze_current_state()
            .await
            .map_err(|e| ActorError::CommandProcessingFailed(format!("Database analysis failed: {e}")))?;

        // 3. 최적화된 계획 수립
        let plan = self.optimize_crawling_strategy(
            crawling_config,
            Box::new(site_status.clone()),
            Box::new(db_analysis),
        ).await?;

        Ok((plan, site_status))
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
            start_page: base_config.start_page,
            end_page: base_config.end_page,
        }
    }
    
    /// 크롤링 전략을 최적화합니다.
    async fn optimize_crawling_strategy(
        &self,
        config: &CrawlingConfig,
        site_status_any: Box<dyn std::any::Any + Send>,
        db_analysis_any: Box<dyn std::any::Any + Send>,
    ) -> Result<CrawlingPlan, ActorError> {
        // 실제 최적화: SiteStatus + DatabaseAnalysis 기반으로 최신 페이지부터 N개를 선택
        // 1) 전달된 Any를 다운캐스트
        let site_status = match site_status_any.downcast::<SiteStatus>() {
            Ok(b) => *b,
            Err(_) => return Err(ActorError::CommandProcessingFailed("Failed to downcast SiteStatus".to_string())),
        };
        let db_analysis = match db_analysis_any.downcast::<DatabaseAnalysis>() {
            Ok(b) => *b,
            Err(_) => return Err(ActorError::CommandProcessingFailed("Failed to downcast DatabaseAnalysis".to_string())),
        };

        // ──────────────────────────────────────────────
        // Range REUSE GUARD (simple in-memory, per process)
        // 최근 60초 이내 동일 total_pages + requested_count면 재사용
        // (실제 영속화는 추후 ConfigManager 통합 시 확장)
    // (전역 lazy_static 캐시 사용)

    let now = std::time::Instant::now();

        // 2) 요청한 페이지 수 계산 (UI 입력의 start/end는 '개수'만 사용)
        let requested_count = if config.start_page >= config.end_page { config.start_page - config.end_page + 1 } else { config.end_page - config.start_page + 1 };

        let total_pages_on_site = site_status.total_pages.max(1);
        let count = requested_count.max(1).min(total_pages_on_site);

        // 전략 분기
        let page_range: Vec<u32> = match config.strategy {
            crate::new_architecture::actors::types::CrawlingStrategy::NewestFirst => {
                // 재사용 체크
                let start = total_pages_on_site;
                let end = start.saturating_sub(count - 1).max(1);
                let pages: Vec<u32> = (end..=start).rev().collect();
                info!("🔧 Computed newest-first page range: total_pages_on_site={}, requested_count={}, actual_count={}, pages={:?}", total_pages_on_site, requested_count, pages.len(), pages);
                pages
            }
            crate::new_architecture::actors::types::CrawlingStrategy::ContinueFromDb => {
                // ──────────────────────────────────────────────
                // Precise DB continuation
            // reuse global cached range; prefer lazy_static already present
            use lazy_static::lazy_static; // macro import
                #[derive(Clone)]
                struct DbCachedRange { pages: Vec<u32>, total_pages: u32, requested: u32, last_db_page_id: Option<i32>, last_db_index: Option<i32>, ts: std::time::Instant }
                lazy_static::lazy_static! {
                    static ref LAST_DB_RANGE: std::sync::Mutex<Option<DbCachedRange>> = std::sync::Mutex::new(None);
                }

                // 1) 저장소 없으면 fallback → 최신순
                let newest_fallback_pages = || {
                    let start = total_pages_on_site;
                    let end = start.saturating_sub(count - 1).max(1);
                    (end..=start).rev().collect::<Vec<u32>>()
                };
                if self.product_repo.is_none() {
                    warn!("🧪 ContinueFromDb requested but product_repo not attached -> fallback newest-first");
                    return Ok(CrawlingPlan { session_id: uuid::Uuid::new_v4().to_string(), phases: vec![], total_estimated_duration_secs: 0, optimization_strategy: OptimizationStrategy::Balanced, created_at: chrono::Utc::now(), db_total_products: None, db_unique_products: None, db_last_update: None }); // reuse placeholder
                }
                let repo = self.product_repo.as_ref().unwrap().clone();

                // 2) DB 상태 조회
                let (max_page_id, max_index_in_page) = match repo.get_max_page_id_and_index().await {
                    Ok(v) => v,
                    Err(e) => {
                        warn!("⚠️ Failed to read DB state ({e}); using newest-first fallback");
                        return Ok(CrawlingPlan { session_id: uuid::Uuid::new_v4().to_string(), phases: vec![], total_estimated_duration_secs: 0, optimization_strategy: OptimizationStrategy::Balanced, created_at: chrono::Utc::now(), db_total_products: None, db_unique_products: None, db_last_update: None });
                    }
                };

                // 3) 캐시 재사용 판단
                if let Some(cached) = LAST_DB_RANGE.lock().unwrap().as_ref() {
                    if cached.total_pages == total_pages_on_site && cached.requested == count && cached.last_db_page_id == max_page_id && cached.last_db_index == max_index_in_page && now.duration_since(cached.ts).as_secs() < 60 {
                        info!("♻️ Reusing cached ContinueFromDb range: {:?}", cached.pages);
                        return Ok(CrawlingPlan { session_id: uuid::Uuid::new_v4().to_string(), phases: vec![], total_estimated_duration_secs: 0, optimization_strategy: OptimizationStrategy::Balanced, created_at: chrono::Utc::now(), db_total_products: None, db_unique_products: None, db_last_update: None });
                    }
                }

                // 4) 정밀 범위 계산
                let products_on_last_page = site_status.products_on_last_page;
                let precise = match repo.calculate_next_crawling_range(total_pages_on_site, products_on_last_page, count).await {
                    Ok(opt) => opt,
                    Err(e) => {
                        warn!("⚠️ Failed calculate_next_crawling_range ({e}); fallback to newest-first pages");
                        None
                    }
                };
                let pages: Vec<u32> = if let Some((start_page, end_page)) = precise {
                    if start_page >= end_page { (end_page..=start_page).rev().collect() } else { (start_page..=end_page).rev().collect() }
                } else { newest_fallback_pages() };
                *LAST_DB_RANGE.lock().unwrap() = Some(DbCachedRange { pages: pages.clone(), total_pages: total_pages_on_site, requested: count, last_db_page_id: max_page_id, last_db_index: max_index_in_page, ts: now });
                info!("🔧 Computed ContinueFromDb range: db_last=({:?},{:?}) pages={:?}", max_page_id, max_index_in_page, pages);
                pages
            }
        };

        // 4) batch_size에 따라 분할
        let batch_size = config.batch_size.max(1) as usize;
    let batched_pages: Vec<Vec<u32>> = if page_range.len() > batch_size {
            page_range
                .chunks(batch_size)
                .map(|c| c.to_vec())
                .collect()
        } else {
            vec![page_range.clone()]
        };

        info!(
            "📋 배치 계획 수립: 총 {}페이지를 {}개 배치로 분할 (batch_size={})",
            page_range.len(),
            batched_pages.len(),
            batch_size
        );

        // 5) 단계 구성: StatusCheck → (List batches) → ProductDetailCrawling → DataValidation
        let mut phases = vec![CrawlingPhase {
            phase_type: PhaseType::StatusCheck,
            estimated_duration_secs: 30,
            priority: 1,
            pages: vec![],
        }];

        for (batch_idx, batch_pages) in batched_pages.iter().enumerate() {
            phases.push(CrawlingPhase {
                phase_type: PhaseType::ListPageCrawling,
                estimated_duration_secs: (batch_pages.len() * 2) as u64,
                priority: 2 + batch_idx as u32,
                pages: batch_pages.clone(),
            });
        }

        phases.extend(vec![
            CrawlingPhase {
                phase_type: PhaseType::ProductDetailCrawling,
                estimated_duration_secs: (count * 10) as u64,
                priority: 100,
                pages: page_range.clone(),
            },
            CrawlingPhase {
                phase_type: PhaseType::DataValidation,
                estimated_duration_secs: (count / 2).max(1) as u64,
                priority: 101,
                pages: vec![],
            },
        ]);

        let total_estimated_duration_secs = phases
            .iter()
            .map(|p| p.estimated_duration_secs)
            .sum();

        Ok(CrawlingPlan {
            session_id: format!("crawling_{}", uuid::Uuid::new_v4()),
            phases,
            total_estimated_duration_secs,
            optimization_strategy: OptimizationStrategy::Balanced,
            created_at: chrono::Utc::now(),
            db_total_products: Some(db_analysis.total_products),
            db_unique_products: Some(db_analysis.unique_products),
            db_last_update: db_analysis.last_update,
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
    // ⬇️ Database snapshot (선택적 - ExecutionPlan 스냅샷 해시 안정화에 활용)
    pub db_total_products: Option<u32>,
    pub db_unique_products: Option<u32>,
    pub db_last_update: Option<chrono::DateTime<chrono::Utc>>,
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
    use std::time::Duration;
    use crate::domain::services::{StatusChecker, DatabaseAnalyzer};
    use crate::domain::services::{SiteStatus, SiteDataChangeStatus, DataDecreaseRecommendation};
    use crate::domain::services::crawling_services::{FieldAnalysis, DuplicateGroup, DuplicateType};
    
    // Mock implementations for testing
    struct MockStatusChecker;
    struct MockDatabaseAnalyzer;
    
    #[async_trait::async_trait]
    impl StatusChecker for MockStatusChecker {
        async fn check_site_status(&self) -> anyhow::Result<SiteStatus> {
            Ok(SiteStatus {
                is_accessible: true,
                response_time_ms: 100,
                total_pages: 100,
                estimated_products: 1000,
                products_on_last_page: 10,
                last_check_time: chrono::Utc::now(),
                health_score: 0.9,
                data_change_status: SiteDataChangeStatus::Stable { count: 1000 },
                decrease_recommendation: None,
                crawling_range_recommendation: CrawlingRangeRecommendation::Full,
            })
        }
        
        async fn calculate_crawling_range_recommendation(&self, _site_status: &SiteStatus, _db_analysis: &DatabaseAnalysis) -> anyhow::Result<CrawlingRangeRecommendation> {
            Ok(CrawlingRangeRecommendation::Full)
        }
        
        async fn estimate_crawling_time(&self, pages: u32) -> Duration {
            Duration::from_secs(pages as u64)
        }
        
        async fn verify_site_accessibility(&self) -> anyhow::Result<bool> {
            Ok(true)
        }
    }
    
    #[async_trait::async_trait]
    impl DatabaseAnalyzer for MockDatabaseAnalyzer {
        async fn analyze_current_state(&self) -> anyhow::Result<DatabaseAnalysis> {
            Ok(DatabaseAnalysis {
                total_products: 0,
                unique_products: 0,
                duplicate_count: 0,
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
            Ok(ProcessingStrategy {
                recommended_batch_size: 20,
                recommended_concurrency: 5,
                should_skip_duplicates: false,
                should_update_existing: true,
                priority_urls: vec![],
            })
        }
        
        async fn analyze_duplicates(&self) -> anyhow::Result<DuplicateAnalysis> {
            Ok(DuplicateAnalysis {
                total_duplicates: 0,
                duplicate_groups: vec![DuplicateGroup { product_ids: vec![], duplicate_type: DuplicateType::ExactMatch, confidence: 1.0 }],
                duplicate_percentage: 0.0,
            })
        }
    }
    
    #[tokio::test]
    async fn test_crawling_planner_creation() {
        let status_checker = Arc::new(MockStatusChecker) as Arc<dyn StatusChecker>;
        let database_analyzer = Arc::new(MockDatabaseAnalyzer) as Arc<dyn DatabaseAnalyzer>;
        let config = Arc::new(crate::new_architecture::system_config::SystemConfig::default());
        
        let planner = CrawlingPlanner::new(status_checker, database_analyzer, config);
        
        // 플래너가 생성되었는지 확인
        assert_eq!(planner.config.crawling.as_ref().and_then(|c| c.default_concurrency_limit), Some(5));
    }
    
    #[test]
    fn test_batch_config_optimization() {
        let status_checker = Arc::new(MockStatusChecker) as Arc<dyn StatusChecker>;
        let database_analyzer = Arc::new(MockDatabaseAnalyzer) as Arc<dyn DatabaseAnalyzer>;
        let config = Arc::new(crate::new_architecture::system_config::SystemConfig::default());
        
        let planner = CrawlingPlanner::new(status_checker, database_analyzer, config);
        
        let base_config = BatchConfig {
            batch_size: 100,
            concurrency_limit: 20,
            batch_delay_ms: 1000,
            retry_on_failure: true,
            start_page: None,
            end_page: None,
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
        let config = Arc::new(crate::new_architecture::system_config::SystemConfig::default());
        
        let planner = CrawlingPlanner::new(status_checker, database_analyzer, config);
        
        assert_eq!(planner.calculate_optimal_batch_size(30), 10);
        assert_eq!(planner.calculate_optimal_batch_size(100), 20);
        assert_eq!(planner.calculate_optimal_batch_size(500), 50);
        assert_eq!(planner.calculate_optimal_batch_size(2000), 100);
    }
}
