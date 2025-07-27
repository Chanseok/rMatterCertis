//! Configuration management commands for Tauri IPC
//! 
//! This module provides IPC commands for managing application configuration
//! in a unified way. The frontend should always get configuration from the
//! backend through these commands to ensure a single source of truth.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;
use tracing::{info, error, debug};
use crate::domain::services::crawling_services::StatusChecker;
use crate::infrastructure::MatterDataExtractor;

use crate::{
    application::state::AppState,
    infrastructure::config::{AppConfig, ConfigManager, LoggingConfig, csa_iot, utils},
    commands::modern_crawling::get_databasestats,
};

/// Frontend-friendly configuration structure
/// This is what gets exposed to the frontend via IPC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendConfig {
    /// CSA-IoT URLs and site configuration
    pub site: SiteConfig,
    
    /// User-configurable crawling settings
    pub crawling: CrawlingSettings,
    
    /// User-configurable settings (including logging)
    pub user: UserSettings,
    
    /// Application metadata
    pub app: AppMetadata,
}

/// Site-specific configuration (URLs, domains, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    /// Base URL for the target site
    pub base_url: String,
    
    /// General products page URL
    pub products_page_general: String,
    
    /// Matter-filtered products page URL
    pub products_page_matter_only: String,
    
    /// URL pattern for paginated Matter products (use with page number)
    pub products_page_matter_paginated: String,
    
    /// Filter parameters for Matter products
    pub matter_filters: MatterFilters,
    
    /// Supported domains for crawling
    pub allowed_domains: Vec<String>,
}

/// Matter-specific filter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatterFilters {
    /// Matter product type ID
    pub product_type: String,
    
    /// Matter program type ID  
    pub program_type: String,
    
    /// Query parameter names
    pub params: FilterParams,
}

/// Filter parameter names
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterParams {
    pub type_param: String,
    pub program_type_param: String,
    pub keywords_param: String,
    pub certificate_param: String,
    pub family_param: String,
    pub firmware_ver_param: String,
}

/// User-configurable crawling settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingSettings {
    /// Delay between requests in milliseconds
    pub request_delay_ms: u64,
    
    /// Maximum concurrent requests
    pub max_concurrent_requests: u32,
    
    /// Enable verbose logging
    pub verbose_logging: bool,
    
    /// Advanced settings
    pub advanced: AdvancedSettings,
    
    /// Maximum pages to crawl (moved from top level)
    pub page_range_limit: u32,
}

/// User settings (including logging configuration)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettings {
    /// Delay between requests in milliseconds
    pub request_delay_ms: u64,
    
    /// Maximum concurrent requests
    pub max_concurrent_requests: u32,
    
    /// Enable verbose logging
    pub verbose_logging: bool,
    
    /// Logging configuration
    pub logging: LoggingConfig,
}

/// Advanced crawling settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedSettings {
    /// Starting page for last page search
    pub last_page_search_start: u32,
    
    /// Maximum search attempts
    pub max_search_attempts: u32,
    
    /// Retry attempts for failed requests
    pub retry_attempts: u32,
    
    /// Retry delay in milliseconds
    pub retry_delay_ms: u64,
    
    /// Request timeout in seconds
    pub request_timeout_seconds: u64,
    
    /// CSS selectors for finding products
    pub product_selectors: Vec<String>,
}

/// Application metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMetadata {
    /// Application name
    pub name: String,
    
    /// Application version
    pub version: String,
    
    /// User agent string for HTTP requests
    pub user_agent: String,
    
    /// Configuration version for migration
    pub config_version: u32,
}

/// Comprehensive Crawler Configuration - Single Source of Truth
/// This structure includes all configuration options from both simple and advanced crawling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComprehensiveCrawlerConfig {
    // === Core Crawling Settings ===
    pub start_page: u32,
    pub end_page: u32,
    pub concurrency: u32,
    pub delay_ms: u64,
    
    // === Advanced Settings (from Frontend CrawlerConfig) ===
    pub page_range_limit: u32,              // 기본값: 10
    pub product_list_retry_count: u32,       // 기본값: 9
    pub product_detail_retry_count: u32,     // 기본값: 9
    pub products_per_page: u32,              // 기본값: 12
    pub auto_add_to_local_db: bool,          // 기본값: true
    pub auto_status_check: bool,             // 기본값: true
    pub crawler_type: String,                // 'axios' | 'playwright'

    // === Batch Processing Settings ===
    pub batch_size: u32,                     // 기본값: 30
    pub batch_delay_ms: u64,                 // 기본값: 2000
    pub enable_batch_processing: bool,       // 기본값: true
    pub batch_retry_limit: u32,              // 기본값: 3

    // === URL Settings ===
    pub base_url: String,                    // CSA-IoT 기본 URL
    pub matter_filter_url: String,           // Matter 필터 적용된 URL
    
    // === Timeout Settings ===
    pub page_timeout_ms: u64,                // 기본값: 90000
    pub product_detail_timeout_ms: u64,      // 기본값: 90000
    
    // === Concurrency & Performance Settings ===
    pub initial_concurrency: u32,            // 기본값: 16
    pub detail_concurrency: u32,             // 기본값: 16
    pub retry_concurrency: u32,              // 기본값: 9
    pub min_request_delay_ms: u64,           // 기본값: 100
    pub max_request_delay_ms: u64,           // 기본값: 2200
    pub retry_start: u32,                    // 기본값: 2
    pub retry_max: u32,                      // 기본값: 10
    pub cache_ttl_ms: u64,                   // 기본값: 300000

    // === Browser Settings ===
    pub headless_browser: bool,              // 기본값: true
    pub max_concurrent_tasks: u32,           // 기본값: 16
    pub request_delay: u64,                  // 기본값: 100
    pub custom_user_agent: Option<String>,   // 선택적
    
    // === Logging Settings ===
    pub logging: CrawlerLoggingConfig,
}

