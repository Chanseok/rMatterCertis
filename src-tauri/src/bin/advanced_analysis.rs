//! Advanced site analysis to get exact product count
//! 
//! This test goes to the last page to calculate exact product count

#![allow(clippy::single_component_path_imports)]

use anyhow::Result;
use matter_certis_v2_lib::infrastructure::{HttpClient, config::{csa_iot, utils, ConfigManager}};
use matter_certis_v2_lib::infrastructure::simple_http_client::HttpClientConfig;
use matter_certis_v2_lib::application::PageDiscoveryService;
use tracing::{info, warn};
use chrono;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    info!("🚀 Advanced CSA-IoT product count analysis");

    // Load configuration
    let config_manager = ConfigManager::new()?;
    let config = config_manager.load_config().await?;
    info!("📋 Using configuration:");
    info!("   Starting page search from: {}", config.advanced.last_page_search_start);
    info!("   Max search attempts: {}", config.advanced.max_search_attempts);
    info!("   Request delay: {}ms", config.user.request_delay_ms);

    let http_client = HttpClient::with_config(HttpClientConfig::default())?;
    
    // Create page discovery service
    let mut page_discovery = PageDiscoveryService::new(http_client);
    
    // First, get the last page number using enhanced search strategy
    let last_page = page_discovery.find_last_page(&config).await?;
    info!("📊 Actual last page number: {}", last_page);
    
    // Get first page HTML for comparison
    let mut http_client2 = HttpClient::with_config(HttpClientConfig::default())?;
    let html = http_client2.fetch_html(csa_iot::PRODUCTS_PAGE_MATTER_ONLY).await?;
    
    // Now fetch the last page to count products there
    let last_page_url = utils::matter_products_page_url(last_page);
    info!("🔗 Fetching last page: {}", last_page_url);
    
    let last_page_html = http_client2.fetch_html(&last_page_url).await?;
    
    let products_on_last_page = PageDiscoveryService::count_products(&last_page_html, &config.advanced.product_selectors);
    info!("🛍️ Products on last page: {}", products_on_last_page);
    
    // Also count products on first page to verify consistency
    let products_on_first_page = PageDiscoveryService::count_products(&html, &config.advanced.product_selectors);
    info!("🛍️ Products on first page: {}", products_on_first_page);
    
    // Calculate total
    let total_products = if last_page > 1 {
        (last_page - 1) * products_on_first_page + products_on_last_page
    } else {
        products_on_first_page
    };
    
    info!("📈 ANALYSIS RESULTS:");
    info!("   Total pages: {}", last_page);
    info!("   Products per page (typical): {}", products_on_first_page);
    info!("   Products on last page: {}", products_on_last_page);
    info!("   🎯 ESTIMATED TOTAL PRODUCTS: {}", total_products);
    
    // Update app-managed config with discovered information
    config_manager.update_app_managed(|app_managed| {
        app_managed.last_known_max_page = Some(last_page);
        app_managed.last_crawl_product_count = Some(total_products);
        app_managed.avg_products_per_page = Some(products_on_first_page as f64);
        app_managed.last_successful_crawl = Some(chrono::Utc::now().to_rfc3339());
    }).await?;
    
    info!("💾 Updated configuration with crawl results");
    
    // Let's also try some middle pages to verify consistency
    if last_page > 10 {
        let middle_page = last_page / 2;
        let middle_url = utils::matter_products_page_url(middle_page);
        info!("🔗 Checking middle page {}: {}", middle_page, middle_url);
        
        match http_client2.fetch_html(&middle_url).await {
            Ok(middle_html) => {
                let products_on_middle_page = PageDiscoveryService::count_products(&middle_html, &config.advanced.product_selectors);
                info!("🛍️ Products on middle page {}: {}", middle_page, products_on_middle_page);
            }
            Err(e) => {
                warn!("⚠️ Failed to fetch middle page: {}", e);
            }
        }
    }
    
    Ok(())
}
