use anyhow::{Context, Result};
use scraper::{Html, Selector};
use std::sync::Arc;
use sqlx::SqlitePool;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::domain::matter_product::{MatterProduct, MatterCrawlerConfig, MatterCrawlingSession, CrawlingStage};
use crate::infrastructure::http_client::{HttpClient, HttpClientConfig};
use crate::infrastructure::matter_product_repository::MatterProductRepository;

/// Main crawler for CSA-IoT Matter products
pub struct MatterProductsCrawler {
    http_client: HttpClient,
    repository: MatterProductRepository,
    config: MatterCrawlerConfig,
    session: Arc<Mutex<MatterCrawlingSession>>,
}

impl MatterProductsCrawler {
    /// Create a new Matter products crawler
    pub fn new(
        pool: SqlitePool,
        config: MatterCrawlerConfig,
    ) -> Result<Self> {
        // Configure HTTP client for CSA-IoT crawling
        let http_config = HttpClientConfig {
            user_agent: config.user_agent.clone().unwrap_or_else(|| "MatterCertis/1.0".to_string()),
            timeout_seconds: config.request_timeout_seconds,
            max_requests_per_second: (1000 / config.rate_limit_ms.max(100)) as u32,
            respect_robots_txt: true,
            follow_redirects: true,
        };
        
        let http_client = HttpClient::new(http_config)
            .context("Failed to create HTTP client")?;
            
        let repository = MatterProductRepository::new(pool);
        
        let session_id = Uuid::new_v4().to_string();
        let session = Arc::new(Mutex::new(MatterCrawlingSession::new(session_id, config.clone())));
        
        Ok(Self {
            http_client,
            repository,
            config,
            session,
        })
    }
    
    /// Start crawling Matter products
    pub async fn crawl_matter_products(&self) -> Result<CrawlingResults> {
        info!("Starting Matter products crawling");
        
        let mut results = CrawlingResults::new();
        
        // Phase 1: Crawl product listing pages
        match self.crawl_listing_pages().await {
            Ok(listing_results) => {
                results.products_found = listing_results.len();
                info!("Found {} products from listing pages", listing_results.len());
                
                // Phase 2: Crawl product detail pages
                match self.crawl_detail_pages(listing_results).await {
                    Ok(detailed_results) => {
                        results.products_detailed = detailed_results;
                        info!("Successfully detailed {} products", detailed_results);
                        
                        // Mark session as completed
                        {
                            let mut session = self.session.lock().await;
                            session.complete();
                        }
                        
                        results.success = true;
                    }
                    Err(err) => {
                        error!("Failed to crawl detail pages: {}", err);
                        results.add_error(format!("Detail crawling failed: {}", err));
                        
                        // Mark session as failed
                        {
                            let mut session = self.session.lock().await;
                            session.fail(err.to_string());
                        }
                    }
                }
            }
            Err(err) => {
                error!("Failed to crawl listing pages: {}", err);
                results.add_error(format!("Listing crawling failed: {}", err));
                
                // Mark session as failed
                {
                    let mut session = self.session.lock().await;
                    session.fail(err.to_string());
                }
            }
        }
        
        Ok(results)
    }
    
    /// Crawl product listing pages to find all Matter products
    async fn crawl_listing_pages(&self) -> Result<Vec<MatterProduct>> {
        info!("Starting to crawl Matter product listing pages");
        
        let mut all_products = Vec::new();
        let start_page = self.config.start_page;
        let max_pages = self.config.max_pages.unwrap_or(u32::MAX);
        
        for page_num in start_page..=(start_page + max_pages - 1) {
            // Update session progress
            {
                let mut session = self.session.lock().await;
                session.update_progress(page_num, all_products.len() as u32);
            }
            
            info!("Crawling page {}", page_num);
            
            match self.crawl_single_listing_page(page_num).await {
                Ok(mut products) => {
                    if products.is_empty() {
                        info!("No products found on page {}, stopping", page_num);
                        break;
                    }
                    
                    info!("Found {} products on page {}", products.len(), page_num);
                    all_products.append(&mut products);
                }
                Err(err) => {
                    warn!("Failed to crawl page {}: {}", page_num, err);
                    
                    // Update session with error
                    {
                        let mut session = self.session.lock().await;
                        session.add_error(format!("Page {} failed: {}", page_num, err));
                    }
                    
                    // Continue with next page instead of failing completely
                    continue;
                }
            }
            
            // Check if we've reached our limit
            if let Some(max) = self.config.max_pages {
                if page_num >= start_page + max - 1 {
                    info!("Reached maximum page limit of {}", max);
                    break;
                }
            }
        }
        
        info!("Completed listing pages crawling. Found {} total products", all_products.len());
        Ok(all_products)
    }
    
    /// Crawl a single listing page
    async fn crawl_single_listing_page(&self, page: u32) -> Result<Vec<MatterProduct>> {
        let url = self.build_listing_page_url(page)?;
        
        debug!("Fetching listing page: {}", url);
        
        // Update session current URL
        {
            let mut session = self.session.lock().await;
            session.current_url = Some(url.clone());
        }
        
        let html_content = self.http_client.get_text(&url).await
            .context("Failed to fetch listing page")?;
            
        self.parse_listing_page(&html_content, page, &url).await
    }
    
