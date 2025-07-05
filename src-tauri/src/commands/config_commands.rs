//! Configuration management commands for Tauri IPC
//! 
//! This module provides IPC commands for managing application configuration
//! in a unified way. The frontend should always get configuration from the
//! backend through these commands to ensure a single source of truth.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;
use tracing::{info, warn};

use crate::{
    application::state::AppState,
    infrastructure::config::{AppConfig, ConfigManager, LoggingConfig, csa_iot, utils},
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
    /// Maximum pages to crawl
    pub max_pages: u32,
    
    /// Delay between requests in milliseconds
    pub request_delay_ms: u64,
    
    /// Maximum concurrent requests
    pub max_concurrent_requests: u32,
    
    /// Enable verbose logging
    pub verbose_logging: bool,
    
    /// Advanced settings
    pub advanced: AdvancedSettings,
}

/// User settings (including logging configuration)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettings {
    /// Maximum pages to crawl
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

/// Get the complete frontend configuration
#[tauri::command]
pub async fn get_frontend_config(
    state: State<'_, AppState>
) -> Result<FrontendConfig, String> {
    info!("Frontend requesting complete configuration");
    
    let app_config = state.get_config().await;
    let frontend_config = convert_to_frontend_config(&app_config);
    
    info!("Providing frontend config: site={}, crawling_max_pages={}", 
          frontend_config.site.base_url, 
          frontend_config.crawling.max_pages);
    
    Ok(frontend_config)
}

/// Get only the site configuration (URLs and domains)
#[tauri::command]
pub async fn get_site_config() -> Result<SiteConfig, String> {
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

/// Update user-configurable settings
#[tauri::command]
pub async fn update_crawling_settings(
    settings: CrawlingSettings,
    _state: State<'_, AppState>
) -> Result<(), String> {
    info!("Frontend updating crawling settings: max_pages={}, delay={}ms", 
          settings.max_pages, settings.request_delay_ms);
    
    let config_manager = ConfigManager::new()
        .map_err(|e| format!("Failed to create config manager: {}", e))?;
    
    config_manager.update_app_managed(|_app_managed| {
        // Update any app-managed settings that are relevant
        // For now, we don't update app-managed settings from user input
    }).await
        .map_err(|e| format!("Failed to update config: {}", e))?;
    
    // TODO: Implement user config update
    warn!("User config update not yet implemented - settings received but not persisted");
    
    Ok(())
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

/// Build a URL for a specific page number using the site configuration
#[tauri::command]
pub async fn build_page_url(page: u32) -> Result<String, String> {
    let url = utils::matter_products_page_url(page);
    Ok(url)
}

/// Resolve a relative URL to an absolute URL
#[tauri::command]
pub async fn resolve_url(relative_url: String) -> Result<String, String> {
    let absolute_url = utils::resolve_url(&relative_url);
    Ok(absolute_url)
}

/// Get default crawling configuration
#[tauri::command]
pub async fn get_default_crawling_config() -> Result<CrawlingSettings, String> {
    info!("Frontend requesting default crawling configuration");
    
    // Use the same defaults as in AppConfig
    let app_config = crate::infrastructure::config::AppConfig::default();
    let crawling_settings = CrawlingSettings {
        max_pages: app_config.user.max_pages,
        request_delay_ms: app_config.user.request_delay_ms,
        max_concurrent_requests: app_config.user.max_concurrent_requests,
        verbose_logging: app_config.user.verbose_logging,
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
pub async fn get_comprehensive_crawler_config() -> Result<ComprehensiveCrawlerConfig, String> {
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
            max_pages: app_config.user.max_pages,
            request_delay_ms: app_config.user.request_delay_ms,
            max_concurrent_requests: app_config.user.max_concurrent_requests,
            verbose_logging: app_config.user.verbose_logging,
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
            max_pages: app_config.user.max_pages,
            request_delay_ms: app_config.user.request_delay_ms,
            max_concurrent_requests: app_config.user.max_concurrent_requests,
            verbose_logging: app_config.user.verbose_logging,
            logging: app_config.user.logging.clone(),
        },
        app: AppMetadata {
            name: "rMatterCertis".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            user_agent: format!("rMatterCertis/{} (Research Tool)", env!("CARGO_PKG_VERSION")),
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
pub async fn get_app_directories() -> Result<AppDirectoriesInfo, String> {
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
pub async fn is_first_run() -> Result<bool, String> {
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
pub async fn cleanup_logs() -> Result<String, String> {
    use crate::infrastructure::logging::cleanup_logs_keep_latest;
    
    match cleanup_logs_keep_latest() {
        Ok(message) => Ok(message),
        Err(e) => Err(format!("Failed to cleanup logs: {}", e)),
    }
}

/// Get the current log directory path for frontend reference
#[tauri::command]
pub async fn get_log_directory_path() -> Result<String, String> {
    use crate::infrastructure::logging::get_log_directory;
    let log_dir = get_log_directory();
    Ok(log_dir.to_string_lossy().to_string())
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
        assert_eq!(frontend_config.crawling.max_pages, app_config.user.max_pages);
        assert_eq!(frontend_config.app.name, "rMatterCertis");
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
