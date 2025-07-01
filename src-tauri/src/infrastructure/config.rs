//! Configuration infrastructure
//! 
//! Contains configuration loading and management for CSA-IoT crawling.
//! 
//! Configuration is organized into three tiers:
//! 1. User-configurable settings (exposed in UI)
//! 2. Hidden/Advanced settings (in config file only)
//! 3. Application-managed settings (auto-updated by app)

#![allow(clippy::uninlined_format_args)]
#![allow(clippy::derivable_impls)]
#![allow(clippy::useless_format)]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::{Result, Context};
use tokio::fs;
use tracing::info;

/// Complete application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// User-configurable settings (exposed in UI)
    pub user: UserConfig,
    
    /// Hidden/Advanced settings (config file only)
    pub advanced: AdvancedConfig,
    
    /// Application-managed settings (auto-updated)
    pub app_managed: AppManagedConfig,
}

/// User-configurable settings that can be changed from the UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    /// Maximum pages to crawl in a single session
    pub max_pages: u32,
    
    /// Delay between requests in milliseconds
    pub request_delay_ms: u64,
    
    /// Maximum concurrent requests
    pub max_concurrent_requests: u32,
    
    /// Enable verbose logging
    pub verbose_logging: bool,
    
    /// Logging configuration
    pub logging: LoggingConfig,
}

/// Logging configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level: "error", "warn", "info", "debug", "trace"
    pub level: String,
    
    /// Enable JSON formatted logs
    pub json_format: bool,
    
    /// Enable console output
    pub console_output: bool,
    
    /// Enable file output
    pub file_output: bool,
    
    /// Maximum log file size in MB (for rotation)
    pub max_file_size_mb: u64,
    
    /// Number of log files to keep
    pub max_files: u32,
}

/// Hidden/Advanced settings that are in config file but not exposed in UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedConfig {
    /// Starting page number for last page search (e.g., 470)
    pub last_page_search_start: u32,
    
    /// Maximum attempts when searching for last page
    pub max_search_attempts: u32,
    
    /// Retry attempts for failed requests
    pub retry_attempts: u32,
    
    /// Retry delay in milliseconds
    pub retry_delay_ms: u64,
    
    /// CSS selectors for finding products
    pub product_selectors: Vec<String>,
    
    /// Timeout for HTTP requests in seconds
    pub request_timeout_seconds: u64,
}

/// Application-managed settings that are automatically updated by the app
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppManagedConfig {
    /// Last known maximum page number
    pub last_known_max_page: Option<u32>,
    
    /// Timestamp of last successful crawl
    pub last_successful_crawl: Option<String>,
    
    /// Total products found in last crawl
    pub last_crawl_product_count: Option<u32>,
    
    /// Average products per page from recent crawls
    pub avg_products_per_page: Option<f64>,
    
    /// Configuration version for migration purposes
    pub config_version: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            user: UserConfig::default(),
            advanced: AdvancedConfig::default(),
            app_managed: AppManagedConfig::default(),
        }
    }
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            max_pages: defaults::MAX_PAGES,
            request_delay_ms: defaults::REQUEST_DELAY_MS,
            max_concurrent_requests: defaults::MAX_CONCURRENT_REQUESTS,
            verbose_logging: false,
            logging: LoggingConfig::default(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: defaults::LOG_LEVEL.to_string(),
            json_format: defaults::LOG_JSON_FORMAT,
            console_output: defaults::LOG_CONSOLE_OUTPUT,
            file_output: defaults::LOG_FILE_OUTPUT,
            max_file_size_mb: defaults::LOG_MAX_FILE_SIZE_MB,
            max_files: defaults::LOG_MAX_FILES,
        }
    }
}

impl Default for AdvancedConfig {
    fn default() -> Self {
        Self {
            last_page_search_start: defaults::LAST_PAGE_SEARCH_START,
            max_search_attempts: defaults::MAX_SEARCH_ATTEMPTS,
            retry_attempts: defaults::RETRY_ATTEMPTS,
            retry_delay_ms: defaults::RETRY_DELAY_MS,
            product_selectors: defaults::PRODUCT_SELECTORS.iter().map(|s| s.to_string()).collect(),
            request_timeout_seconds: defaults::REQUEST_TIMEOUT_SECONDS,
        }
    }
}

impl Default for AppManagedConfig {
    fn default() -> Self {
        Self {
            last_known_max_page: None,
            last_successful_crawl: None,
            last_crawl_product_count: None,
            avg_products_per_page: None,
            config_version: 1,
        }
    }
}

/// Configuration manager for loading and saving settings
pub struct ConfigManager {
    config_path: PathBuf,
}

impl ConfigManager {
    /// Create a new configuration manager
    pub fn new() -> Result<Self> {
        let config_dir = Self::get_config_dir()?;
        let config_path = config_dir.join("matter_certis_config.json");
        
        Ok(Self { config_path })
    }
    
    /// Get the application configuration directory
    fn get_config_dir() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Failed to get user config directory")?
            .join("matter-certis-v2");
        
