//! # Domain Entities
//!
//! Pure business objects representing the core concepts of the crawling domain.
//! These entities have no external dependencies and contain only business logic.
//!
//! Following Clean Architecture principles, these entities are:
//! - Independent of frameworks, databases, and external systems
//! - Focused on business rules and logic
//! - Highly testable and maintainable

#![allow(missing_docs)]
#![allow(clippy::unnecessary_operation)]
#![allow(unused_must_use)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for domain entities
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId(Uuid);

impl EntityId {
    /// Creates a new unique entity ID
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns the inner UUID
    #[must_use]
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for EntityId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Crawling session entity for tracking crawling operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingSession {
    pub id: String,
    pub url: String,
    pub start_page: u32,
    pub end_page: u32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub total_pages: Option<u32>,
    pub processed_pages: u32,
    pub success_count: u32,
    pub error_count: u32,
    pub error_messages: Vec<String>,
}

impl Default for CrawlingSession {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            url: String::new(),
            start_page: 1,
            end_page: 1,
            status: "created".to_string(),
            created_at: Utc::now(),
            updated_at: None,
            completed_at: None,
            total_pages: None,
            processed_pages: 0,
            success_count: 0,
            error_count: 0,
            error_messages: Vec::new(),
        }
    }
}

/// Basic product information (Stage 1 collection result)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ts_rs::TS)]
#[ts(export)]
pub struct Product {
    pub url: String,                    // Product detail page URL (Primary Key)
    pub manufacturer: Option<String>,   // Manufacturer name
    pub model: Option<String>,          // Model name
    pub certificate_id: Option<String>, // Certificate ID
    pub page_id: Option<i32>,           // Collected page number
    pub index_in_page: Option<i32>,     // Order within page
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
    pub vid: Option<String>, // Vendor ID
    pub pid: Option<String>, // Product ID
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
    pub id: String,
    pub vendor_number: u32,
    pub vendor_name: String,
    pub company_legal_name: String,
    pub vendor_url: Option<String>,
    pub csa_assigned_number: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
    pub request_delay: Option<u64>,   // Delay between requests (ms)
    pub request_timeout: Option<u64>, // Request timeout (ms)

    // Batch processing settings
    pub enable_batch_processing: Option<bool>,
    pub batch_size: Option<u32>,        // Pages per batch (default 30)
    pub batch_delay_ms: Option<u64>,    // Delay between batches (default 2000ms)
    pub batch_retry_limit: Option<u32>, // Batch retry limit

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
pub struct CrawlingProgressData {
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
            products_per_page: crate::infrastructure::config::defaults::DEFAULT_PRODUCTS_PER_PAGE,
            auto_add_to_local_db: false,
            auto_status_check: true,
            headless_browser: Some(true),
            crawler_type: Some(CrawlerType::Reqwest),
            user_agent: Some("matter-certis-v2/1.0".to_string()),
            max_concurrent_tasks: Some(10),
            request_delay: Some(1000),
            request_timeout: Some(30000),
            enable_batch_processing: Some(true),
            batch_size: Some(30),
            batch_delay_ms: Some(2000),
            batch_retry_limit: Some(3),
            base_url: Some({
                use crate::infrastructure::config::csa_iot;
                csa_iot::BASE_URL.to_string()
            }),
            matter_filter_url: None,
        }
    }
}