/// Logging configuration for the crawler
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlerLoggingConfig {
    pub level: String,                       // 'ERROR' | 'WARN' | 'INFO' | 'DEBUG'
    pub enable_stack_trace: bool,
    pub enable_timestamp: bool,
    pub components: std::collections::HashMap<String, String>,
}

impl Default for ComprehensiveCrawlerConfig {
    fn default() -> Self {
        use crate::infrastructure::config::csa_iot;
        
        Self {
            // Core settings
            start_page: 1,
            end_page: 10,
            concurrency: 16,
            delay_ms: 100,
            
            // Advanced settings
            page_range_limit: 10,
            product_list_retry_count: 9,
            product_detail_retry_count: 9,
            products_per_page: crate::infrastructure::config::defaults::DEFAULT_PRODUCTS_PER_PAGE,
            auto_add_to_local_db: true,
            auto_status_check: true,
            crawler_type: "axios".to_string(),

            // Batch processing
            batch_size: 30,
            batch_delay_ms: 2000,
            enable_batch_processing: true,
            batch_retry_limit: 3,

            // URLs (from backend config)
            base_url: csa_iot::BASE_URL.to_string(),
            matter_filter_url: csa_iot::PRODUCTS_PAGE_MATTER_ONLY.to_string(),
            
            // Timeouts
            page_timeout_ms: 90000,
            product_detail_timeout_ms: 90000,
            
            // Concurrency & Performance
            initial_concurrency: 16,
            detail_concurrency: 16,
            retry_concurrency: 9,
            min_request_delay_ms: 100,
            max_request_delay_ms: 2200,
            retry_start: 2,
            retry_max: 10,
            cache_ttl_ms: 300000,

            // Browser settings
            headless_browser: true,
            max_concurrent_tasks: 16,
            request_delay: 100,
            custom_user_agent: None,
            
            // Logging
            logging: CrawlerLoggingConfig {
                level: "INFO".to_string(),
                enable_stack_trace: false,
                enable_timestamp: true,
                components: std::collections::HashMap::new(),
            },
        }
    }
}

// ❌ REMOVED: get_frontend_config - 설정 전송 API 제거
// 백엔드는 matter_certis_config.json 파일만 읽고, 프론트엔드로 설정을 전송하지 않음

/// Get only the site configuration (URLs and domains)
#[tauri::command]
pub fn get_site_config() -> Result<SiteConfig, String> {
    info!("Frontend requesting site configuration");
    
    let site_config = SiteConfig {
        base_url: csa_iot::BASE_URL.to_string(),
        products_page_general: csa_iot::PRODUCTS_PAGE_GENERAL.to_string(),
        products_page_matter_only: csa_iot::PRODUCTS_PAGE_MATTER_ONLY.to_string(),
        products_page_matter_paginated: csa_iot::PRODUCTS_PAGE_MATTER_PAGINATED.to_string(),
        matter_filters: MatterFilters {
            product_type: csa_iot::filters::MATTER_PRODUCT_TYPE.to_string(),
            program_type: csa_iot::filters::MATTER_PROGRAM_TYPE.to_string(),
            params: FilterParams {
                type_param: csa_iot::filters::PARAM_TYPE.to_string(),
                program_type_param: csa_iot::filters::PARAM_PROGRAM_TYPE.to_string(),
                keywords_param: csa_iot::filters::PARAM_KEYWORDS.to_string(),
                certificate_param: csa_iot::filters::PARAM_CERTIFICATE.to_string(),
                family_param: csa_iot::filters::PARAM_FAMILY.to_string(),
                firmware_ver_param: csa_iot::filters::PARAM_FIRMWARE_VER.to_string(),
            },
        },
        allowed_domains: vec![
            "csa-iot.org".to_string(),
            "certification.csa-iot.org".to_string(),
            "certifications.csa-iot.org".to_string(),
        ],
    };
    
    Ok(site_config)
}

/// Update logging configuration settings
#[tauri::command]
pub async fn update_logging_settings(
    level: String,
    separate_frontend_backend: bool,
    max_file_size_mb: u64,
    max_files: u32,
    auto_cleanup_logs: bool,
    keep_only_latest: bool,
    module_filters: HashMap<String, String>,
    state: State<'_, AppState>
) -> Result<(), String> {
    info!("Frontend updating logging settings: level={}, separate={}, modules={:?}", 
          level, separate_frontend_backend, module_filters);
    
    let config_manager = ConfigManager::new()
        .map_err(|e| format!("Failed to create config manager: {}", e))?;
    
    config_manager.update_user_config(|user_config| {
        user_config.logging.level = level;
        user_config.logging.separate_frontend_backend = separate_frontend_backend;
        user_config.logging.max_file_size_mb = max_file_size_mb;
        user_config.logging.max_files = max_files;
        user_config.logging.auto_cleanup_logs = auto_cleanup_logs;
        user_config.logging.keep_only_latest = keep_only_latest;
        user_config.logging.module_filters = module_filters;
    }).await
    .map_err(|e| format!("Failed to update logging settings: {}", e))?;
    
    // Update the app state with new configuration
    let updated_config = config_manager.load_config().await
        .map_err(|e| format!("Failed to reload config: {}", e))?;
    let _ = state.update_config(updated_config).await;
    
    info!("Logging settings updated successfully");
    Ok(())
}

