//! Smart crawling commands - uses the range calculation logic from prompts6
//! 
//! This module provides commands for smart crawling that automatically calculates
//! the next pages to crawl based on the current database state and site information.

use crate::application::AppState;
use crate::infrastructure::crawling_service_impls::{CrawlingRangeCalculator, CrawlingProgress};
use crate::infrastructure::config::ConfigManager;
use crate::infrastructure::integrated_product_repository::IntegratedProductRepository;
use crate::infrastructure::DatabaseConnection;
use anyhow::Result;
use tauri::State;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

/// Response for crawling range calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingRangeResponse {
    pub success: bool,
    pub range: Option<(u32, u32)>,
    pub progress: CrawlingProgressInfo,
    pub message: String,
}

/// Crawling progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingProgressInfo {
    pub total_products: u32,
    pub saved_products: u32,
    pub progress_percentage: f64,
    pub max_page_id: Option<i32>,
    pub max_index_in_page: Option<i32>,
    pub is_completed: bool,
}

/// Request for calculating crawling range
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingRangeRequest {
    pub total_pages_on_site: u32,
    pub products_on_last_page: u32,
}

/// Create product repository from database connection
async fn create_product_repo() -> Result<IntegratedProductRepository, String> {
    let db_conn = DatabaseConnection::new(".local/dev-database.sqlite").await
        .map_err(|e| format!("Failed to create database connection: {}", e))?;
    
    let pool = db_conn.pool().clone();
    
    Ok(IntegratedProductRepository::new(pool))
}

/// Calculate the next crawling range based on current DB state
#[tauri::command]
pub async fn calculate_crawling_range(
    _state: State<'_, AppState>,
    request: CrawlingRangeRequest,
) -> Result<CrawlingRangeResponse, String> {
    info!("🎯 Calculating next crawling range with: total_pages={}, products_on_last_page={}", 
          request.total_pages_on_site, request.products_on_last_page);

    // Get configuration
    let config_manager = ConfigManager::new()
        .map_err(|e| format!("Failed to initialize config manager: {}", e))?;
    
    let config = config_manager.load_config().await
        .map_err(|e| format!("Failed to get config: {}", e))?;

    // Create product repository
    let product_repo = create_product_repo().await?;

    // Create range calculator
    let range_calculator = CrawlingRangeCalculator::new(
        Arc::new(product_repo),
        config,
    );

    // Calculate next range
    let result = range_calculator.calculate_next_crawling_range(
        request.total_pages_on_site,
        request.products_on_last_page,
    ).await
    .map_err(|e| format!("Failed to calculate crawling range: {}", e))?;

    // Get progress information
    let progress = range_calculator.analyze_crawling_progress(
        request.total_pages_on_site,
        request.products_on_last_page,
    ).await
    .map_err(|e| format!("Failed to analyze progress: {}", e))?;

    let response = match result {
        Some((start_page, end_page)) => {
            let message = format!("Next crawling range: pages {} to {} (total: {} pages)", 
                                start_page, end_page, start_page - end_page + 1);
            info!("✅ {}", message);
            
            CrawlingRangeResponse {
                success: true,
                range: Some((start_page, end_page)),
                progress: convert_progress(&progress),
                message,
            }
        }
        None => {
            let message = "All products have been crawled - no more pages to process".to_string();
            info!("🏁 {}", message);
            
            CrawlingRangeResponse {
                success: true,
                range: None,
                progress: convert_progress(&progress),
                message,
            }
        }
    };

    Ok(response)
}

/// Get current crawling progress
#[tauri::command]
pub async fn get_crawling_progress(
    _state: State<'_, AppState>,
    total_pages_on_site: u32,
    products_on_last_page: u32,
) -> Result<CrawlingProgressInfo, String> {
    info!("📊 Getting crawling progress information");

    // Get configuration
    let config_manager = ConfigManager::new()
        .map_err(|e| format!("Failed to initialize config manager: {}", e))?;
    
    let config = config_manager.load_config().await
        .map_err(|e| format!("Failed to get config: {}", e))?;

    // Create product repository
    let product_repo = create_product_repo().await?;

    // Create range calculator
    let range_calculator = CrawlingRangeCalculator::new(
        Arc::new(product_repo),
        config,
    );

    // Analyze progress
    let progress = range_calculator.analyze_crawling_progress(
        total_pages_on_site,
        products_on_last_page,
    ).await
    .map_err(|e| format!("Failed to analyze progress: {}", e))?;

    Ok(convert_progress(&progress))
}

