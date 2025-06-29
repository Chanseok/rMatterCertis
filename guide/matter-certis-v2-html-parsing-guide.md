# Matter Certis v2 - Rust HTML Parsing Implementation Guide

## Overview

This document provides a comprehensive guide for implementing HTML parsing functionality in the Rust+Tauri backend, specifically designed for the Matter Certis v2 application. It covers product list extraction and detailed product information parsing using Rust's powerful HTML parsing ecosystem.

## 1. Technology Stack & Dependencies

### 1.1 Core Dependencies

```toml
# src-tauri/Cargo.toml
[dependencies]
# HTTP Client
reqwest = { version = "0.11", features = ["json", "cookies", "gzip"] }

# HTML Parsing
scraper = "0.19"
select = "0.6"

# Async Runtime
tokio = { version = "1.0", features = ["full"] }

# Error Handling
anyhow = "1.0"
thiserror = "1.0"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"

# Utilities
lazy_static = "1.4"
rayon = "1.7"  # For parallel processing
url = "2.4"
regex = "1.10"
```

### 1.2 Why These Dependencies

- **reqwest**: Modern, async HTTP client with excellent performance
- **scraper**: Fast, memory-efficient HTML parsing built on html5ever
- **select**: Alternative CSS selector engine for complex queries
- **anyhow/thiserror**: Comprehensive error handling ecosystem
- **rayon**: Data parallelism for batch processing

## 2. Domain Model Definitions

### 2.1 Core Entities

```rust
// src-tauri/src/domain/entities/product.rs
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Product {
    /// Product detail page URL
    pub url: String,
    
    /// Product model name/title
    pub model_name: String,
    
    /// Brand/manufacturer name
    pub brand: String,
    
    /// Product category
    pub category: String,
    
    /// Page number where this product was found
    pub page_id: u32,
    
    /// Position index within the page (0-based)
    pub index_in_page: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatterProduct {
    /// Product detail page URL
    pub url: String,
    
    /// Product model name
    pub model_name: String,
    
    /// Brand/manufacturer
    pub brand: String,
    
    /// Product category
    pub category: String,
    
    /// Matter Vendor ID
    pub vid: Option<String>,
    
    /// Matter Product ID
    pub pid: Option<String>,
    
    /// Matter certification type
    pub certification_type: Option<String>,
    
    /// Certification date
    pub certification_date: Option<String>,
    
    /// Product description
    pub product_description: Option<String>,
    
    /// Additional Matter-specific information
    pub additional_info: Option<String>,
}

impl fmt::Display for Product {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} - {} ({})", self.brand, self.model_name, self.category)
    }
}

impl Product {
    /// Validate product data integrity
    pub fn is_valid(&self) -> bool {
        !self.url.is_empty() && 
        !self.model_name.is_empty() && 
        !self.brand.is_empty()
    }
}
```

### 2.2 Configuration Integration

```rust
// src-tauri/src/domain/value_objects/parsing_config.rs
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsingConfig {
    /// Base URL for resolving relative links
    pub base_url: String,
    
    /// User agent string for HTTP requests
    pub user_agent: String,
    
    /// Request timeout in milliseconds
    pub timeout_ms: u64,
    
    /// Retry attempts for failed requests
    pub retry_count: u32,
    
    /// Delay between requests to avoid rate limiting
    pub request_delay_ms: u64,
}

impl Default for ParsingConfig {
    fn default() -> Self {
        Self {
            base_url: "https://csa-iot.org/csa-iot_products/".to_string(),
            user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36".to_string(),
            timeout_ms: 30000,
            retry_count: 3,
            request_delay_ms: 1000,
        }
    }
}
```

## 3. HTML Parser Architecture

### 3.1 Parser Traits and Structure

