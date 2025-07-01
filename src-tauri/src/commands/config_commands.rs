//! Configuration management commands for Tauri IPC
//! 
//! This module provides IPC commands for managing application configuration
//! in a unified way. The frontend should always get configuration from the
//! backend through these commands to ensure a single source of truth.

use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::{info, warn};

use crate::{
    application::state::AppState,
    infrastructure::config::{AppConfig, ConfigManager, csa_iot, utils},
};

/// Frontend-friendly configuration structure
/// This is what gets exposed to the frontend via IPC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendConfig {
    /// CSA-IoT URLs and site configuration
    pub site: SiteConfig,
    
    /// User-configurable crawling settings
    pub crawling: CrawlingSettings,
    
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
        app: AppMetadata {
            name: "rMatterCertis".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            user_agent: format!("rMatterCertis/{} (Research Tool)", env!("CARGO_PKG_VERSION")),
            config_version: app_config.app_managed.config_version,
        },
    }
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
}
