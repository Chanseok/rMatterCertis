//! Configuration infrastructure
//! 
//! Contains configuration loading and management for CSA-IoT crawling.
//! 
//! Configuration is organized into three tiers:
//! 1. User-configurable settings (exposed in UI)
//! 2. Hidden/Advanced settings (in config file only)
//! 3. Application-managed settings (auto-updated by app)

#![allow(clippy::uninlined_format_args)]
#![allow(missing_docs)]
#![allow(clippy::unnecessary_qualification)]
#![allow(unused_must_use)]
#![allow(clippy::derivable_impls)]
#![allow(clippy::useless_format)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    
    /// Batch processing configuration
    pub batch: BatchConfig,
    
    /// Crawling specific configuration
    pub crawling: CrawlingConfig,
}

/// Crawling specific configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingConfig {
    /// Maximum pages to process (페이지 범위 제한)
    pub page_range_limit: u32,
    
    /// Retry count for product list pages (제품 목록 재시도 횟수)
    pub product_list_retry_count: u32,
    
    /// Retry count for product detail pages (제품 상세 재시도 횟수)
    pub product_detail_retry_count: u32,
    
    /// Automatically add crawled products to local database (자동으로 로컬 DB에 추가)
    pub auto_add_to_local_db: bool,
    
    /// Worker configuration
    pub workers: WorkerConfig,
    
    /// Timing configuration
    pub timing: TimingConfig,
}

/// Worker configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    /// Maximum concurrent requests for list page fetcher
    pub list_page_max_concurrent: usize,
    
    /// Maximum concurrent requests for product detail fetcher
    pub product_detail_max_concurrent: usize,
    
    /// Request timeout in seconds
    pub request_timeout_seconds: u64,
    
    /// Maximum retry attempts
    pub max_retries: u32,
    
    /// Batch size for database operations
    pub db_batch_size: usize,
    
    /// Maximum concurrency for database operations
    pub db_max_concurrency: usize,
}

/// Timing configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingConfig {
    /// Scheduler interval in milliseconds
    pub scheduler_interval_ms: u64,
    
    /// Shutdown timeout in seconds
    pub shutdown_timeout_seconds: u64,
    
    /// Stats reporting interval in seconds
    pub stats_interval_seconds: u64,
    
    /// Retry delay in milliseconds
    pub retry_delay_ms: u64,
    
    /// Operation timeout in seconds
    pub operation_timeout_seconds: u64,
}

/// Batch processing configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchConfig {
    /// Number of items to process in each batch
    pub batch_size: u32,
    
    /// Delay between batches in milliseconds
    pub batch_delay_ms: u64,
    
    /// Enable batch processing
    pub enable_batch_processing: bool,
    
    /// Maximum retry attempts for failed batches
    pub batch_retry_limit: u32,
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
    
    /// Use separate log files for frontend and backend (true) or unified log file (false)
    pub separate_frontend_backend: bool,
    
    /// Log file naming strategy: "unified", "separated", "timestamped"
    pub file_naming_strategy: String,
    
    /// Maximum log file size in MB (for rotation)
    pub max_file_size_mb: u64,
    
    /// Number of log files to keep (older files will be deleted)
    pub max_files: u32,
    
    /// Enable automatic log cleanup on startup
    pub auto_cleanup_logs: bool,
    
    /// Keep only the most recent log file (delete all others)
    pub keep_only_latest: bool,
    
    /// Module-specific log level filters (e.g., "sqlx": "warn", "reqwest": "info")
    pub module_filters: std::collections::HashMap<String, String>,
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
    
    /// Window state for UI restoration
    pub window_state: Option<String>,
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
            batch: BatchConfig::default(),
            crawling: CrawlingConfig::default(),
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
            separate_frontend_backend: defaults::LOG_SEPARATE_FRONTEND_BACKEND,
            file_naming_strategy: defaults::LOG_FILE_NAMING_STRATEGY.to_string(),
            max_file_size_mb: defaults::LOG_MAX_FILE_SIZE_MB,
            max_files: defaults::LOG_MAX_FILES,
            auto_cleanup_logs: defaults::LOG_AUTO_CLEANUP,
            keep_only_latest: defaults::LOG_KEEP_ONLY_LATEST,
            module_filters: {
                let mut filters = HashMap::new();
                filters.insert("sqlx".to_string(), "warn".to_string());
                filters.insert("reqwest".to_string(), "info".to_string());
                filters.insert("hyper".to_string(), "warn".to_string());
                filters.insert("tokio".to_string(), "info".to_string());
                filters.insert("tauri".to_string(), "info".to_string());
                filters.insert("wry".to_string(), "warn".to_string());
                filters.insert("matter_certis_v2".to_string(), "info".to_string());
                filters
            },
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
            window_state: None,
        }
    }
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            batch_size: defaults::BATCH_SIZE,
            batch_delay_ms: defaults::BATCH_DELAY_MS,
            enable_batch_processing: defaults::ENABLE_BATCH_PROCESSING,
            batch_retry_limit: defaults::BATCH_RETRY_LIMIT,
        }
    }
}