    /// Parse a listing page HTML to extract product information
    async fn parse_listing_page(&self, html: &str, page: u32, page_url: &str) -> Result<Vec<MatterProduct>> {
        debug!("Parsing listing page HTML");
        
        let document = Html::parse_document(html);
        
        // Try different selectors for product cards
        let selectors_to_try = [
            // These selectors need to be refined based on actual CSA-IoT HTML structure
            "article.product-card",
            ".product-listing .product",
            "[class*='product']",
            "a[href*='/csa_product/']",
        ];
        
        let mut products = Vec::new();
        
        for selector_str in &selectors_to_try {
            if let Ok(selector) = Selector::parse(selector_str) {
                let elements = document.select(&selector);
                let found_count = elements.count();
                
                if found_count > 0 {
                    debug!("Found {} elements with selector: {}", found_count, selector_str);
                    
                    // Re-select because iterator was consumed
                    let elements = document.select(&selector);
                    
                    for (index, element) in elements.enumerate() {
                        match self.extract_product_from_element(&element, page, index, page_url).await {
                            Ok(Some(product)) => products.push(product),
                            Ok(None) => continue, // Not a valid product
                            Err(err) => {
                                warn!("Failed to extract product from element {}: {}", index, err);
                                continue;
                            }
                        }
                    }
                    
                    if !products.is_empty() {
                        break; // Found products with this selector
                    }
                }
            }
        }
        
        if products.is_empty() {
            // Fallback: try to extract from text patterns
            products = self.extract_products_from_text(html, page, page_url).await?;
        }
        
        debug!("Extracted {} products from page {}", products.len(), page);
        Ok(products)
    }
    
    /// Extract product information from HTML element
    async fn extract_product_from_element(
        &self,
        element: &scraper::ElementRef<'_>,
        page: u32,
        index: usize,
        page_url: &str,
    ) -> Result<Option<MatterProduct>> {
        // This needs to be implemented based on actual HTML structure
        // For now, return None as placeholder
        debug!("Extracting product from element {} on page {}", index, page);
        Ok(None)
    }
    
    /// Fallback method to extract products from text patterns
    async fn extract_products_from_text(&self, _html: &str, page: u32, _page_url: &str) -> Result<Vec<MatterProduct>> {
        // This will be implemented to parse the text-based format we saw in the analysis
        debug!("Using text-based extraction for page {}", page);
        Ok(Vec::new())
    }
    
    /// Build URL for a specific listing page
    fn build_listing_page_url(&self, page: u32) -> Result<String> {
        if page == 1 {
            Ok(self.config.matter_filter_url.clone())
        } else {
            // Add pagination to the URL
            let base_url = &self.config.matter_filter_url;
            
            // Insert /page/{page}/ before the query parameters
            if let Some(query_start) = base_url.find('?') {
                let (path_part, query_part) = base_url.split_at(query_start);
                let path_with_page = if path_part.ends_with('/') {
                    format!("{}page/{}/", path_part, page)
                } else {
                    format!("{}/page/{}/", path_part, page)
                };
                Ok(format!("{}{}", path_with_page, query_part))
            } else {
                let path_with_page = if base_url.ends_with('/') {
                    format!("{}page/{}/", base_url, page)
                } else {
                    format!("{}/page/{}/", base_url, page)
                };
                Ok(path_with_page)
            }
        }
    }
    
    /// Crawl detail pages for all products
    async fn crawl_detail_pages(&self, products: Vec<MatterProduct>) -> Result<usize> {
        info!("Starting to crawl {} product detail pages", products.len());
        
        // Update session stage
        {
            let mut session = self.session.lock().await;
            session.move_to_details_stage();
        }
        
        let mut detailed_count = 0;
        
        for (index, mut product) in products.into_iter().enumerate() {
            debug!("Crawling detail page {} of {}: {}", index + 1, products.len(), product.detail_url);
            
            match self.crawl_product_detail(&mut product).await {
                Ok(()) => {
                    // Save to database
                    match self.repository.upsert(&product).await {
                        Ok(_) => {
                            detailed_count += 1;
                            debug!("Successfully saved product: {}", product.certificate_id);
                        }
                        Err(err) => {
                            error!("Failed to save product {}: {}", product.certificate_id, err);
                        }
                    }
                }
                Err(err) => {
                    warn!("Failed to crawl detail page for {}: {}", product.certificate_id, err);
                    
                    // Save basic info even if detail crawling failed
                    if let Err(save_err) = self.repository.upsert(&product).await {
                        error!("Failed to save basic product info for {}: {}", product.certificate_id, save_err);
                    }
                }
            }
            
            // Update session progress
            {
                let mut session = self.session.lock().await;
                session.products_detailed = detailed_count;
            }
        }
        
        info!("Completed detail pages crawling. Successfully detailed {} products", detailed_count);
        Ok(detailed_count as usize)
    }
    
    /// Crawl a single product detail page
    async fn crawl_product_detail(&self, product: &mut MatterProduct) -> Result<()> {
        debug!("Fetching product detail page: {}", product.detail_url);
        
        let html_content = self.http_client.get_text(&product.detail_url).await
            .context("Failed to fetch product detail page")?;
            
        self.parse_product_detail(product, &html_content).await
    }
    
    /// Parse product detail page HTML
    async fn parse_product_detail(&self, product: &mut MatterProduct, _html: &str) -> Result<()> {
        debug!("Parsing product detail HTML for: {}", product.certificate_id);
        
        // This will be implemented based on the actual HTML structure
        // For now, just mark as parsed
        
        Ok(())
    }
    
    /// Get current session state
    pub async fn get_session_state(&self) -> MatterCrawlingSession {
        self.session.lock().await.clone()
    }
}

/// Results from a crawling operation
#[derive(Debug, Clone)]
pub struct CrawlingResults {
    pub success: bool,
    pub products_found: usize,
    pub products_detailed: usize,
    pub errors: Vec<String>,
}

impl CrawlingResults {
    pub fn new() -> Self {
        Self {
            success: false,
            products_found: 0,
            products_detailed: 0,
            errors: Vec::new(),
        }
    }
    
    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
    }
}
