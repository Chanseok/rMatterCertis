//! Parsing service layer following the guide's architecture
//!
//! High-level service for coordinating HTML parsing operations with
//! business logic and validation.

use crate::domain::product::{Product, ProductDetail};
use crate::infrastructure::parsing::context::DetailParseContext;
use crate::infrastructure::parsing::{
    ContextualParser, ParseContext, ParsingConfig, ParsingResult, ProductDetailParser,
    ProductListParser,
};
use anyhow::{Context, Result};
use scraper::Html;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// High-level parsing service that coordinates parsing operations
pub struct ParsingService {
    list_parser: Arc<ProductListParser>,
    detail_parser: Arc<ProductDetailParser>,
    config: ParsingConfig,
}

impl ParsingService {
    /// Create a new parsing service with the given configuration
    pub fn new(config: ParsingConfig) -> Result<Self> {
        let list_parser = Arc::new(
            ProductListParser::with_config(&config.product_list_selectors)
                .context("Failed to create product list parser")?,
        );

        let detail_parser = Arc::new(
            ProductDetailParser::with_config(&config.product_detail_selectors)
                .context("Failed to create product detail parser")?,
        );

        Ok(Self {
            list_parser,
            detail_parser,
            config,
        })
    }

    /// Parse product list from HTML content
    pub fn parse_product_list(&self, html: &str, page_id: u32) -> ParsingResult<Vec<Product>> {
        debug!("Parsing product list for page {}", page_id);

        let html_doc = Html::parse_document(html);
        let context = ParseContext::new(page_id, self.config.base_url.clone());

        let products = self.list_parser.parse_with_context(&html_doc, &context)?;

        info!(
            "Successfully parsed {} products from page {}",
            products.len(),
            page_id
        );
        Ok(products)
    }

    /// Parse product detail from HTML content
    pub fn parse_product_detail(&self, html: &str, url: &str) -> ParsingResult<ProductDetail> {
        debug!("Parsing product detail for URL: {}", url);

        let html_doc = Html::parse_document(html);
        let context = DetailParseContext::new(url.to_string(), self.config.base_url.clone());

        let product = self.detail_parser.parse_with_context(&html_doc, &context)?;

        info!(
            "Successfully parsed product detail: {}",
            product.model.as_ref().unwrap_or(&"Unknown".to_string())
        );
        Ok(product)
    }

    /// Check if a listing page has pagination
    pub fn has_next_page(&self, html: &str) -> bool {
        let html_doc = Html::parse_document(html);
        self.list_parser.has_next_page(&html_doc)
    }

    /// Validate parsed product data
    pub fn validate_product(&self, product: &Product) -> ParsingResult<()> {
        if product.url.is_empty() {
            return Err(
                crate::infrastructure::parsing_error::ParsingError::ProductValidationFailed {
                    reason: "Product URL is empty".to_string(),
                    field_errors: vec!["url".to_string()],
                },
            );
        }

        if product.model.is_none() || product.model.as_ref().unwrap().is_empty() {
            return Err(
                crate::infrastructure::parsing_error::ParsingError::ProductValidationFailed {
                    reason: "Product model is missing".to_string(),
                    field_errors: vec!["model".to_string()],
                },
            );
        }

        Ok(())
    }

    /// Validate parsed product detail data
    pub fn validate_product_detail(&self, product: &ProductDetail) -> ParsingResult<()> {
        if product.url.is_empty() {
            return Err(
                crate::infrastructure::parsing_error::ParsingError::ProductValidationFailed {
                    reason: "Product detail URL is empty".to_string(),
                    field_errors: vec!["url".to_string()],
                },
            );
        }

        if product.model.is_none() || product.model.as_ref().unwrap().is_empty() {
            return Err(
                crate::infrastructure::parsing_error::ParsingError::ProductValidationFailed {
                    reason: "Product detail model is missing".to_string(),
                    field_errors: vec!["model".to_string()],
                },
            );
        }

        Ok(())
    }

    /// Get current configuration
    pub fn get_config(&self) -> &ParsingConfig {
        &self.config
    }

    /// Update service configuration
    pub fn update_config(&mut self, config: ParsingConfig) -> Result<()> {
        self.list_parser = Arc::new(
            ProductListParser::with_config(&config.product_list_selectors)
                .context("Failed to update product list parser")?,
        );

        self.detail_parser = Arc::new(
            ProductDetailParser::with_config(&config.product_detail_selectors)
                .context("Failed to update product detail parser")?,
        );

        self.config = config;
        info!("Parsing service configuration updated successfully");
        Ok(())
    }
}

