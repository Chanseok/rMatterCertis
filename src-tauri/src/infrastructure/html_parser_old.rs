//! HTML parsi/// Configuration for CSA-IoT websixtraction for Matter certification data
//! 
//! This module provides specialized extractors for parsing HTML content
//! from certification websites and extracting product information.

#![allow(clippy::uninlined_format_args)]

use anyhow::{anyhow, Result};
use scraper::{Html, Selector, ElementRef};
use tracing::debug;
use crate::infrastructure::csa_iot;

use crate::domain::product::{Product, ProductDetail};

/// Configuratio    /// Check if certificate ID format is valid (simplified)
    fn is_valid_certificate_id(&self, cert_id: &str) -> bool {
        // Basic validation: non-empty and has reasonable format
        if cert_id.len() < 5 || cert_id.chars().all(|c| c.is_numeric()) {
            return false;
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

    /// Extract product URLs from a product listing page
    pub fn extract_product_urls(&self, html: &Html, base_url: &str) -> Result<Vec<String>> {
        debug!("Extracting product URLs from listing page");
        
        let link_selector = Selector::parse(&self.config.product_list_selectors.product_link)
            .map_err(|e| anyhow!("Invalid product link selector: {}", e))?;

        let urls: Vec<String> = html
            .select(&link_selector)
            .filter_map(|element| {
                element.value().attr("href").map(|href| {
                    let url = self.resolve_url(href, base_url);
                    // 실제 제품 페이지인지 확인 (csa_product 경로만 허용)
                    if url.contains("/csa_product/") && !url.contains("/csa-iot_products/") {
                        Some(url)
                    } else {
                        debug!("Filtered out non-product URL: {}", url);
                        None
                    }
                })
            })
            .flatten()
            .collect();

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
        
        // CSA-IoT 사이트의 페이지네이션에서 마지막 페이지 번호 추출
        // 예: "Page 1 of 23" 또는 페이지 링크에서 최대값 찾기
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

        // "Page X of Y" 형태의 텍스트에서  총 페이지 수 추출
        let page_info_selectors = vec![
            ".pagination-info",
            ".page-info", 
            ".showing-info",
        ];

        // 모든 루프 외부로 정규식 컴파일 이동
        let re = regex::Regex::new(r"(?i)page\s+\d+\s+of\s+(\d+)").unwrap();

        for selector_str in page_info_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                for element in html.select(&selector) {
                    let text = element.text().collect::<String>();
                    // "Page 1 of 23" 형태에서 23 추출
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
        
        // URL이 필요하지만 여기서는 알 수 없으므로 기본값 사용
        let product_detail = self.extract_product_detail(&html, "".to_string())?;
        
        // ProductDetail을 JSON으로 변환
        let json_value = serde_json::to_value(product_detail)
            .map_err(|e| anyhow!("Failed to serialize product detail: {}", e))?;
        
        Ok(json_value)
    }

    /// Extract basic product information from a page (listing or detail page)
    /// This is the main entry point for extracting fundamental product data
    pub fn extract_basic_product_info(&self, html_content: &str, page_url: &str) -> Result<Vec<Product>> {
        debug!("Extracting basic product info from: {}", page_url);
        let html = Html::parse_document(html_content);
        
        // Determine if this is a listing page or detail page
        if self.is_listing_page(&html, page_url) {
            // Extract products from listing page
            let page_id = self.extract_page_id_from_url(page_url);
            self.extract_products_from_list(&html, page_id)
        } else {
            // Single product detail page
            let product = self.extract_single_product_from_detail_page(&html, page_url)?;
            Ok(vec![product])
        }
    }

    /// Check if the given HTML represents a listing page
    fn is_listing_page(&self, html: &Html, url: &str) -> bool {
        // Check for listing page indicators
        if url.contains("page=") || url.contains("products") && !url.contains("csa_product") {
            return true;
        }
        
        // Check for presence of multiple product containers
        if let Ok(container_selector) = Selector::parse(&self.config.product_list_selectors.product_container) {
            let container_count = html.select(&container_selector).count();
            return container_count > 1;
        }
        
        false
    }

    /// Extract page ID from URL (for listing pages)
    fn extract_page_id_from_url(&self, url: &str) -> i32 {
        // Extract page number from URL like "?page=5"
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
        
        // Simple extraction using guide selectors
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
            page_id: None, // Detail pages don't have page IDs
            index_in_page: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// Extract basic product information from listing page (guide-based approach)
    pub fn extract_products_from_list(&self, html: &Html, page_id: i32) -> Result<Vec<Product>> {
        debug!("Extracting products from listing page {}", page_id);
        
        // Use the exact selector from guide: "div.post-feed article"
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
        
        // Extract from table using guide approach
        let mut detail = ProductDetail {
            url,
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
            created_at: now,
            updated_at: now,
        };

        // Extract from information table (guide approach)
        self.extract_from_table(html, &mut detail)?;
        
        // Extract from detail list items (guide approach)  
        self.extract_from_detail_list(html, &mut detail)?;

        Ok(detail)
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
        let detail_list_selector = Selector::parse("div.entry-product-details > div > ul li")
            .map_err(|e| anyhow!("Invalid detail list selector: {}", e))?;

        for item in html.select(&detail_list_selector) {
            let full_text = item.text().collect::<Vec<_>>().join("").trim().to_string();
            
            if let Some(colon_index) = full_text.find(':') {
                let raw_label = full_text[..colon_index].trim().to_lowercase();
                let raw_value = full_text[colon_index + 1..].trim().to_string();
                
                if !raw_value.is_empty() {
                    self.map_detail_field(&raw_label, &raw_value, detail);
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

    /// Extract a single product from a list container element (guide-based approach)
    fn extract_single_product_from_list(&self, container: ElementRef, page_id: i32, index: i32) -> Result<Product> {
        let now = chrono::Utc::now();
        
        // Extract URL - simple and direct approach
        let link_selector = Selector::parse("a").unwrap();
        let url = container
            .select(&link_selector)
            .next()
            .and_then(|link| link.value().attr("href"))
            .map(|href| self.resolve_url(href, &self.config.base_url))
            .unwrap_or_else(|| format!("unknown-{}-{}", page_id, index));

        // Extract manufacturer - exactly as in guide
        let manufacturer_selector = Selector::parse("p.entry-company.notranslate").unwrap();
        let manufacturer = container
            .select(&manufacturer_selector)
            .next()
            .map(|el| el.text().collect::<Vec<_>>().join("").trim().to_string())
            .filter(|s| !s.is_empty());

        // Extract model - exactly as in guide  
        let model_selector = Selector::parse("h3.entry-title").unwrap();
        let model = container
            .select(&model_selector)
            .next()
            .map(|el| el.text().collect::<Vec<_>>().join("").trim().to_string())
            .filter(|s| !s.is_empty());

        // Extract certificate ID with fallback logic from guide
        let certificate_id = self.extract_certificate_id_from_article(&container);

        debug!("Extracted from listing container {}: manufacturer={:?}, model={:?}, cert_id={:?}", 
               index, manufacturer, model, certificate_id);

        Ok(Product {
            url,
            manufacturer,
            model,
            certificate_id,
            device_type: None, // Not available in product list parsing
            certification_date: None, // Not available in product list parsing
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

    /// Extract text content from an element using a CSS selector
    fn extract_field_text(&self, html: &Html, selector: &str) -> Option<String> {
        let selector_parsed = Selector::parse(selector).ok()?;
        html.select(&selector_parsed)
            .next()
            .map(|element| element.text().collect::<String>().trim().to_string())
            .filter(|text| !text.is_empty())
    }

    /// Extract text from element using a selector
    fn extract_text_from_element(&self, element: &ElementRef, selector: &str) -> Option<String> {
        let selector_parsed = Selector::parse(selector).ok()?;
        element.select(&selector_parsed)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .filter(|text| !text.is_empty())
    }

    /// Validate certificate ID format
    fn is_valid_certificate_id(&self, cert_id: &str) -> bool {
        // Certificate IDs typically contain:
        // - Letters and numbers
        // - Hyphens or underscores
        // - At least 5 characters
        // - No pure numeric strings (those are usually other IDs)
        
        if cert_id.len() < 5 || cert_id.chars().all(|c| c.is_numeric()) {
            return false;
        }
        
        // Check for typical certificate patterns
        let has_letters = cert_id.chars().any(|c| c.is_alphabetic());
        let has_separators = cert_id.contains('-') || cert_id.contains('_');
        
        has_letters && (has_separators || cert_id.len() >= 8)
    }

    /// Extract certificate ID using regex patterns from page text
    /// Check if certificate ID format is valid (simplified)  
    fn is_valid_certificate_id(&self, cert_id: &str) -> bool {
        // Basic validation: non-empty and has reasonable format
        if cert_id.len() < 5 || cert_id.chars().all(|c| c.is_numeric()) {
            return false;
        }
        
        // Check for typical certificate patterns
        let has_letters = cert_id.chars().any(|c| c.is_alphabetic());
        let has_separators = cert_id.contains('-') || cert_id.contains('_');
        
        has_letters && (has_separators || cert_id.len() >= 8)
    }

    /// Extract numeric field (like VID/PID) and parse as i32
    /// Extract text from a specific element using a CSS selector
    fn extract_text_from_element(&self, element: &ElementRef, selector: &str) -> Option<String> {
        let selector_parsed = Selector::parse(selector).ok()?;
        element.select(&selector_parsed)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extractor_creation() {
        let extractor = MatterDataExtractor::new();
        assert!(extractor.is_ok());
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
    fn test_basic_product_info_extraction() {
        let extractor = MatterDataExtractor::new().unwrap();
        let html_content = r#"
            <div class="post-feed">
                <article class="type-product">
                    <p class="entry-company notranslate">Test Company</p>
                    <h3 class="entry-title">Smart Device Model X</h3>
                    <span class="entry-cert-id">CSA-2024-001</span>
                    <a href="/csa_product/123">View Details</a>
                </article>
            </div>
        "#;

        let products = extractor.extract_basic_product_info(html_content, "https://example.com/products?page=1").unwrap();
        assert_eq!(products.len(), 1);
        
        let product = &products[0];
        assert_eq!(product.manufacturer, Some("Test Company".to_string()));
        assert_eq!(product.model, Some("Smart Device Model X".to_string()));
        assert_eq!(product.certificate_id, Some("CSA-2024-001".to_string()));
        assert!(product.url.contains("/csa_product/123"));
    }

    #[test]
    fn test_certificate_id_validation() {
        let extractor = MatterDataExtractor::new().unwrap();
        
        // Valid certificate IDs
        assert!(extractor.is_valid_certificate_id("CSA-2024-001"));
        assert!(extractor.is_valid_certificate_id("CERT-123-ABC"));
        assert!(extractor.is_valid_certificate_id("ABC123DEF"));
        
        // Invalid certificate IDs
        assert!(!extractor.is_valid_certificate_id("123"));
        assert!(!extractor.is_valid_certificate_id("ABC"));
        assert!(!extractor.is_valid_certificate_id("12345"));
    }

    #[test]
    fn test_page_type_detection() {
        let extractor = MatterDataExtractor::new().unwrap();
        
        // Listing page
        let listing_html = Html::parse_document(r#"
            <div class="post-feed">
                <article class="type-product">Product 1</article>
                <article class="type-product">Product 2</article>
            </div>
        "#);
        assert!(extractor.is_listing_page(&listing_html, "https://example.com/products?page=1"));
        
        // Detail page
        let detail_html = Html::parse_document(r#"
            <div class="product-detail">
                <h1>Single Product</h1>
            </div>
        "#);
        assert!(!extractor.is_listing_page(&detail_html, "https://example.com/csa_product/123"));
    }

    #[test]
    fn test_enhanced_manufacturer_extraction() {
        let extractor = MatterDataExtractor::new().unwrap();
        let html = Html::parse_document(r#"
            <div>
                <p class="entry-company notranslate">Enhanced Test Company</p>
            </div>
        "#);

        let manufacturer = extractor.extract_manufacturer_enhanced(&html);
        assert_eq!(manufacturer, Some("Enhanced Test Company".to_string()));
    }

    #[test]
    fn test_product_list_extraction() {
        let extractor = MatterDataExtractor::new().unwrap();
        let html = Html::parse_document(r#"
            <table>
                <tr class="product-row">
                    <td class="certificate-id">CERT-001</td>
                    <td class="manufacturer">Acme Corp</td>
                    <td class="model">Smart Switch</td>
                    <td><a href="/product/123">Details</a></td>
                </tr>
                <tr class="product-row">
                    <td class="certificate-id">CERT-002</td>
                    <td class="manufacturer">Beta Inc</td>
                    <td class="model">Smart Bulb</td>
                    <td><a href="/product/456">Details</a></td>
                </tr>
            </table>
        "#);

        let products = extractor.extract_products_from_list(&html, 1).unwrap();
        assert_eq!(products.len(), 2);
        
        assert_eq!(products[0].manufacturer, Some("Acme Corp".to_string()));
        assert_eq!(products[0].model, Some("Smart Switch".to_string()));
        assert_eq!(products[0].certificate_id, Some("CERT-001".to_string()));
        
        assert_eq!(products[1].manufacturer, Some("Beta Inc".to_string()));
        assert_eq!(products[1].model, Some("Smart Bulb".to_string()));
    }
}
