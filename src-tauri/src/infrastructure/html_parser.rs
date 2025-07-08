//! HTML parsing and data extraction for Matter certification data
//! 
//! This module provides specialized extractors for parsing HTML content
//! from certification websites and extracting product information.
//! 
//! Implementation follows the guide in .local/Rust-Tauri-DOM-Extraction-Guide.md

#![allow(clippy::uninlined_format_args)]

use anyhow::{anyhow, Result};
use scraper::{Html, Selector, ElementRef};
use tracing::debug;
use crate::infrastructure::csa_iot;
use crate::domain::product::{Product, ProductDetail};

/// Configuration for CSA-IoT website data extraction
#[derive(Debug, Clone)]
pub struct MatterExtractorConfig {
    /// CSS selectors for product list pages
    pub product_list_selectors: ProductListSelectors,
    /// CSS selectors for product detail pages
    pub product_detail_selectors: ProductDetailSelectors,
    /// Base URL for resolving relative links
    pub base_url: String,
}

#[derive(Debug, Clone)]
pub struct ProductListSelectors {
    /// Selector for product link containers based on CSA-IoT structure
    pub product_container: String,
    /// Selector for product detail page links
    pub product_link: String,
    /// Selector for manufacturer name in list
    pub manufacturer: String,
    /// Selector for model name in list
    pub model: String,
    /// Selector for certification ID in list
    pub certificate_id: String,
}

#[derive(Debug, Clone)]
pub struct ProductDetailSelectors {
    /// Selector for manufacturer field
    pub manufacturer: String,
    /// Selector for model field
    pub model: String,
    /// Selector for device type
    pub device_type: String,
    /// Selector for certification ID
    pub certification_id: String,
    /// Selector for certification date
    pub certification_date: String,
    /// Selector for software version
    pub software_version: String,
    /// Selector for hardware version
    pub hardware_version: String,
    /// Selector for VID (Vendor ID)
    pub vid: String,
    /// Selector for PID (Product ID)
    pub pid: String,
    /// Selector for specification version
    pub specification_version: String,
    /// Selector for transport interface
    pub transport_interface: String,
    /// Selector for information tables
    pub info_table: String,
}

impl Default for MatterExtractorConfig {
    fn default() -> Self {
        Self {
            product_list_selectors: ProductListSelectors {
                // 가이드에 따른 정확한 CSA-IoT 페이지 구조 셀렉터
                product_container: "div.post-feed article".to_string(),
                product_link: "a".to_string(),
                manufacturer: "p.entry-company.notranslate".to_string(),
                model: "h3.entry-title".to_string(),
                certificate_id: "span.entry-cert-id".to_string(),
            },
            product_detail_selectors: ProductDetailSelectors {
                manufacturer: ".manufacturer, .company-info".to_string(),
                model: "h1.entry-title, h1".to_string(),
                device_type: ".device-type, .category".to_string(),
                certification_id: ".cert-id, .certification-id".to_string(),
                certification_date: ".cert-date, .certification-date".to_string(),
                software_version: ".software-version".to_string(),
                hardware_version: ".hardware-version".to_string(),
                vid: ".vid".to_string(),
                pid: ".pid".to_string(),
                specification_version: ".spec-version".to_string(),
                transport_interface: ".transport-interface".to_string(),
                info_table: ".product-certificates-table".to_string(),
            },
            base_url: csa_iot::BASE_URL.to_string(),
        }
    }
}

/// Specialized data extractor for Matter certification websites
/// Following the guide approach for clean, direct DOM extraction
#[derive(Clone)]
pub struct MatterDataExtractor {
    config: MatterExtractorConfig,
}

