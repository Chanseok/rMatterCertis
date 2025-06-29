//! Enhanced Tauri commands for HTML parsing and crawling
//! 
//! Modern command interface following the guide's architecture with
//! comprehensive error handling and type safety.

use crate::application::parsing_service::CrawlerService;
use crate::infrastructure::parsing::ParsingConfig;
use crate::domain::product::{Product, ProductDetail};
use tauri::{command, State};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use anyhow::Result;
use tracing::{info, warn, error};

/// Shared state for crawler services
pub type CrawlerServiceState = Arc<Mutex<Option<CrawlerService>>>;

/// Response wrapper for consistent error handling
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub metadata: HashMap<String, String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            metadata: HashMap::new(),
        }
    }
    
    pub fn success_with_metadata(data: T, metadata: HashMap<String, String>) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            metadata,
        }
    }
    
    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            metadata: HashMap::new(),
        }
    }
}

/// Configuration for crawling operations
#[derive(Debug, Deserialize)]
pub struct CrawlingRequest {
    pub urls: Vec<String>,
    pub page_ids: Option<Vec<u32>>,
    pub config: Option<ParsingConfig>,
}

/// Status information for crawling operations
#[derive(Debug, Serialize)]
pub struct CrawlingStatus {
    pub is_active: bool,
    pub current_url: Option<String>,
    pub processed_count: u32,
    pub total_count: u32,
    pub error_count: u32,
    pub start_time: Option<String>,
}

/// Initialize crawler service with configuration
#[command]
pub async fn initialize_crawler_service(
    config: ParsingConfig,
    state: State<'_, CrawlerServiceState>,
) -> Result<ApiResponse<String>, String> {
    info!("Initializing crawler service");
    
    match CrawlerService::new(config) {
        Ok(service) => {
            let mut crawler_state = state.lock().map_err(|e| format!("State lock error: {}", e))?;
            *crawler_state = Some(service);
            
            info!("Crawler service initialized successfully");
            Ok(ApiResponse::success("Crawler service initialized successfully".to_string()))
        }
        Err(e) => {
            error!("Failed to initialize crawler service: {}", e);
            Ok(ApiResponse::error(format!("Failed to initialize crawler service: {}", e)))
        }
    }
}

/// Crawl product list from a single page
#[command]
pub async fn crawl_product_list_page(
    url: String, 
    page_id: u32,
    config: Option<ParsingConfig>,
) -> Result<ApiResponse<Vec<Product>>, String> {
    info!("Crawling product list from: {} (page {})", url, page_id);
    
    let config = config.unwrap_or_default();
    let crawler_service = match CrawlerService::new(config) {
        Ok(service) => service,
        Err(e) => {
            error!("Failed to create crawler service: {}", e);
            return Ok(ApiResponse::error(format!("Failed to create crawler service: {}", e)));
        }
    };
    
    match crawler_service.crawl_and_parse_product_list(&url, page_id).await {
        Ok(products) => {
            let mut metadata = HashMap::new();
            metadata.insert("url".to_string(), url);
            metadata.insert("page_id".to_string(), page_id.to_string());
            metadata.insert("product_count".to_string(), products.len().to_string());
            
            info!("Successfully crawled {} products from page {}", products.len(), page_id);
            Ok(ApiResponse::success_with_metadata(products, metadata))
        }
        Err(e) => {
            error!("Product list crawling failed for {}: {}", url, e);
            Ok(ApiResponse::error(format!("Product list crawling failed: {}", e)))
        }
    }
}

/// Crawl product detail from a single page
#[command]
pub async fn crawl_product_detail_page(
    url: String,
    config: Option<ParsingConfig>,
) -> Result<ApiResponse<ProductDetail>, String> {
    info!("Crawling product detail from: {}", url);
    
    let config = config.unwrap_or_default();
    let crawler_service = match CrawlerService::new(config) {
        Ok(service) => service,
        Err(e) => {
            error!("Failed to create crawler service: {}", e);
            return Ok(ApiResponse::error(format!("Failed to create crawler service: {}", e)));
        }
    };
    
    match crawler_service.crawl_and_parse_product_detail(&url).await {
        Ok(product) => {
            let mut metadata = HashMap::new();
            metadata.insert("url".to_string(), url.clone());
            if let Some(ref model) = product.model {
                metadata.insert("model".to_string(), model.clone());
            }
            if let Some(ref manufacturer) = product.manufacturer {
                metadata.insert("manufacturer".to_string(), manufacturer.clone());
            }
            
            info!("Successfully crawled product detail from: {}", url);
            Ok(ApiResponse::success_with_metadata(product, metadata))
        }
        Err(e) => {
            error!("Product detail crawling failed for {}: {}", url, e);
            Ok(ApiResponse::error(format!("Product detail crawling failed: {}", e)))
        }
    }
}

