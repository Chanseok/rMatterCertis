//! Small sanity run to verify CrawlingPlanner newest-first range and batching

use matter_certis_v2_lib::crawl_engine::actors::types::{CrawlingConfig, CrawlingStrategy};
use matter_certis_v2_lib::crawl_engine::services::crawling_planner::{CrawlingPlanner, PhaseType};
use matter_certis_v2_lib::crawl_engine::system_config::SystemConfig;
use matter_certis_v2_lib::domain::services::crawling_services::{
    CrawlingRangeRecommendation, DatabaseAnalysis, DuplicateAnalysis, DuplicateGroup,
    DuplicateType, FieldAnalysis, ProcessingStrategy, SiteDataChangeStatus, SiteStatus,
};
use matter_certis_v2_lib::domain::services::{DatabaseAnalyzer, StatusChecker};
use std::sync::Arc;

struct MockStatusChecker {
    total_pages: u32,
    products_on_last_page: u32,
}

#[async_trait::async_trait]
impl StatusChecker for MockStatusChecker {
    async fn check_site_status(&self) -> anyhow::Result<SiteStatus> {
        Ok(SiteStatus {
            is_accessible: true,
            response_time_ms: 50,
            total_pages: self.total_pages,
            estimated_products: self.total_pages * 10,
            products_on_last_page: self.products_on_last_page,
            last_check_time: chrono::Utc::now(),
            health_score: 0.95,
            data_change_status: SiteDataChangeStatus::Stable {
                count: self.total_pages * 10,
            },
            decrease_recommendation: None,
            crawling_range_recommendation: CrawlingRangeRecommendation::Full,
        })
    }

    async fn calculate_crawling_range_recommendation(
        &self,
        _site_status: &SiteStatus,
        _db_analysis: &DatabaseAnalysis,
    ) -> anyhow::Result<CrawlingRangeRecommendation> {
        Ok(CrawlingRangeRecommendation::Full)
    }

    async fn estimate_crawling_time(&self, pages: u32) -> std::time::Duration {
        std::time::Duration::from_secs(pages as u64)
    }

    async fn verify_site_accessibility(&self) -> anyhow::Result<bool> {
        Ok(true)
    }
}

struct MockDatabaseAnalyzer;

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
            duplicate_groups: vec![DuplicateGroup {
                product_ids: vec![],
                duplicate_type: DuplicateType::ExactMatch,
                confidence: 1.0,
            }],
            duplicate_percentage: 0.0,
        })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Given: site has 498 pages, last page has 8 products
    let total_pages = 498u32;
    let last_page_products = 8u32;

    // Planner with mocks
    let status_checker: Arc<dyn StatusChecker> = Arc::new(MockStatusChecker {
        total_pages,
        products_on_last_page: last_page_products,
    });
    let db_analyzer: Arc<dyn DatabaseAnalyzer> = Arc::new(MockDatabaseAnalyzer);
    let system_config = Arc::new(SystemConfig::default());
    let planner = CrawlingPlanner::new(status_checker, db_analyzer, system_config);

    // Request: newest-first 12 pages, batch_size 9 => expect [498..490] and [489..487]
    let cfg = CrawlingConfig {
        site_url: "https://example.com".into(),
        start_page: 498,
        end_page: 487,
        // For this standalone sanity binary, use an explicit small concurrency.
        // The main app path uses config-driven values.
        concurrency_limit: 3,
        batch_size: 9,
        request_delay_ms: 1000,
        timeout_secs: 30,
        max_retries: 1,
        strategy: CrawlingStrategy::NewestFirst,
    };

    let cached = Some(SiteStatus {
        is_accessible: true,
        response_time_ms: 50,
        total_pages,
        estimated_products: total_pages * 10,
        products_on_last_page: last_page_products,
        last_check_time: chrono::Utc::now(),
        health_score: 0.95,
        data_change_status: SiteDataChangeStatus::Stable {
            count: total_pages * 10,
        },
        decrease_recommendation: None,
        crawling_range_recommendation: CrawlingRangeRecommendation::Full,
    });

    let (plan, _status) = planner
        .create_crawling_plan_with_cache(&cfg, cached)
        .await?;

    println!("phases: {}", plan.phases.len());
    for (i, ph) in plan.phases.iter().enumerate() {
        match ph.phase_type {
            PhaseType::ListPageCrawling => {
                println!("phase[{i}] List pages ({}): {:?}", ph.pages.len(), ph.pages);
            }
            PhaseType::StatusCheck => println!("phase[{i}] StatusCheck"),
            PhaseType::ProductDetailCrawling => {
                println!("phase[{i}] Detail ({} pages)", ph.pages.len())
            }
            PhaseType::DataValidation => println!("phase[{i}] Validation"),
            PhaseType::DataSaving => println!("phase[{i}] Saving"),
        }
    }

    Ok(())
}
