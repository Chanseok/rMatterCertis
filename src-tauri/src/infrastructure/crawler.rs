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
// use crate::infrastructure::repositories::{SqliteProductRepository, SqliteVendorRepository};  // Temporarily disabled
use crate::infrastructure::integrated_product_repository::IntegratedProductRepository;
use crate::domain::session_manager::{SessionManager, SessionStatus, CrawlingStage};
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

/// Extracted product data from crawled page
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExtractedProduct {
    pub url: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub certificate_id: Option<String>,
    pub page_id: u32,
    pub index_in_page: u32,
}

/// Extracted Matter product data with detailed certification info
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExtractedMatterProduct {
    pub url: String,
    pub page_id: u32,
    pub index_in_page: u32,
    pub id: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub device_type: Option<String>,
    pub certificate_id: Option<String>,
    pub certification_date: Option<String>,
    pub software_version: Option<String>,
    pub hardware_version: Option<String>,
    pub vid: Option<String>,
    pub pid: Option<String>,
    pub family_sku: Option<String>,
    pub family_variant_sku: Option<String>,
    pub firmware_version: Option<String>,
    pub family_id: Option<String>,
    pub tis_trp_tested: Option<String>,
    pub specification_version: Option<String>,
    pub transport_interface: Option<String>,
    pub primary_device_type_id: Option<String>,
    pub application_categories: Option<Vec<String>>,
}

/// Main web crawler engine
pub struct WebCrawler {
    http_client: HttpClient,
    session_manager: Arc<SessionManager>,
    visited_urls: Arc<Mutex<HashSet<String>>>,
    product_repo: Arc<SqliteProductRepository>,
    vendor_repo: Arc<SqliteVendorRepository>,
}