```rust
// src-tauri/src/infrastructure/parsing/mod.rs
use anyhow::Result;
use scraper::{Html, Selector};
use crate::domain::entities::{Product, MatterProduct};

/// Generic HTML parser trait
pub trait HtmlParser {
    type Output;
    type Config;
    
    fn parse(&self, html: &str, config: &Self::Config) -> Result<Self::Output>;
}

/// Product list parser for extracting product information from listing pages
pub struct ProductListParser {
    // CSS selectors for different page elements
    product_container_selector: Selector,
    url_selector: Selector,
    title_selector: Selector,
    brand_selector: Selector,
    category_selector: Selector,
    pagination_selector: Selector,
}

/// Product detail parser for extracting detailed Matter certification info
pub struct ProductDetailParser {
    // Selectors for Matter-specific information
    vid_selector: Selector,
    pid_selector: Selector,
    certification_type_selector: Selector,
    certification_date_selector: Selector,
    description_selector: Selector,
    info_table_selector: Selector,
}

#[derive(Debug, Clone)]
pub struct ParseContext {
    pub page_id: u32,
    pub base_url: String,
    pub expected_products_per_page: u32,
}
```

### 3.2 Error Handling

```rust
// src-tauri/src/infrastructure/parsing/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParsingError {
    #[error("Required field '{field}' not found in HTML")]
    RequiredFieldMissing { field: String },
    
    #[error("Invalid CSS selector: {selector} - {reason}")]
    InvalidSelector { selector: String, reason: String },
    
    #[error("HTML parsing failed: {message}")]
    HtmlParsingFailed { message: String },
    
    #[error("No products found on page {page_id}")]
    NoProductsFound { page_id: u32 },
    
    #[error("Product validation failed: {reason}")]
    ProductValidationFailed { reason: String },
    
    #[error("URL resolution failed: {url} - {reason}")]
    UrlResolutionFailed { url: String, reason: String },
    
    #[error("Matter field extraction failed: {field} - {reason}")]
    MatterFieldExtractionFailed { field: String, reason: String },
}

pub type ParsingResult<T> = Result<T, ParsingError>;
```

## 4. Product List Parser Implementation

### 4.1 Core Parser Implementation

