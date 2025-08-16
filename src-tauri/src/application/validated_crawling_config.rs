use crate::infrastructure::config::AppConfig;
use serde::{Deserialize, Serialize};

/// Configuration validation using default values for proposal6.md compliance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedCrawlingConfig {
    pub max_concurrent_requests: u32,
    pub request_delay_ms: u64,
    pub max_retries: u32,
    pub timeout_seconds: u64,
    pub max_pages: Option<u32>,
    pub respect_robots_txt: bool,
    pub user_agent: String,
    pub enable_javascript: bool,
    // Additional fields needed by the infrastructure
    pub batch_size: u32,
    pub max_retry_attempts: u32,
    pub request_timeout_ms: u64,
    pub products_per_page: u32,
    pub page_range_limit: u32,
    pub enable_debug_logging: bool,
    pub list_page_max_concurrent: u32,
    pub product_detail_max_concurrent: u32,
}

impl ValidatedCrawlingConfig {
    /// Create validated configuration from AppConfig using safe defaults
    pub fn from_app_config(app_config: &AppConfig) -> Self {
        Self {
            max_concurrent_requests: app_config
                .user
                .max_concurrent_requests
                .min(10) // Max 10 concurrent requests
                .max(1), // At least 1
            request_delay_ms: app_config.user.request_delay_ms.max(500), // At least 500ms delay
            max_retries: app_config.user.crawling.workers.max_retries.min(5), // Max 5 retries
            timeout_seconds: app_config
                .user
                .crawling
                .timing
                .operation_timeout_seconds
                .min(60) // Max 60 seconds
                .max(5), // At least 5 seconds
            max_pages: Some(app_config.user.crawling.page_range_limit),
            respect_robots_txt: true, // Default to safe behavior
            user_agent: "rMatterCertis/1.0".to_string(), // Default user agent
            enable_javascript: false, // Default to disabled
            // Infrastructure-specific fields
            batch_size: app_config.user.batch.batch_size,
            max_retry_attempts: app_config.user.crawling.workers.max_retries.min(5),
            request_timeout_ms: app_config.user.crawling.timing.operation_timeout_seconds * 1000, // Convert to milliseconds
            products_per_page: 12, // Matters.town has 12 products per page
            page_range_limit: app_config.user.crawling.page_range_limit,
            enable_debug_logging: app_config.user.verbose_logging,
            list_page_max_concurrent: app_config.user.crawling.workers.list_page_max_concurrent
                as u32,
            product_detail_max_concurrent: app_config
                .user
                .crawling
                .workers
                .product_detail_max_concurrent as u32,
        }
    }

    // Add helper methods that infrastructure expects
    pub fn request_delay(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.request_delay_ms)
    }

    pub fn batch_size(&self) -> u32 {
        self.batch_size
    }

    pub fn max_concurrent(&self) -> u32 {
        self.max_concurrent_requests
    }

    pub fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

impl Default for ValidatedCrawlingConfig {
    fn default() -> Self {
        Self {
            max_concurrent_requests: 3,
            request_delay_ms: 1000,
            max_retries: 3,
            timeout_seconds: 30,
            max_pages: Some(100),
            respect_robots_txt: true,
            user_agent: "rMatterCertis/1.0".to_string(),
            enable_javascript: false,
            // Infrastructure defaults
            batch_size: 50,
            max_retry_attempts: 3,
            request_timeout_ms: 30000,        // 30 seconds in milliseconds
            products_per_page: 12,            // Matters.town has 12 products per page
            page_range_limit: 20,             // Default page range limit
            enable_debug_logging: false,      // Default debug logging off
            list_page_max_concurrent: 10,     // Default list page concurrency
            product_detail_max_concurrent: 8, // Default product detail concurrency
        }
    }
}