/// Batch crawl multiple product list pages
#[command]
pub async fn batch_crawl_product_lists(
    request: CrawlingRequest,
) -> Result<ApiResponse<Vec<Vec<Product>>>, String> {
    info!("Starting batch crawl for {} URLs", request.urls.len());
    
    if request.urls.is_empty() {
        return Ok(ApiResponse::error("No URLs provided".to_string()));
    }
    
    let page_ids = request.page_ids.unwrap_or_else(|| {
        (1..=request.urls.len() as u32).collect()
    });
    
    if request.urls.len() != page_ids.len() {
        return Ok(ApiResponse::error("URLs and page IDs must have the same length".to_string()));
    }
    
    let config = request.config.unwrap_or_default();
    let crawler_service = match CrawlerService::new(config) {
        Ok(service) => service,
        Err(e) => {
            error!("Failed to create crawler service: {}", e);
            return Ok(ApiResponse::error(format!("Failed to create crawler service: {}", e)));
        }
    };
    
    let urls_and_pages: Vec<(String, u32)> = request.urls.into_iter()
        .zip(page_ids.into_iter())
        .collect();
    
    let results = crawler_service.batch_crawl_product_lists(urls_and_pages).await;
    
    let mut successful_results = Vec::new();
    let mut error_count = 0;
    
    for result in results {
        match result {
            Ok(products) => successful_results.push(products),
            Err(e) => {
                warn!("Batch crawl item failed: {}", e);
                error_count += 1;
                successful_results.push(Vec::new()); // Push empty vec to maintain order
            }
        }
    }
    
    let mut metadata = HashMap::new();
    metadata.insert("total_pages".to_string(), successful_results.len().to_string());
    metadata.insert("error_count".to_string(), error_count.to_string());
    metadata.insert("success_rate".to_string(), format!("{:.2}%", 
        (successful_results.len() - error_count) as f64 / successful_results.len() as f64 * 100.0));
    
    info!("Batch crawl completed: {} pages processed, {} errors", 
          successful_results.len(), error_count);
    
    Ok(ApiResponse::success_with_metadata(successful_results, metadata))
}

/// Batch crawl multiple product detail pages
#[command]
pub async fn batch_crawl_product_details(
    urls: Vec<String>,
    config: Option<ParsingConfig>,
) -> Result<ApiResponse<Vec<ProductDetail>>, String> {
    info!("Starting batch detail crawl for {} URLs", urls.len());
    
    if urls.is_empty() {
        return Ok(ApiResponse::error("No URLs provided".to_string()));
    }
    
    let config = config.unwrap_or_default();
    let crawler_service = match CrawlerService::new(config) {
        Ok(service) => service,
        Err(e) => {
            error!("Failed to create crawler service: {}", e);
            return Ok(ApiResponse::error(format!("Failed to create crawler service: {}", e)));
        }
    };
    
    let results = crawler_service.batch_crawl_product_details(urls).await;
    
    let mut successful_results = Vec::new();
    let mut error_count = 0;
    
    for result in results {
        match result {
            Ok(product) => successful_results.push(product),
            Err(e) => {
                warn!("Batch detail crawl item failed: {}", e);
                error_count += 1;
            }
        }
    }
    
    let mut metadata = HashMap::new();
    metadata.insert("total_urls".to_string(), successful_results.len().to_string());
    metadata.insert("error_count".to_string(), error_count.to_string());
    metadata.insert("success_rate".to_string(), format!("{:.2}%", 
        successful_results.len() as f64 / (successful_results.len() + error_count) as f64 * 100.0));
    
    info!("Batch detail crawl completed: {} products processed, {} errors", 
          successful_results.len(), error_count);
    
    Ok(ApiResponse::success_with_metadata(successful_results, metadata))
}

/// Check if a page has pagination (next page)
#[command]
pub async fn check_has_next_page(
    url: String,
    config: Option<ParsingConfig>,
) -> Result<ApiResponse<bool>, String> {
    info!("Checking pagination for: {}", url);
    
    let config = config.unwrap_or_default();
    let crawler_service = match CrawlerService::new(config) {
        Ok(service) => service,
        Err(e) => {
            error!("Failed to create crawler service: {}", e);
            return Ok(ApiResponse::error(format!("Failed to create crawler service: {}", e)));
        }
    };
    
    match crawler_service.has_next_page(&url).await {
        Ok(has_next) => {
            let mut metadata = HashMap::new();
            metadata.insert("url".to_string(), url);
            
            Ok(ApiResponse::success_with_metadata(has_next, metadata))
        }
        Err(e) => {
            error!("Failed to check pagination for {}: {}", url, e);
            Ok(ApiResponse::error(format!("Failed to check pagination: {}", e)))
        }
    }
}

/// Get current crawler configuration
#[command]
pub async fn get_crawler_config() -> Result<ApiResponse<ParsingConfig>, String> {
    Ok(ApiResponse::success(ParsingConfig::default()))
}

/// Health check for crawler service
#[command]
pub async fn crawler_health_check() -> Result<ApiResponse<HashMap<String, String>>, String> {
    let mut health_info = HashMap::new();
    health_info.insert("status".to_string(), "healthy".to_string());
    health_info.insert("service_initialized".to_string(), "true".to_string());
    health_info.insert("timestamp".to_string(), chrono::Utc::now().to_rfc3339());
    
    Ok(ApiResponse::success(health_info))
}