```rust
// src-tauri/src/infrastructure/parsing/product_list_parser.rs
use super::*;
use crate::domain::entities::Product;
use scraper::{Html, Selector, ElementRef};
use anyhow::{Result, Context};
use url::Url;

impl ProductListParser {
    pub fn new() -> Result<Self> {
        Ok(Self {
            // Robust selectors based on CSA-IoT website structure
            product_container_selector: Selector::parse(
                "div.product-item, .product-card, .product-listing-item, article.product"
            ).context("Failed to parse product container selector")?,
            
            url_selector: Selector::parse(
                "a[href*='/products/'], a[href*='product'], .product-link a, h3 a"
            ).context("Failed to parse URL selector")?,
            
            title_selector: Selector::parse(
                ".product-title, .product-name, h3, h2, .title, .name"
            ).context("Failed to parse title selector")?,
            
            brand_selector: Selector::parse(
                ".brand, .manufacturer, .vendor, .company-name, .brand-name"
            ).context("Failed to parse brand selector")?,
            
            category_selector: Selector::parse(
                ".category, .product-category, .type, .product-type"
            ).context("Failed to parse category selector")?,
            
            pagination_selector: Selector::parse(
                ".pagination a, .page-numbers a, .next-page"
            ).context("Failed to parse pagination selector")?,
        })
    }
    
    /// Extract product information from HTML with comprehensive error handling
    pub fn parse_products(&self, html: &str, context: &ParseContext) -> ParsingResult<Vec<Product>> {
        let document = Html::parse_document(html);
        let mut products = Vec::new();
        
        // Find all product containers
        let product_elements: Vec<ElementRef> = document
            .select(&self.product_container_selector)
            .collect();
        
        if product_elements.is_empty() {
            tracing::warn!(
                "No product containers found on page {} using selector: {}", 
                context.page_id, 
                self.product_container_selector
            );
            return Err(ParsingError::NoProductsFound { 
                page_id: context.page_id 
            });
        }
        
        // Extract data from each product element
        for (index, element) in product_elements.iter().enumerate() {
            match self.extract_product_from_element(element, index as u32, context) {
                Ok(product) => {
                    if product.is_valid() {
                        products.push(product);
                    } else {
                        tracing::warn!(
                            "Skipping invalid product at index {} on page {}", 
                            index, 
                            context.page_id
                        );
                    }
                },
                Err(e) => {
                    tracing::error!(
                        "Failed to extract product at index {} on page {}: {}", 
                        index, 
                        context.page_id, 
                        e
                    );
                    // Continue processing other products
                }
            }
        }
        
        if products.is_empty() {
            return Err(ParsingError::NoProductsFound { 
                page_id: context.page_id 
            });
        }
        
        tracing::info!(
            "Successfully extracted {} products from page {}", 
            products.len(), 
            context.page_id
        );
        
        Ok(products)
    }
    
    /// Extract individual product data from HTML element
    fn extract_product_from_element(
        &self, 
        element: &ElementRef, 
        index: u32,
        context: &ParseContext
    ) -> ParsingResult<Product> {
        
        // Extract product URL
        let url = self.extract_product_url(element, &context.base_url)?;
        
        // Extract product title/model name
        let model_name = self.extract_text_by_selector(element, &self.title_selector)
            .ok_or_else(|| ParsingError::RequiredFieldMissing { 
                field: "model_name".to_string() 
            })?;
        
        // Extract brand (with fallback to "Unknown")
        let brand = self.extract_text_by_selector(element, &self.brand_selector)
            .unwrap_or_else(|| "Unknown".to_string());
        
        // Extract category (with fallback to "Unknown")
        let category = self.extract_text_by_selector(element, &self.category_selector)
            .unwrap_or_else(|| "Unknown".to_string());
        
        let product = Product {
            url,
            model_name: model_name.trim().to_string(),
            brand: brand.trim().to_string(),
            category: category.trim().to_string(),
            page_id: context.page_id,
            index_in_page: index,
        };
        
        // Validate extracted product
        if !product.is_valid() {
            return Err(ParsingError::ProductValidationFailed {
                reason: "Essential fields are missing or empty".to_string(),
            });
        }
        
        Ok(product)
    }
    
    /// Extract and resolve product URL
    fn extract_product_url(&self, element: &ElementRef, base_url: &str) -> ParsingResult<String> {
        let url_element = element
            .select(&self.url_selector)
            .next()
            .ok_or_else(|| ParsingError::RequiredFieldMissing { 
                field: "url".to_string() 
            })?;
        
        let href = url_element
            .value()
            .attr("href")
            .ok_or_else(|| ParsingError::RequiredFieldMissing { 
                field: "href attribute".to_string() 
            })?;
        
        // Resolve relative URLs to absolute URLs
        let resolved_url = if href.starts_with("http") {
            href.to_string()
        } else if href.starts_with("/") {
            // Absolute path
            let base = Url::parse(base_url)
                .map_err(|e| ParsingError::UrlResolutionFailed {
                    url: base_url.to_string(),
                    reason: format!("Invalid base URL: {}", e),
                })?;
            
            base.join(href)
                .map_err(|e| ParsingError::UrlResolutionFailed {
                    url: href.to_string(),
                    reason: format!("Failed to join URL: {}", e),
                })?
                .to_string()
        } else {
            // Relative path
            format!("{}/{}", base_url.trim_end_matches('/'), href.trim_start_matches('/'))
        };
        
        Ok(resolved_url)
    }
    
    /// Extract text content using CSS selector
    fn extract_text_by_selector(&self, element: &ElementRef, selector: &Selector) -> Option<String> {
        element
            .select(selector)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .filter(|text| !text.is_empty())
    }
    
    /// Check if there are more pages to crawl
    pub fn has_next_page(&self, html: &str) -> bool {
        let document = Html::parse_document(html);
        
        // Look for pagination indicators
        document.select(&self.pagination_selector).any(|element| {
            let text = element.text().collect::<String>().to_lowercase();
            text.contains("next") || text.contains("→") || text.contains("»")
        })
    }
}
```

