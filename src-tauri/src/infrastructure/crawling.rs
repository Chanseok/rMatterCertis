//! Modern web crawler implementation following the guide's architecture
//! 
//! HTTP client with retry logic, rate limiting, and comprehensive error handling.

use crate::infrastructure::parsing::{ParsingConfig, ProductListParser, ProductDetailParser, ParseContext, ContextualParser};
use crate::infrastructure::parsing::context::DetailParseContext;
use crate::domain::product::{Product, ProductDetail};
use reqwest::{Client, Response};
use anyhow::{Result, Context};
use std::time::Duration;
use tokio::time::sleep;
use std::sync::Arc;
use tracing::{debug, warn, error, info};
use scraper::Html;

/// Web crawler with retry logic and rate limiting
pub struct WebCrawler {
    client: Client,
    list_parser: Arc<ProductListParser>,
    detail_parser: Arc<ProductDetailParser>,
    config: ParsingConfig,
}

impl WebCrawler {
    /// Create a new web crawler with the specified configuration
    pub fn new(config: ParsingConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .user_agent(&config.user_agent)
            .gzip(true)
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .context("Failed to create HTTP client")?;
        
        let list_parser = Arc::new(
            ProductListParser::with_config(&config.product_list_selectors)
                .context("Failed to create product list parser")?
        );
        
        let detail_parser = Arc::new(
            ProductDetailParser::with_config(&config.product_detail_selectors)
                .context("Failed to create product detail parser")?
        );
        
        Ok(Self {
            client,
            list_parser,
            detail_parser,
            config,
        })
    }
    
    /// Crawl product list from a specific page
    pub async fn crawl_product_list(&self, url: &str, page_id: u32) -> Result<Vec<Product>> {
        info!("Crawling product list from: {} (page {})", url, page_id);
        
        let html_content = self.fetch_html_with_retry(url).await?;
        
        // Parse and process HTML synchronously to avoid Send issues
        let products = {
            let html = Html::parse_document(&html_content);
            let context = ParseContext::new(page_id, self.config.base_url.clone());
            self.list_parser.parse_with_context(&html, &context)
                .map_err(|e| anyhow::anyhow!("Product list parsing failed: {}", e))?
        };
        
        // Add delay to avoid overwhelming the server
        if self.config.request_delay_ms > 0 {
            sleep(Duration::from_millis(self.config.request_delay_ms)).await;
        }
        
        info!("Successfully crawled {} products from page {}", products.len(), page_id);
        Ok(products)
    }
    
    /// Crawl detailed product information
    pub async fn crawl_product_detail(&self, url: &str) -> Result<ProductDetail> {
        info!("Crawling product detail from: {}", url);
        
        let html_content = self.fetch_html_with_retry(url).await?;
        
        // Parse and process HTML synchronously to avoid Send issues
        let product = {
            let html = Html::parse_document(&html_content);
            let context = DetailParseContext::new(url.to_string(), self.config.base_url.clone());
            self.detail_parser.parse_with_context(&html, &context)
                .map_err(|e| anyhow::anyhow!("Product detail parsing failed: {}", e))?
        };
        
        // Add delay between requests
        if self.config.request_delay_ms > 0 {
            sleep(Duration::from_millis(self.config.request_delay_ms)).await;
        }
        
        info!("Successfully crawled product detail: {}", product.model.as_ref().unwrap_or(&"Unknown".to_string()));
        Ok(product)
    }
    
    /// Batch crawl multiple product list pages
    pub async fn batch_crawl_product_lists(
        &self, 
        urls_and_pages: Vec<(String, u32)>
    ) -> Vec<Result<Vec<Product>>> {
        let mut results = Vec::new();
        
        for (url, page_id) in urls_and_pages {
            let result = self.crawl_product_list(&url, page_id).await;
            results.push(result);
            
            // Add delay between batch requests
            if self.config.request_delay_ms > 0 {
                sleep(Duration::from_millis(self.config.request_delay_ms)).await;
            }
        }
        
        results
    }
    
    /// Batch crawl multiple product detail pages
    pub async fn batch_crawl_product_details(
        &self, 
        urls: Vec<String>
    ) -> Vec<Result<ProductDetail>> {
        let mut results = Vec::new();
        
        for url in urls {
            let result = self.crawl_product_detail(&url).await;
            results.push(result);
            
            // Add delay between batch requests
            if self.config.request_delay_ms > 0 {
                sleep(Duration::from_millis(self.config.request_delay_ms)).await;
            }
        }
        
        results
    }
    