impl Default for CrawlingConfig {
    fn default() -> Self {
        Self {
            page_range_limit: defaults::PAGE_RANGE_LIMIT,
            product_list_retry_count: defaults::PRODUCT_LIST_RETRY_COUNT,
            product_detail_retry_count: defaults::PRODUCT_DETAIL_RETRY_COUNT,
            auto_add_to_local_db: defaults::AUTO_ADD_TO_LOCAL_DB,
            workers: WorkerConfig::default(),
            timing: TimingConfig::default(),
        }
    }
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            list_page_max_concurrent: defaults::LIST_PAGE_MAX_CONCURRENT,
            product_detail_max_concurrent: defaults::PRODUCT_DETAIL_MAX_CONCURRENT,
            request_timeout_seconds: defaults::REQUEST_TIMEOUT_SECONDS,
            max_retries: defaults::MAX_RETRIES,
            db_batch_size: defaults::DB_BATCH_SIZE,
            db_max_concurrency: defaults::DB_MAX_CONCURRENCY,
        }
    }
}

impl Default for TimingConfig {
    fn default() -> Self {
        Self {
            scheduler_interval_ms: defaults::SCHEDULER_INTERVAL_MS,
            shutdown_timeout_seconds: defaults::SHUTDOWN_TIMEOUT_SECONDS,
            stats_interval_seconds: defaults::STATS_INTERVAL_SECONDS,
            retry_delay_ms: defaults::WORKER_RETRY_DELAY_MS,
            operation_timeout_seconds: defaults::SHUTDOWN_TIMEOUT_SECONDS, // Use existing timeout
        }
    }
}

/// Configuration manager for loading and saving settings
pub struct ConfigManager {
    pub config_path: PathBuf,
}

impl ConfigManager {
    /// Get the application configuration directory
    pub fn get_config_dir() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Failed to get user config directory")?
            .join("matter-certis-v2");
        
