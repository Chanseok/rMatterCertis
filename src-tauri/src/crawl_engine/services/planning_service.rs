//! PlanningService: centralized ExecutionPlan planning with Strategy pattern (Phase 1)
//!
//! Provides an initial Intelligent strategy that reuses existing CrawlingPlanner logic
//! and shared caches to build an ExecutionPlan. Also exposes lightweight overrides
//! (batch_size, concurrency, delay_ms, page range limits) to keep command code minimal.

use chrono::Utc;
use tauri::{AppHandle, Manager};
use tracing::{error, info, warn};

use crate::application::{AppState, shared_state::SharedStateCache};
use crate::crawl_engine::actors::contract::ACTOR_CONTRACT_VERSION;
use crate::crawl_engine::actors::types::{ExecutionPlan, PageRange};
use crate::crawl_engine::context::SystemConfig;
use crate::domain::services::crawling_services::{
    CrawlingRangeRecommendation, DatabaseAnalysis, SiteDataChangeStatus,
};
use crate::infrastructure::config::{AppConfig, ConfigManager};
use crate::infrastructure::html_parser::MatterDataExtractor;
use crate::infrastructure::integrated_product_repository::IntegratedProductRepository;
use crate::infrastructure::simple_http_client::HttpClient;

/// Optional overrides accepted from UI or tests.
#[derive(Debug, Clone, Default)]
pub struct PlanOverrides {
    pub batch_size: Option<u32>,
    pub concurrency: Option<u32>,
    pub delay_ms: Option<u64>,
    pub start_page: Option<u32>,
    pub end_page: Option<u32>,
    pub page_count: Option<u32>,
}

/// Strategy trait (extensible in later phases)
use async_trait::async_trait;

#[async_trait]
pub trait PlanningStrategy: Send + Sync {
    async fn plan(
        &self,
        app: &AppHandle,
        overrides: Option<&PlanOverrides>,
    ) -> Result<
        (
            ExecutionPlan,
            AppConfig,
            crate::domain::services::SiteStatus,
        ),
        String,
    >;
}

/// Intelligent strategy using CrawlingPlanner and caches
pub struct IntelligentPlanningStrategy;

impl IntelligentPlanningStrategy {
    fn compute_plan_hash(
        snapshot: &crate::crawl_engine::actors::types::PlanInputSnapshot,
        ranges: &[PageRange],
        strategy: &str,
    ) -> String {
        let hash_input =
            serde_json::json!({ "snapshot": snapshot, "ranges": ranges, "strategy": strategy });
        let hash_string = serde_json::to_string(&hash_input).unwrap_or_default();
        blake3::hash(hash_string.as_bytes()).to_hex().to_string()
    }

    fn normalize_override(val: Option<u32>) -> Option<u32> {
        match val {
            Some(0) => None,
            other => other,
        }
    }

