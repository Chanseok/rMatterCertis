//! Web crawler engine for Matter certification data collection
//! 
//! Provides the main crawling engine that orchestrates HTTP requests,
//! session management, and data extraction.

use std::sync::Arc;
use std::collections::HashSet;
use anyhow::{Result, Context};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::infrastructure::http_client::{HttpClient, HttpClientConfig};
use crate::domain::session_manager::{SessionManager, CrawlingSessionState, SessionStatus, CrawlingStage};
use crate::application::dto::StartCrawlingDto;

/// Configuration for crawling session
#[derive(Debug, Clone, serde::Serialize)]
pub struct CrawlingConfig {
    pub session_id: String,
    pub start_url: String,
    pub target_domains: Vec<String>,
    pub max_pages: u32,
    pub concurrent_requests: u32,
    pub delay_ms: u64,
    pub http_config: HttpClientConfig,
}

impl From<StartCrawlingDto> for CrawlingConfig {
    fn from(dto: StartCrawlingDto) -> Self {
        Self {
            session_id: Uuid::new_v4().to_string(),
            start_url: dto.start_url,
            target_domains: dto.target_domains,
            max_pages: dto.max_pages.unwrap_or(100),
            concurrent_requests: dto.concurrent_requests.unwrap_or(3),
            delay_ms: dto.delay_ms.unwrap_or(1000),
            http_config: HttpClientConfig {
                max_requests_per_second: 2,
                ..Default::default()
            },
        }
    }
}

/// Represents a crawled page with extracted data
#[derive(Debug, Clone)]
pub struct CrawledPage {
    pub url: String,
    pub content: String,
    pub title: Option<String>,
    pub links: Vec<String>,
    pub status_code: u16,
}

/// Main web crawler engine
pub struct WebCrawler {
    http_client: HttpClient,
    session_manager: Arc<SessionManager>,
    visited_urls: Arc<Mutex<HashSet<String>>>,
}

