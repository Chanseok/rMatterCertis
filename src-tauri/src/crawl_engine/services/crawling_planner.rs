//! `CrawlingPlanner` - 지능형 크롤링 계획 수립 시스템
//!
//! Actor 기반 아키텍처에서 크롤링 전략을 수립하고
//! 최적화된 실행 계획을 생성하는 모듈입니다.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};
use ts_rs::TS;

use super::super::{
    SystemConfig,
    actors::types::{ActorError, BatchConfig, CrawlingConfig},
};
use crate::domain::services::SiteStatus;
use crate::domain::services::crawling_services::{
    CrawlingRangeRecommendation, DatabaseAnalysis, ProcessingStrategy,
};
use crate::domain::services::{DatabaseAnalyzer, StatusChecker};
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

    /// (선택) 통합 제품 저장소 - `ContinueFromDb` 전략 정밀 계산에 사용
    product_repo: Option<Arc<crate::infrastructure::IntegratedProductRepository>>,
}

impl CrawlingPlanner {
    /// 새로운 `CrawlingPlanner` 인스턴스를 생성합니다.
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
    pub fn with_repository(
        mut self,
        repo: Arc<crate::infrastructure::IntegratedProductRepository>,
    ) -> Self {
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
        let site_status = self.status_checker.check_site_status().await.map_err(|e| {
            ActorError::CommandProcessingFailed(format!("Site status check failed: {e}"))
        })?;

        // 2. 데이터베이스 분석
        let db_analysis = self
            .database_analyzer
            .analyze_current_state()
            .await
            .map_err(|e| {
                ActorError::CommandProcessingFailed(format!("Database analysis failed: {e}"))
            })?;

        // 3. 최적화된 계획 수립
        let plan = self
            .optimize_crawling_strategy(
                crawling_config,
                Box::new(site_status),
                Box::new(db_analysis),
            )
            .await?;

        Ok(plan)
    }

    /// 캐시된 `SiteStatus를` 활용해 크롤링 계획을 수립하고, 사용된 `SiteStatus도` 함께 반환합니다.
    pub async fn create_crawling_plan_with_cache(
        &self,
        crawling_config: &CrawlingConfig,
        cached_site_status: Option<SiteStatus>,
    ) -> Result<(CrawlingPlan, SiteStatus), ActorError> {
        // 1. 사이트 상태 확인 (캐시 우선)
        let site_status = if let Some(cached) = cached_site_status {
            cached
        } else {
            self.status_checker.check_site_status().await.map_err(|e| {
                ActorError::CommandProcessingFailed(format!("Site status check failed: {e}"))
            })?
        };

        // 2. 데이터베이스 분석 (SharedStateCache 재사용 시도)
        let db_analysis = self
            .database_analyzer
            .analyze_current_state()
            .await
            .map_err(|e| {
                ActorError::CommandProcessingFailed(format!("Database analysis failed: {e}"))
            })?;

        // 3. 최적화된 계획 수립
        let plan = self
            .optimize_crawling_strategy(
                crawling_config,
                Box::new(site_status.clone()),
                Box::new(db_analysis),
            )
            .await?;

        Ok((plan, site_status))
    }

    /// 캐시된 `SiteStatus` 및 `DatabaseAnalysis를` 활용해 크롤링 계획을 수립하고 모두 반환합니다.
    /// 기존 `create_crawling_plan_with_cache` 와 달리 DB 분석도 캐시를 재사용합니다.
    pub async fn create_crawling_plan_with_caches(
        &self,
        crawling_config: &CrawlingConfig,
        cached_site_status: Option<SiteStatus>,
        cached_db_analysis: Option<DatabaseAnalysis>,
    ) -> Result<(CrawlingPlan, SiteStatus, DatabaseAnalysis), ActorError> {
        // 1. 사이트 상태 (캐시 우선)
        let site_status = if let Some(cached) = cached_site_status {
            cached
        } else {
            self.status_checker.check_site_status().await.map_err(|e| {
                ActorError::CommandProcessingFailed(format!("Site status check failed: {e}"))
            })?
        };

        // 2. DB 분석 (캐시 우선)
        let db_analysis = if let Some(cached) = cached_db_analysis {
            cached
        } else {
            self.database_analyzer
                .analyze_current_state()
                .await
                .map_err(|e| {
                    ActorError::CommandProcessingFailed(format!("Database analysis failed: {e}"))
                })?
        };

        // 3. 최적화된 계획 수립
        let plan = self
            .optimize_crawling_strategy(
                crawling_config,
                Box::new(site_status.clone()),
                Box::new(db_analysis.clone()),
            )
            .await?;

        Ok((plan, site_status, db_analysis))
    }

