//! Parsing configuration for HTML extraction
//!
//! Centralized configuration for CSS selectors and parsing behavior.

use serde::{Deserialize, Serialize};

/// Main parsing configuration
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

    /// Product list selectors
    pub product_list_selectors: ProductListSelectors,

    /// Product detail selectors
    pub product_detail_selectors: ProductDetailSelectors,
}

impl Default for ParsingConfig {
    fn default() -> Self {
        use crate::infrastructure::config::csa_iot;
        Self {
            base_url: csa_iot::PRODUCTS_PAGE_GENERAL.to_string(),
            user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36".to_string(),
            timeout_ms: 30000,
            retry_count: 3,
            request_delay_ms: 1000,
            product_list_selectors: ProductListSelectors::default(),
            product_detail_selectors: ProductDetailSelectors::default(),
        }
    }
}

/// CSS selectors for product list pages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductListSelectors {
    /// Selectors for product containers - multiple fallbacks
    pub product_container: Vec<String>,

    /// Selectors for product detail page links
    pub product_link: Vec<String>,

    /// Selectors for manufacturer/brand name
    pub manufacturer: Vec<String>,

    /// Selectors for model/product name
    pub model: Vec<String>,

    /// Selectors for certification ID
    pub certificate_id: Vec<String>,

    /// Selectors for pagination
    pub pagination: Vec<String>,
}

impl Default for ProductListSelectors {
    fn default() -> Self {
        Self {
            product_container: vec![
                "div.product-item".to_string(),
                ".product-card".to_string(),
                ".product-listing-item".to_string(),
                "article.product".to_string(),
                "tr.product-row".to_string(),
                ".cert-product".to_string(),
            ],
            product_link: vec![
                "a[href*='/products/']".to_string(),
                "a[href*='product']".to_string(),
                ".product-link a".to_string(),
                "h3 a".to_string(),
                "a[href*='/certification/']".to_string(),
                "a[href*='csa-iot_products']".to_string(),
            ],
            manufacturer: vec![
                ".brand".to_string(),
                ".manufacturer".to_string(),
                ".vendor".to_string(),
                ".company-name".to_string(),
                ".brand-name".to_string(),
                "td:nth-child(2)".to_string(),
            ],
            model: vec![
                ".product-title".to_string(),
                ".product-name".to_string(),
                "h3".to_string(),
                "h2".to_string(),
                ".title".to_string(),
                ".name".to_string(),
                "td:nth-child(3)".to_string(),
            ],
            certificate_id: vec![
                ".certificate-id".to_string(),
                ".cert-id".to_string(),
                "td:nth-child(1)".to_string(),
                ".certification-number".to_string(),
            ],
            pagination: vec![
                ".pagination a".to_string(),
                ".page-numbers a".to_string(),
                ".next-page".to_string(),
                "a[rel='next']".to_string(),
            ],
        }
    }
}

/// CSS selectors for product detail pages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductDetailSelectors {
    /// Basic product information
    pub title: Vec<String>,
    pub manufacturer: Vec<String>,
    pub model: Vec<String>,
    pub category: Vec<String>,
    pub description: Vec<String>,

    /// Matter certification specific
    pub vid: Vec<String>,
    pub pid: Vec<String>,
    pub certification_type: Vec<String>,
    pub certification_date: Vec<String>,
    pub specification_version: Vec<String>,
    pub transport_interface: Vec<String>,

    /// Structured data containers
    pub info_table: Vec<String>,
    pub definition_list: Vec<String>,
}

impl Default for ProductDetailSelectors {
    fn default() -> Self {
        Self {
            title: vec![
                "h1".to_string(),
                ".product-title".to_string(),
                ".product-name".to_string(),
                ".title".to_string(),
            ],
            manufacturer: vec![
                ".manufacturer".to_string(),
                ".vendor-name".to_string(),
                "td:contains('Manufacturer') + td".to_string(),
                ".company-name".to_string(),
                ".brand".to_string(),
            ],
            model: vec![
                ".model".to_string(),
                ".product-name".to_string(),
                "td:contains('Model') + td".to_string(),
                ".product-title".to_string(),
            ],
            category: vec![
                ".category".to_string(),
                ".product-category".to_string(),
                ".type".to_string(),
                ".product-type".to_string(),
                "td:contains('Category') + td".to_string(),
            ],
            description: vec![
                ".product-description".to_string(),
                ".description".to_string(),
                ".product-info".to_string(),
                ".details".to_string(),
                ".summary".to_string(),
            ],
            vid: vec![
                ".vid".to_string(),
                "td:contains('VID') + td".to_string(),
                "td:contains('Vendor ID') + td".to_string(),
                ".vid-value".to_string(),
                "[data-field='vid']".to_string(),
            ],
            pid: vec![
                ".pid".to_string(),
                "td:contains('PID') + td".to_string(),
                "td:contains('Product ID') + td".to_string(),
                ".pid-value".to_string(),
                "[data-field='pid']".to_string(),
            ],
            certification_type: vec![
                ".certification-type".to_string(),
                ".cert-type".to_string(),
                "td:contains('Certification') + td".to_string(),
                ".program-type".to_string(),
            ],
            certification_date: vec![
                ".certification-date".to_string(),
                ".cert-date".to_string(),
                "td:contains('Date') + td".to_string(),
                "td:contains('Certified') + td".to_string(),
            ],
            specification_version: vec![
                ".spec-version".to_string(),
                "td:contains('Specification') + td".to_string(),
                ".specification-version".to_string(),
            ],
            transport_interface: vec![
                ".transport".to_string(),
                "td:contains('Transport') + td".to_string(),
                ".interface".to_string(),
                ".connectivity".to_string(),
            ],
            info_table: vec![
                "table".to_string(),
                ".info-table".to_string(),
                ".product-details-table".to_string(),
                ".specifications".to_string(),
                ".cert-details".to_string(),
            ],
            definition_list: vec![
                "dl".to_string(),
                ".definition-list".to_string(),
                ".specs-list".to_string(),
            ],
        }
    }
}