    /// Adjusts an existing `ExecutionPlan` with optional `start_page/end_page/page_count` overrides.
    fn adjust_execution_plan_with_page_overrides(
        plan: &mut ExecutionPlan,
        start_page: Option<u32>,
        end_page: Option<u32>,
        page_count: Option<u32>,
    ) -> Result<(), String> {
        use crate::crawl_engine::actors::types::PageSlot;
        use crate::domain::pagination::PaginationCalculator;

        let norm_start = Self::normalize_override(start_page);
        let norm_end = Self::normalize_override(end_page);
        let norm_count = Self::normalize_override(page_count);
        if norm_start.is_none() && norm_end.is_none() && norm_count.is_none() {
            return Ok(());
        }
        let mut pages: Vec<u32> = plan
            .crawling_ranges
            .iter()
            .flat_map(|r| {
                if r.reverse_order {
                    (r.end_page..=r.start_page).rev().collect::<Vec<_>>()
                } else {
                    (r.start_page..=r.end_page).collect::<Vec<_>>()
                }
            })
            .collect();
        // newest -> oldest
        pages.sort_by(|a, b| b.cmp(a));
        pages.dedup();

        let original_len = pages.len();
        let min_page = *pages.iter().min().unwrap_or(&1);
        let max_page = *pages.iter().max().unwrap_or(&1);
        info!(
            "🛠️ overrides(start={:?}, end={:?}, count={:?}) plan_range=[{}..{}] total_pages_in_plan={}",
            norm_start, norm_end, norm_count, min_page, max_page, original_len
        );

        // Heuristic: interpret start_page < min_page as page_count for legacy callers
        let mut synthetic_count: Option<u32> = None;
        if norm_count.is_none() && norm_end.is_none() {
            if let Some(sp) = norm_start {
                if sp < min_page {
                    synthetic_count = Some(sp);
                }
            }
        }

        if let Some(pc) = norm_count.or(synthetic_count) {
            if (pc as usize) < pages.len() && pc > 0 {
                pages.truncate(pc as usize);
            }
        }

        if norm_start.is_some() || norm_end.is_some() {
            let high = norm_start.unwrap_or_else(|| pages.first().copied().unwrap_or(1));
            let low = norm_end.unwrap_or_else(|| pages.last().copied().unwrap_or(1));
            if low > high {
                return Err(format!(
                    "invalid override range: end_page {} > start_page {}",
                    low, high
                ));
            }
            pages.retain(|p| *p <= high && *p >= low);
        }

        if pages.is_empty() {
            warn!("⚠️ override produced empty plan; keeping original");
            return Ok(());
        }

        // Rebuild crawling_ranges respecting reverse order newest→oldest
        let batch_size = plan.batch_size.max(1) as usize;
        let mut new_ranges: Vec<PageRange> = Vec::new();
        for chunk in pages.chunks(batch_size) {
            let first = *chunk.first().unwrap();
            let last = *chunk.last().unwrap();
            new_ranges.push(PageRange {
                start_page: first,
                end_page: last,
                estimated_products: ((first - last) + 1) * 12,
                reverse_order: true,
            });
        }

        // Recompute page_slots (canonical)
        let calc = PaginationCalculator::default();
        let mut page_slots: Vec<PageSlot> = Vec::new();
        let total_pages_site = plan.input_snapshot.total_pages;
        for p in &pages {
            for idx in 0..crate::domain::constants::site::PRODUCTS_PER_PAGE as u32 {
                let pos = calc.calculate(*p, idx, total_pages_site);
                page_slots.push(PageSlot {
                    physical_page: *p,
                    page_id: i64::from(pos.page_id),
                    index_in_page: pos.index_in_page as i16,
                });
            }
        }
        page_slots.sort_by(|a, b| match a.page_id.cmp(&b.page_id) {
            core::cmp::Ordering::Equal => a.index_in_page.cmp(&b.index_in_page),
            other => other,
        });
        page_slots.dedup_by(|a, b| a.page_id == b.page_id && a.index_in_page == b.index_in_page);

        plan.crawling_ranges = new_ranges;
        plan.page_slots = page_slots;
        if let Some(kpi) = &mut plan.kpi_meta {
            kpi.total_pages = pages.len() as u32;
            kpi.batches = plan.crawling_ranges.len();
        }
        Ok(())
    }
}

/// Manual strategy: build ExecutionPlan directly from explicit page overrides
pub struct ManualPlanningStrategy;

impl ManualPlanningStrategy {
    /// Build page ranges newest -> oldest using provided overrides and fallback hints
    fn build_ranges_from_overrides(high: u32, low: u32, batch_size: u32) -> Vec<PageRange> {
        let mut pages: Vec<u32> = if low <= high {
            (low..=high).rev().collect()
        } else {
            // invalid provided range; return empty. Caller should guard
            Vec::new()
        };
        // Chunk into ranges with reverse_order=true (newest -> oldest)
        let bs = batch_size.max(1) as usize;
        let mut ranges = Vec::new();
        for chunk in pages.drain(..).collect::<Vec<_>>().chunks(bs) {
            let first = *chunk.first().unwrap();
            let last = *chunk.last().unwrap();
            let pages_count = first.saturating_sub(last) + 1;
            ranges.push(PageRange {
                start_page: first,
                end_page: last,
                estimated_products: pages_count * 12,
                reverse_order: true,
            });
        }
        ranges
    }
}