## 5. Product Detail Parser Implementation

### 5.1 Matter-Specific Data Extraction

```rust
// src-tauri/src/infrastructure/parsing/product_detail_parser.rs
use super::*;
use crate::domain::entities::MatterProduct;
use scraper::{Html, Selector, ElementRef};
use regex::Regex;
use std::collections::HashMap;

impl ProductDetailParser {
    pub fn new() -> Result<Self> {
        Ok(Self {
            vid_selector: Selector::parse(
                "td:contains('VID'), .vid-value, [data-field='vid'], *:contains('Vendor ID')"
            ).context("Failed to parse VID selector")?,
            
            pid_selector: Selector::parse(
                "td:contains('PID'), .pid-value, [data-field='pid'], *:contains('Product ID')"
            ).context("Failed to parse PID selector")?,
            
            certification_type_selector: Selector::parse(
                ".certification-type, .cert-type, *:contains('Certification')"
            ).context("Failed to parse certification type selector")?,
            
            certification_date_selector: Selector::parse(
                ".certification-date, .cert-date, *:contains('Date')"
            ).context("Failed to parse certification date selector")?,
            
            description_selector: Selector::parse(
                ".product-description, .description, .product-info, .details"
            ).context("Failed to parse description selector")?,
            
            info_table_selector: Selector::parse(
                "table, .info-table, .product-details-table, .specifications"
            ).context("Failed to parse info table selector")?,
        })
    }
    
    /// Extract detailed Matter product information
    pub fn parse_product_detail(&self, html: &str, url: &str) -> ParsingResult<MatterProduct> {
        let document = Html::parse_document(html);
        
        // Extract basic product information
        let model_name = self.extract_basic_info(&document, "title")?;
        let brand = self.extract_basic_info(&document, "brand")
            .unwrap_or_else(|_| "Unknown".to_string());
        let category = self.extract_basic_info(&document, "category")
            .unwrap_or_else(|_| "Unknown".to_string());
        
        // Extract Matter-specific certification data
        let certification_data = self.extract_matter_certification_data(&document)?;
        
        let product = MatterProduct {
            url: url.to_string(),
            model_name,
            brand,
            category,
            vid: certification_data.get("vid").cloned(),
            pid: certification_data.get("pid").cloned(),
            certification_type: certification_data.get("certification_type").cloned(),
            certification_date: certification_data.get("certification_date").cloned(),
            product_description: self.extract_product_description(&document),
            additional_info: certification_data.get("additional_info").cloned(),
        };
        
        tracing::info!("Extracted Matter product details for: {}", product.model_name);
        Ok(product)
    }
    
    /// Extract basic product information (title, brand, category)
    fn extract_basic_info(&self, document: &Html, field: &str) -> ParsingResult<String> {
        let selectors = match field {
            "title" => vec!["h1", ".product-title", ".product-name", ".title"],
            "brand" => vec![".brand", ".manufacturer", ".vendor", ".company-name"],
            "category" => vec![".category", ".product-category", ".type"],
            _ => return Err(ParsingError::RequiredFieldMissing { 
                field: field.to_string() 
            }),
        };
        
        for selector_str in selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    let text = element.text().collect::<String>().trim().to_string();
                    if !text.is_empty() {
                        return Ok(text);
                    }
                }
            }
        }
        
        Err(ParsingError::RequiredFieldMissing { 
            field: field.to_string() 
        })
    }
    
    /// Extract Matter certification data from various HTML structures
    fn extract_matter_certification_data(&self, document: &Html) -> ParsingResult<HashMap<String, String>> {
        let mut certification_data = HashMap::new();
        
        // Strategy 1: Extract from structured tables
        if let Some(table_data) = self.extract_from_tables(document) {
            certification_data.extend(table_data);
        }
        
        // Strategy 2: Extract from definition lists
        if let Some(dl_data) = self.extract_from_definition_lists(document) {
            certification_data.extend(dl_data);
        }
        
        // Strategy 3: Extract from labeled paragraphs/divs
        if let Some(labeled_data) = self.extract_from_labeled_elements(document) {
            certification_data.extend(labeled_data);
        }
        
        // Strategy 4: Extract using regex patterns
        let html_text = document.html();
        if let Some(regex_data) = self.extract_using_regex(&html_text) {
            certification_data.extend(regex_data);
        }
        
        if certification_data.is_empty() {
            tracing::warn!("No Matter certification data found in product detail page");
        }
        
        Ok(certification_data)
    }
    
    /// Extract data from HTML tables - Most reliable method for Matter data
    fn extract_from_tables(&self, document: &Html) -> Option<HashMap<String, String>> {
        let mut data = HashMap::new();
        
        for table in document.select(&self.info_table_selector) {
            let row_selector = Selector::parse("tr").ok()?;
            
            for row in table.select(&row_selector) {
                let cell_selector = Selector::parse("td, th").ok()?;
                let cells: Vec<_> = row.select(&cell_selector).collect();
                
                if cells.len() >= 2 {
                    let key = cells[0].text().collect::<String>().trim().to_lowercase();
                    let value = cells[1].text().collect::<String>().trim().to_string();
                    
                    if !value.is_empty() && value != "-" && value != "N/A" {
                        if key.contains("vid") || key.contains("vendor id") {
                            data.insert("vid".to_string(), value);
                        } else if key.contains("pid") || key.contains("product id") {
                            data.insert("pid".to_string(), value);
                        } else if key.contains("certification") && !key.contains("date") {
                            data.insert("certification_type".to_string(), value);
                        } else if key.contains("date") && key.contains("cert") {
                            data.insert("certification_date".to_string(), value);
                        }
                    }
                }
            }
        }
        
        if data.is_empty() { None } else { Some(data) }
    }
    
    /// Extract data using regex patterns as fallback
    fn extract_using_regex(&self, html: &str) -> Option<HashMap<String, String>> {
        let mut data = HashMap::new();
        
        // Regex patterns for Matter certification data
        let patterns = vec![
            (r"VID[:\s]+([A-Fa-f0-9]{4})", "vid"),
            (r"PID[:\s]+([A-Fa-f0-9]{4})", "pid"),
            (r"Vendor\s+ID[:\s]+([A-Fa-f0-9]{4})", "vid"),
            (r"Product\s+ID[:\s]+([A-Fa-f0-9]{4})", "pid"),
        ];
        
        for (pattern, key) in patterns {
            if let Ok(regex) = Regex::new(pattern) {
                if let Some(captures) = regex.captures(html) {
                    if let Some(value) = captures.get(1) {
                        data.insert(key.to_string(), value.as_str().to_string());
                    }
                }
            }
        }
        
        if data.is_empty() { None } else { Some(data) }
    }
    
    /// Extract product description
    fn extract_product_description(&self, document: &Html) -> Option<String> {
        document
            .select(&self.description_selector)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .filter(|desc| !desc.is_empty() && desc.len() > 10) // Filter out very short descriptions
    }
}
```