impl WebCrawler {
    /// Create a new web crawler instance
    pub fn new(
        http_config: HttpClientConfig,
        session_manager: Arc<SessionManager>,
        product_repo: Arc<SqliteProductRepository>,
        vendor_repo: Arc<SqliteVendorRepository>,
    ) -> Result<Self> {
        let http_client = HttpClient::new(http_config)?;
        
        Ok(Self {
            http_client,
            session_manager,
            visited_urls: Arc::new(Mutex::new(HashSet::new())),
            product_repo,
            vendor_repo,
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

                    // Extract and save product data
                    if let Err(e) = self.extract_and_save_products(&page, &session_id, pages_crawled).await {
                        tracing::error!("Failed to extract/save products from {}: {}", current_url, e);
                        self.session_manager.add_error(&session_id, format!("Product extraction failed for {current_url}: {e}")).await
                            .map_err(|e| anyhow::anyhow!("Failed to add error: {}", e))?;
                    }

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
                    self.session_manager.add_error(&session_id, format!("Failed to crawl {current_url}: {e}")).await
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
            .with_context(|| format!("Failed to read response body from: {url}"))?;

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
            Ok(format!("https:{href}"))
        } else if href.starts_with('/') {
            // Need base URL context - simplified for now
            Ok(format!("https://certification.csa-iot.org{href}"))
        } else {
            Ok(href.to_string())
        }
    }

    /// Extract and save product data from a crawled page
    async fn extract_and_save_products(&self, page: &CrawledPage, _session_id: &str, page_id: u32) -> Result<()> {
        // Try to extract Matter product data first (more detailed)
        if let Ok(matter_products) = self.extract_matter_products(&page.content, page_id) {
            if !matter_products.is_empty() {
                tracing::info!("Extracted {} Matter products from page: {}", matter_products.len(), page.url);
                
                for (index, product) in matter_products.iter().enumerate() {
                    if let Err(e) = self.save_matter_product(product).await {
                        tracing::error!("Failed to save Matter product {}: {}", index, e);
                    } else {
                        tracing::debug!("Saved Matter product: {} - {}", 
                            product.manufacturer.as_deref().unwrap_or("Unknown"),
                            product.model.as_deref().unwrap_or("Unknown"));
                    }
                }
                return Ok(());
            }
        }

        // Fallback to basic product extraction
        if let Ok(products) = self.extract_basic_products(&page.content, page_id) {
            if !products.is_empty() {
                tracing::info!("Extracted {} basic products from page: {}", products.len(), page.url);
                
                for (index, product) in products.iter().enumerate() {
                    if let Err(e) = self.save_basic_product(product).await {
                        tracing::error!("Failed to save basic product {}: {}", index, e);
                    } else {
                        tracing::debug!("Saved basic product: {} - {}", 
                            product.manufacturer.as_deref().unwrap_or("Unknown"),
                            product.model.as_deref().unwrap_or("Unknown"));
                    }
                }
            }
        }

        Ok(())
    }

    /// Extract Matter product data from HTML content
    fn extract_matter_products(&self, html: &str, page_id: u32) -> Result<Vec<ExtractedMatterProduct>> {
        use scraper::{Html, Selector};

        let document = Html::parse_document(html);
        let mut products = Vec::new();

        // Look for Matter certification table or similar structured data
        // This is a simplified implementation - in production, you'd need more sophisticated parsing
        
        // Try to find table rows with product data
        if let Ok(row_selector) = Selector::parse("tr, .product-row, .certification-item") {
            for (index, element) in document.select(&row_selector).enumerate() {
                if let Some(product) = self.parse_matter_product_from_element(&element, page_id, index as u32) {
                    products.push(product);
                }
            }
        }

        // If no structured data found, try to parse from JSON-LD or other metadata
        if products.is_empty() {
            if let Ok(script_selector) = Selector::parse("script[type='application/ld+json']") {
                for element in document.select(&script_selector) {
                    if let Some(json_text) = element.text().next() {
                        if let Ok(matter_product) = self.parse_matter_product_from_json(json_text, page_id) {
                            products.push(matter_product);
                        }
                    }
                }
            }
        }

        Ok(products)
    }

    /// Extract basic product data from HTML content
    fn extract_basic_products(&self, html: &str, page_id: u32) -> Result<Vec<ExtractedProduct>> {
        use scraper::{Html, Selector};

        let document = Html::parse_document(html);
        let mut products = Vec::new();

        // Look for product listings or individual product info
        if let Ok(product_selector) = Selector::parse(".product, .item, .certification-entry, tr") {
            for (index, element) in document.select(&product_selector).enumerate() {
                if let Some(product) = self.parse_basic_product_from_element(&element, page_id, index as u32) {
                    products.push(product);
                }
            }
        }

        Ok(products)
    }

    /// Parse Matter product from HTML element
    fn parse_matter_product_from_element(&self, element: &scraper::ElementRef, page_id: u32, index: u32) -> Option<ExtractedMatterProduct> {
        use scraper::Selector;

        let text_content = element.text().collect::<String>();
        
        // Skip empty or irrelevant rows
        if text_content.trim().is_empty() || text_content.len() < 10 {
            return None;
        }

        // Try to extract fields from table cells or structured data
        let cells: Vec<String> = if let Ok(cell_selector) = Selector::parse("td, th, .field, .value") {
            element.select(&cell_selector)
                .map(|cell| cell.text().collect::<String>().trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        } else {
            vec![text_content.trim().to_string()]
        };

        // Skip headers or non-product rows
        if cells.iter().any(|cell| cell.to_lowercase().contains("manufacturer") || 
                                   cell.to_lowercase().contains("device type") ||
                                   cell.to_lowercase().contains("header")) {
            return None;
        }

        // Basic heuristic parsing - this would need refinement for specific website structures
        let manufacturer = cells.first().filter(|s| !s.is_empty()).cloned();
        let model = cells.get(1).filter(|s| !s.is_empty()).cloned();
        let device_type = cells.get(2).filter(|s| !s.is_empty()).cloned();
        let certificate_id = cells.iter().find(|cell| 
            cell.len() > 5 && (cell.contains("CSA") || cell.contains("CERT") || cell.chars().all(|c| c.is_alphanumeric() || c == '-'))
        ).cloned();

        // Only create product if we have meaningful data
        if manufacturer.is_some() || model.is_some() || certificate_id.is_some() {
            Some(ExtractedMatterProduct {
                url: format!("page_{page_id}"), // Placeholder - would be actual product URL
                page_id,
                index_in_page: index,
                id: None,
                manufacturer,
                model,
                device_type,
                certificate_id,
                certification_date: None,
                software_version: None,
                hardware_version: None,
                vid: None,
                pid: None,
                family_sku: None,
                family_variant_sku: None,
                firmware_version: None,
                family_id: None,
                tis_trp_tested: None,
                specification_version: None,
                transport_interface: None,
                primary_device_type_id: None,
                application_categories: None,
            })
        } else {
            None
        }
    }

    /// Parse basic product from HTML element
    fn parse_basic_product_from_element(&self, element: &scraper::ElementRef, page_id: u32, index: u32) -> Option<ExtractedProduct> {
        use scraper::Selector;

        let text_content = element.text().collect::<String>();
        
        if text_content.trim().is_empty() || text_content.len() < 5 {
            return None;
        }

        // Extract from table cells or structured elements
        let cells: Vec<String> = if let Ok(cell_selector) = Selector::parse("td, .field-value, .product-field") {
            element.select(&cell_selector)
                .map(|cell| cell.text().collect::<String>().trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        } else {
            vec![text_content.trim().to_string()]
        };

        let manufacturer = cells.first().filter(|s| !s.is_empty()).cloned();
        let model = cells.get(1).filter(|s| !s.is_empty()).cloned();
        let certificate_id = cells.iter().find(|cell| 
            cell.len() > 3 && cell.chars().any(|c| c.is_numeric())
        ).cloned();

        if manufacturer.is_some() || model.is_some() {
            Some(ExtractedProduct {
                url: format!("page_{page_id}_item_{index}"),
                manufacturer,
                model,
                certificate_id,
                page_id,
                index_in_page: index,
            })
        } else {
            None
        }
    }

    /// Parse Matter product from JSON-LD data
    fn parse_matter_product_from_json(&self, _json_text: &str, page_id: u32) -> Result<ExtractedMatterProduct> {
        // Simplified JSON parsing - in production, use proper JSON parsing with serde
        let product = ExtractedMatterProduct {
            url: format!("json_page_{page_id}"),
            page_id,
            index_in_page: 0,
            id: None,
            manufacturer: None,
            model: None,
            device_type: None,
            certificate_id: None,
            certification_date: None,
            software_version: None,
            hardware_version: None,
            vid: None,
            pid: None,
            family_sku: None,
            family_variant_sku: None,
            firmware_version: None,
            family_id: None,
            tis_trp_tested: None,
            specification_version: None,
            transport_interface: None,
            primary_device_type_id: None,
            application_categories: None,
        };

        Ok(product)
    }

    /// Save Matter product to database
    async fn save_matter_product(&self, product: &ExtractedMatterProduct) -> Result<()> {
        // Create MatterProduct entity
        let matter_product = crate::domain::entities::MatterProduct {
            url: product.url.clone(),
            page_id: Some(product.page_id),
            index_in_page: Some(product.index_in_page),
            id: product.id.clone(),
            manufacturer: product.manufacturer.clone(),
            model: product.model.clone(),
            device_type: product.device_type.clone(),
            certificate_id: product.certificate_id.clone(),
            certification_date: product.certification_date.clone(),
            software_version: product.software_version.clone(),
            hardware_version: product.hardware_version.clone(),
            vid: product.vid.clone(),
            pid: product.pid.clone(),
            family_sku: product.family_sku.clone(),
            family_variant_sku: product.family_variant_sku.clone(),
            firmware_version: product.firmware_version.clone(),
            family_id: product.family_id.clone(),
            tis_trp_tested: product.tis_trp_tested.clone(),
            specification_version: product.specification_version.clone(),
            transport_interface: product.transport_interface.clone(),
            primary_device_type_id: product.primary_device_type_id.clone(),
            application_categories: product.application_categories.clone().unwrap_or_default(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        // Save using repository
        use crate::domain::repositories::ProductRepository;
        self.product_repo.save_matter_product(&matter_product).await
            .context("Failed to save Matter product to database")?;

        tracing::debug!("Saved Matter product: {} - {}", 
            product.manufacturer.as_deref().unwrap_or("Unknown"),
            product.model.as_deref().unwrap_or("Unknown"));
        
        Ok(())
    }

    /// Save basic product to database
    async fn save_basic_product(&self, product: &ExtractedProduct) -> Result<()> {
        // Create Product entity
        let basic_product = crate::domain::entities::Product {
            url: product.url.clone(),
            manufacturer: product.manufacturer.clone(),
            model: product.model.clone(),
            certificate_id: product.certificate_id.clone(),
            page_id: Some(product.page_id),
            index_in_page: Some(product.index_in_page),
            created_at: chrono::Utc::now(),
        };

        // Save using repository
        use crate::domain::repositories::ProductRepository;
        self.product_repo.save_product(&basic_product).await
            .context("Failed to save basic product to database")?;

        tracing::debug!("Saved basic product: {} - {}", 
            product.manufacturer.as_deref().unwrap_or("Unknown"),
            product.model.as_deref().unwrap_or("Unknown"));
        
        Ok(())
    }
}

// Clone implementation for tokio::spawn
impl Clone for WebCrawler {
    fn clone(&self) -> Self {
        Self {
            http_client: HttpClient::new(self.http_client.config().clone()).unwrap(),
            session_manager: Arc::clone(&self.session_manager),
            visited_urls: Arc::clone(&self.visited_urls),
            product_repo: Arc::clone(&self.product_repo),
            vendor_repo: Arc::clone(&self.vendor_repo),
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
        use std::sync::Arc;
        use crate::infrastructure::{SqliteProductRepository, SqliteVendorRepository, DatabaseConnection};
        
        // Create test database
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (product_repo, vendor_repo) = rt.block_on(async {
            let db = DatabaseConnection::new("sqlite::memory:").await.unwrap();
            db.migrate().await.unwrap();
            let product_repo = Arc::new(SqliteProductRepository::new(db.pool().clone()));
            let vendor_repo = Arc::new(SqliteVendorRepository::new(db.pool().clone()));
            (product_repo, vendor_repo)
        });
        
        let session_manager = Arc::new(SessionManager::new());
        WebCrawler::new(HttpClientConfig::default(), session_manager, product_repo, vendor_repo).unwrap()
    }
}