/// Update batch processing configuration settings
#[tauri::command]
pub async fn update_batch_settings(
    batch_size: u32,
    batch_delay_ms: u64,
    enable_batch_processing: bool,
    batch_retry_limit: u32,
    state: State<'_, AppState>
) -> Result<(), String> {
    info!("Frontend updating batch settings: size={}, delay={}ms, enabled={}, retry_limit={}", 
          batch_size, batch_delay_ms, enable_batch_processing, batch_retry_limit);
    
    let config_manager = ConfigManager::new()
        .map_err(|e| format!("Failed to create config manager: {}", e))?;
    
    config_manager.update_user_config(|user_config| {
        user_config.batch.batch_size = batch_size;
        user_config.batch.batch_delay_ms = batch_delay_ms;
        user_config.batch.enable_batch_processing = enable_batch_processing;
        user_config.batch.batch_retry_limit = batch_retry_limit;
    }).await
    .map_err(|e| format!("Failed to update batch settings: {}", e))?;
    
    // Update the app state with new configuration
    let updated_config = config_manager.load_config().await
        .map_err(|e| format!("Failed to reload config: {}", e))?;
    let _ = state.update_config(updated_config).await;
    
    info!("Batch settings updated successfully");
    Ok(())
}

/// Update crawling configuration settings
#[tauri::command]
pub async fn update_crawling_settings(
    page_range_limit: u32,
    product_list_retry_count: u32,
    product_detail_retry_count: u32,
    auto_add_to_local_db: bool,
    state: State<'_, AppState>
) -> Result<(), String> {
    info!("Frontend updating crawling settings: page_limit={}, list_retry={}, detail_retry={}, auto_add={}", 
          page_range_limit, product_list_retry_count, product_detail_retry_count, auto_add_to_local_db);
    
    let config_manager = ConfigManager::new()
        .map_err(|e| format!("Failed to create config manager: {}", e))?;
    
    config_manager.update_user_config(|user_config| {
        user_config.crawling.page_range_limit = page_range_limit;
        user_config.crawling.product_list_retry_count = product_list_retry_count;
        user_config.crawling.product_detail_retry_count = product_detail_retry_count;
        user_config.crawling.auto_add_to_local_db = auto_add_to_local_db;
    }).await
    .map_err(|e| format!("Failed to update crawling settings: {}", e))?;
    
    // Update the app state with new configuration
    let updated_config = config_manager.load_config().await
        .map_err(|e| format!("Failed to reload config: {}", e))?;
    let _ = state.update_config(updated_config).await;
    
    info!("Crawling settings updated successfully");
    Ok(())
}

/// Build a URL for a specific page number using the site configuration
#[tauri::command]
pub fn build_page_url(page: u32) -> Result<String, String> {
    let url = utils::matter_products_page_url(page);
    Ok(url)
}

/// Resolve a relative URL to an absolute URL
#[tauri::command]
pub fn resolve_url(relative_url: String) -> Result<String, String> {
    let absolute_url = utils::resolve_url(&relative_url);
    Ok(absolute_url)
}

/// Get default crawling configuration
#[tauri::command]
pub fn get_default_crawling_config() -> Result<CrawlingSettings, String> {
    info!("Frontend requesting default crawling configuration");
    
    // Use the same defaults as in AppConfig
    let app_config = crate::infrastructure::config::AppConfig::default();
    let crawling_settings = CrawlingSettings {
        request_delay_ms: app_config.user.request_delay_ms,
        max_concurrent_requests: app_config.user.max_concurrent_requests,
        verbose_logging: app_config.user.verbose_logging,
        page_range_limit: app_config.user.crawling.page_range_limit,
        advanced: AdvancedSettings {
            last_page_search_start: app_config.advanced.last_page_search_start,
            max_search_attempts: app_config.advanced.max_search_attempts,
            retry_attempts: app_config.advanced.retry_attempts,
            retry_delay_ms: app_config.advanced.retry_delay_ms,
            request_timeout_seconds: app_config.advanced.request_timeout_seconds,
            product_selectors: app_config.advanced.product_selectors.clone(),
        },
    };
    
    Ok(crawling_settings)
}

/// Get comprehensive crawler configuration including all advanced settings
#[tauri::command]
pub fn get_comprehensive_crawler_config() -> Result<ComprehensiveCrawlerConfig, String> {
    info!("Frontend requesting comprehensive crawler configuration");
    
    let config = ComprehensiveCrawlerConfig::default();
    
    info!("Providing comprehensive crawler config with {} fields", 
          format!("batch_size={}, concurrency={}, page_range_limit={}", 
                  config.batch_size, config.concurrency, config.page_range_limit));
    
    Ok(config)
}