## 6. Web Crawler Implementation

### 6.1 HTTP Client with Retry Logic

```rust
// src-tauri/src/infrastructure/crawling/web_crawler.rs
use crate::application::services::ParsingService;
use crate::domain::entities::{Product, MatterProduct};
use crate::domain::value_objects::ParsingConfig;
use reqwest::{Client, Response};
use anyhow::{Result, Context};
use std::time::Duration;
use tokio::time::sleep;

pub struct WebCrawler {
    client: Client,
    parsing_service: Arc<ParsingService>,
    config: ParsingConfig,
}

impl WebCrawler {
    pub fn new(config: ParsingConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .user_agent(&config.user_agent)
            .gzip(true)
            .brotli(true)
            .cookie_store(true)
            .build()
            .context("Failed to create HTTP client")?;
        
        let parsing_service = Arc::new(
            ParsingService::new(config.clone())
                .context("Failed to create parsing service")?
        );
        
        Ok(Self {
            client,
            parsing_service,
            config,
        })
    }
    
    /// Crawl product list from a specific page
    pub async fn crawl_product_list(&self, url: &str, page_id: u32) -> Result<Vec<Product>> {
        tracing::info!("Crawling product list from: {} (page {})", url, page_id);
        
        let html = self.fetch_html_with_retry(url).await?;
        let products = self.parsing_service.parse_product_list(&html, page_id)?;
        
        // Add delay to avoid overwhelming the server
        if self.config.request_delay_ms > 0 {
            sleep(Duration::from_millis(self.config.request_delay_ms)).await;
        }
        
        Ok(products)
    }
    
    /// Crawl detailed product information
    pub async fn crawl_product_detail(&self, url: &str) -> Result<MatterProduct> {
        tracing::info!("Crawling product detail from: {}", url);
        
        let html = self.fetch_html_with_retry(url).await?;
        let product = self.parsing_service.parse_product_detail(&html, url)?;
        
        // Add delay between requests
        if self.config.request_delay_ms > 0 {
            sleep(Duration::from_millis(self.config.request_delay_ms)).await;
        }
        
        Ok(product)
    }
    
    /// Fetch HTML with retry logic
    async fn fetch_html_with_retry(&self, url: &str) -> Result<String> {
        let mut last_error = None;
        
        for attempt in 1..=self.config.retry_count {
            match self.fetch_html(url).await {
                Ok(html) => return Ok(html),
                Err(e) => {
                    tracing::warn!(
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
        
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All retry attempts failed")))
    }
    
    /// Fetch HTML from URL
    async fn fetch_html(&self, url: &str) -> Result<String> {
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
        
        if html.len() < 100 {
            return Err(anyhow::anyhow!("Response body too short, likely an error page"));
        }
        
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
            if !content_type_str.contains("text/html") {
                tracing::warn!("Unexpected content type: {}", content_type_str);
            }
        }
        
        Ok(())
    }
}
```