        Ok(config_dir)
    }
    
    /// Create a new configuration manager with automatic setup
    pub fn new() -> Result<Self> {
        let config_dir = Self::get_config_dir()?;
        let config_path = config_dir.join("matter_certis_config.json");
        
        Ok(Self { config_path })
    }
    
    /// Initialize configuration system on first run
    pub async fn initialize_on_first_run(&self) -> Result<AppConfig> {
        let config_dir = self.config_path.parent()
            .context("Failed to get config directory")?;
        
        // Create config directory if it doesn't exist
        if !config_dir.exists() {
            fs::create_dir_all(config_dir).await
                .context("Failed to create config directory")?;
            info!("✅ Created configuration directory: {:?}", config_dir);
        }
        
        // Check if this is a first run
        let is_first_run = !self.config_path.exists();
        
        if is_first_run {
            info!("🎉 First run detected - initializing default configuration");
            
            // Create default configuration
            let default_config = AppConfig::default();
            
            // Save initial configuration
            self.save_config(&default_config).await?;
            
            // Create additional directories
            self.create_data_directories().await?;
            
            info!("✅ Initial configuration setup completed");
            Ok(default_config)
        } else {
            // Load existing configuration
            self.load_config().await
        }
    }
    
    /// Create necessary data directories
    async fn create_data_directories(&self) -> Result<()> {
        let app_data_dir = Self::get_app_data_dir()?;
        
        // Create subdirectories
        let directories = [
            app_data_dir.join("database"),
            app_data_dir.join("logs"),
            app_data_dir.join("exports"),
            app_data_dir.join("backups"),
            app_data_dir.join("cache"),
        ];
        
        for dir in &directories {
            if !dir.exists() {
                fs::create_dir_all(dir).await
                    .with_context(|| format!("Failed to create directory: {:?}", dir))?;
                info!("📁 Created directory: {:?}", dir);
            }
        }
        
        Ok(())
    }
    
    /// Get application data directory
    pub fn get_app_data_dir() -> Result<PathBuf> {
        let data_dir = dirs::data_local_dir()
            .context("Failed to get user data directory")?
            .join("matter-certis-v2");
        
        Ok(data_dir)
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
        
        // Try to parse as current format first
        match serde_json::from_str::<AppConfig>(&content) {
            Ok(config) => {
                info!("Loaded configuration from: {:?}", self.config_path);
                Ok(config)
            }
            Err(parse_error) => {
                // If parsing fails, try to migrate from old format
                info!("Configuration file format outdated, attempting migration...");
                match self.migrate_config_format(&content).await {
                    Ok(migrated_config) => {
                        info!("✅ Successfully migrated configuration");
                        Ok(migrated_config)
                    }
                    Err(migration_error) => {
                        tracing::warn!("⚠️  Configuration migration failed: {}", migration_error);
                        tracing::warn!("⚠️  Original parse error: {}", parse_error);
                        tracing::warn!("⚠️  Resetting to default configuration");
                        
                        // Create backup of corrupted config
                        let backup_path = self.config_path.with_extension("json.corrupted");
                        if let Err(e) = fs::copy(&self.config_path, &backup_path).await {
                            tracing::warn!("Failed to create backup of corrupted config: {}", e);
                        } else {
                            tracing::info!("Backed up corrupted config to: {:?}", backup_path);
                        }
                        
                        // Reset to default configuration
                        let default_config = AppConfig::default();
                        self.save_config(&default_config).await
                            .context("Failed to save default configuration")?;
                        
                        tracing::info!("✅ Reset to default configuration");
                        Ok(default_config)
                    }
                }
            }
        }
    }
    
    /// Migrate configuration from older format to current format
    async fn migrate_config_format(&self, content: &str) -> Result<AppConfig> {
        // Try to parse as a generic JSON value first to check structure
        let mut json_value: serde_json::Value = serde_json::from_str(content)
            .context("Configuration file contains invalid JSON")?;
        
        // Check if we need to add missing fields to user config
        if let Some(user_obj) = json_value.get_mut("user").and_then(|v| v.as_object_mut()) {
            // Add missing crawling config if not present
            if !user_obj.contains_key("crawling") {
                let crawling_config = serde_json::to_value(CrawlingConfig::default())
                    .context("Failed to serialize default crawling config")?;
                user_obj.insert("crawling".to_string(), crawling_config);
                info!("Added missing 'crawling' configuration section");
            }
            
            // Ensure batch config exists (should already be there from previous migration)
            if !user_obj.contains_key("batch") {
                let batch_config = serde_json::to_value(BatchConfig::default())
                    .context("Failed to serialize default batch config")?;
                user_obj.insert("batch".to_string(), batch_config);
                info!("Added missing 'batch' configuration section");
            }
        }
        
        // Try to parse the migrated JSON as AppConfig
        let migrated_config: AppConfig = serde_json::from_value(json_value)
            .context("Failed to parse migrated configuration")?;
        
        // Save the migrated configuration
        self.save_config(&migrated_config).await
            .context("Failed to save migrated configuration")?;
        
        info!("✅ Successfully migrated configuration to new format");
        Ok(migrated_config)
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
    
    /// Update user configuration settings
    pub async fn update_user_config<F>(&self, updater: F) -> Result<()>
    where
        F: FnOnce(&mut UserConfig),
    {
        let mut config = self.load_config().await?;
        updater(&mut config.user);
        self.save_config(&config).await
    }

    /// Reset configuration to defaults (useful for troubleshooting)
    pub async fn reset_to_defaults(&self) -> Result<AppConfig> {
        info!("🔄 Resetting configuration to defaults");
        
        let default_config = AppConfig::default();
        self.save_config(&default_config).await?;
        
        info!("✅ Configuration reset to defaults");
        Ok(default_config)
    }
    
    /// Migrate configuration from older versions
    pub async fn migrate_config_if_needed(&self, config: &mut AppConfig) -> Result<bool> {
        const CURRENT_VERSION: u32 = 1;
        
        if config.app_managed.config_version < CURRENT_VERSION {
            info!("🔄 Migrating configuration from version {} to {}", 
                  config.app_managed.config_version, CURRENT_VERSION);
            
            // Add migration logic here as needed
            match config.app_managed.config_version {
                0 => {
                    // Migrate from version 0 to 1
                    config.app_managed.config_version = 1;
                    info!("✅ Migrated to version 1");
                }
                _ => {
                    // No migration needed
                }
            }
            
            // Save migrated configuration
            self.save_config(config).await?;
            Ok(true)
        } else {
            Ok(false)
        }
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
    
    /// Base products URL (without query parameters)
    pub const PRODUCTS_BASE: &str = "https://csa-iot.org/csa-iot_products";
    
    /// Fixed query parameters for Matter products filtering
    pub const MATTER_QUERY_PARAMS: &str = "/?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver";
    
    /// General products page (includes all product types - Matter, Zigbee, etc.)
    pub const PRODUCTS_PAGE_GENERAL: &str = "https://csa-iot.org/csa-iot_products/";
    
    /// Matter products only - filtered URL with specific parameters
    /// Parameters explanation:
    /// - p_type[0]=14: Matter product type filter
    /// - p_program_type[0]=1049: Matter program type filter
    /// - Other parameters are left empty for maximum coverage
    pub const PRODUCTS_PAGE_MATTER_ONLY: &str = "https://csa-iot.org/csa-iot_products/?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver";
    
    /// URL pattern for Matter products with page pagination
    /// Format: https://csa-iot.org/csa-iot_products/page/{page_number}/?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver
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
    
    /// Default retry delay in milliseconds (for advanced config)
    pub const RETRY_DELAY_MS: u64 = 2000;
    
    /// Default starting page number for last page search
    /// This is an initial guess - the app will learn and update this value
    pub const LAST_PAGE_SEARCH_START: u32 = 100;
    
    /// Default maximum search attempts
    pub const MAX_SEARCH_ATTEMPTS: u32 = 10;
    
    /// Default request timeout in seconds
    pub const REQUEST_TIMEOUT_SECONDS: u64 = 30;
    
    /// Default number of products per page (based on actual site analysis)
    pub const DEFAULT_PRODUCTS_PER_PAGE: u32 = 12;
    
    // Orchestrator configuration defaults
    /// Default scheduler interval in milliseconds
    pub const SCHEDULER_INTERVAL_MS: u64 = 100;
    
    /// Default shutdown timeout in seconds
    pub const SHUTDOWN_TIMEOUT_SECONDS: u64 = 30;
    
    /// Default stats interval in seconds
    pub const STATS_INTERVAL_SECONDS: u64 = 10;
    
    /// Default maximum retries for orchestrator
    pub const MAX_RETRIES: u32 = 3;
    
    /// Default worker retry delay in milliseconds
    pub const WORKER_RETRY_DELAY_MS: u64 = 1000;
    
    /// Default backpressure threshold
    pub const BACKPRESSURE_THRESHOLD: usize = 1000;
    
    // Worker configuration defaults
    /// Default maximum concurrent requests for list page fetcher
    pub const LIST_PAGE_MAX_CONCURRENT: usize = 5;
    
    /// Default maximum concurrent requests for product detail fetcher  
    pub const PRODUCT_DETAIL_MAX_CONCURRENT: usize = 10;
    
    /// Default batch size for database operations
    pub const DB_BATCH_SIZE: usize = 100;
    
    /// Default maximum concurrency for database operations
    pub const DB_MAX_CONCURRENCY: usize = 10;
    
    // Crawling configuration defaults
    /// Default page range limit
    pub const PAGE_RANGE_LIMIT: u32 = 100;
    
    /// Default product list retry count
    pub const PRODUCT_LIST_RETRY_COUNT: u32 = 3;
    
    /// Default product detail retry count
    pub const PRODUCT_DETAIL_RETRY_COUNT: u32 = 3;
    
    /// Default auto add to local database
    pub const AUTO_ADD_TO_LOCAL_DB: bool = true;
    
    // Batch configuration defaults
    /// Default batch size
    pub const BATCH_SIZE: u32 = 50;
    
    /// Default batch delay in milliseconds
    pub const BATCH_DELAY_MS: u64 = 100;
    
    /// Default enable batch processing
    pub const ENABLE_BATCH_PROCESSING: bool = true;
    
    /// Default batch retry limit
    pub const BATCH_RETRY_LIMIT: u32 = 3;
    
    // Log configuration defaults
    /// Default log level
    pub const LOG_LEVEL: &str = "info";
    
    /// Default JSON format setting
    pub const LOG_JSON_FORMAT: bool = false;
    
    /// Default console output setting
    pub const LOG_CONSOLE_OUTPUT: bool = true;
    
    /// Default file output setting
    pub const LOG_FILE_OUTPUT: bool = true;
    
    /// Default separate frontend/backend logs setting
    pub const LOG_SEPARATE_FRONTEND_BACKEND: bool = false;
    
    /// Default log file naming strategy
    pub const LOG_FILE_NAMING_STRATEGY: &str = "unified";
    
    /// Default maximum log file size in MB
    pub const LOG_MAX_FILE_SIZE_MB: u64 = 10;
    
    /// Default maximum log files to keep
    pub const LOG_MAX_FILES: u32 = 5;
    
    /// Default auto cleanup logs setting
    pub const LOG_AUTO_CLEANUP: bool = true;
    
    /// Default keep only latest setting
    pub const LOG_KEEP_ONLY_LATEST: bool = false;
    
    /// Default CSS selectors for finding products
    pub const PRODUCT_SELECTORS: &[&str] = &[
        "div.post-feed article.type-product",  // 정확한 제품 selector
        "article.type-product",                // 백업 selector
        ".product",                           // 일반적인 제품 selector
    ];
}

/// URL building helper functions
pub mod utils {
    use super::csa_iot::*;
    
    /// Build a Matter products URL for a specific page number
    /// Uses the new URL structure: https://csa-iot.org/csa-iot_products/page/{page}/?p_keywords&p_type%5B0%5D=14&p_program_type%5B0%5D=1049&p_certificate&p_family&p_firmware_ver
    pub fn matter_products_page_url(page: u32) -> String {
        if page <= 1 {
            // First page uses base URL without /page/ path
            format!("{}{}", PRODUCTS_BASE, MATTER_QUERY_PARAMS)
        } else {
            // Pages 2+ use /page/{number}/ path
            format!("{}/page/{}{}", PRODUCTS_BASE, page, MATTER_QUERY_PARAMS)
        }
    }
    
    /// Build a Matter products URL by using the same structure as matter_products_page_url
    /// This function is kept for compatibility but now uses the same logic
    pub fn matter_products_page_url_simple(page: u32) -> String {
        matter_products_page_url(page)
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
