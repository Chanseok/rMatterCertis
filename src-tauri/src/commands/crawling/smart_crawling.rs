#![allow(clippy::used_underscore_binding)]
//! Smart crawling commands - uses the range calculation logic from prompts6

use crate::application::AppState;
use crate::domain::pagination::CanonicalPageIdCalculator;
use crate::infrastructure::DatabaseConnection;
use crate::infrastructure::config::ConfigManager;
use crate::infrastructure::crawling_service_impls::CrawlingRangeCalculator;
use crate::infrastructure::crawling_service_impls::RangeSimpleProgress;
use crate::infrastructure::integrated_product_repository::IntegratedProductRepository;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingRangeResponse {
    pub success: bool,
    pub range: Option<(u32, u32)>,
    pub progress: CrawlingProgressInfo,
    pub site_info: SiteInfo,
    pub local_db_info: LocalDbInfo,
    pub crawling_info: CrawlingInfo,
    pub batch_plan: BatchPlan,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchPlan {
    pub batch_size: u32,
    pub total_batches: u32,
    pub concurrency_limit: u32,
    pub batches: Vec<BatchInfo>,
    pub execution_strategy: String,
    pub estimated_duration_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchInfo {
    pub batch_id: u32,
    pub pages: Vec<u32>,
    pub estimated_products: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteInfo {
    pub total_pages: u32,
    pub products_on_last_page: u32,
    pub estimated_total_products: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalDbInfo {
    pub total_saved_products: u32,
    pub last_crawled_page: Option<u32>,
    pub last_crawled_page_id: Option<i32>,
    pub coverage_percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingInfo {
    pub pages_to_crawl: Option<u32>,
    pub estimated_new_products: Option<u32>,
    pub strategy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingProgressInfo {
    pub total_products: u32,
    pub saved_products: u32,
    pub progress_percentage: f64,
    pub max_page_id: Option<i32>,
    pub max_index_in_page: Option<i32>,
    pub is_completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingRangeRequest {
    pub total_pages_on_site: u32,
    pub products_on_last_page: u32,
}

async fn create_product_repo() -> Result<IntegratedProductRepository, String> {
    let database_url = {
        let app_data_dir = if cfg!(target_os = "macos") {
            std::env::var("HOME").map_or_else(
                |_| "./data".to_string(),
                |h| format!("{}/Library/Application Support", h),
            )
        } else {
            std::env::var("APPDATA")
                .or_else(|_| std::env::var("HOME").map(|h| format!("{}/.local/share", h)))
                .unwrap_or_else(|_| "./data".to_string())
        };
        let data_dir = format!("{}/matter-certis-v2/database", app_data_dir);
        format!("sqlite:{}/matter_certis.db", data_dir)
    };
    info!("Using database at: {}", database_url);
    let db_conn = DatabaseConnection::new(&database_url)
        .await
        .map_err(|e| format!("Failed to create database connection: {}", e))?;
    let pool = db_conn.pool().clone();
    Ok(IntegratedProductRepository::new(pool))
}

#[tauri::command]
/// # Errors
/// Returns an error string if configuration or repository access fails.
pub async fn calculate_crawling_range(
    state: State<'_, AppState>,
    request: CrawlingRangeRequest,
) -> Result<CrawlingRangeResponse, String> {
    info!(
        "🎯 Calculating next crawling range with: total_pages={}, products_on_last_page={}",
        request.total_pages_on_site, request.products_on_last_page
    );
    let config_manager =
        ConfigManager::new().map_err(|e| format!("Failed to initialize config manager: {}", e))?;
    let config = config_manager
        .load_config()
        .await
        .map_err(|e| format!("Failed to get config: {}", e))?;
    let product_repo = create_product_repo().await?;
    let range_calculator = CrawlingRangeCalculator::new(Arc::new(product_repo), config);
    let result = range_calculator
        .calculate_next_crawling_range(request.total_pages_on_site, request.products_on_last_page)
        .await
        .map_err(|e| format!("Failed to calculate crawling range: {}", e))?;
    let progress = range_calculator
        .analyze_simple_progress(request.total_pages_on_site, request.products_on_last_page)
        .await
        .map_err(|e| format!("Failed to analyze progress: {}", e))?;
    // Use saturating arithmetic to avoid underflow when total_pages_on_site is 0
    let estimated_total_products =
        (request.total_pages_on_site.saturating_sub(1) * 12) + request.products_on_last_page;
    let site_info = SiteInfo {
        total_pages: request.total_pages_on_site,
        products_on_last_page: request.products_on_last_page,
        estimated_total_products,
    };
    let local_db_info = if progress.current > 0 {
        let calculator = CanonicalPageIdCalculator::new(
            request.total_pages_on_site,
            request.products_on_last_page as usize,
        );
        let max_page_id = progress
            .current_batch
            .unwrap_or(0)
            .try_into()
            .unwrap_or(i32::MAX);
        let max_index_in_page = 0;
        if let Some((actual_page, _)) = calculator.reverse_calculate(max_page_id, max_index_in_page)
        {
            LocalDbInfo {
                total_saved_products: progress.current,
                last_crawled_page: Some(actual_page),
                last_crawled_page_id: Some(max_page_id),
                coverage_percentage: progress.percentage,
            }
        } else {
            LocalDbInfo {
                total_saved_products: progress.current,
                last_crawled_page: None,
                last_crawled_page_id: None,
                coverage_percentage: progress.percentage,
            }
        }
    } else {
        LocalDbInfo {
            total_saved_products: 0,
            last_crawled_page: None,
            last_crawled_page_id: None,
            coverage_percentage: 0.0,
        }
    };
    let response = if let Some((start_page, end_page)) = result {
        let total_pages = if start_page >= end_page {
            start_page - end_page + 1
        } else {
            end_page - start_page + 1
        };
        let estimated_new_products = total_pages * 12;
        let crawling_info = CrawlingInfo {
            pages_to_crawl: Some(total_pages),
            estimated_new_products: Some(estimated_new_products),
            strategy: "partial".to_string(),
        };
        info!(
            "🔧 Creating batch plan for range: {} to {}",
            start_page, end_page
        );
        let batch_plan = create_batch_plan(start_page, end_page).await;
        let message = format!(
            "Next crawling range: pages {} to {} (total: {} pages)",
            start_page, end_page, total_pages
        );
        info!("✅ {}", message);
        CrawlingRangeResponse {
            success: true,
            range: Some((start_page, end_page)),
            progress: convert_progress(&progress),
            site_info,
            local_db_info,
            crawling_info,
            batch_plan,
            message,
        }
    } else {
        let crawling_info = CrawlingInfo {
            pages_to_crawl: Some(0),
            estimated_new_products: Some(0),
            strategy: "none".to_string(),
        };
        let batch_plan = BatchPlan {
            batch_size: 0,
            total_batches: 0,
            concurrency_limit: 0,
            batches: vec![],
            execution_strategy: "none".to_string(),
            estimated_duration_seconds: 0,
        };
        let message = "All products have been crawled - no more pages to process".to_string();
        info!("🏁 {}", message);
        CrawlingRangeResponse {
            success: true,
            range: None,
            progress: convert_progress(&progress),
            site_info,
            local_db_info,
            crawling_info,
            batch_plan,
            message,
        }
    };
    Ok(response)
}

async fn create_batch_plan(start_page: u32, end_page: u32) -> BatchPlan {
    info!(
        "🔧 Creating batch plan: start_page={}, end_page={}",
        start_page, end_page
    );
    let app_config = match ConfigManager::new() {
        Ok(cm) => match cm.load_config().await {
            Ok(cfg) => cfg,
            Err(_e) => crate::infrastructure::config::AppConfig::for_development(),
        },
        Err(_e) => crate::infrastructure::config::AppConfig::for_development(),
    };
    // Clamp to safe minimums to avoid divide-by-zero or invalid chunk sizing
    let batch_size = app_config.user.batch.batch_size.max(1);
    let concurrency_limit = app_config.user.max_concurrent_requests.max(1);
    let pages: Vec<u32> = if start_page >= end_page {
        (end_page..=start_page).rev().collect()
    } else {
        (start_page..=end_page).collect()
    };
    let total_pages = u32::try_from(pages.len()).unwrap_or(u32::MAX);
    let total_batches = total_pages.div_ceil(batch_size);
    let mut batches = Vec::new();
    for (batch_id, chunk) in pages.chunks(batch_size as usize).enumerate() {
        let batch_info = BatchInfo {
            batch_id: u32::try_from(batch_id).unwrap_or(u32::MAX),
            pages: chunk.to_vec(),
            estimated_products: u32::try_from(chunk.len())
                .unwrap_or(u32::MAX)
                .saturating_mul(12),
        };
        batches.push(batch_info);
    }
    let estimated_duration_seconds =
        total_pages.saturating_mul(2) + (total_batches.saturating_mul(batch_size) / concurrency_limit);
    BatchPlan {
        batch_size,
        total_batches,
        concurrency_limit,
        batches,
        execution_strategy: "concurrent".to_string(),
        estimated_duration_seconds,
    }
}

#[tauri::command]
/// # Errors
/// Returns an error string if progress analysis fails.
pub async fn get_crawling_progress(
    state: State<'_, AppState>,
    total_pages_on_site: u32,
    products_on_last_page: u32,
) -> Result<CrawlingProgressInfo, String> {
    let config_manager =
        ConfigManager::new().map_err(|e| format!("Failed to initialize config manager: {}", e))?;
    let config = config_manager
        .load_config()
        .await
        .map_err(|e| format!("Failed to get config: {}", e))?;
    let product_repo = create_product_repo().await?;
    let range_calculator = CrawlingRangeCalculator::new(Arc::new(product_repo), config);
    let progress = range_calculator
        .analyze_simple_progress(total_pages_on_site, products_on_last_page)
        .await
        .map_err(|e| format!("Failed to analyze progress: {}", e))?;
    Ok(convert_progress(&progress))
}

#[tauri::command]
/// # Errors
/// Returns an error string if database state retrieval fails.
pub async fn get_database_state_for_range_calculation(
    state: State<'_, AppState>,
) -> Result<DatabaseStateInfo, String> {
    let product_repo = create_product_repo().await?;
    let (max_page_id, max_index_in_page) = product_repo
        .get_max_page_id_and_index()
        .await
        .map_err(|e| format!("Failed to get max page ID and index: {}", e))?;
    let total_products = product_repo
        .get_product_count()
        .await
        .map_err(|e| format!("Failed to get product count: {}", e))?;
    Ok(DatabaseStateInfo {
        max_page_id,
        max_index_in_page,
        total_products: u32::try_from(total_products).unwrap_or(u32::MAX),
        has_data: max_page_id.is_some() && max_index_in_page.is_some(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStateInfo {
    pub max_page_id: Option<i32>,
    pub max_index_in_page: Option<i32>,
    pub total_products: u32,
    pub has_data: bool,
}

#[tauri::command]
/// # Errors
/// Returns an error string if the demo pipeline fails.
pub async fn demo_prompts6_calculation() -> Result<String, String> {
    Ok("prompts6 demo".to_string())
}

fn convert_progress(progress: &RangeSimpleProgress) -> CrawlingProgressInfo {
    CrawlingProgressInfo {
        total_products: progress.total,
        saved_products: progress.current,
        progress_percentage: progress.percentage,
        max_page_id: progress.current_batch.and_then(|b| i32::try_from(b).ok()),
        max_index_in_page: progress.total_batches.and_then(|b| i32::try_from(b).ok()),
        is_completed: progress.is_completed,
    }
}