/// Convert internal AppConfig to frontend-friendly FrontendConfig
fn convert_to_frontend_config(app_config: &AppConfig) -> FrontendConfig {
    FrontendConfig {
        site: SiteConfig {
            base_url: csa_iot::BASE_URL.to_string(),
            products_page_general: csa_iot::PRODUCTS_PAGE_GENERAL.to_string(),
            products_page_matter_only: csa_iot::PRODUCTS_PAGE_MATTER_ONLY.to_string(),
            products_page_matter_paginated: csa_iot::PRODUCTS_PAGE_MATTER_PAGINATED.to_string(),
            matter_filters: MatterFilters {
                product_type: csa_iot::filters::MATTER_PRODUCT_TYPE.to_string(),
                program_type: csa_iot::filters::MATTER_PROGRAM_TYPE.to_string(),
                params: FilterParams {
                    type_param: csa_iot::filters::PARAM_TYPE.to_string(),
                    program_type_param: csa_iot::filters::PARAM_PROGRAM_TYPE.to_string(),
                    keywords_param: csa_iot::filters::PARAM_KEYWORDS.to_string(),
                    certificate_param: csa_iot::filters::PARAM_CERTIFICATE.to_string(),
                    family_param: csa_iot::filters::PARAM_FAMILY.to_string(),
                    firmware_ver_param: csa_iot::filters::PARAM_FIRMWARE_VER.to_string(),
                },
            },
            allowed_domains: vec![
                "csa-iot.org".to_string(),
                "certification.csa-iot.org".to_string(),
                "certifications.csa-iot.org".to_string(),
            ],
        },
        crawling: CrawlingSettings {
            request_delay_ms: app_config.user.request_delay_ms,
            max_concurrent_requests: app_config.user.max_concurrent_requests,
            verbose_logging: app_config.user.verbose_logging,
            page_range_limit: app_config.user.crawling.page_range_limit,
            advanced: AdvancedSettings {
                last_page_search_start: app_config.advanced.last_page_search_start,
                max_search_attempts: app_config.advanced.max_search_attempts,
                retry_attempts: app_config.advanced.retry_attempts,
                retry_delay_ms: app_config.advanced.retry_delay_ms,
                request_timeout_seconds: app_config.advanced.request_timeout_seconds,
                product_selectors: app_config.advanced.product_selectors.clone(),
            },
        },
        user: UserSettings {
            request_delay_ms: app_config.user.request_delay_ms,
            max_concurrent_requests: app_config.user.max_concurrent_requests,
            verbose_logging: app_config.user.verbose_logging,
            logging: app_config.user.logging.clone(),
        },
        app: AppMetadata {
            name: "matter-certis-v2".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            user_agent: format!("matter-certis-v2/{} (Research Tool)", env!("CARGO_PKG_VERSION")),
            config_version: app_config.app_managed.config_version,
        },
    }
}

/// Initialize configuration system on first run
#[tauri::command]
pub async fn initialize_app_config() -> Result<FrontendConfig, String> {
    info!("Frontend requesting app config initialization");
    
    let config_manager = ConfigManager::new()
        .map_err(|e| format!("Failed to create config manager: {}", e))?;
    
    let app_config = config_manager.initialize_on_first_run().await
        .map_err(|e| format!("Failed to initialize config: {}", e))?;
    
    let frontend_config = convert_to_frontend_config(&app_config);
    
    info!("✅ App configuration initialized successfully");
    Ok(frontend_config)
}

/// Reset configuration to defaults
#[tauri::command]
pub async fn reset_config_to_defaults() -> Result<FrontendConfig, String> {
    info!("Frontend requesting config reset to defaults");
    
    let config_manager = ConfigManager::new()
        .map_err(|e| format!("Failed to create config manager: {}", e))?;
    
    let app_config = config_manager.reset_to_defaults().await
        .map_err(|e| format!("Failed to reset config: {}", e))?;
    
    let frontend_config = convert_to_frontend_config(&app_config);
    
    info!("✅ Configuration reset to defaults");
    Ok(frontend_config)
}

/// Get application data directories info
#[tauri::command]
pub fn get_app_directories() -> Result<AppDirectoriesInfo, String> {
    info!("Frontend requesting app directories info");
    
    let _config_manager = ConfigManager::new()
        .map_err(|e| format!("Failed to create config manager: {}", e))?;
    
    let config_dir = ConfigManager::get_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?;
    
    let data_dir = ConfigManager::get_app_data_dir()
        .map_err(|e| format!("Failed to get data dir: {}", e))?;
    
    let directories = AppDirectoriesInfo {
        config_dir: config_dir.to_string_lossy().to_string(),
        data_dir: data_dir.to_string_lossy().to_string(),
        database_dir: data_dir.join("database").to_string_lossy().to_string(),
        logs_dir: data_dir.join("logs").to_string_lossy().to_string(),
        exports_dir: data_dir.join("exports").to_string_lossy().to_string(),
        backups_dir: data_dir.join("backups").to_string_lossy().to_string(),
        cache_dir: data_dir.join("cache").to_string_lossy().to_string(),
    };
    
    Ok(directories)
}