/// Crawler service that combines web crawling with parsing
pub struct CrawlerService {
    parsing_service: Arc<ParsingService>,
}

impl CrawlerService {
    /// Create a new crawler service
    pub fn new(config: ParsingConfig) -> Result<Self> {
        let parsing_service = Arc::new(
            ParsingService::new(config).context("Failed to create parsing service")?,
        );
        Ok(Self { parsing_service })
    }

    /// Crawl and parse product list from URL
    pub async fn crawl_and_parse_product_list(
        &self,
        url: &str,
        page_id: u32,
    ) -> Result<Vec<Product>> {
        // Fetch HTML and parse via ParsingService; keep HTTP stack centralized
        let client = crate::infrastructure::HttpClient::create_from_global_config()
            .map_err(|e| anyhow::anyhow!("Failed to create HTTP client: {}", e))?;
        let html_doc = client
            .fetch_html(url)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch product list: {}", e))?;
        let html_str = html_doc.root_element().html();
        let products = self.parsing_service.parse_product_list(&html_str, page_id)?;

        // Validate all products
        for product in &products {
            if let Err(e) = self.parsing_service.validate_product(product) {
                warn!("Product validation failed: {}", e);
            }
        }

        Ok(products)
    }

    /// Crawl and parse product detail from URL
    pub async fn crawl_and_parse_product_detail(&self, url: &str) -> Result<ProductDetail> {
        let client = crate::infrastructure::HttpClient::create_from_global_config()
            .map_err(|e| anyhow::anyhow!("Failed to create HTTP client: {}", e))?;
        let html_doc = client
            .fetch_html(url)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch product detail: {}", e))?;
        let html_str = html_doc.root_element().html();
        let product = self.parsing_service.parse_product_detail(&html_str, url)?;

        // Validate product detail
        if let Err(e) = self.parsing_service.validate_product_detail(&product) {
            warn!("Product detail validation failed: {}", e);
        }

        Ok(product)
    }

    /// Batch crawl multiple product list pages
    pub async fn batch_crawl_product_lists(
        &self,
        urls_and_pages: Vec<(String, u32)>,
    ) -> Vec<Result<Vec<Product>>> {
        // Simple sequential version to keep surface compatible; can be optimized later
        let mut results = Vec::with_capacity(urls_and_pages.len());
        for (url, page_id) in urls_and_pages {
            let res = async {
                let client = crate::infrastructure::HttpClient::create_from_global_config()
                    .map_err(|e| anyhow::anyhow!("Failed to create HTTP client: {}", e))?;
                let html_doc = client.fetch_html(&url).await
                    .map_err(|e| anyhow::anyhow!("Failed to fetch product list: {}", e))?;
                let html_str = html_doc.root_element().html();
                self.parsing_service
                    .parse_product_list(&html_str, page_id)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))
            };
            results.push(res.await);
        }
        results
    }

    /// Batch crawl multiple product detail pages
    pub async fn batch_crawl_product_details(
        &self,
        urls: Vec<String>,
    ) -> Vec<Result<ProductDetail>> {
        let mut results = Vec::with_capacity(urls.len());
        for url in urls {
            let res = async {
                let client = crate::infrastructure::HttpClient::create_from_global_config()
                    .map_err(|e| anyhow::anyhow!("Failed to create HTTP client: {}", e))?;
                let html_doc = client.fetch_html(&url).await
                    .map_err(|e| anyhow::anyhow!("Failed to fetch product detail: {}", e))?;
                let html_str = html_doc.root_element().html();
                self.parsing_service
                    .parse_product_detail(&html_str, &url)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))
            };
            results.push(res.await);
        }
        results
    }

    /// Check if URL has next page
    pub async fn has_next_page(&self, url: &str) -> Result<bool> {
        let client = crate::infrastructure::HttpClient::create_from_global_config()
            .map_err(|e| anyhow::anyhow!("Failed to create HTTP client: {}", e))?;
        let html_doc = client.fetch_html(url).await
            .map_err(|e| anyhow::anyhow!("Failed to fetch page: {}", e))?;
        let html_str = html_doc.root_element().html();
        Ok(self.parsing_service.has_next_page(&html_str))
    }

    /// Get crawler configuration
    pub fn get_config(&self) -> &ParsingConfig {
    self.parsing_service.get_config()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsing_service_creation() {
        let config = ParsingConfig::default();
        let service = ParsingService::new(config);
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_crawler_service_creation() {
        let config = ParsingConfig::default();
        let service = CrawlerService::new(config);
        assert!(service.is_ok());
    }
}