## 7. Tauri Integration

### 7.1 Command Interface

```rust
// src-tauri/src/commands/parsing_commands.rs
use crate::infrastructure::crawling::WebCrawler;
use crate::domain::entities::{Product, MatterProduct};
use crate::domain::value_objects::ParsingConfig;
use tauri::command;

#[command]
pub async fn crawl_product_list_page(
    url: String, 
    page_id: u32,
    config: ParsingConfig
) -> Result<Vec<Product>, String> {
    let crawler = WebCrawler::new(config)
        .map_err(|e| format!("Failed to create crawler: {}", e))?;
    
    crawler.crawl_product_list(&url, page_id)
        .await
        .map_err(|e| format!("Product list crawling failed: {}", e))
}

#[command]
pub async fn crawl_product_detail_page(
    url: String,
    config: ParsingConfig
) -> Result<MatterProduct, String> {
    let crawler = WebCrawler::new(config)
        .map_err(|e| format!("Failed to create crawler: {}", e))?;
    
    crawler.crawl_product_detail(&url)
        .await
        .map_err(|e| format!("Product detail crawling failed: {}", e))
}

#[command]
pub async fn batch_crawl_products(
    urls: Vec<String>,
    page_ids: Vec<u32>,
    config: ParsingConfig
) -> Result<Vec<Vec<Product>>, String> {
    if urls.len() != page_ids.len() {
        return Err("URLs and page IDs must have the same length".to_string());
    }
    
    let crawler = WebCrawler::new(config)
        .map_err(|e| format!("Failed to create crawler: {}", e))?;
    
    let mut results = Vec::new();
    
    for (url, page_id) in urls.into_iter().zip(page_ids.into_iter()) {
        match crawler.crawl_product_list(&url, page_id).await {
            Ok(products) => results.push(products),
            Err(e) => {
                tracing::error!("Failed to crawl page {}: {}", page_id, e);
                results.push(Vec::new()); // Empty result for failed page
            }
        }
    }
    
    Ok(results)
}
```