/// Application directories information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppDirectoriesInfo {
    pub config_dir: String,
    pub data_dir: String,
    pub database_dir: String,
    pub logs_dir: String,
    pub exports_dir: String,
    pub backups_dir: String,
    pub cache_dir: String,
}

/// Check if this is the first run of the application
#[tauri::command]
pub fn is_first_run() -> Result<bool, String> {
    let config_manager = ConfigManager::new()
        .map_err(|e| format!("Failed to create config manager: {}", e))?;
    
    // Check if config file exists
    let config_path = &config_manager.config_path;
    let is_first = !config_path.exists();
    
    info!("First run check: {}", is_first);
    Ok(is_first)
}

/// Logging-related commands for frontend
#[derive(Debug, Serialize, Deserialize)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
    pub timestamp: String,
    pub component: Option<String>,
}

/// Write frontend log entry to the appropriate log file based on configuration
#[tauri::command]
pub async fn write_frontend_log(entry: LogEntry, state: State<'_, AppState>) -> Result<(), String> {
    use std::fs::OpenOptions;
    use std::io::Write;
    use chrono::{Utc, FixedOffset};
    use crate::infrastructure::logging::get_log_directory;
    
    let log_dir = get_log_directory();
    
    // Ensure log directory exists
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        return Err(format!("Failed to create log directory: {}", e));
    }
    
    // Get current configuration to determine log file strategy
    let config = state.get_config().await;
    let logging_config = &config.user.logging;
    
    // Determine which log file to write to
    let log_file_path = if logging_config.separate_frontend_backend {
        log_dir.join("front.log")
    } else {
        // Use unified log file
        match logging_config.file_naming_strategy.as_str() {
            "timestamped" => {
                let now = Utc::now().with_timezone(&FixedOffset::east_opt(9 * 3600).unwrap());
                log_dir.join(format!("back_front-{}.log", now.format("%Y%m%d")))
            },
            _ => log_dir.join("back_front.log"), // Default unified log
        }
    };
    
    // Format timestamp in KST
    let kst_offset = FixedOffset::east_opt(9 * 3600).unwrap();
    let kst_time = Utc::now().with_timezone(&kst_offset);
    let formatted_time = kst_time.format("%Y-%m-%d %H:%M:%S%.3f %Z");
    
    // Format log entry
    let component_str = entry.component
        .map(|c| format!(" [{}]", c))
        .unwrap_or_default();
    
    let log_line = format!(
        "{} [{}]{} {}\n",
        formatted_time,
        entry.level.to_uppercase(),
        component_str,
        entry.message
    );
    
    // Append to log file
    match OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file_path)
    {
        Ok(mut file) => {
            if let Err(e) = file.write_all(log_line.as_bytes()) {
                return Err(format!("Failed to write to log file: {}", e));
            }
            if let Err(e) = file.flush() {
                return Err(format!("Failed to flush log file: {}", e));
            }
            Ok(())
        },
        Err(e) => Err(format!("Failed to open log file: {}", e)),
    }
}

/// Clean up old log files and keep only the latest
#[tauri::command]
pub fn cleanup_logs() -> Result<String, String> {
    use crate::infrastructure::logging::cleanup_logs_keep_latest;
    
    match cleanup_logs_keep_latest() {
        Ok(message) => Ok(message),
        Err(e) => Err(format!("Failed to cleanup logs: {}", e)),
    }
}

/// Get the current log directory path for frontend reference
#[tauri::command]
pub fn get_log_directory_path() -> Result<String, String> {
    use crate::infrastructure::logging::get_log_directory;
    let log_dir = get_log_directory();
    Ok(log_dir.to_string_lossy().to_string())
}

/// Database status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStatus {
    pub total_products: u32,
    pub last_crawl_time: Option<String>,
    pub page_range: (u32, u32), // (min_page, max_page)
    pub health: String, // "Healthy", "Warning", "Critical"
    pub size_mb: f32,
    pub last_updated: String,
}

/// Site status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteStatus {
    pub is_accessible: bool,
    pub response_time_ms: u32,
    pub total_pages: u32,
    pub estimated_products: u32,
    pub products_on_last_page: u32,
    pub last_check_time: String,
    pub health_score: f32, // 0.0 ~ 1.0
    pub data_change_status: String, // Simplified for now
}

/// Smart recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartRecommendation {
    pub action: String, // 'crawl', 'cleanup', 'wait', 'manual_check'
    pub priority: String, // 'low', 'medium', 'high', 'critical'
    pub reason: String,
    pub suggested_range: Option<(u32, u32)>, // (start_page, end_page)
    pub estimated_new_items: u32,
    pub efficiency_score: f32, // 0.0 - 1.0
    pub next_steps: Vec<String>,
}

/// Sync comparison information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncComparison {
    pub database_count: u32,
    pub site_estimated_count: u32,
    pub sync_percentage: f32,
    pub last_sync_time: Option<String>,
}

/// Crawling status check result - Improved Structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingStatusCheck {
    pub database_status: DatabaseStatus,
    pub site_status: SiteStatus,
    pub recommendation: SmartRecommendation,
    pub sync_comparison: SyncComparison,
}