#[async_trait]
impl PlanningStrategy for IntelligentPlanningStrategy {
    async fn plan(
        &self,
        app: &AppHandle,
        overrides: Option<&PlanOverrides>,
    ) -> Result<
        (
            ExecutionPlan,
            AppConfig,
            crate::domain::services::SiteStatus,
        ),
        String,
    > {
        // 1) Load config and DB pool
        let config_manager = ConfigManager::new().map_err(|e| e.to_string())?;
        let mut app_config = config_manager
            .load_config()
            .await
            .map_err(|e| e.to_string())?;
        crate::crawl_engine::runtime::session_registry::update_global_failure_policy_from_config(
            &app_config,
        );

        let app_state = app.state::<AppState>();
        let db_pool = {
            let pool_guard = app_state.database_pool.read().await;
            pool_guard
                .as_ref()
                .ok_or_else(|| "Database pool not initialized".to_string())?
                .clone()
        };

        // 2) Services
        let product_repo = std::sync::Arc::new(IntegratedProductRepository::new(db_pool.clone()));
        info!("🔍 Testing database connection before planning...");
        if let Err(e) = product_repo.get_product_count().await {
            error!("❌ Database connection failed in planning: {}", e);
            return Err(format!("Database connection test failed: {}", e));
        }
        let http_client = HttpClient::create_from_global_config()
            .map_err(|e| e.to_string())?
            .with_context_label("Planner");
        let data_extractor = MatterDataExtractor::new().map_err(|e| e.to_string())?;
        let status_checker = std::sync::Arc::new(
            crate::infrastructure::crawling_service_impls::StatusCheckerImpl::with_product_repo(
                http_client.clone(),
                data_extractor.clone(),
                app_config.clone(),
                product_repo.clone(),
            ),
        );
        let database_analyzer = std::sync::Arc::new(
            crate::infrastructure::crawling_service_impls::DatabaseAnalyzerImpl::new(
                product_repo.clone(),
            ),
        );

        // 3) Planner with caches
        let planner = crate::crawl_engine::services::crawling_planner::CrawlingPlanner::new(
            status_checker,
            database_analyzer,
            std::sync::Arc::new(SystemConfig::default()),
        )
        .with_repository(product_repo.clone());

        let shared_cache: Option<tauri::State<SharedStateCache>> =
            app.try_state::<SharedStateCache>();
        // Site status cache
        let cached_site_status: Option<crate::domain::services::SiteStatus> =
            if let Some(c) = shared_cache.as_ref() {
                if let Some(cached) = c.get_valid_site_analysis_async(Some(5)).await {
                    Some(crate::domain::services::SiteStatus {
                        is_accessible: true,
                        response_time_ms: 0,
                        total_pages: cached.total_pages,
                        estimated_products: cached.estimated_products,
                        products_on_last_page: cached.products_on_last_page,
                        last_check_time: cached.analyzed_at,
                        health_score: cached.health_score,
                        data_change_status: SiteDataChangeStatus::Stable {
                            count: cached.estimated_products,
                        },
                        decrease_recommendation: None,
                        crawling_range_recommendation: CrawlingRangeRecommendation::Full,
                    })
                } else {
                    None
                }
            } else {
                None
            };
        // DB analysis cache
        let cached_db_analysis: Option<DatabaseAnalysis> = if let Some(c) = shared_cache.as_ref() {
            c.get_valid_db_analysis_async(Some(3))
                .await
                .map(|d| DatabaseAnalysis {
                    total_products: d.total_products,
                    unique_products: d.total_products,
                    duplicate_count: 0,
                    missing_products_count: 0,
                    last_update: Some(d.analyzed_at),
                    missing_fields_analysis:
                        crate::domain::services::crawling_services::FieldAnalysis {
                            missing_company: 0,
                            missing_model: 0,
                            missing_matter_version: 0,
                            missing_connectivity: 0,
                            missing_certification_date: 0,
                        },
                    data_quality_score: d.quality_score,
                })
        } else {
            None
        };

        // Strategy pre-choice by DB count
        let existing_product_count = product_repo.get_product_count().await.unwrap_or_default();
        let chosen_strategy = if existing_product_count > 0 {
            crate::crawl_engine::actors::types::CrawlingStrategy::ContinueFromDb
        } else {
            crate::crawl_engine::actors::types::CrawlingStrategy::NewestFirst
        };

        let crawling_config = crate::crawl_engine::actors::types::CrawlingConfig {
            site_url: "https://csa-iot.org/csa-iot_products/".to_string(),
            start_page: app_config.user.crawling.page_range_limit.max(1),
            end_page: 1,
            concurrency_limit: app_config.user.max_concurrent_requests,
            batch_size: app_config.user.batch.batch_size,
            request_delay_ms: 1000,
            timeout_secs: 300,
            max_retries: app_config.user.crawling.workers.max_retries,
            strategy: chosen_strategy,
        };

        let (crawling_plan, site_status, db_analysis_used) = planner
            .create_crawling_plan_with_caches(
                &crawling_config,
                cached_site_status,
                cached_db_analysis,
            )
            .await
            .map_err(|e| e.to_string())?;

        // Build PageRanges from plan phases
        let mut crawling_ranges: Vec<PageRange> = crawling_plan
            .phases
            .iter()
            .filter(|p| {
                matches!(
                    p.phase_type,
                    crate::crawl_engine::services::crawling_planner::PhaseType::ListPageCrawling
                )
            })
            .filter(|p| !p.pages.is_empty())
            .map(|p| {
                let first = *p.pages.first().unwrap();
                let last = *p.pages.last().unwrap();
                let pages_count = first.saturating_sub(last) + 1;
                PageRange {
                    start_page: first,
                    end_page: last,
                    estimated_products: pages_count * 12,
                    reverse_order: true,
                }
            })
            .collect();

        // Apply global page_range_limit
        let page_limit = app_config.user.crawling.page_range_limit.max(1);
        if page_limit > 0 {
            let mut accumulated: u32 = 0;
            let mut trim_index: Option<usize> = None;
            for (idx, r) in crawling_ranges.iter_mut().enumerate() {
                let pages_in_range = r.start_page.saturating_sub(r.end_page) + 1;
                if accumulated + pages_in_range > page_limit {
                    let remaining = page_limit - accumulated;
                    if remaining == 0 {
                        trim_index = Some(idx);
                    } else if remaining < pages_in_range {
                        let new_end = r.start_page.saturating_sub(remaining - 1);
                        if new_end > r.end_page {
                            r.end_page = new_end;
                            let new_pages = r.start_page.saturating_sub(r.end_page) + 1;
                            r.estimated_products = new_pages * 12;
                            trim_index = Some(idx + 1);
                        } else {
                            trim_index = Some(idx + 1);
                        }
                    }
                    break;
                }
                accumulated += pages_in_range;
            }
            if let Some(cut) = trim_index {
                crawling_ranges.truncate(cut);
            }
        }

        if crawling_ranges.is_empty() {
            crawling_ranges.push(PageRange {
                start_page: 1,
                end_page: 1,
                estimated_products: 12,
                reverse_order: true,
            });
        }

        let total_pages: u32 = crawling_ranges
            .iter()
            .map(|r| {
                if r.reverse_order {
                    r.start_page - r.end_page + 1
                } else {
                    r.end_page - r.start_page + 1
                }
            })
            .sum();

        // DB snapshot enrichment
        let (db_max_page_id, db_max_index_in_page) =
            match product_repo.get_max_page_id_and_index().await {
                Ok(v) => v,
                Err(e) => {
                    warn!("⚠️ Failed to read max page/index: {}", e);
                    (None, None)
                }
            };
        if let Some(cache_state) = shared_cache.as_ref() {
            cache_state
                .enrich_db_analysis_position(db_max_page_id, db_max_index_in_page)
                .await;
        }

        let snapshot = crate::crawl_engine::actors::types::PlanInputSnapshot {
            total_pages: site_status.total_pages,
            products_on_last_page: site_status.products_on_last_page,
            db_max_page_id,
            db_max_index_in_page,
            db_total_products: u64::from(crawling_plan.db_total_products.unwrap_or(0)),
            page_range_limit: app_config.user.crawling.page_range_limit,
            batch_size: app_config.user.batch.batch_size,
            concurrency_limit: app_config.user.max_concurrent_requests,
            created_at: Utc::now(),
        };

        let plan_id = format!("plan_{}", Utc::now().timestamp());
        let session_id = format!("actor_session_{}", Utc::now().timestamp());
        let strategy_string = format!("{:?}", crawling_plan.optimization_strategy);
        let plan_hash = Self::compute_plan_hash(&snapshot, &crawling_ranges, &strategy_string);

        if let Some(cache_state) = shared_cache.as_ref() {
            if let Some(hit) = cache_state.get_cached_execution_plan(&plan_hash).await {
                let mut hit = hit;
                // Apply overrides on cached plan (batch/concurrency/delay)
                if let Some(o) = overrides {
                    if let Some(b) = o.batch_size {
                        if b > 0 {
                            hit.batch_size = b;
                        }
                    }
                    if let Some(c) = o.concurrency {
                        if c > 0 {
                            hit.concurrency_limit = c;
                        }
                    }
                }
                return Ok((hit, app_config, site_status));
            }
        }

        // Build canonical page_slots
        use crate::domain::pagination::PaginationCalculator;
        let calc = PaginationCalculator::default();
        let mut page_slots: Vec<crate::crawl_engine::actors::types::PageSlot> = Vec::new();
        for range in &crawling_ranges {
            let pages_iter: Box<dyn Iterator<Item = u32>> = if range.reverse_order {
                Box::new(range.end_page..=range.start_page)
            } else {
                Box::new(range.start_page..=range.end_page)
            };
            for physical_page in pages_iter {
                if physical_page == 0 {
                    continue;
                }
                let assumed_capacity = crate::domain::constants::site::PRODUCTS_PER_PAGE as u32;
                for idx in 0..assumed_capacity {
                    let pos = calc.calculate(physical_page, idx, site_status.total_pages);
                    page_slots.push(crate::crawl_engine::actors::types::PageSlot {
                        physical_page,
                        page_id: i64::from(pos.page_id),
                        index_in_page: pos.index_in_page as i16,
                    });
                }
            }
        }
        page_slots.sort_by(|a, b| match a.page_id.cmp(&b.page_id) {
            core::cmp::Ordering::Equal => a.index_in_page.cmp(&b.index_in_page),
            other => other,
        });
        page_slots.dedup_by(|a, b| a.page_id == b.page_id && a.index_in_page == b.index_in_page);

        let mut execution_plan = ExecutionPlan {
            plan_id,
            session_id,
            crawling_ranges,
            batch_size: app_config.user.batch.batch_size,
            concurrency_limit: app_config.user.max_concurrent_requests,
            estimated_duration_secs: crawling_plan.total_estimated_duration_secs,
            created_at: Utc::now(),
            analysis_summary: format!(
                "Strategy: {:?}, Total pages: {}",
                strategy_string, total_pages
            ),
            original_strategy: strategy_string.clone(),
            input_snapshot: snapshot,
            plan_hash,
            skip_duplicate_urls: true,
            kpi_meta: Some(crate::crawl_engine::actors::types::ExecutionPlanKpi {
                total_ranges: execution_plan_placeholder_total_ranges(total_pages), // temporary placeholder overwritten below
                total_pages,
                batches: 0, // overwritten below
                strategy: strategy_string,
                created_at: Utc::now(),
            }),
            contract_version: ACTOR_CONTRACT_VERSION,
            page_slots,
        };
        // Fix kpi_meta using actual ranges
        if let Some(kpi) = &mut execution_plan.kpi_meta {
            kpi.total_ranges = execution_plan.crawling_ranges.len();
            kpi.batches = execution_plan.crawling_ranges.len();
        }

        // Apply overrides (batch/concurrency/delay + page overrrides)
        if let Some(o) = overrides {
            if let Some(b) = o.batch_size {
                if b > 0 {
                    execution_plan.batch_size = b;
                }
            }
            if let Some(c) = o.concurrency {
                if c > 0 {
                    execution_plan.concurrency_limit = c;
                }
            }
            if let Some(d) = o.delay_ms {
                app_config.user.request_delay_ms = d;
            }
            // Range overrides mutate plan ranges and slots
            Self::adjust_execution_plan_with_page_overrides(
                &mut execution_plan,
                o.start_page,
                o.end_page,
                o.page_count,
            )?;
        }

        if let Some(cache_state) = app.try_state::<SharedStateCache>() {
            cache_state
                .cache_execution_plan(execution_plan.clone())
                .await;
        }

        Ok((execution_plan, app_config, site_status))
    }
}

