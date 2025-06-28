//! Domain entities for Matter Certification crawling
//! 
//! Contains the core business entities specific to CSA-IoT Matter Certification database.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Basic product information (Stage 1 collection result)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub url: String,                    // Product detail page URL (Primary Key)
    pub manufacturer: Option<String>,   // Manufacturer name
    pub model: Option<String>,          // Model name
    pub certificate_id: Option<String>, // Certificate ID
    pub page_id: Option<u32>,          // Collected page number
    pub index_in_page: Option<u32>,    // Order within page
    pub created_at: DateTime<Utc>,
}

/// Complete Matter product information (Stage 2 collection result)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatterProduct {
    // Basic Product fields
    pub url: String,
    pub page_id: Option<u32>,
    pub index_in_page: Option<u32>,
    pub id: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    
    // Detailed Matter certification fields
    pub device_type: Option<String>,
    pub certificate_id: Option<String>,
    pub certification_date: Option<String>,
    pub software_version: Option<String>,
    pub hardware_version: Option<String>,
    pub vid: Option<String>,              // Vendor ID
    pub pid: Option<String>,              // Product ID
    pub family_sku: Option<String>,
    pub family_variant_sku: Option<String>,
    pub firmware_version: Option<String>,
    pub family_id: Option<String>,
    pub tis_trp_tested: Option<String>,
    pub specification_version: Option<String>,
    pub transport_interface: Option<String>,
    pub primary_device_type_id: Option<String>,
    pub application_categories: Vec<String>, // JSON array as Vec
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Vendor information from Matter certification database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vendor {
    pub vendor_id: String,
    pub vendor_number: u32,
    pub vendor_name: String,
    pub company_legal_name: String,
    pub created_at: DateTime<Utc>,
}

/// Crawler configuration for Matter certification site
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlerConfig {
    // Core settings
    pub page_range_limit: u32,           // Maximum pages to crawl
    pub product_list_retry_count: u32,   // Retry count for list collection
    pub product_detail_retry_count: u32, // Retry count for detail collection
    pub products_per_page: u32,          // Products per page (default 12)
    pub auto_add_to_local_db: bool,      // Auto save to DB
    pub auto_status_check: bool,         // Auto status check
    
    // Browser settings
    pub headless_browser: Option<bool>,
    pub crawler_type: Option<CrawlerType>, // "reqwest" or "playwright"
    pub user_agent: Option<String>,
    
    // Performance settings
    pub max_concurrent_tasks: Option<u32>,
    pub request_delay: Option<u64>,       // Delay between requests (ms)
    pub request_timeout: Option<u64>,     // Request timeout (ms)
    
    // Batch processing settings
    pub enable_batch_processing: Option<bool>,
    pub batch_size: Option<u32>,          // Pages per batch (default 30)
    pub batch_delay_ms: Option<u64>,      // Delay between batches (default 2000ms)
    pub batch_retry_limit: Option<u32>,   // Batch retry limit
    
    // URL settings
    pub base_url: Option<String>,
    pub matter_filter_url: Option<String>, // Matter filter applied URL
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrawlerType {
    Reqwest,
    Playwright,
}

/// Progress tracking for crawling operations (UI display only)
/// Note: Actual session state is managed in memory by SessionManager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingProgress {
    pub current: u32,
    pub total: u32,
    pub percentage: f32,
    pub current_step: String,
    pub elapsed_time_secs: u64,
    pub remaining_time_secs: Option<u64>,
    pub message: Option<String>,
    // Batch processing info
    pub current_batch: Option<u32>,
    pub total_batches: Option<u32>,
    // Error info
    pub retry_count: u32,
    pub failed_items: u32,
}

/// Validation result for duplicate checking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub new_products: Vec<Product>,
    pub existing_products: Vec<Product>,
    pub duplicate_products: Vec<Product>,
    pub summary: ValidationSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSummary {
    pub total_products: usize,
    pub new_products: usize,
    pub existing_products: usize,
    pub duplicate_products: usize,
    pub validation_time_ms: u64,
}

/// Database summary statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSummary {
    pub total_products: u32,
    pub total_matter_products: u32,
    pub total_vendors: u32,
    pub last_crawling_date: Option<DateTime<Utc>>,
    pub database_size_mb: f64,
}

impl Default for CrawlerConfig {
    fn default() -> Self {
        Self {
            page_range_limit: 10,
            product_list_retry_count: 9,
            product_detail_retry_count: 9,
            products_per_page: 12,
            auto_add_to_local_db: false,
            auto_status_check: true,
            headless_browser: Some(true),
            crawler_type: Some(CrawlerType::Reqwest),
            user_agent: Some("rMatterCertis/1.0".to_string()),
            max_concurrent_tasks: Some(10),
            request_delay: Some(1000),
            request_timeout: Some(30000),
            enable_batch_processing: Some(true),
            batch_size: Some(30),
            batch_delay_ms: Some(2000),
            batch_retry_limit: Some(3),
            base_url: Some("https://csa-iot.org".to_string()),
            matter_filter_url: None,
        }
    }
}