/// Get current crawling status and recommendations
#[tauri::command]
pub async fn get_crawling_status_check(
    state: State<'_, AppState>
) -> Result<CrawlingStatusCheck, String> {
    info!("Frontend requesting crawling status check");
    
    // Get database stats
    let db_stats = get_databasestats(state.clone()).await
        .map_err(|e| format!("Failed to get database stats: {}", e))?;
    
    // Get current configuration
    let config = state.get_config().await;
    let current_time = chrono::Utc::now().to_rfc3339();
    
    // Analyze local database
    let local_product_count = db_stats.total_products as u32;
    
    // Get last crawl info from app_managed config
    let last_crawl_time = config.app_managed.last_successful_crawl.clone();
    
    // Calculate local DB page range (estimate)
    let avg_products_per_page = config.app_managed.avg_products_per_page.unwrap_or(12.0) as f32;
    let estimated_max_local_page = if avg_products_per_page > 0.0 {
        (local_product_count as f32 / avg_products_per_page).ceil() as u32
    } else {
        0
    };
    
    // Initialize StatusChecker for real site analysis
    info!("🔍 Initializing real-time site analysis...");
    
    // Create HTTP client and data extractor
    let http_client = crate::infrastructure::simple_http_client::HttpClient::new()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    let data_extractor = MatterDataExtractor::new()
        .map_err(|e| format!("Failed to create data extractor: {}", e))?;
    
    let status_checker = crate::infrastructure::crawling_service_impls::StatusCheckerImpl::new(
        http_client,
        data_extractor,
        config.clone(),
    );
    
    // Perform comprehensive site analysis
    let site_status = status_checker.check_site_status().await
        .map_err(|e| format!("Site analysis failed: {}", e))?;
    
    info!("✅ Real-time site analysis completed: accessible={}, max_page={:?}", 
          site_status.is_accessible, site_status.total_pages);
    
    // Calculate estimated total products from real site data
    let estimated_total_products = if site_status.total_pages > 0 {
        Some((site_status.total_pages as f32 * avg_products_per_page) as u32)
    } else {
        None
    };
    
    // Get real DB page range analysis using the same database connection pattern
    let database_url = {
        let app_data_dir = std::env::var("APPDATA")
            .or_else(|_| std::env::var("HOME").map(|h| format!("{}/.local/share", h)))
            .unwrap_or_else(|_| "./data".to_string());
        let data_dir = format!("{}/matter-certis-v2/database", app_data_dir);
        format!("sqlite:{}/matter_certis.db", data_dir)
    };
    
    let db_pool = sqlx::SqlitePool::connect(&database_url).await
        .map_err(|e| format!("Failed to connect to database: {}", e))?;
    
    let (min_page, max_page) = sqlx::query_as::<_, (Option<i64>, Option<i64>)>(
        "SELECT MIN(CAST(SUBSTR(url, INSTR(url, 'page=') + 5) AS INTEGER)) as min_page,
                MAX(CAST(SUBSTR(url, INSTR(url, 'page=') + 5) AS INTEGER)) as max_page 
         FROM products 
         WHERE url LIKE '%page=%'"
    )
    .fetch_one(&db_pool)
    .await
    .unwrap_or((None, None));
    
    let local_db_page_range = if let (Some(min), Some(max)) = (min_page, max_page) {
        [min as u32, max as u32]
    } else {
        [0, estimated_max_local_page]
    };
    
    // Generate smart recommendations based on all available data
    let user_max_pages = config.user.crawling.page_range_limit;
    let actual_max_page = site_status.total_pages.max(50);
    
    // 사용자가 설정한 페이지 제한과 실제 최대 페이지 중 작은 값을 사용
    let effective_max_page = std::cmp::min(user_max_pages, actual_max_page);
    
    let recommended_start_page = if estimated_max_local_page > 0 {
        // 로컬 DB에 데이터가 있으면 효율적인 증분 업데이트
        if estimated_max_local_page >= effective_max_page {
            // 로컬 DB가 최신이면 최근 몇 페이지만 다시 확인
            std::cmp::max(1, effective_max_page.saturating_sub(5))
        } else {
            // 로컬 DB가 뒤처져 있으면 마지막 로컬 페이지 근처부터
            std::cmp::max(1, estimated_max_local_page.saturating_sub(3))
        }
    } else {
        // 로컬 DB가 비어있으면 처음부터
        1
    };
    
    let recommended_end_page = effective_max_page;
    
    // 예상 신규 제품 수 계산 (설정 제한 고려)
    let estimated_new_products = if let Some(total) = estimated_total_products {
        let limited_total = std::cmp::min(
            total,
            (effective_max_page as f32 * avg_products_per_page) as u32
        );
        limited_total.saturating_sub(local_product_count)
    } else {
        let pages_to_crawl = recommended_end_page.saturating_sub(recommended_start_page) + 1;
        (pages_to_crawl as f32 * avg_products_per_page) as u32
    };

    // Calculate efficiency score considering user settings
    let efficiency_score = if estimated_new_products > 0 {
        let pages_to_crawl = recommended_end_page.saturating_sub(recommended_start_page) + 1;
        let efficiency = estimated_new_products as f32 / (pages_to_crawl as f32 * avg_products_per_page);
        efficiency.min(1.0)
    } else {
        // 신규 제품이 없어도 데이터 신선도에 따라 점수 부여
        if estimated_max_local_page > 0 && local_product_count > 0 {
            let freshness = (local_product_count as f32 / estimated_total_products.unwrap_or(1) as f32).min(1.0);
            freshness * 0.5 // 최대 50% 효율성
        } else {
            0.0
        }
    };

    // Generate comprehensive recommendation reason
    let recommendation_reason = if local_product_count == 0 {
        if user_max_pages < actual_max_page {
            format!("로컬 DB가 비어있습니다. 사용자 설정에 따라 {}페이지까지 크롤링을 권장합니다. (실제 최대: {}페이지)", 
                    user_max_pages, actual_max_page)
        } else {
            "로컬 DB가 비어있습니다. 전체 크롤링을 권장합니다.".to_string()
        }
    } else if estimated_new_products > 100 {
        format!("약 {}개의 새로운 제품이 예상됩니다. 효율적인 업데이트 크롤링을 권장합니다. ({}~{}페이지)", 
                estimated_new_products, recommended_start_page, recommended_end_page)
    } else if estimated_new_products > 0 {
        format!("약 {}개의 새로운 제품이 있을 수 있습니다. ({}~{}페이지 확인 권장)", 
                estimated_new_products, recommended_start_page, recommended_end_page)
    } else if user_max_pages < actual_max_page {
        format!("현재 데이터가 비교적 최신 상태입니다. 사용자 설정 범위({} 페이지)에서 최신 확인을 권장합니다.", user_max_pages)
    } else {
        "현재 데이터가 최신 상태로 보입니다. 필요시 최근 몇 페이지만 확인해보세요.".to_string()
    };

    // Determine recommended action and priority
    let (action, priority) = if local_product_count == 0 {
        ("crawl".to_string(), "high".to_string())
    } else if estimated_new_products > 100 {
        ("crawl".to_string(), "medium".to_string())
    } else if estimated_new_products > 10 {
        ("crawl".to_string(), "low".to_string())
    } else if local_product_count > 0 && efficiency_score < 0.3 {
        ("cleanup".to_string(), "low".to_string())
    } else {
        ("wait".to_string(), "low".to_string())
    };

    // Generate next steps
    let next_steps = match action.as_str() {
        "crawl" => vec![
            format!("크롤링 범위를 {}~{}페이지로 설정", recommended_start_page, recommended_end_page),
            "크롤링 시작 버튼 클릭".to_string(),
            "진행 상황 모니터링".to_string(),
        ],
        "cleanup" => vec![
            "중복 데이터 정리 고려".to_string(),
            "데이터베이스 최적화 실행".to_string(),
            "필요시 부분 재크롤링".to_string(),
        ],
        _ => vec![
            "현재 상태가 양호합니다".to_string(),
            "정기적으로 상태를 확인하세요".to_string(),
        ],
    };

    // Calculate database health
    let db_health = if local_product_count == 0 {
        "Critical".to_string()
    } else if local_product_count < 100 {
        "Warning".to_string()
    } else {
        "Healthy".to_string()
    };

    // Calculate database size (estimate)
    let avg_record_size_kb = 2.0; // Estimate 2KB per product record
    let db_size_mb = (local_product_count as f32 * avg_record_size_kb) / 1024.0;

    // Calculate sync percentage
    let site_estimated_total = estimated_total_products.unwrap_or(0);
    let sync_percentage = if site_estimated_total > 0 {
        (local_product_count as f32 / site_estimated_total as f32 * 100.0).min(100.0)
    } else {
        0.0
    };
    
    let status_check = CrawlingStatusCheck {
        database_status: DatabaseStatus {
            total_products: local_product_count,
            last_crawl_time: last_crawl_time.clone(),
            page_range: (local_db_page_range[0], local_db_page_range[1]),
            health: db_health,
            size_mb: db_size_mb,
            last_updated: current_time.clone(),
        },
        site_status: SiteStatus {
            is_accessible: site_status.is_accessible,
            response_time_ms: site_status.response_time_ms as u32,
            total_pages: site_status.total_pages,
            estimated_products: estimated_total_products.unwrap_or(0),
            products_on_last_page: site_status.products_on_last_page,
            last_check_time: current_time.clone(),
            health_score: site_status.health_score as f32,
            data_change_status: "Stable".to_string(), // Simplified for now
        },
        recommendation: SmartRecommendation {
            action,
            priority,
            reason: recommendation_reason,
            suggested_range: Some((recommended_start_page, recommended_end_page)),
            estimated_new_items: estimated_new_products,
            efficiency_score,
            next_steps,
        },
        sync_comparison: SyncComparison {
            database_count: local_product_count,
            site_estimated_count: site_estimated_total,
            sync_percentage,
            last_sync_time: last_crawl_time,
        },
    };
    
    info!("Status check completed: local_products={}, site_products={}, action={}, efficiency={:.2}", 
          local_product_count, site_estimated_total, status_check.recommendation.action, efficiency_score);
    
    Ok(status_check)
}