// Small helper for KPI placeholder init; avoids borrow issues above
fn execution_plan_placeholder_total_ranges(_total_pages: u32) -> usize {
    0
}

#[async_trait]
impl PlanningStrategy for ManualPlanningStrategy {
    async fn plan(
        &self,
        app: &AppHandle,
        overrides: Option<&PlanOverrides>,
    ) -> Result<
        (
            ExecutionPlan,
            AppConfig,
            crate::domain::services::SiteStatus,
        ),
        String,
    > {
        // Load config
        let config_manager = ConfigManager::new().map_err(|e| e.to_string())?;
        let mut app_config = config_manager
            .load_config()
            .await
            .map_err(|e| e.to_string())?;

        // Read cached site status if available (no network calls here)
        let shared_cache: Option<tauri::State<SharedStateCache>> =
            app.try_state::<SharedStateCache>();
        let cached_site_status: Option<crate::domain::services::SiteStatus> =
            if let Some(c) = shared_cache.as_ref() {
                c.get_valid_site_analysis_async(Some(5))
                    .await
                    .map(|cached| crate::domain::services::SiteStatus {
                        is_accessible: true,
                        response_time_ms: 0,
                        total_pages: cached.total_pages,
                        estimated_products: cached.estimated_products,
                        products_on_last_page: cached.products_on_last_page,
                        last_check_time: cached.analyzed_at,
                        health_score: cached.health_score,
                        data_change_status: SiteDataChangeStatus::Stable {
                            count: cached.estimated_products,
                        },
                        decrease_recommendation: None,
                        crawling_range_recommendation: CrawlingRangeRecommendation::Full,
                    })
            } else {
                None
            };

        // Derive high/low from overrides
        let ov =
            overrides.ok_or_else(|| "ManualPlanningStrategy requires overrides".to_string())?;
        let norm_start = IntelligentPlanningStrategy::normalize_override(ov.start_page);
        let norm_end = IntelligentPlanningStrategy::normalize_override(ov.end_page);
        let norm_count = IntelligentPlanningStrategy::normalize_override(ov.page_count);

        // Choose defaults from cache when possible
        let default_high = cached_site_status
            .as_ref()
            .map(|s| s.total_pages)
            .unwrap_or(10);
        let high = norm_start.unwrap_or(default_high).max(1);

        let low = if let Some(e) = norm_end {
            e.max(1)
        } else if let Some(c) = norm_count {
            high.saturating_sub(c.saturating_sub(1))
        } else {
            1
        };
        if low > high {
            return Err(format!(
                "invalid manual range: end_page {} > start_page {}",
                low, high
            ));
        }

        // Build ranges
        let batch_size = ov
            .batch_size
            .unwrap_or(app_config.user.batch.batch_size)
            .max(1);
        let mut ranges = Self::build_ranges_from_overrides(high, low, batch_size);
        if ranges.is_empty() {
            ranges.push(PageRange {
                start_page: high,
                end_page: high,
                estimated_products: 12,
                reverse_order: true,
            });
        }

        // Compute total pages
        let total_pages_selected: u32 = ranges
            .iter()
            .map(|r| r.start_page.saturating_sub(r.end_page) + 1)
            .sum();

        // Snapshot (DB metrics omitted in manual)
        let snapshot = crate::crawl_engine::actors::types::PlanInputSnapshot {
            total_pages: cached_site_status
                .as_ref()
                .map(|s| s.total_pages)
                .unwrap_or(high),
            products_on_last_page: cached_site_status
                .as_ref()
                .map(|s| s.products_on_last_page)
                .unwrap_or(12),
            db_max_page_id: None,
            db_max_index_in_page: None,
            db_total_products: 0,
            page_range_limit: app_config.user.crawling.page_range_limit,
            batch_size,
            concurrency_limit: app_config.user.max_concurrent_requests,
            created_at: Utc::now(),
        };

        // Compute hash/ids
        let strategy_string = "Manual".to_string();
        let plan_hash =
            IntelligentPlanningStrategy::compute_plan_hash(&snapshot, &ranges, &strategy_string);
        let plan_id = format!("plan_{}", Utc::now().timestamp());
        let session_id = format!("actor_session_{}", Utc::now().timestamp());

        // Build page_slots
        use crate::domain::pagination::PaginationCalculator;
        let calc = PaginationCalculator::default();
        let mut page_slots: Vec<crate::crawl_engine::actors::types::PageSlot> = Vec::new();
        let site_total_pages = snapshot.total_pages;
        for r in &ranges {
            let iter: Box<dyn Iterator<Item = u32>> = if r.reverse_order {
                Box::new(r.end_page..=r.start_page)
            } else {
                Box::new(r.start_page..=r.end_page)
            };
            for physical_page in iter {
                if physical_page == 0 {
                    continue;
                }
                for idx in 0..crate::domain::constants::site::PRODUCTS_PER_PAGE as u32 {
                    let pos = calc.calculate(physical_page, idx, site_total_pages);
                    page_slots.push(crate::crawl_engine::actors::types::PageSlot {
                        physical_page,
                        page_id: i64::from(pos.page_id),
                        index_in_page: pos.index_in_page as i16,
                    });
                }
            }
        }
        page_slots.sort_by(|a, b| match a.page_id.cmp(&b.page_id) {
            core::cmp::Ordering::Equal => a.index_in_page.cmp(&b.index_in_page),
            other => other,
        });
        page_slots.dedup_by(|a, b| a.page_id == b.page_id && a.index_in_page == b.index_in_page);

        // Build plan
        let mut execution_plan = ExecutionPlan {
            plan_id,
            session_id,
            crawling_ranges: ranges,
            batch_size,
            concurrency_limit: app_config.user.max_concurrent_requests,
            estimated_duration_secs: 0,
            created_at: Utc::now(),
            analysis_summary: format!(
                "Manual range {}..{} ({} pages)",
                high, low, total_pages_selected
            ),
            original_strategy: strategy_string.clone(),
            input_snapshot: snapshot,
            plan_hash,
            skip_duplicate_urls: true,
            kpi_meta: Some(crate::crawl_engine::actors::types::ExecutionPlanKpi {
                total_ranges: 0,
                total_pages: total_pages_selected,
                batches: 0,
                strategy: strategy_string,
                created_at: Utc::now(),
            }),
            contract_version: ACTOR_CONTRACT_VERSION,
            page_slots,
        };
        if let Some(kpi) = &mut execution_plan.kpi_meta {
            kpi.total_ranges = execution_plan.crawling_ranges.len();
            kpi.batches = execution_plan.crawling_ranges.len();
        }

        // Apply non-range overrides
        if let Some(o) = overrides {
            if let Some(c) = o.concurrency {
                if c > 0 {
                    execution_plan.concurrency_limit = c;
                }
            }
            if let Some(d) = o.delay_ms {
                app_config.user.request_delay_ms = d;
            }
        }

        // Return site status for downstream usage
        let site_status_out = execution_plan.input_snapshot_to_site_status();
        Ok((execution_plan, app_config, site_status_out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_build_ranges_chunks_reverse_order() {
        // high=10, low=7, batch_size=3 => [10,9,8], [7]
        let ranges = ManualPlanningStrategy::build_ranges_from_overrides(10, 7, 3);
        assert_eq!(ranges.len(), 2);
        assert!(ranges.iter().all(|r| r.reverse_order));
        assert_eq!(ranges[0].start_page, 10);
        assert_eq!(ranges[0].end_page, 8);
        assert_eq!(ranges[0].estimated_products, (10 - 8 + 1) * 12);
        assert_eq!(ranges[1].start_page, 7);
        assert_eq!(ranges[1].end_page, 7);
        assert_eq!(ranges[1].estimated_products, 12);
    }

    #[test]
    fn adjust_execution_plan_with_page_count_truncates_newest() {
        // Plan with pages 10..8 and 7..5 (reverse newest->oldest)
        let mut plan = ExecutionPlan {
            plan_id: "p".into(),
            session_id: "s".into(),
            crawling_ranges: vec![
                PageRange {
                    start_page: 10,
                    end_page: 8,
                    estimated_products: 36,
                    reverse_order: true,
                },
                PageRange {
                    start_page: 7,
                    end_page: 5,
                    estimated_products: 36,
                    reverse_order: true,
                },
            ],
            batch_size: 2,
            concurrency_limit: 3,
            estimated_duration_secs: 0,
            created_at: Utc::now(),
            analysis_summary: "t".into(),
            original_strategy: "Manual".into(),
            input_snapshot: crate::crawl_engine::actors::types::PlanInputSnapshot {
                total_pages: 20,
                products_on_last_page: 12,
                db_max_page_id: None,
                db_max_index_in_page: None,
                db_total_products: 0,
                page_range_limit: 100,
                batch_size: 2,
                concurrency_limit: 3,
                created_at: Utc::now(),
            },
            plan_hash: "h".into(),
            skip_duplicate_urls: true,
            kpi_meta: Some(crate::crawl_engine::actors::types::ExecutionPlanKpi {
                total_ranges: 2,
                total_pages: 6,
                batches: 2,
                strategy: "Manual".into(),
                created_at: Utc::now(),
            }),
            contract_version: ACTOR_CONTRACT_VERSION,
            page_slots: vec![],
        };
        // Keep only 2 newest pages overall => pages [10,9]
        let res = IntelligentPlanningStrategy::adjust_execution_plan_with_page_overrides(
            &mut plan,
            None,
            None,
            Some(2),
        );
        assert!(res.is_ok());
        // Expect a single range with start=10, end=9 and reverse_order=true
        assert_eq!(plan.crawling_ranges.len(), 1);
        let r = &plan.crawling_ranges[0];
        assert_eq!(r.start_page, 10);
        assert_eq!(r.end_page, 9);
        assert!(r.reverse_order);
        // KPI should reflect 2 pages total
        assert_eq!(plan.kpi_meta.as_ref().unwrap().total_pages, 2);
    }

    #[test]
    fn adjust_execution_plan_with_invalid_range_errors() {
        let mut plan = ExecutionPlan {
            plan_id: "p".into(),
            session_id: "s".into(),
            crawling_ranges: vec![PageRange {
                start_page: 5,
                end_page: 3,
                estimated_products: 36,
                reverse_order: true,
            }],
            batch_size: 2,
            concurrency_limit: 3,
            estimated_duration_secs: 0,
            created_at: Utc::now(),
            analysis_summary: "t".into(),
            original_strategy: "Manual".into(),
            input_snapshot: crate::crawl_engine::actors::types::PlanInputSnapshot {
                total_pages: 20,
                products_on_last_page: 12,
                db_max_page_id: None,
                db_max_index_in_page: None,
                db_total_products: 0,
                page_range_limit: 100,
                batch_size: 2,
                concurrency_limit: 3,
                created_at: Utc::now(),
            },
            plan_hash: "h".into(),
            skip_duplicate_urls: true,
            kpi_meta: Some(crate::crawl_engine::actors::types::ExecutionPlanKpi {
                total_ranges: 1,
                total_pages: 3,
                batches: 1,
                strategy: "Manual".into(),
                created_at: Utc::now(),
            }),
            contract_version: ACTOR_CONTRACT_VERSION,
            page_slots: vec![],
        };
        // start_page(lower bound via overrides set higher than end_page => invalid
        let res = IntelligentPlanningStrategy::adjust_execution_plan_with_page_overrides(
            &mut plan,
            Some(5),
            Some(10),
            None,
        );
        assert!(res.is_err());
    }
}