### 7.2 Main Application Setup

```rust
// src-tauri/src/main.rs
use tauri::Manager;

mod commands;
mod infrastructure;
mod application;
mod domain;

use commands::parsing_commands::*;

fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            crawl_product_list_page,
            crawl_product_detail_page,
            batch_crawl_products
        ])
        .setup(|app| {
            tracing::info!("Matter Certis v2 Tauri application started");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

## 8. Performance Optimization

### 8.1 Concurrent Processing

```rust
// src-tauri/src/infrastructure/crawling/concurrent_crawler.rs
use crate::infrastructure::crawling::WebCrawler;
use crate::domain::entities::Product;
use tokio::sync::Semaphore;
use futures::future::join_all;
use std::sync::Arc;

pub struct ConcurrentCrawler {
    crawler: Arc<WebCrawler>,
    semaphore: Arc<Semaphore>,
}

impl ConcurrentCrawler {
    pub fn new(crawler: WebCrawler, max_concurrent: usize) -> Self {
        Self {
            crawler: Arc::new(crawler),
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }
    
    /// Crawl multiple pages concurrently with rate limiting
    pub async fn crawl_pages_concurrent(
        &self, 
        urls_and_pages: Vec<(String, u32)>
    ) -> Vec<Result<Vec<Product>, String>> {
        let tasks = urls_and_pages.into_iter().map(|(url, page_id)| {
            let crawler = Arc::clone(&self.crawler);
            let semaphore = Arc::clone(&self.semaphore);
            
            async move {
                let _permit = semaphore.acquire().await.unwrap();
                crawler.crawl_product_list(&url, page_id)
                    .await
                    .map_err(|e| e.to_string())
            }
        });
        
        join_all(tasks).await
    }
}
```

### 8.2 Caching Layer

```rust
// src-tauri/src/infrastructure/caching/html_cache.rs
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

pub struct HtmlCache {
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    ttl: Duration,
}

struct CacheEntry {
    html: String,
    timestamp: Instant,
}

impl HtmlCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            ttl,
        }
    }
    
    pub fn get(&self, url: &str) -> Option<String> {
        let cache = self.cache.read().unwrap();
        
        if let Some(entry) = cache.get(url) {
            if entry.timestamp.elapsed() < self.ttl {
                return Some(entry.html.clone());
            }
        }
        
        None
    }
    
    pub fn set(&self, url: String, html: String) {
        let mut cache = self.cache.write().unwrap();
        cache.insert(url, CacheEntry {
            html,
            timestamp: Instant::now(),
        });
    }
    
    pub fn clear_expired(&self) {
        let mut cache = self.cache.write().unwrap();
        cache.retain(|_, entry| entry.timestamp.elapsed() < self.ttl);
    }
}
```

## 9. Testing

### 9.1 Unit Tests

```rust
// src-tauri/src/infrastructure/parsing/tests.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::Product;
    
    #[test]
    fn test_product_list_parser_creation() {
        let parser = ProductListParser::new();
        assert!(parser.is_ok());
    }
    
    #[test]
    fn test_product_extraction() {
        let html = r#"
            <div class="product-item">
                <h3><a href="/products/sample-product">Sample Product</a></h3>
                <div class="brand">Sample Brand</div>
                <div class="category">Sample Category</div>
            </div>
        "#;
        
        let parser = ProductListParser::new().unwrap();
        let context = ParseContext {
            page_id: 1,
            base_url: "https://example.com".to_string(),
            expected_products_per_page: 12,
        };
        
        let products = parser.parse_products(html, &context).unwrap();
        assert_eq!(products.len(), 1);
        assert_eq!(products[0].model_name, "Sample Product");
        assert_eq!(products[0].brand, "Sample Brand");
    }
    
    #[tokio::test]
    async fn test_web_crawler_integration() {
        let config = ParsingConfig::default();
        let crawler = WebCrawler::new(config).unwrap();
        
        // Test with a mock server or skip in CI
        // This would require setting up a test server
    }
}
```

## 10. Real-World Usage Examples

### 10.1 CSA-IoT Matter Products Parsing

Based on the actual CSA-IoT website structure, here are the specific selectors and patterns:

```rust
// Real selectors for CSA-IoT website
const CSA_IOT_SELECTORS: &str = r#"
// Product list page selectors
.product-list-item              // Main product container
.product-title a                // Product title with link
.company-name                   // Manufacturer/brand
.product-category               // Product category