/// Window state structure for saving/restoring UI state
/// ts-rs를 통해 TypeScript 타입 자동 생성 (Modern Rust 2024 정책)
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct WindowState {
    pub position: WindowPosition,
    pub size: WindowSize,
    pub zoom_level: f32,
    pub last_active_tab: String,
    pub is_maximized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct WindowSize {
    pub width: i32,
    pub height: i32,
}

/// Save window state to config file
#[tauri::command]
pub async fn save_window_state(state: WindowState, app_state: State<'_, AppState>) -> Result<(), String> {
    debug!("💾 Saving window state: {:?}", state);
    
    let config_manager = ConfigManager::new()
        .map_err(|e| format!("Failed to create config manager: {}", e))?;
    let mut config = config_manager.load_config().await
        .map_err(|e| format!("Failed to load config: {}", e))?;
    
    // Store window state in the app_managed section of config
    let window_state_json = serde_json::to_string(&state)
        .map_err(|e| format!("Failed to serialize window state: {}", e))?;
    
    config.app_managed.window_state = Some(window_state_json);
    
    config_manager.save_config(&config).await
        .map_err(|e| format!("Failed to save config: {}", e))?;
    info!("✅ Window state saved successfully");
    
    // Also update the app state
    app_state.update_config(config).await
        .map_err(|e| format!("Failed to update app state: {}", e))?;
    
    Ok(())
}

/// Load window state from config file
#[tauri::command]
pub async fn load_window_state(app_state: State<'_, AppState>) -> Result<Option<WindowState>, String> {
    info!("📁 Loading window state");
    
    let config = app_state.get_config().await;
    
    if let Some(window_state_str) = &config.app_managed.window_state {
        let window_state: WindowState = serde_json::from_str(window_state_str)
            .map_err(|e| format!("Failed to deserialize window state: {}", e))?;
        
        info!("✅ Window state loaded successfully: {:?}", window_state);
        return Ok(Some(window_state));
    }
    
    info!("ℹ️ No window state found in config");
    Ok(None)
}

/// Set window position (Tauri command)
#[tauri::command]
pub fn set_window_position(window: tauri::Window, x: i32, y: i32) -> Result<(), String> {
    window.set_position(tauri::LogicalPosition::new(x, y))
        .map_err(|e| format!("Failed to set window position: {}", e))?;
    Ok(())
}

/// Set window size (Tauri command)
#[tauri::command]
pub fn set_window_size(window: tauri::Window, width: i32, height: i32) -> Result<(), String> {
    window.set_size(tauri::LogicalSize::new(width, height))
        .map_err(|e| format!("Failed to set window size: {}", e))?;
    Ok(())
}

/// Maximize window (Tauri command)
#[tauri::command]
pub fn maximize_window(window: tauri::Window) -> Result<(), String> {
    window.maximize()
        .map_err(|e| format!("Failed to maximize window: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_frontend_config_conversion() {
        let app_config = AppConfig::default();
        let frontend_config = convert_to_frontend_config(&app_config);
        
        assert_eq!(frontend_config.site.base_url, "https://csa-iot.org");
        assert!(frontend_config.site.products_page_matter_only.contains("p_type%5B%5D=14"));
        assert_eq!(frontend_config.crawling.page_range_limit, app_config.user.crawling.page_range_limit);
        assert_eq!(frontend_config.app.name, "matter-certis-v2");
    }
    
    #[test]
    fn test_site_config_structure() {
        let site_config = SiteConfig {
            base_url: csa_iot::BASE_URL.to_string(),
            products_page_general: csa_iot::PRODUCTS_PAGE_GENERAL.to_string(),
            products_page_matter_only: csa_iot::PRODUCTS_PAGE_MATTER_ONLY.to_string(),
            products_page_matter_paginated: csa_iot::PRODUCTS_PAGE_MATTER_PAGINATED.to_string(),
            matter_filters: MatterFilters {
                product_type: csa_iot::filters::MATTER_PRODUCT_TYPE.to_string(),
                program_type: csa_iot::filters::MATTER_PROGRAM_TYPE.to_string(),
                params: FilterParams {
                    type_param: csa_iot::filters::PARAM_TYPE.to_string(),
                    program_type_param: csa_iot::filters::PARAM_PROGRAM_TYPE.to_string(),
                    keywords_param: csa_iot::filters::PARAM_KEYWORDS.to_string(),
                    certificate_param: csa_iot::filters::PARAM_CERTIFICATE.to_string(),
                    family_param: csa_iot::filters::PARAM_FAMILY.to_string(),
                    firmware_ver_param: csa_iot::filters::PARAM_FIRMWARE_VER.to_string(),
                },
            },
            allowed_domains: vec!["csa-iot.org".to_string()],
        };
        
        assert!(!site_config.base_url.is_empty());
        assert!(site_config.products_page_matter_only.contains("p_type"));
        assert_eq!(site_config.matter_filters.product_type, "14");
        assert_eq!(site_config.matter_filters.program_type, "1049");
    }
    
    #[tokio::test]
    async fn test_frontend_log_writing() {
        let entry = LogEntry {
            level: "info".to_string(),
            message: "Test log message".to_string(),
            timestamp: "2025-07-04T14:30:00.000Z".to_string(),
            component: Some("StatusTab".to_string()),
        };
        
        // This would normally write to the actual log directory
        // In a real test, we'd want to use a temporary directory and mock AppState
        // For now, just test that the function signature is correct
        // let result = write_frontend_log(entry, mock_state).await;
        // Note: This test is incomplete due to the need for AppState dependency
    }
}
