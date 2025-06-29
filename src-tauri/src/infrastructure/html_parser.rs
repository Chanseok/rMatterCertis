//! HTML parsing and data extraction for Matter certification data
//! 
//! This module provides specialized extractors for parsing HTML content
//! from certification websites and extracting product information.

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
                // CSA-IoT 웹사이트의 실제 구조에 맞춘 셀렉터
                product_container: "tr.product-row, .product-item, .product-list-item, .cert-product".to_string(),
                product_link: "a[href*='/product/'], a[href*='/certification/'], a[href*='csa-iot_products']".to_string(),
                manufacturer: ".manufacturer, .company-name, td:nth-child(2), .vendor".to_string(),
                model: ".model, .product-name, .product-title, td:nth-child(3)".to_string(),
                certificate_id: ".certificate-id, .cert-id, td:nth-child(1), .certification-number".to_string(),
            },
            product_detail_selectors: ProductDetailSelectors {
                manufacturer: ".manufacturer, .vendor-name, td:contains('Manufacturer') + td, .company-name".to_string(),
                model: ".model, .product-name, td:contains('Model') + td, .product-title".to_string(),
                device_type: ".device-type, td:contains('Device Type') + td, .category".to_string(),
                certification_id: ".certification-id, td:contains('Certification ID') + td, .cert-id".to_string(),
                certification_date: ".certification-date, td:contains('Date') + td, .cert-date".to_string(),
                software_version: ".software-version, td:contains('Software') + td".to_string(),
                hardware_version: ".hardware-version, td:contains('Hardware') + td".to_string(),
                vid: ".vid, td:contains('VID') + td, td:contains('Vendor ID') + td".to_string(),
                pid: ".pid, td:contains('PID') + td, td:contains('Product ID') + td".to_string(),
                specification_version: ".spec-version, td:contains('Specification') + td".to_string(),
                transport_interface: ".transport, td:contains('Transport') + td".to_string(),
                info_table: "table, .info-table, .product-details-table, .specifications".to_string(),
            },
            base_url: csa_iot::BASE_URL.to_string(),
        }
    }
}

/// Specialized data extractor for Matter certification websites
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
                    self.resolve_url(href, base_url)
                })
            })
            .collect();

        debug!("Extracted {} product URLs", urls.len());
        Ok(urls)
    }

    /// Extract basic product information from listing page
    pub fn extract_products_from_list(&self, html: &Html, page_id: i32) -> Result<Vec<Product>> {
        debug!("Extracting products from listing page {}", page_id);
        
        let container_selector = Selector::parse(&self.config.product_list_selectors.product_container)
            .map_err(|e| anyhow!("Invalid container selector: {}", e))?;

        let mut products = Vec::new();
        
        for (index, container) in html.select(&container_selector).enumerate() {
            if let Ok(product) = self.extract_single_product_from_list(container, page_id, index as i32) {
                products.push(product);
            }
        }

        debug!("Extracted {} products from listing page", products.len());
        Ok(products)
    }

    /// Extract detailed product information from a product detail page
    pub fn extract_product_detail(&self, html: &Html, url: String) -> Result<ProductDetail> {
        debug!("Extracting product detail from: {}", url);
        
        let now = chrono::Utc::now();
        
        Ok(ProductDetail {
            url,
            page_id: None,
            index_in_page: None,
            id: self.extract_field_text(html, &self.config.product_detail_selectors.certification_id),
            manufacturer: self.extract_field_text(html, &self.config.product_detail_selectors.manufacturer),
            model: self.extract_field_text(html, &self.config.product_detail_selectors.model),
            device_type: self.extract_field_text(html, &self.config.product_detail_selectors.device_type),
            certification_id: self.extract_field_text(html, &self.config.product_detail_selectors.certification_id),
            certification_date: self.extract_field_text(html, &self.config.product_detail_selectors.certification_date),
            software_version: self.extract_field_text(html, &self.config.product_detail_selectors.software_version),
            hardware_version: self.extract_field_text(html, &self.config.product_detail_selectors.hardware_version),
            vid: self.extract_numeric_field(html, &self.config.product_detail_selectors.vid),
            pid: self.extract_numeric_field(html, &self.config.product_detail_selectors.pid),
            family_sku: None, // These fields would need specific selectors
            family_variant_sku: None,
            firmware_version: None,
            family_id: None,
            tis_trp_tested: None,
            specification_version: self.extract_field_text(html, &self.config.product_detail_selectors.specification_version),
            transport_interface: self.extract_field_text(html, &self.config.product_detail_selectors.transport_interface),
            primary_device_type_id: None,
            application_categories: None,
            description: None,
            compliance_document_url: None,
            program_type: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// Extract a single product from a list container element
    fn extract_single_product_from_list(&self, container: ElementRef, page_id: i32, index: i32) -> Result<Product> {
        let now = chrono::Utc::now();
        
        // Extract URL - first try to find a link within the container
        let url = container
            .select(&Selector::parse("a").unwrap())
            .next()
            .and_then(|link| link.value().attr("href"))
            .map(|href| self.resolve_url(href, &self.config.base_url))
            .unwrap_or_else(|| format!("unknown-{}-{}", page_id, index));

        Ok(Product {
            url,
            manufacturer: self.extract_text_from_element(&container, &self.config.product_list_selectors.manufacturer),
            model: self.extract_text_from_element(&container, &self.config.product_list_selectors.model),
            certificate_id: self.extract_text_from_element(&container, &self.config.product_list_selectors.certificate_id),
            page_id: Some(page_id),
            index_in_page: Some(index),
            created_at: now,
            updated_at: now,
        })
    }

    /// Extract text content from an element using a CSS selector
    fn extract_field_text(&self, html: &Html, selector: &str) -> Option<String> {
        let selector_parsed = Selector::parse(selector).ok()?;
        html.select(&selector_parsed)
            .next()
            .map(|element| element.text().collect::<String>().trim().to_string())
            .filter(|text| !text.is_empty())
    }

    /// Extract numeric field (like VID/PID) and parse as i32
    fn extract_numeric_field(&self, html: &Html, selector: &str) -> Option<i32> {
        self.extract_field_text(html, selector)
            .and_then(|text| {
                // Try to extract hex or decimal number
                if text.starts_with("0x") || text.starts_with("0X") {
                    i32::from_str_radix(&text[2..], 16).ok()
                } else {
                    text.parse::<i32>().ok()
                }
            })
    }

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
