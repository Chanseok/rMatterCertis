//! Domain entities
//! 
//! Contains the core business entities and their logic.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: String,
    pub name: String,
    pub price: Option<f64>,
    pub currency: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub product_url: String,
    pub vendor_id: String,
    pub category: Option<String>,
    pub in_stock: bool,
    pub collected_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vendor {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub crawling_config: CrawlingConfig,
    pub is_active: bool,
    pub last_crawled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingConfig {
    pub max_concurrent_requests: u32,
    pub delay_between_requests: u64, // milliseconds
    pub user_agent: String,
    pub selectors: ProductSelectors,
    pub pagination: PaginationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductSelectors {
    pub name: String,
    pub price: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub product_url: String,
    pub in_stock: Option<String>,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationConfig {
    pub next_page_selector: Option<String>,
    pub page_url_pattern: Option<String>,
    pub max_pages: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingSession {
    pub id: String,
    pub vendor_id: String,
    pub status: CrawlingStatus,
    pub total_pages: Option<u32>,
    pub processed_pages: u32,
    pub products_found: u32,
    pub errors_count: u32,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrawlingStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl Default for CrawlingConfig {
    fn default() -> Self {
        Self {
            max_concurrent_requests: 10,
            delay_between_requests: 1000,
            user_agent: "rMatterCertis/1.0".to_string(),
            selectors: ProductSelectors::default(),
            pagination: PaginationConfig::default(),
        }
    }
}

impl Default for ProductSelectors {
    fn default() -> Self {
        Self {
            name: ".product-name".to_string(),
            price: ".product-price".to_string(),
            description: None,
            image_url: None,
            product_url: ".product-link".to_string(),
            in_stock: None,
            category: None,
        }
    }
}

impl Default for PaginationConfig {
    fn default() -> Self {
        Self {
            next_page_selector: Some(".next-page".to_string()),
            page_url_pattern: None,
            max_pages: Some(100),
        }
    }
}