impl MatterDataExtractor {
    /// Create a new data extractor with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(MatterExtractorConfig::default())
    }

    /// Create a new data extractor with custom configuration
    pub fn with_config(config: MatterExtractorConfig) -> Result<Self> {
        Ok(Self { config })
    }

    /// Extract product URLs from a product listing page (guide-based approach)
    pub fn extract_product_urls(&self, html: &Html, base_url: &str) -> Result<Vec<String>> {
        debug!("Extracting product URLs from listing page");
        
        let article_selector = Selector::parse("div.post-feed article")
            .map_err(|e| anyhow!("Invalid article selector: {}", e))?;
        let link_selector = Selector::parse("a")
            .map_err(|e| anyhow!("Invalid link selector: {}", e))?;

        let mut urls = Vec::new();
        
        for article in html.select(&article_selector) {
            if let Some(link) = article.select(&link_selector).next() {
                if let Some(href) = link.value().attr("href") {
                    let url = self.resolve_url(href, base_url);
                    // Filter for actual product pages
                    if url.contains("/csa_product/") && !url.contains("/csa-iot_products/") {
                        urls.push(url);
                    }
                }
            }
        }

        debug!("Extracted {} product URLs", urls.len());
        Ok(urls)
    }

    /// Extract product URLs from a product listing page (string input version)
    pub fn extract_product_urls_from_content(&self, html_content: &str) -> Result<Vec<String>> {
        let html = Html::parse_document(html_content);
        self.extract_product_urls(&html, &self.config.base_url)
    }

    /// Extract total number of pages from pagination
    pub fn extract_total_pages(&self, html_content: &str) -> Result<u32> {
        let html = Html::parse_document(html_content);
        
        let pagination_selectors = vec![
            "a[href*='page=']",  // 페이지 링크
            ".pagination a",     // 페이지네이션 링크
            ".page-numbers a",   // 워드프레스 스타일
        ];

        let mut max_page = 1u32;

        for selector_str in pagination_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                for element in html.select(&selector) {
                    // href에서 page= 파라미터 추출
                    if let Some(href) = element.value().attr("href") {
                        if let Some(page_param) = href.split("page=").nth(1) {
                            let page_num_str = page_param.split('&').next().unwrap_or("");
                            if let Ok(page_num) = page_num_str.parse::<u32>() {
                                max_page = max_page.max(page_num);
                            }
                        }
                    }
                    
                    // 텍스트에서 페이지 번호 추출
                    let text = element.text().collect::<String>();
                    if let Ok(page_num) = text.trim().parse::<u32>() {
                        max_page = max_page.max(page_num);
                    }
                }
            }
        }

        // "Page X of Y" 형태의 텍스트에서 총 페이지 수 추출
        let page_info_selectors = vec![
            ".pagination-info",
            ".page-info", 
            ".showing-info",
        ];

        let re = regex::Regex::new(r"(?i)page\s+\d+\s+of\s+(\d+)").unwrap();

        for selector_str in page_info_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                for element in html.select(&selector) {
                    let text = element.text().collect::<String>();
                    if let Some(captures) = re.captures(&text) {
                        if let Some(total_str) = captures.get(1) {
                            if let Ok(total) = total_str.as_str().parse::<u32>() {
                                max_page = max_page.max(total);
                            }
                        }
                    }
                }
            }
        }

        debug!("Extracted total pages: {}", max_page);
        Ok(max_page)
    }

    /// Extract product data from a detail page (returns JSON for flexibility)
    pub fn extract_product_data(&self, html_content: &str) -> Result<serde_json::Value> {
        let html = Html::parse_document(html_content);
        
        let product_detail = self.extract_product_detail(&html, "".to_string())?;
        
        let json_value = serde_json::to_value(product_detail)
            .map_err(|e| anyhow!("Failed to serialize product detail: {}", e))?;
        
        Ok(json_value)
    }

    /// Extract basic product information from a page (guide-based main entry point)
    pub fn extract_basic_product_info(&self, html_content: &str, page_url: &str) -> Result<Vec<Product>> {
        debug!("Extracting basic product info from: {}", page_url);
        let html = Html::parse_document(html_content);
        
        if self.is_listing_page(&html, page_url) {
            let page_id = self.extract_page_id_from_url(page_url);
            self.extract_products_from_list(&html, page_id)
        } else {
            let product = self.extract_single_product_from_detail_page(&html, page_url)?;
            Ok(vec![product])
        }
    }

    /// Check if the given HTML represents a listing page
    fn is_listing_page(&self, html: &Html, url: &str) -> bool {
        if url.contains("page=") || url.contains("products") && !url.contains("csa_product") {
            return true;
        }
        
        if let Ok(article_selector) = Selector::parse("div.post-feed article") {
            let article_count = html.select(&article_selector).count();
            return article_count > 1;
        }
        
        false
    }

    /// Extract page ID from URL (for listing pages)
    fn extract_page_id_from_url(&self, url: &str) -> i32 {
        if let Some(page_param) = url.split("page=").nth(1) {
            if let Some(page_str) = page_param.split('&').next() {
                if let Ok(page_num) = page_str.parse::<i32>() {
                    return page_num;
                }
            }
        }
        1 // Default to page 1
    }

    /// Extract a single product from a detail page (guide-based approach)
    fn extract_single_product_from_detail_page(&self, html: &Html, url: &str) -> Result<Product> {
        debug!("Extracting single product from detail page: {}", url);
        let now = chrono::Utc::now();
        
        let manufacturer = self.extract_field_text(html, ".manufacturer, .company-info");
        let model = self.extract_field_text(html, "h1.entry-title, h1");
        let certificate_id = self.extract_field_text(html, ".cert-id, .certification-id");
        let device_type = self.extract_field_text(html, ".device-type, .category");
        let certification_date = self.extract_field_text(html, ".cert-date, .certification-date");

        debug!("Extracted from {}: manufacturer={:?}, model={:?}, cert_id={:?}", 
               url, manufacturer, model, certificate_id);

        Ok(Product {
            url: url.to_string(),
            manufacturer,
            model,
            certificate_id,
            device_type,
            certification_date,
            page_id: None,
            index_in_page: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// Extract basic product information from listing page (guide-based approach)
    pub fn extract_products_from_list(&self, html: &Html, page_id: i32) -> Result<Vec<Product>> {
        debug!("Extracting products from listing page {}", page_id);
        
        let article_selector = Selector::parse("div.post-feed article")
            .map_err(|e| anyhow!("Invalid article selector: {}", e))?;

        let mut products = Vec::new();
        let articles: Vec<_> = html.select(&article_selector).collect();
        debug!("Found {} article elements", articles.len());
        
        // Process articles in reverse order to match expected index order (guide approach)
        for (index, article) in articles.iter().rev().enumerate() {
            if let Ok(product) = self.extract_single_product_from_list(*article, page_id, index as i32) {
                products.push(product);
            }
        }

        debug!("Extracted {} products from listing page", products.len());
        Ok(products)
    }

    /// Extract detailed product information from a product detail page (guide-based approach)
    pub fn extract_product_detail(&self, html: &Html, url: String) -> Result<ProductDetail> {
        debug!("Extracting product detail from: {}", url);
        
        let now = chrono::Utc::now();
        
        // Extract basic product information from page headers/title
        let model = self.extract_field_text(html, &self.config.product_detail_selectors.model);
        let manufacturer = self.extract_field_text(html, "p.company-info, .company-name, .manufacturer, p.entry-company, .entry-company");
        let device_type = self.extract_field_text(html, "p.device-category, .product-type, .device-type, p.entry-category, .entry-category, h6.entry-category");
        
        let mut detail = ProductDetail {
            url,
            page_id: None,
            index_in_page: None,
            id: None,
            manufacturer,
            model,
            device_type,
            certification_id: None,
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
            description: None,
            compliance_document_url: None,
            program_type: None,
            created_at: now,
            updated_at: now,
        };

        // Extract from information table (guide approach)
        self.extract_from_table(html, &mut detail)?;
        
        // Extract from detail list items (guide approach)  
        self.extract_from_detail_list(html, &mut detail)?;

        debug!("Extracted product detail: model={:?}, manufacturer={:?}, device_type={:?}, cert_id={:?}",
               detail.model, detail.manufacturer, detail.device_type, detail.certification_id);

        Ok(detail)
    }

    /// Extract a single product from a list container element (guide-based approach)
    fn extract_single_product_from_list(&self, article: ElementRef, page_id: i32, index: i32) -> Result<Product> {
        let now = chrono::Utc::now();
        
        // Extract URL - simple and direct approach
        let link_selector = Selector::parse("a").unwrap();
        let url = article
            .select(&link_selector)
            .next()
            .and_then(|link| link.value().attr("href"))
            .map(|href| self.resolve_url(href, &self.config.base_url))
            .unwrap_or_else(|| format!("unknown-{}-{}", page_id, index));

        // Extract manufacturer - exactly as in guide
        let manufacturer_selector = Selector::parse("p.entry-company.notranslate").unwrap();
        let manufacturer = article
            .select(&manufacturer_selector)
            .next()
            .map(|el| el.text().collect::<Vec<_>>().join("").trim().to_string())
            .filter(|s| !s.is_empty());

        // Extract model - exactly as in guide  
        let model_selector = Selector::parse("h3.entry-title").unwrap();
        let model = article
            .select(&model_selector)
            .next()
            .map(|el| el.text().collect::<Vec<_>>().join("").trim().to_string())
            .filter(|s| !s.is_empty());

        // Extract certificate ID with fallback logic from guide
        let certificate_id = self.extract_certificate_id_from_article(&article);

        debug!("Extracted from listing article {}: manufacturer={:?}, model={:?}, cert_id={:?}", 
               index, manufacturer, model, certificate_id);

        Ok(Product {
            url,
            manufacturer,
            model,
            certificate_id,
            device_type: None,
            certification_date: None,
            page_id: Some(page_id),
            index_in_page: Some(index),
            created_at: now,
            updated_at: now,
        })
    }

    /// Extract certificate ID from article element following the guide's approach
    fn extract_certificate_id_from_article(&self, article: &ElementRef) -> Option<String> {
        // Try p.entry-certificate-id first (guide approach)
        let cert_id_p_selector = Selector::parse("p.entry-certificate-id").unwrap();
        if let Some(cert_p_el) = article.select(&cert_id_p_selector).next() {
            let text = cert_p_el.text().collect::<Vec<_>>().join("").trim().to_string();
            if text.starts_with("Certificate ID: ") {
                return Some(text.replace("Certificate ID: ", "").trim().to_string());
            } else if !text.is_empty() {
                return Some(text);
            }
        }

        // Fallback to span.entry-cert-id (guide approach)
        let cert_id_selector = Selector::parse("span.entry-cert-id").unwrap();
        if let Some(cert_span_el) = article.select(&cert_id_selector).next() {
            let text = cert_span_el.text().collect::<Vec<_>>().join("").trim().to_string();
            if !text.is_empty() {
                return Some(text);
            }
        }

        None
    }

    /// Extract product information from table elements (guide-based approach)
    fn extract_from_table(&self, html: &Html, detail: &mut ProductDetail) -> Result<()> {
        let table_selector = Selector::parse(".product-certificates-table")
            .map_err(|e| anyhow!("Invalid table selector: {}", e))?;
        
        if let Some(table) = html.select(&table_selector).next() {
            let row_selector = Selector::parse("tr").unwrap();
            let cell_selector = Selector::parse("td").unwrap();

            for row in table.select(&row_selector) {
                let cells: Vec<_> = row.select(&cell_selector).collect();
                if cells.len() >= 2 {
                    let key = cells[0].text().collect::<Vec<_>>().join("").trim().to_lowercase();
                    let value = cells[1].text().collect::<Vec<_>>().join("").trim().to_string();

                    if !value.is_empty() {
                        self.map_table_field(&key, &value, detail);
                    }
                }
            }
        }
        
        Ok(())
    }

    /// Extract product information from detail list items (guide-based approach)
    fn extract_from_detail_list(&self, html: &Html, detail: &mut ProductDetail) -> Result<()> {
        // First try the new CSA-IoT site format with label/value spans
        let item_selector = Selector::parse("div.entry-product-details li.item")
            .or_else(|_| Selector::parse("ul.detail-items li.item"))
            .map_err(|e| anyhow!("Invalid detail list item selector: {}", e))?;
        
        let label_selector = Selector::parse("span.label")
            .map_err(|e| anyhow!("Invalid label selector: {}", e))?;
            
        let value_selector = Selector::parse("span.value")
            .map_err(|e| anyhow!("Invalid value selector: {}", e))?;
        
        // Try to extract from structured list items with label/value spans
        let mut found_items = false;
        for item in html.select(&item_selector) {
            if let (Some(label_el), Some(value_el)) = (
                item.select(&label_selector).next(),
                item.select(&value_selector).next()
            ) {
                let label = label_el.text().collect::<String>().trim().to_lowercase();
                let value = value_el.text().collect::<String>().trim().to_string();
                
                if !value.is_empty() {
                    self.map_detail_field(&label, &value, detail);
                    found_items = true;
                }
            }
        }
        
        // If we didn't find any items with the span structure, try the old format with colon-separated text
        if !found_items {
            // Fall back to the original selector for backwards compatibility
            let fallback_selector = Selector::parse("div.entry-product-details > div > ul li")
                .map_err(|e| anyhow!("Invalid fallback list selector: {}", e))?;
                
            for item in html.select(&fallback_selector) {
                let full_text = item.text().collect::<Vec<_>>().join("").trim().to_string();
                
                if let Some(colon_index) = full_text.find(':') {
                    let raw_label = full_text[..colon_index].trim().to_lowercase();
                    let raw_value = full_text[colon_index + 1..].trim().to_string();
                    
                    if !raw_value.is_empty() {
                        self.map_detail_field(&raw_label, &raw_value, detail);
                    }
                }
            }
        }
        
        Ok(())
    }

    /// Map table field to ProductDetail field (guide-based approach)
    fn map_table_field(&self, key: &str, value: &str, detail: &mut ProductDetail) {
        match key {
            k if k.contains("certification id") => detail.certification_id = Some(value.to_string()),
            k if k.contains("certification date") => detail.certification_date = Some(value.to_string()),
            k if k.contains("manufacturer") || k.contains("company") => detail.manufacturer = Some(value.to_string()),
            k if k.contains("vid") => detail.vid = self.parse_numeric_id(value),
            k if k.contains("pid") => detail.pid = self.parse_numeric_id(value),
            k if k.contains("hardware version") => detail.hardware_version = Some(value.to_string()),
            k if k.contains("software version") => detail.software_version = Some(value.to_string()),
            k if k.contains("firmware version") => detail.firmware_version = Some(value.to_string()),
            k if k.contains("family id") => detail.family_id = Some(value.to_string()),
            k if k.contains("family sku") => detail.family_sku = Some(value.to_string()),
            k if k.contains("family variant sku") => detail.family_variant_sku = Some(value.to_string()),
            k if k.contains("tis") && k.contains("trp tested") => detail.tis_trp_tested = Some(value.to_string()),
            k if k.contains("specification version") => detail.specification_version = Some(value.to_string()),
            k if k.contains("transport interface") => detail.transport_interface = Some(value.to_string()),
            k if k.contains("primary device type id") => detail.primary_device_type_id = Some(value.to_string()),
            k if k.contains("device type") || k.contains("product type") => detail.device_type = Some(value.to_string()),
            _ => {} // Ignore unrecognized fields
        }
    }

    /// Map detail field to ProductDetail field (guide-based approach)
    fn map_detail_field(&self, label: &str, value: &str, detail: &mut ProductDetail) {
        match label {
            l if l.contains("manufacturer") || l.contains("company") => detail.manufacturer = Some(value.to_string()),
            l if l.contains("vendor") || l.contains("vid") => detail.vid = self.parse_numeric_id(value),
            l if l.contains("product id") || l.contains("pid") => detail.pid = self.parse_numeric_id(value),
            l if l.contains("certificate") || l.contains("cert id") => {
                // Extract certificate ID with regex pattern matching
                if let Ok(regex) = regex::Regex::new(r"([A-Za-z0-9-]+\d+[-][A-Za-z0-9-]+)") {
                    if let Some(captures) = regex.captures(value) {
                        detail.certification_id = Some(captures.get(1).unwrap().as_str().to_string());
                    } else {
                        detail.certification_id = Some(value.to_string());
                    }
                }
            },
            l if l.contains("certification date") || (l.contains("date") && l.contains("cert")) => {
                detail.certification_date = Some(value.to_string());
            },
            l if l.contains("family id") => detail.family_id = Some(value.to_string()),
            l if l.contains("family sku") => detail.family_sku = Some(value.to_string()),
            l if l.contains("family variant sku") => detail.family_variant_sku = Some(value.to_string()),
            l if l.contains("firmware version") || (l.contains("firmware") && !l.contains("hardware")) => {
                detail.firmware_version = Some(value.to_string());
            },
            l if l.contains("hardware version") || (l.contains("hardware") && !l.contains("firmware")) => {
                detail.hardware_version = Some(value.to_string());
            },
            l if l.contains("software") && !l.contains("hardware") => {
                detail.software_version = Some(value.to_string());
            },
            l if l.contains("tis") && l.contains("trp") => detail.tis_trp_tested = Some(value.to_string()),
            l if l.contains("specification version") || l.contains("spec version") => {
                detail.specification_version = Some(value.to_string());
            },
            l if l.contains("transport interface") => detail.transport_interface = Some(value.to_string()),
            l if l.contains("primary device type") || l.contains("device type id") => {
                detail.primary_device_type_id = Some(value.to_string());
            },
            l if l.contains("device type") || l.contains("product type") || l.contains("category") => {
                detail.device_type = Some(value.to_string());
            },
            _ => {} // Ignore unrecognized fields
        }
    }

    /// Parse numeric ID from string (guide approach for hex/decimal handling)
    fn parse_numeric_id(&self, text: &str) -> Option<i32> {
        if text.starts_with("0x") || text.starts_with("0X") {
            i32::from_str_radix(&text[2..], 16).ok()
        } else {
            text.parse::<i32>().ok()
        }
    }

    /// Extract text content from an element using a CSS selector
    fn extract_field_text(&self, html: &Html, selector: &str) -> Option<String> {
        let selector_parsed = Selector::parse(selector).ok()?;
        html.select(&selector_parsed)
            .next()
            .map(|element| element.text().collect::<String>().trim().to_string())
            .filter(|text| !text.is_empty())
    }

    /// Resolve relative URLs to absolute URLs
    fn resolve_url(&self, href: &str, base_url: &str) -> String {
        if href.starts_with("http") {
            href.to_string()
        } else if href.starts_with("/") {
            format!("{}{}", base_url.trim_end_matches('/'), href)
        } else {
            format!("{}/{}", base_url.trim_end_matches('/'), href)
        }
    }
}

// ============================================================================
// TESTS MODULE - Integrated tests for better cohesion
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::Html;

    // Minimal test data - only what's needed for comprehensive testing
    const SAMPLE_LISTING_HTML: &str = r#"
<div class="post-feed item-count-3" data-item-count="3">
    <div class="inner">
        <article class="post-103277 product type-product">
            <a href="https://csa-iot.org/csa_product/wi-fi-plug-27/">
                <div class="inner">
                    <div class="contents">
                        <p class="entry-company notranslate">Tuya Global Inc.</p>
                        <h3 class="entry-title">Wi-Fi plug</h3>
                        <p class="entry-certificate-id">Certificate ID: CSA22059MAT40059-24</p>
                    </div>
                </div>
            </a>
        </article>
        <article class="post-103278 product type-product">
            <a href="https://csa-iot.org/csa_product/wi-fi-plug-28/">
                <div class="inner">
                    <div class="contents">
                        <p class="entry-company notranslate">Tuya Global Inc.</p>
                        <h3 class="entry-title">Wi-Fi plug 2</h3>
                        <p class="entry-certificate-id">Certificate ID: CSA22060MAT40060-24</p>
                    </div>
                </div>
            </a>
        </article>
        <article class="post-95714 product type-product">
            <a href="https://csa-iot.org/csa_product/test-product/">
                <div class="inner">
                    <div class="contents">
                        <p class="entry-company notranslate">Test Company</p>
                        <h3 class="entry-title">Test Product</h3>
                    </div>
                </div>
            </a>
        </article>
    </div>
</div>
"#;

    const SAMPLE_DETAIL_HTML: &str = r#"
<html>
    <body>
        <h1 class="entry-title">Test Product Detail</h1>
        <p class="entry-company">Test Manufacturer</p>
        <h6 class="entry-category">Test Device Type</h6>
        
        <div class="entry-product-details">
            <div>
                <ul>
                    <li class="item">
                        <span class="label">Certificate ID</span>
                        <span class="value">CSA12345MAT12345-24</span>
                    </li>
                    <li class="item">
                        <span class="label">Vendor ID</span>
                        <span class="value">0x1234</span>
                    </li>
                    <li class="item">
                        <span class="label">Product ID</span>
                        <span class="value">5678</span>
                    </li>
                    <li class="item">
                        <span class="label">Hardware Version</span>
                        <span class="value">1.0</span>
                    </li>
                </ul>
            </div>
        </div>
        
        <table class="product-certificates-table">
            <tr>
                <td>Certification Date</td>
                <td>2024-01-15</td>
            </tr>
            <tr>
                <td>Software Version</td>
                <td>2.1.0</td>
            </tr>
        </table>
    </body>
</html>
"#;

    #[test]
    fn test_extractor_creation() {
        let extractor = MatterDataExtractor::new();
        assert!(extractor.is_ok());
        
        let extractor = extractor.unwrap();
        assert_eq!(extractor.config.base_url, csa_iot::BASE_URL);
    }

    #[test]
    fn test_url_resolution() {
        let extractor = MatterDataExtractor::new().unwrap();
        
        assert_eq!(
            extractor.resolve_url("/product/123", "https://example.com"),
            "https://example.com/product/123"
        );
        
        assert_eq!(
            extractor.resolve_url("https://other.com/test", "https://example.com"),
            "https://other.com/test"
        );
        
        assert_eq!(
            extractor.resolve_url("relative/path", "https://example.com"),
            "https://example.com/relative/path"
        );
    }

    #[test]
    fn test_parse_numeric_id() {
        let extractor = MatterDataExtractor::new().unwrap();
        
        assert_eq!(extractor.parse_numeric_id("123"), Some(123));
        assert_eq!(extractor.parse_numeric_id("0x1A"), Some(26));
        assert_eq!(extractor.parse_numeric_id("0X1A"), Some(26));
        assert_eq!(extractor.parse_numeric_id("not_a_number"), None);
        assert_eq!(extractor.parse_numeric_id(""), None);
    }

    #[test]
    fn test_page_type_detection() {
        let extractor = MatterDataExtractor::new().unwrap();
        let html = Html::parse_document(SAMPLE_LISTING_HTML);
        
        // Should detect listing pages
        assert!(extractor.is_listing_page(&html, "https://csa-iot.org/csa-iot_products/?page=1"));
        assert!(extractor.is_listing_page(&html, "https://csa-iot.org/products"));
        
        // Should not detect detail pages - use a simple HTML without multiple articles
        let detail_html = Html::parse_document("<html><body><h1>Single Product</h1></body></html>");
        assert!(!extractor.is_listing_page(&detail_html, "https://csa-iot.org/csa_product/test-123"));
    }

    #[test]
    fn test_product_url_extraction() {
        let extractor = MatterDataExtractor::new().unwrap();
        let html = Html::parse_document(SAMPLE_LISTING_HTML);
        
        let urls = extractor.extract_product_urls(&html, "https://csa-iot.org").unwrap();
        
        assert_eq!(urls.len(), 3);
        assert!(urls.contains(&"https://csa-iot.org/csa_product/wi-fi-plug-27/".to_string()));
        assert!(urls.contains(&"https://csa-iot.org/csa_product/wi-fi-plug-28/".to_string()));
        assert!(urls.contains(&"https://csa-iot.org/csa_product/test-product/".to_string()));
    }

    #[test]
    fn test_certificate_id_extraction() {
        let extractor = MatterDataExtractor::new().unwrap();
        let html = Html::parse_document(SAMPLE_LISTING_HTML);
        
        if let Ok(article_selector) = scraper::Selector::parse("div.post-feed article") {
            let articles: Vec<_> = html.select(&article_selector).collect();
            
            // First article should have certificate ID
            let cert_id = extractor.extract_certificate_id_from_article(&articles[0]);
            assert_eq!(cert_id, Some("CSA22059MAT40059-24".to_string()));
            
            // Third article should have no certificate ID
            let cert_id = extractor.extract_certificate_id_from_article(&articles[2]);
            assert_eq!(cert_id, None);
        }
    }

    #[test]
    fn test_product_list_extraction() {
        let extractor = MatterDataExtractor::new().unwrap();
        
        let products = extractor.extract_basic_product_info(
            SAMPLE_LISTING_HTML, 
            "https://csa-iot.org/csa-iot_products/?page=1"
        ).unwrap();
        
        assert_eq!(products.len(), 3);
        
        // Test reverse order processing - first product should be the last in HTML
        let first_product = &products[0];
        assert_eq!(first_product.manufacturer, Some("Test Company".to_string()));
        assert_eq!(first_product.model, Some("Test Product".to_string()));
        assert_eq!(first_product.certificate_id, None);
        assert_eq!(first_product.page_id, Some(1));
        assert_eq!(first_product.index_in_page, Some(0));
        
        // Second product
        let second_product = &products[1];
        assert_eq!(second_product.manufacturer, Some("Tuya Global Inc.".to_string()));
        assert_eq!(second_product.model, Some("Wi-Fi plug 2".to_string()));
        assert_eq!(second_product.certificate_id, Some("CSA22060MAT40060-24".to_string()));
        assert_eq!(second_product.page_id, Some(1));
        assert_eq!(second_product.index_in_page, Some(1));
    }

    #[test]
    fn test_product_detail_extraction() {
        let extractor = MatterDataExtractor::new().unwrap();
        
        let html = Html::parse_document(SAMPLE_DETAIL_HTML);
        let detail = extractor.extract_product_detail(&html, "https://test.com/product/123".to_string()).unwrap();
        
        assert_eq!(detail.url, "https://test.com/product/123");
        assert_eq!(detail.model, Some("Test Product Detail".to_string()));
        assert_eq!(detail.manufacturer, Some("Test Manufacturer".to_string()));
        assert_eq!(detail.device_type, Some("Test Device Type".to_string()));
        assert_eq!(detail.certification_id, Some("CSA12345MAT12345-24".to_string()));
        assert_eq!(detail.certification_date, Some("2024-01-15".to_string()));
        assert_eq!(detail.vid, Some(0x1234));
        assert_eq!(detail.pid, Some(5678));
        assert_eq!(detail.hardware_version, Some("1.0".to_string()));
        assert_eq!(detail.software_version, Some("2.1.0".to_string()));
    }

    #[test]
    fn test_detail_list_extraction() {
        let extractor = MatterDataExtractor::new().unwrap();
        let html = Html::parse_document(SAMPLE_DETAIL_HTML);
        
        let mut detail = ProductDetail {
            url: "test".to_string(),
            page_id: None,
            index_in_page: None,
            id: None,
            manufacturer: None,
            model: None,
            device_type: None,
            certification_id: None,
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
            description: None,
            compliance_document_url: None,
            program_type: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        extractor.extract_from_detail_list(&html, &mut detail).unwrap();
        
        assert_eq!(detail.certification_id, Some("CSA12345MAT12345-24".to_string()));
        assert_eq!(detail.vid, Some(0x1234));
        assert_eq!(detail.pid, Some(5678));
        assert_eq!(detail.hardware_version, Some("1.0".to_string()));
    }

    #[test]
    fn test_total_pages_extraction() {
        let extractor = MatterDataExtractor::new().unwrap();
        
        let html_with_pagination = r#"
        <div class="pagination">
            <a href="?page=1">1</a>
            <a href="?page=2">2</a>
            <a href="?page=5">5</a>
        </div>
        "#;
        
        let total_pages = extractor.extract_total_pages(html_with_pagination).unwrap();
        assert_eq!(total_pages, 5);
    }

    #[test]
    fn test_extract_product_data_json() {
        let extractor = MatterDataExtractor::new().unwrap();
        
        let result = extractor.extract_product_data(SAMPLE_DETAIL_HTML);
        assert!(result.is_ok());
        
        let json = result.unwrap();
        assert!(json["model"].as_str().unwrap().contains("Test Product Detail"));
        assert!(json["manufacturer"].as_str().unwrap().contains("Test Manufacturer"));
    }
}