// Product detail page selectors
.product-details table tr       // Product information table
td:contains('VID')              // Vendor ID row
td:contains('PID')              // Product ID row
.certification-info             // Certification details
"#;

/// Example usage for CSA-IoT specific parsing
pub fn create_csa_iot_parser() -> Result<ProductListParser> {
    // Use actual selectors from the current TypeScript implementation
    ProductListParser::with_custom_selectors(
        "div.product-list-item, .product-item",
        "a[href*='product']",
        ".product-title, h3.title",
        ".company-name, .manufacturer",
        ".product-category, .category"
    )
}
```

### 10.2 Frontend Integration (SolidJS)

```typescript
// Frontend usage example
import { invoke } from '@tauri-apps/api/tauri';

// Crawl a single page
const crawlProductPage = async (url: string, pageId: number) => {
  try {
    const products = await invoke('crawl_product_list_page', {
      url,
      pageId,
      config: {
        baseUrl: 'https://csa-iot.org/csa-iot_products/',
        userAgent: 'Matter Certis v2.0',
        timeoutMs: 30000,
        retryCount: 3,
        requestDelayMs: 1000
      }
    });
    
    return products;
  } catch (error) {
    console.error('Crawling failed:', error);
    throw error;
  }
};

// Batch crawl multiple pages
const crawlMultiplePages = async (urls: string[], pageIds: number[]) => {
  return await invoke('batch_crawl_products', {
    urls,
    pageIds,
    config: getDefaultConfig()
  });
};
```

## 11. Migration Notes from TypeScript

### 11.1 Key Differences

1. **Memory Management**: Rust's ownership system eliminates memory leaks
2. **Error Handling**: Result<T, E> provides explicit error handling
3. **Performance**: Significant improvements in parsing speed and memory usage
4. **Type Safety**: Compile-time guarantees prevent runtime errors

### 11.2 TypeScript to Rust Mapping

```rust
// TypeScript: cheerio selectors
$('.product-item').each((i, el) => { ... })

// Rust: scraper selectors
for element in document.select(&product_selector) { ... }

// TypeScript: async/await with promises
async function fetchData() { ... }

// Rust: async/await with Result
async fn fetch_data() -> Result<Data, Error> { ... }
```

## 12. Best Practices

### 12.1 Error Handling

- Always use `Result<T, E>` for fallible operations
- Provide meaningful error messages with context
- Log errors at appropriate levels
- Implement graceful degradation for non-critical failures

### 12.2 Performance

- Use `Arc` for shared immutable data
- Implement connection pooling for HTTP clients
- Cache parsed selectors to avoid re-compilation
- Use parallel processing for independent operations

### 12.3 Maintainability

- Keep selectors configurable and externalized
- Use type-safe configuration structures
- Implement comprehensive logging
- Write tests for critical parsing logic

This guide provides a complete foundation for implementing robust HTML parsing in Rust+Tauri, leveraging the knowledge gained from the original TypeScript implementation while significantly improving performance and reliability.