    /// Check if a listing page has more pages to crawl
    pub async fn has_next_page(&self, url: &str) -> Result<bool> {
        let html_content = self.fetch_html_with_retry(url).await?;
        let html = Html::parse_document(&html_content);
        
        Ok(self.list_parser.has_next_page(&html))
    }
    
    /// Fetch HTML with comprehensive retry logic
    async fn fetch_html_with_retry(&self, url: &str) -> Result<String> {
        let mut last_error = None;
        
        for attempt in 1..=self.config.retry_count {
            match self.fetch_html(url).await {
                Ok(html) => {
                    debug!("Successfully fetched HTML from {} on attempt {}", url, attempt);
                    return Ok(html);
                }
                Err(e) => {
                    warn!(
                        "Attempt {} failed for {}: {}. Retrying...", 
                        attempt, 
                        url, 
                        e
                    );
                    last_error = Some(e);
                    
                    if attempt < self.config.retry_count {
                        let delay = Duration::from_millis(1000 * attempt as u64);
                        sleep(delay).await;
                    }
                }
            }
        }
        
        error!("All {} retry attempts failed for {}", self.config.retry_count, url);
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All retry attempts failed")))
    }
    
    /// Fetch HTML from URL with validation
    async fn fetch_html(&self, url: &str) -> Result<String> {
        debug!("Fetching HTML from: {}", url);
        
        let response = self.client
            .get(url)
            .send()
            .await
            .context("HTTP request failed")?;
        
        self.validate_response(&response)?;
        
        let html = response
            .text()
            .await
            .context("Failed to read response body")?;
        
        self.validate_html_content(&html, url)?;
        
        debug!("Successfully fetched {} bytes of HTML", html.len());
        Ok(html)
    }
    
    /// Validate HTTP response
    fn validate_response(&self, response: &Response) -> Result<()> {
        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "HTTP error: {} - {}", 
                response.status().as_u16(), 
                response.status().canonical_reason().unwrap_or("Unknown")
            ));
        }
        
        // Check content type
        if let Some(content_type) = response.headers().get("content-type") {
            let content_type_str = content_type.to_str().unwrap_or("");
            if !content_type_str.contains("text/html") && !content_type_str.contains("application/xhtml") {
                warn!("Unexpected content type: {}", content_type_str);
            }
        }
        
        Ok(())
    }
    
    /// Validate HTML content for basic sanity checks
    fn validate_html_content(&self, html: &str, url: &str) -> Result<()> {
        if html.len() < 100 {
            return Err(anyhow::anyhow!(
                "Response body too short ({} bytes), likely an error page", 
                html.len()
            ));
        }
        
        // Check for common error indicators
        let html_lower = html.to_lowercase();
        let error_indicators = vec![
            "404 not found",
            "403 forbidden",
            "500 internal server error",
            "access denied",
            "page not found",
            "error occurred",
        ];
        
        for indicator in error_indicators {
            if html_lower.contains(indicator) {
                return Err(anyhow::anyhow!(
                    "Error page detected (contains '{}')", 
                    indicator
                ));
            }
        }
        
        // Check for minimum expected HTML structure
        if !html_lower.contains("<html") && !html_lower.contains("<!doctype") {
            warn!("HTML structure seems unusual for URL: {}", url);
        }
        
        Ok(())
    }
    
    /// Get crawler statistics
    pub fn get_config(&self) -> &ParsingConfig {
        &self.config
    }
    
    /// Update crawler configuration
    pub fn update_config(&mut self, config: ParsingConfig) -> Result<()> {
        // Recreate parsers with new configuration
        self.list_parser = Arc::new(
            ProductListParser::with_config(&config.product_list_selectors)
                .context("Failed to update product list parser")?
        );
        
        self.detail_parser = Arc::new(
            ProductDetailParser::with_config(&config.product_detail_selectors)
                .context("Failed to update product detail parser")?
        );
        
        self.config = config;
        info!("Crawler configuration updated successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_crawler_creation() {
        let config = ParsingConfig::default();
        let crawler = WebCrawler::new(config);
        assert!(crawler.is_ok());
    }
    
    #[test]
    fn test_config_validation() {
        let config = ParsingConfig::default();
        let crawler = WebCrawler::new(config);
        
        // WebCrawler가 성공적으로 생성되는지 테스트
        assert!(crawler.is_ok());
        
        let crawler = crawler.unwrap();
        
        // 기본 설정 확인
        assert!(!crawler.config.base_url.is_empty());
    }
}