    /// 시스템 상태를 분석합니다.
    ///
    /// # Returns
    /// * `Ok((SiteStatus, DatabaseAnalysis))` - 분석된 시스템 상태
    /// * `Err(ActorError)` - 분석 실패
    pub async fn analyze_system_state(
        &self,
    ) -> Result<(crate::domain::services::SiteStatus, DatabaseAnalysis), ActorError> {
        // 사이트 상태 확인
        let site_status = self.status_checker.check_site_status().await.map_err(|e| {
            ActorError::CommandProcessingFailed(format!("Site status check failed: {e}"))
        })?;

        // 데이터베이스 분석
        let db_analysis = self
            .database_analyzer
            .analyze_current_state()
            .await
            .map_err(|e| {
                ActorError::CommandProcessingFailed(format!("Database analysis failed: {e}"))
            })?;

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
    pub async fn analyze_system_state_with_cache(
        &self,
        cached_site_status: Option<crate::domain::services::SiteStatus>,
    ) -> Result<(crate::domain::services::SiteStatus, DatabaseAnalysis), ActorError> {
        // 캐시된 상태가 있으면 사용, 없으면 새로 확인
        let site_status = if let Some(cached) = cached_site_status {
            cached
        } else {
            self.status_checker.check_site_status().await.map_err(|e| {
                ActorError::CommandProcessingFailed(format!("Site status check failed: {e}"))
            })?
        };

        // 데이터베이스 분석
        let db_analysis = self
            .database_analyzer
            .analyze_current_state()
            .await
            .map_err(|e| {
                ActorError::CommandProcessingFailed(format!("Database analysis failed: {e}"))
            })?;

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
    pub fn determine_crawling_strategy(
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
            Err(_) => {
                return Err(ActorError::CommandProcessingFailed(
                    "Failed to downcast SiteStatus".to_string(),
                ));
            }
        };
        let db_analysis = match db_analysis_any.downcast::<DatabaseAnalysis>() {
            Ok(b) => *b,
            Err(_) => {
                return Err(ActorError::CommandProcessingFailed(
                    "Failed to downcast DatabaseAnalysis".to_string(),
                ));
            }
        };

        // ──────────────────────────────────────────────
        // Range REUSE GUARD (simple in-memory, per process)
        // 최근 60초 이내 동일 total_pages + requested_count면 재사용
        // (실제 영속화는 추후 ConfigManager 통합 시 확장)
        // (전역 lazy_static 캐시 사용)

        let now = std::time::Instant::now();

        // 2) 요청한 페이지 수 계산 (UI 입력의 start/end는 '개수'만 사용)
        let requested_count = if config.start_page >= config.end_page {
            config.start_page - config.end_page + 1
        } else {
            config.end_page - config.start_page + 1
        };

        let total_pages_on_site = site_status.total_pages.max(1);
        let count = requested_count.max(1).min(total_pages_on_site);

        // 전략 분기
        // Track DB position if fetched during strategy-specific branch (to avoid duplicate query later)
        let mut db_position_for_reuse: Option<(Option<i32>, Option<i32>)> = None;
        let mut page_range: Vec<u32> = match config.strategy {
            crate::crawl_engine::actors::types::CrawlingStrategy::NewestFirst => {
                // Enhanced newest-first: prioritize pages containing products lacking details if repository available
                if let Some(repo) = &self.product_repo {
                    let fetch_limit: i32 = ((count * 20).max(20)).min(2000) as i32;
                    match repo.get_products_without_details(fetch_limit).await {
                        Ok(missing) if !missing.is_empty() => {
                            let mut pages: Vec<u32> = missing
                                .iter()
                                .filter_map(|p| p.page_id.map(|pid| pid as u32))
                                .collect();
                            pages.sort_unstable();
                            pages.dedup();
                            pages.sort_by(|a, b| b.cmp(a));
                            let mut selected: Vec<u32> =
                                pages.iter().take(count as usize).copied().collect();
                            if let (Some(min_sel), Some(max_sel)) = (
                                selected.iter().min().copied(),
                                selected.iter().max().copied(),
                            ) {
                                let mut boundary = vec![];
                                if min_sel > 1 {
                                    boundary.push(min_sel - 1);
                                }
                                if max_sel < total_pages_on_site {
                                    boundary.push(max_sel + 1);
                                }
                                for b in boundary {
                                    if !selected.contains(&b) {
                                        selected.push(b);
                                    }
                                }
                            }
                            if selected.len() < count as usize {
                                let mut candidate = total_pages_on_site;
                                while selected.len() < count as usize && candidate >= 1 {
                                    if !selected.contains(&candidate) {
                                        selected.push(candidate);
                                    }
                                    if candidate == 1 {
                                        break;
                                    }
                                    candidate -= 1;
                                }
                            }
                            selected.sort_by(|a, b| b.cmp(a));
                            info!(
                                "🧭 Missing-detail aware page range computed: base_missing_pages={} requested={} final_selected={} pages={:?}",
                                pages.len(),
                                count,
                                selected.len(),
                                selected
                            );
                            if selected.is_empty() {
                                let start = total_pages_on_site;
                                let end = start.saturating_sub(count - 1).max(1);
                                (end..=start).rev().collect()
                            } else {
                                selected
                            }
                        }
                        Ok(_) => {
                            let force_recrawl = std::env::var("MC_FORCE_RECRAWL_ON_COMPLETE")
                                .map(|v| {
                                    let t = v.trim();
                                    !(t.eq("0") || t.eq_ignore_ascii_case("false"))
                                })
                                .unwrap_or(false);
                            if force_recrawl {
                                let start = total_pages_on_site;
                                let end = start.saturating_sub(count - 1).max(1);
                                let pages: Vec<u32> = (end..=start).rev().collect();
                                info!(
                                    "♻️ Force recrawl enabled (MC_FORCE_RECRAWL_ON_COMPLETE) -> selecting newest pages again count={} pages={:?}",
                                    pages.len(),
                                    pages
                                );
                                pages
                            } else {
                                info!(
                                    "✅ All products already have details (no missing detail rows) -> skipping list page crawling (use MC_FORCE_RECRAWL_ON_COMPLETE=1 to override)"
                                );
                                Vec::new()
                            }
                        }
                        Err(e) => {
                            let start = total_pages_on_site;
                            let end = start.saturating_sub(count - 1).max(1);
                            let pages: Vec<u32> = (end..=start).rev().collect();
                            warn!(
                                "⚠️ Missing-detail fetch failed ({}), fallback newest-first pages count={}",
                                e,
                                pages.len()
                            );
                            pages
                        }
                    }
                } else {
                    let start = total_pages_on_site;
                    let end = start.saturating_sub(count - 1).max(1);
                    let pages: Vec<u32> = (end..=start).rev().collect();
                    info!(
                        "🔧 Computed newest-first page range (no repo): total_pages_on_site={} requested_count={} actual_count={} pages={:?}",
                        total_pages_on_site,
                        requested_count,
                        pages.len(),
                        pages
                    );
                    pages
                }
            }
            crate::crawl_engine::actors::types::CrawlingStrategy::ContinueFromDb => {
                // Reintroduce lightweight cache + fallback helper removed during cleanup
                #[derive(Clone)]
                struct DbCachedRange {
                    pages: Vec<u32>,
                    total_pages: u32,
                    requested: u32,
                    last_db_page_id: Option<i32>,
                    last_db_index: Option<i32>,
                    ts: std::time::Instant,
                }
                lazy_static::lazy_static! {
                    static ref LAST_DB_RANGE: std::sync::Mutex<Option<DbCachedRange>> = std::sync::Mutex::new(None);
                }
                let newest_fallback_pages = || {
                    let start = total_pages_on_site;
                    let end = start.saturating_sub(count - 1).max(1);
                    (end..=start).rev().collect::<Vec<u32>>()
                };
                let repo = if let Some(repo) = &self.product_repo {
                    repo.clone()
                } else {
                    warn!(
                        "🧪 ContinueFromDb requested but product_repo not attached -> fallback newest-first"
                    );
                    return Ok(CrawlingPlan {
                        session_id: uuid::Uuid::new_v4().to_string(),
                        phases: vec![],
                        total_estimated_duration_secs: 0,
                        optimization_strategy: OptimizationStrategy::Balanced,
                        created_at: chrono::Utc::now(),
                        db_total_products: None,
                        db_unique_products: None,
                        db_last_update: None,
                    });
                };

                // 2) DB 상태 조회
                let (max_page_id, max_index_in_page) = match repo.get_max_page_id_and_index().await
                {
                    Ok(v) => v,
                    Err(e) => {
                        warn!("⚠️ Failed to read DB state ({e}); using newest-first fallback");
                        return Ok(CrawlingPlan {
                            session_id: uuid::Uuid::new_v4().to_string(),
                            phases: vec![],
                            total_estimated_duration_secs: 0,
                            optimization_strategy: OptimizationStrategy::Balanced,
                            created_at: chrono::Utc::now(),
                            db_total_products: None,
                            db_unique_products: None,
                            db_last_update: None,
                        });
                    }
                };
                db_position_for_reuse = Some((max_page_id, max_index_in_page));

                // 3) 캐시 재사용 판단
                if let Ok(cache_guard) = LAST_DB_RANGE.lock() {
                    if let Some(cached) = cache_guard.as_ref() {
                        if cached.total_pages == total_pages_on_site
                            && cached.requested == count
                            && cached.last_db_page_id == max_page_id
                            && cached.last_db_index == max_index_in_page
                            && now.duration_since(cached.ts).as_secs() < 60
                        {
                            info!("♻️ Reusing cached ContinueFromDb range: {:?}", cached.pages);
                            return Ok(CrawlingPlan {
                                session_id: uuid::Uuid::new_v4().to_string(),
                                phases: vec![],
                                total_estimated_duration_secs: 0,
                                optimization_strategy: OptimizationStrategy::Balanced,
                                created_at: chrono::Utc::now(),
                                db_total_products: None,
                                db_unique_products: None,
                                db_last_update: None,
                            });
                        }
                    }
                } else {
                    warn!("⚠️ Failed to lock LAST_DB_RANGE cache; proceeding without reuse");
                }

                // 4) 정밀 범위 계산
                let products_on_last_page = site_status.products_on_last_page;
                let precise = match repo
                    .calculate_next_crawling_range(
                        total_pages_on_site,
                        products_on_last_page,
                        count,
                    )
                    .await
                {
                    Ok(opt) => opt,
                    Err(e) => {
                        warn!(
                            "⚠️ Failed calculate_next_crawling_range ({e}); fallback to newest-first pages"
                        );
                        None
                    }
                };
                let mut pages: Vec<u32> = if let Some((start_page, end_page)) = precise {
                    if start_page >= end_page {
                        (end_page..=start_page).rev().collect()
                    } else {
                        (start_page..=end_page).rev().collect()
                    }
                } else {
                    newest_fallback_pages()
                };
                // Drop pages that are already fully detailed to reduce no-op batches
                if let Some(repo_ref) = self.product_repo.as_ref() {
                    let repo = repo_ref.clone();
                    let total_pages = site_status.total_pages;
                    let products_on_last = site_status.products_on_last_page;
                    let mut filtered: Vec<u32> = Vec::with_capacity(pages.len());
                    for sp in pages.iter().copied() {
                        match repo
                            .is_site_page_fully_detailed(sp, total_pages, products_on_last)
                            .await
                        {
                            Ok(true) => {
                                tracing::info!("🧹 Skipping fully detailed page {} from plan", sp);
                            }
                            Ok(false) | Err(_) => {
                                filtered.push(sp);
                            }
                        }
                    }
                    if filtered.len() != pages.len() {
                        tracing::info!(
                            "🧮 Planner filtered pages: before={} after={}",
                            pages.len(),
                            filtered.len()
                        );
                        pages = filtered;
                    }
                }
                if let (Some(mp), Some(mi)) = (max_page_id, max_index_in_page) {
                    if mi < 11 {
                        // partial page
                        let partial_site_page = total_pages_on_site - mp as u32;
                        if !pages.contains(&partial_site_page) {
                            pages.insert(0, partial_site_page);
                            info!(
                                "🔁 Partial page re-included (planner): {} (db_page_id={}, index_in_page={})",
                                partial_site_page, mp, mi
                            );
                        }
                    }
                }
                if let Ok(mut cache_guard) = LAST_DB_RANGE.lock() {
                    *cache_guard = Some(DbCachedRange {
                        pages: pages.clone(),
                        total_pages: total_pages_on_site,
                        requested: count,
                        last_db_page_id: max_page_id,
                        last_db_index: max_index_in_page,
                        ts: now,
                    });
                } else {
                    warn!("⚠️ Failed to update LAST_DB_RANGE cache");
                }
                info!(
                    "🔧 Computed ContinueFromDb range: db_last=({:?},{:?}) pages={:?}",
                    max_page_id, max_index_in_page, pages
                );
                pages
            }
        };

        // 4) Partial page reinclusion (unified)
        if !page_range.is_empty() {
            // Prefer already fetched DB position (ContinueFromDb) to avoid an extra query.
            let position = if let Some(pos) = &db_position_for_reuse {
                Some(*pos)
            } else if let Some(repo) = &self.product_repo {
                repo.get_max_page_id_and_index().await.ok()
            } else {
                None
            };
            if let Some((max_page_id, max_index_in_page)) = position {
                if let (Some(mp), Some(mi)) = (max_page_id, max_index_in_page) {
                    if mi < 11 {
                        // partial page needs refresh
                        let total_pages_on_site = site_status.total_pages.max(1);
                        let partial_site_page = total_pages_on_site - mp as u32;
                        if !page_range.contains(&partial_site_page) {
                            if db_position_for_reuse.is_some() {
                                info!(
                                    "🔁 Partial page re-included (planner unified, reuse db_position): {} (db_page_id={}, index_in_page={})",
                                    partial_site_page, mp, mi
                                );
                            } else {
                                info!(
                                    "🔁 Partial page re-included (planner unified, fresh db_position): {} (db_page_id={}, index_in_page={})",
                                    partial_site_page, mp, mi
                                );
                            }
                            page_range.insert(0, partial_site_page);
                            page_range.sort_by(|a, b| b.cmp(a));
                            page_range.dedup();
                        }
                    }
                }
            }
        }

        // 5) 배치 크기에 따라 분할
        let batch_size = config.batch_size.max(1) as usize;
        // 페이지가 비어 있으면 재크롤링이 필요 없는 상태이므로 배치 생성 생략
        let batched_pages: Vec<Vec<u32>> = if page_range.is_empty() {
            Vec::new()
        } else if page_range.len() > batch_size {
            page_range.chunks(batch_size).map(<[u32]>::to_vec).collect()
        } else {
            vec![page_range.clone()]
        };

        if page_range.is_empty() {
            info!("📋 배치 계획 수립: 수집할 신규 페이지 없음 (모든 detail 이미 존재) batches=0");
        } else {
            info!(
                "📋 배치 계획 수립: 총 {}페이지를 {}개 배치로 분할 (batch_size={})",
                page_range.len(),
                batched_pages.len(),
                batch_size
            );
        }

        // 5) 단계 구성: StatusCheck → (List batches) → DataValidation
        let mut phases = vec![CrawlingPhase {
            phase_type: PhaseType::StatusCheck,
            estimated_duration_secs: 30,
            priority: 1,
            pages: vec![],
        }];

        for (batch_idx, batch_pages) in batched_pages.iter().enumerate() {
            if batch_pages.is_empty() {
                continue;
            }
            phases.push(CrawlingPhase {
                phase_type: PhaseType::ListPageCrawling,
                estimated_duration_secs: (batch_pages.len() * 2) as u64,
                priority: 2 + batch_idx as u32,
                pages: batch_pages.clone(),
            });
        }

        phases.extend(vec![CrawlingPhase {
            phase_type: PhaseType::DataValidation,
            estimated_duration_secs: u64::from((count / 2).max(1)),
            priority: 101,
            pages: vec![],
        }]);

        let total_estimated_duration_secs = phases.iter().map(|p| p.estimated_duration_secs).sum();

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
    const fn calculate_optimal_batch_size(&self, total_pages: u32) -> u32 {
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
        self.config
            .crawling
            .as_ref()
            .and_then(|c| c.default_concurrency_limit)
            .unwrap_or(5)
            .min(10)
    }

    /// 최적 지연 시간을 계산합니다.
    fn calculate_optimal_delay(&self) -> u64 {
        // 설정된 지연 시간 사용
        self.config
            .crawling
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