/// Get database state for range calculation
#[tauri::command]
pub async fn get_database_state_for_range_calculation(
    _state: State<'_, AppState>,
) -> Result<DatabaseStateInfo, String> {
    info!("📊 Getting database state for range calculation");

    // Create product repository
    let product_repo = create_product_repo().await?;

    // Get max pageId and indexInPage
    let (max_page_id, max_index_in_page) = product_repo.get_max_page_id_and_index().await
        .map_err(|e| format!("Failed to get max page ID and index: {}", e))?;

    // Get total product count
    let total_products = product_repo.get_product_count().await
        .map_err(|e| format!("Failed to get product count: {}", e))?;

    let info = DatabaseStateInfo {
        max_page_id,
        max_index_in_page,
        total_products: total_products as u32,
        has_data: max_page_id.is_some() && max_index_in_page.is_some(),
    };

    info!("✅ Database state: max_page_id={:?}, max_index_in_page={:?}, total_products={}", 
          info.max_page_id, info.max_index_in_page, info.total_products);

    Ok(info)
}

/// Database state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStateInfo {
    pub max_page_id: Option<i32>,
    pub max_index_in_page: Option<i32>,
    pub total_products: u32,
    pub has_data: bool,
}

/// Demo function to show the prompts6 example calculation
#[tauri::command]
pub async fn demo_prompts6_calculation() -> Result<String, String> {
    info!("🎯 Running prompts6 example calculation demo");

    // Example data from prompts6
    let max_page_id = 10i32;
    let max_index_in_page = 6i32;
    let total_pages_on_site = 481u32;
    let products_on_last_page = 10u32;
    let crawl_page_limit = 10u32;
    let products_per_page = 12u32;

    let mut result = String::new();
    result.push_str("📊 prompts6 Example Calculation Demo\n\n");
    result.push_str(&format!("Input data:\n"));
    result.push_str(&format!("  max_page_id: {}\n", max_page_id));
    result.push_str(&format!("  max_index_in_page: {}\n", max_index_in_page));
    result.push_str(&format!("  total_pages_on_site: {}\n", total_pages_on_site));
    result.push_str(&format!("  products_on_last_page: {}\n", products_on_last_page));
    result.push_str(&format!("  crawl_page_limit: {}\n", crawl_page_limit));
    result.push_str(&format!("  products_per_page: {}\n\n", products_per_page));

    // Step 1: Calculate last saved index
    let last_saved_index = (max_page_id as u32 * products_per_page) + max_index_in_page as u32;
    result.push_str(&format!("Step 1: lastSavedIndex = ({} * {}) + {} = {}\n", 
                           max_page_id, products_per_page, max_index_in_page, last_saved_index));

    // Step 2: Calculate next product index
    let next_product_index = last_saved_index + 1;
    result.push_str(&format!("Step 2: nextProductIndex = {} + 1 = {}\n", 
                           last_saved_index, next_product_index));

    // Step 3: Calculate total products
    let total_products = ((total_pages_on_site - 1) * products_per_page) + products_on_last_page;
    result.push_str(&format!("Step 3: totalProducts = (({} - 1) * {}) + {} = {}\n", 
                           total_pages_on_site, products_per_page, products_on_last_page, total_products));

    // Step 4: Convert to forward index
    let forward_index = (total_products - 1) - next_product_index;
    result.push_str(&format!("Step 4: forwardIndex = ({} - 1) - {} = {}\n", 
                           total_products, next_product_index, forward_index));

    // Step 5: Calculate target page number
    let target_page_number = (forward_index / products_per_page) + 1;
    result.push_str(&format!("Step 5: targetPageNumber = ({} / {}) + 1 = {}\n", 
                           forward_index, products_per_page, target_page_number));

    // Step 6: Apply crawl page limit
    let start_page = target_page_number;
    let end_page = if start_page >= crawl_page_limit {
        start_page - crawl_page_limit + 1
    } else {
        1
    };
    result.push_str(&format!("Step 6: startPage = {}, endPage = {} - {} + 1 = {}\n", 
                           start_page, start_page, crawl_page_limit, end_page));

    result.push_str(&format!("\n✅ Final result: crawl pages {} to {}\n", start_page, end_page));
    result.push_str(&format!("🎯 This matches the prompts6 specification exactly!\n"));

    Ok(result)
}

/// Convert internal progress to API response format
fn convert_progress(progress: &CrawlingProgress) -> CrawlingProgressInfo {
    CrawlingProgressInfo {
        total_products: progress.total_products,
        saved_products: progress.saved_products,
        progress_percentage: progress.progress_percentage,
        max_page_id: progress.max_page_id,
        max_index_in_page: progress.max_index_in_page,
        is_completed: progress.is_completed,
    }
}
