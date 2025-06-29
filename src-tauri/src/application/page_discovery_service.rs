//! Page Discovery Service
//! 
//! Provides functionality to discover the last page number in paginated product listings.
//! This service is used both by the main application and analysis tools.

use anyhow::Result;
use scraper::{Html, Selector};
use tracing::{info, warn};

use crate::infrastructure::{HttpClient, config::{utils, AppConfig}};

/// Service for discovering pagination information
pub struct PageDiscoveryService {
    http_client: HttpClient,
}

impl PageDiscoveryService {
    /// Create a new page discovery service
    pub fn new(http_client: HttpClient) -> Self {
        Self { http_client }
    }
    
    /// Find the actual last page number using enhanced search strategy
    /// 
    /// This method starts from a configured high page number and uses pagination
    /// links to find the true last page. It's much more efficient than binary search
    /// since product counts only increase over time.
    pub async fn find_last_page(&mut self, config: &AppConfig) -> Result<u32> {
        info!("🔍 Starting search for actual last page...");
        
        // Start from configured page number since products only increase over time
        let mut current_page = config.advanced.last_page_search_start;
        let mut max_attempts = config.advanced.max_search_attempts;
        
        info!("🚀 Starting from page {} (from config)", current_page);
        
        loop {
            let test_url = utils::matter_products_page_url(current_page);
            info!("🔗 Checking page {}: {}", current_page, test_url);
            
            match self.http_client.fetch_html(&test_url).await {
                Ok(html) => {
                    if Self::has_products(&html, &config.advanced.product_selectors) {
                        info!("✅ Page {} has products", current_page);
                        
                        // Check if there are higher page numbers in pagination
                        let max_page_on_this_page = Self::find_max_page_in_pagination(&html);
                        info!("📝 Highest pagination link on page {}: {}", current_page, max_page_on_this_page);
                        
                        if max_page_on_this_page > current_page {
                            // Found a higher page number, go there
                            current_page = max_page_on_this_page;
                            info!("🚀 Moving to higher page: {}", current_page);
                        } else {
                            // No higher page found, this is the last page
                            info!("🎯 Found actual last page: {}", current_page);
                            return Ok(current_page);
                        }
                    } else {
                        // This page has no products, go back one page
                        current_page -= 1;
                        info!("❌ Page {} has no products, trying page {}", current_page + 1, current_page);
                        
                        // Verify the previous page has products
                        let prev_url = utils::matter_products_page_url(current_page);
                        let prev_html = self.http_client.fetch_html(&prev_url).await?;
                        
                        if Self::has_products(&prev_html, &config.advanced.product_selectors) {
                            info!("🎯 Found actual last page: {}", current_page);
                            return Ok(current_page);
                        }
                    }
                }
                Err(e) => {
                    warn!("⚠️ Failed to fetch page {}: {}", current_page, e);
                    current_page -= 1;
                }
            }
            
            max_attempts -= 1;
            if max_attempts <= 0 {
                warn!("🛑 Reached maximum attempts, using current page: {}", current_page);
                break;
            }
            
            // Small delay to be respectful to the server (from config)
            tokio::time::sleep(tokio::time::Duration::from_millis(config.user.request_delay_ms)).await;
        }
        
        Ok(current_page)
    }
    
    /// Check if a page has products (not an empty page or error page)
    fn has_products(html: &Html, selectors: &[String]) -> bool {
        let count = Self::count_products(html, selectors);
        count > 0
    }
    
    /// Count products on a page using configured selectors
    pub fn count_products(html: &Html, product_selectors: &[String]) -> u32 {
        let mut max_count = 0;
        
        for selector_str in product_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                let count = html.select(&selector).count() as u32;
                if count > max_count {
                    max_count = count;
                    info!("📝 Found {} products with selector: {}", count, selector_str);
                }
            }
        }
        
        // If we still haven't found products, try looking for article tags or div containers
        if max_count == 0 {
            if let Ok(article_selector) = Selector::parse("article") {
                let articles = html.select(&article_selector).count() as u32;
                info!("📝 Found {} article elements (potential products)", articles);
                max_count = articles;
            }
        }
        
        max_count
    }
    
    /// Find the maximum page number in pagination links
    fn find_max_page_in_pagination(html: &Html) -> u32 {
        let link_selector = Selector::parse("a[href*='page']").unwrap();
        let mut max_page = 1;
        
        for element in html.select(&link_selector) {
            if let Some(href) = element.value().attr("href") {
                if let Some(page_num) = Self::extract_page_number(href) {
                    max_page = max_page.max(page_num);
                }
            }
        }
        
        max_page
    }
    
    /// Extract page number from URL
    fn extract_page_number(url: &str) -> Option<u32> {
        if let Some(captures) = regex::Regex::new(r"[?&]page[d]?=(\d+)")
            .ok()
            .and_then(|re| re.captures(url)) 
        {
            if let Some(num_match) = captures.get(1) {
                return num_match.as_str().parse().ok();
            }
        }
        
        if let Some(captures) = regex::Regex::new(r"/page/(\d+)")
            .ok()
            .and_then(|re| re.captures(url))
        {
            if let Some(num_match) = captures.get(1) {
                return num_match.as_str().parse().ok();
            }
        }
        
        None
    }
}