        Ok(config_dir)
    }
    
    /// Load configuration from file, creating default if it doesn't exist
    pub async fn load_config(&self) -> Result<AppConfig> {
        if !self.config_path.exists() {
            info!("Configuration file not found, creating default: {:?}", self.config_path);
            let default_config = AppConfig::default();
            self.save_config(&default_config).await?;
            return Ok(default_config);
        }
        
        let content = fs::read_to_string(&self.config_path).await
            .context("Failed to read configuration file")?;
        
        let config: AppConfig = serde_json::from_str(&content)
            .context("Failed to parse configuration file")?;
        
        info!("Loaded configuration from: {:?}", self.config_path);
        Ok(config)
    }
    
    /// Save configuration to file
    pub async fn save_config(&self, config: &AppConfig) -> Result<()> {
        // Ensure config directory exists
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent).await
                .context("Failed to create config directory")?;
        }
        
        let content = serde_json::to_string_pretty(config)
            .context("Failed to serialize configuration")?;
        
        fs::write(&self.config_path, content).await
            .context("Failed to write configuration file")?;
        
        info!("Saved configuration to: {:?}", self.config_path);
        Ok(())
    }
    
    /// Update app-managed settings (like last known max page)
    pub async fn update_app_managed<F>(&self, updater: F) -> Result<()>
    where
        F: FnOnce(&mut AppManagedConfig),
    {
        let mut config = self.load_config().await?;
        updater(&mut config.app_managed);
        self.save_config(&config).await
    }
    
    /// Get the configuration file path
    pub fn config_path(&self) -> &PathBuf {
        &self.config_path
    }
}

/// CSA-IoT website URLs and crawling configuration constants
pub mod csa_iot {
    /// Base URL for CSA-IoT website
    pub const BASE_URL: &str = "https://csa-iot.org";
    
    /// General products page (includes all product types - Matter, Zigbee, etc.)
    pub const PRODUCTS_PAGE_GENERAL: &str = "https://csa-iot.org/csa-iot_products/";
    
    /// Matter products only - filtered URL with specific parameters
    /// Parameters explanation:
    /// - p_type[]=14: Matter product type filter
    /// - p_program_type[]=1049: Matter program type filter
    /// - Other parameters are left empty for maximum coverage
    pub const PRODUCTS_PAGE_MATTER_ONLY: &str = "https://csa-iot.org/csa-iot_products/?p_keywords=&p_type%5B%5D=14&p_program_type%5B%5D=1049&p_certificate=&p_family=&p_firmware_ver=";
    
    /// URL pattern for Matter products with page pagination
    /// Use with format!() macro: format!(PRODUCTS_PAGE_MATTER_PAGINATED, page_number)
    pub const PRODUCTS_PAGE_MATTER_PAGINATED: &str = "https://csa-iot.org/csa-iot_products/page/{}/?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver";
    
    /// Filter parameters for Matter products
    pub mod filters {
        /// Matter product type ID
        pub const MATTER_PRODUCT_TYPE: &str = "14";
        
        /// Matter program type ID
        pub const MATTER_PROGRAM_TYPE: &str = "1049";
        
        /// Query parameter for product type filter
        pub const PARAM_TYPE: &str = "p_type[]";
        
        /// Query parameter for program type filter
        pub const PARAM_PROGRAM_TYPE: &str = "p_program_type[]";
        
        /// Query parameter for keywords
        pub const PARAM_KEYWORDS: &str = "p_keywords";
        
        /// Query parameter for certificate ID
        pub const PARAM_CERTIFICATE: &str = "p_certificate";
        
        /// Query parameter for product family
        pub const PARAM_FAMILY: &str = "p_family";
        
        /// Query parameter for firmware version
        pub const PARAM_FIRMWARE_VER: &str = "p_firmware_ver";
    }
}

/// Default crawling configuration values
pub mod defaults {
    /// Default maximum pages to crawl
    pub const MAX_PAGES: u32 = 10;
    
    /// Default delay between requests in milliseconds
    pub const REQUEST_DELAY_MS: u64 = 1000;
    
    /// Default maximum concurrent requests
    pub const MAX_CONCURRENT_REQUESTS: u32 = 3;
    
    /// Default retry attempts for failed requests
    pub const RETRY_ATTEMPTS: u32 = 3;
    
    /// Default retry delay in milliseconds
    pub const RETRY_DELAY_MS: u64 = 2000;
    
    /// Default starting page number for last page search
    pub const LAST_PAGE_SEARCH_START: u32 = 470;
    
    /// Default maximum search attempts
    pub const MAX_SEARCH_ATTEMPTS: u32 = 10;
    
    /// Default request timeout in seconds
    pub const REQUEST_TIMEOUT_SECONDS: u64 = 30;
    
    /// Default CSS selectors for finding products
    pub const PRODUCT_SELECTORS: &[&str] = &[
        ".product",
        ".product-item", 
        ".product-card",
        "article.product",
        "[class*='product-']:not([class*='pagination']):not([class*='search'])",
    ];
    
    // Logging defaults
    /// Default log level
    pub const LOG_LEVEL: &str = "info";
    
    /// Default JSON format setting
    pub const LOG_JSON_FORMAT: bool = false;
    
    /// Default console output setting
    pub const LOG_CONSOLE_OUTPUT: bool = true;
    
    /// Default file output setting
    pub const LOG_FILE_OUTPUT: bool = true;
    
    /// Default maximum log file size in MB
    pub const LOG_MAX_FILE_SIZE_MB: u64 = 10;
    
    /// Default number of log files to keep
    pub const LOG_MAX_FILES: u32 = 5;
}

/// URL building helper functions
pub mod utils {
    use super::csa_iot::*;
    
    /// Build a Matter products URL for a specific page number
    pub fn matter_products_page_url(page: u32) -> String {
        if page <= 1 {
            PRODUCTS_PAGE_MATTER_ONLY.to_string()
        } else {
            format!("{}", PRODUCTS_PAGE_MATTER_PAGINATED.replace("{}", &page.to_string()))
        }
    }
    
    /// Resolve a relative URL to an absolute URL using the base URL
    pub fn resolve_url(relative_url: &str) -> String {
        if relative_url.starts_with("http://") || relative_url.starts_with("https://") {
            relative_url.to_string()
        } else if relative_url.starts_with('/') {
            format!("{}{}", BASE_URL, relative_url)
        } else {
            format!("{}/{}", BASE_URL, relative_url)
        }
    }
}