impl WebCrawler {
    /// Create a new web crawler instance
    pub fn new(
        http_config: HttpClientConfig,
        session_manager: Arc<SessionManager>,
    ) -> Result<Self> {
        let http_client = HttpClient::new(http_config)?;
        
        Ok(Self {
            http_client,
            session_manager,
            visited_urls: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    /// Start crawling with the given configuration
    pub async fn start_crawling(&self, config: CrawlingConfig) -> Result<String> {
        // Create config snapshot for session
        let config_snapshot = serde_json::to_value(&config)
            .context("Failed to serialize crawling config")?;

        // Start session with SessionManager
        let session_id = self.session_manager.start_session(
            config_snapshot,
            config.max_pages,
            CrawlingStage::ProductList,
        ).await;

        // Start crawling in background
        let crawler = self.clone();
        let session_id_clone = session_id.clone();
        tokio::spawn(async move {
            if let Err(e) = crawler.run_crawling_session(session_id_clone, config).await {
                tracing::error!("Crawling session failed: {}", e);
            }
        });

        Ok(session_id)
    }

    /// Run the actual crawling session
    async fn run_crawling_session(&self, session_id: String, config: CrawlingConfig) -> Result<()> {
        tracing::info!("Starting crawling session: {}", session_id);

        // Update session to running state
        self.session_manager.set_status(&session_id, SessionStatus::Running).await
            .map_err(|e| anyhow::anyhow!("Failed to set session status: {}", e))?;

        // Initialize crawling queue
        let mut urls_to_crawl = vec![config.start_url.clone()];
        let mut pages_crawled = 0u32;

        while !urls_to_crawl.is_empty() && pages_crawled < config.max_pages {
            // Check if session should continue
            if let Some(session) = self.session_manager.get_session(&session_id).await {
                match session.status {
                    SessionStatus::Stopped => {
                        tracing::info!("Crawling session stopped by user: {}", session_id);
                        return Ok(());
                    }
                    SessionStatus::Paused => {
                        tracing::info!("Crawling session paused: {}", session_id);
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                        continue;
                    }
                    _ => {}
                }
            }

            // Get next URL to crawl
            let current_url = urls_to_crawl.remove(0);
            
            // Skip if already visited
            {
                let mut visited = self.visited_urls.lock().await;
                if visited.contains(&current_url) {
                    continue;
                }
                visited.insert(current_url.clone());
            }

            // Check if URL is allowed by domain restrictions
            if !self.is_allowed_domain(&current_url, &config.target_domains) {
                tracing::debug!("Skipping URL (domain restriction): {}", current_url);
                continue;
            }

            // Crawl the page
            match self.crawl_page(&current_url).await {
                Ok(page) => {
                    pages_crawled += 1;
                    
                    tracing::info!("Crawled page {}/{}: {}", 
                        pages_crawled, config.max_pages, current_url);

                    // Update session progress
                    self.session_manager.update_session_progress(
                        &session_id,
                        pages_crawled,
                        current_url.clone(),
                    ).await
                    .map_err(|e| anyhow::anyhow!("Failed to update session progress: {}", e))?;

                    // Extract additional URLs from this page
                    let new_urls = self.extract_product_links(&page.content, &config.target_domains)?;
                    urls_to_crawl.extend(new_urls);

                    // Add delay between requests
                    if config.delay_ms > 0 {
                        tokio::time::sleep(tokio::time::Duration::from_millis(config.delay_ms)).await;
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to crawl {}: {}", current_url, e);
                    
                    // Update error count
                    self.session_manager.add_error(&session_id, format!("Failed to crawl {}: {}", current_url, e)).await
                        .map_err(|e| anyhow::anyhow!("Failed to add error: {}", e))?;
                }
            }
        }

        // Mark session as completed
        self.session_manager.set_status(&session_id, SessionStatus::Completed).await
            .map_err(|e| anyhow::anyhow!("Failed to set session as completed: {}", e))?;
        
        tracing::info!("Crawling session completed: {} ({} pages)", session_id, pages_crawled);
        
        Ok(())
    }

    /// Crawl a single page and return extracted data
    pub async fn crawl_page(&self, url: &str) -> Result<CrawledPage> {
        // Check robots.txt if configured
        if !self.http_client.is_allowed_by_robots(url).await? {
            anyhow::bail!("URL disallowed by robots.txt: {}", url);
        }

        // Fetch the page
        let response = self.http_client.get(url).await?;
        let status_code = response.status().as_u16();
        let content = response.text().await
            .with_context(|| format!("Failed to read response body from: {}", url))?;

        // Extract basic information
        let title = self.extract_title(&content);
        let links = self.extract_links(&content, url)?;

        Ok(CrawledPage {
            url: url.to_string(),
            content,
            title,
            links,
            status_code,
        })
    }

    /// Extract product links from HTML content
    pub fn extract_product_links(&self, html: &str, target_domains: &[String]) -> Result<Vec<String>> {
        use scraper::{Html, Selector};

        let document = Html::parse_document(html);
        let link_selector = Selector::parse("a[href]")
            .map_err(|e| anyhow::anyhow!("Invalid CSS selector: {:?}", e))?;

        let mut links = Vec::new();

        for element in document.select(&link_selector) {
            if let Some(href) = element.value().attr("href") {
                // Convert relative URLs to absolute
                if let Ok(absolute_url) = self.resolve_url(href, &document) {
                    // Check if it's a product-related URL
                    if self.is_product_url(&absolute_url) && 
                       self.is_allowed_domain(&absolute_url, target_domains) {
                        links.push(absolute_url);
                    }
                }
            }
        }

        Ok(links)
    }

    /// Extract title from HTML content
    fn extract_title(&self, html: &str) -> Option<String> {
        use scraper::{Html, Selector};

        let document = Html::parse_document(html);
        let title_selector = Selector::parse("title").ok()?;
        
        document.select(&title_selector)
            .next()
            .map(|element| element.text().collect::<String>().trim().to_string())
    }

    /// Extract all links from HTML content
    fn extract_links(&self, html: &str, _base_url: &str) -> Result<Vec<String>> {
        use scraper::{Html, Selector};

        let document = Html::parse_document(html);
        let link_selector = Selector::parse("a[href]")
            .map_err(|e| anyhow::anyhow!("Invalid CSS selector: {:?}", e))?;

        let mut links = Vec::new();

        for element in document.select(&link_selector) {
            if let Some(href) = element.value().attr("href") {
                if let Ok(absolute_url) = self.resolve_url(href, &document) {
                    links.push(absolute_url);
                }
            }
        }

        Ok(links)
    }

    /// Check if URL is within allowed domains
    fn is_allowed_domain(&self, url: &str, target_domains: &[String]) -> bool {
        if target_domains.is_empty() {
            return true;
        }

        target_domains.iter().any(|domain| url.contains(domain))
    }

    /// Check if URL looks like a product page
    fn is_product_url(&self, url: &str) -> bool {
        // Basic heuristics for product URLs
        let product_indicators = [
            "/product", "/item", "/p/", "/certification", 
            "/certified", "/matter", "/device"
        ];
        
        product_indicators.iter().any(|indicator| url.contains(indicator))
    }

    /// Resolve relative URL to absolute URL
    fn resolve_url(&self, href: &str, _document: &scraper::Html) -> Result<String> {
        // This is a simplified implementation
        // In production, use a proper URL resolution library
        if href.starts_with("http") {
            Ok(href.to_string())
        } else if href.starts_with("//") {
            Ok(format!("https:{}", href))
        } else if href.starts_with('/') {
            // Need base URL context - simplified for now
            Ok(format!("https://certification.csa-iot.org{}", href))
        } else {
            Ok(href.to_string())
        }
    }
}

// Clone implementation for tokio::spawn
impl Clone for WebCrawler {
    fn clone(&self) -> Self {
        Self {
            http_client: HttpClient::new(self.http_client.config().clone()).unwrap(),
            session_manager: Arc::clone(&self.session_manager),
            visited_urls: Arc::clone(&self.visited_urls),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crawling_config_from_dto() {
        let dto = StartCrawlingDto {
            start_url: "https://example.com".to_string(),
            target_domains: vec!["example.com".to_string()],
            max_pages: Some(50),
            concurrent_requests: Some(5),
            delay_ms: Some(2000),
        };

        let config = CrawlingConfig::from(dto);
        assert_eq!(config.start_url, "https://example.com");
        assert_eq!(config.max_pages, 50);
        assert_eq!(config.concurrent_requests, 5);
        assert_eq!(config.delay_ms, 2000);
    }

    #[test]
    fn test_is_product_url() {
        let crawler = create_test_crawler();
        
        assert!(crawler.is_product_url("https://example.com/product/123"));
        assert!(crawler.is_product_url("https://example.com/certification/matter"));
        assert!(!crawler.is_product_url("https://example.com/about"));
    }

    fn create_test_crawler() -> WebCrawler {
        let session_manager = Arc::new(SessionManager::new());
        WebCrawler::new(HttpClientConfig::default(), session_manager).unwrap()
    }
}
