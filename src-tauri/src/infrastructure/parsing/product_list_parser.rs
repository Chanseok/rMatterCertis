//! Product list parser implementation following the guide's architecture
//! 
//! Robust HTML parsing for product listing pages with fallback strategies
//! and comprehensive error handling.

#![allow(clippy::uninlined_format_args)]
#![allow(clippy::unnecessary_map_or)]

use super::{ContextualParser, ParsingError, ParsingResult, ParseContext};
use crate::domain::product::Product;
use scraper::{Html, Selector, ElementRef};
use anyhow::Result;
use url::Url;
use tracing::{debug, warn, error};

/// Parser for extracting product information from listing pages
pub struct ProductListParser {
    /// Compiled CSS selectors for different page elements
    product_container_selectors: Vec<Selector>,
    url_selectors: Vec<Selector>,
    title_selectors: Vec<Selector>,
    brand_selectors: Vec<Selector>,
    category_selectors: Vec<Selector>,
    pagination_selectors: Vec<Selector>,
}

impl ProductListParser {
    /// Create a new product list parser with default selectors
    pub fn new() -> Result<Self> {
        let config = super::config::ParsingConfig::default();
        Self::with_config(&config.product_list_selectors)
    }
    
    /// Create parser with custom selector configuration
    pub fn with_config(selectors: &super::config::ProductListSelectors) -> Result<Self> {
        Ok(Self {
            product_container_selectors: Self::compile_selectors(&selectors.product_container)?,
            url_selectors: Self::compile_selectors(&selectors.product_link)?,
            title_selectors: Self::compile_selectors(&selectors.model)?,
            brand_selectors: Self::compile_selectors(&selectors.manufacturer)?,
            category_selectors: Self::compile_selectors(&selectors.certificate_id)?, // Use cert_id as category fallback
            pagination_selectors: Self::compile_selectors(&selectors.pagination)?,
        })
    }
    
    /// Compile multiple selector strings into Selector objects
    fn compile_selectors(selector_strings: &[String]) -> Result<Vec<Selector>> {
        let mut selectors = Vec::new();
        let mut errors = Vec::new();
        
        for selector_str in selector_strings {
            match Selector::parse(selector_str) {
                Ok(selector) => selectors.push(selector),
                Err(e) => {
                    warn!("Failed to compile selector '{}': {}", selector_str, e);
                    errors.push(format!("'{}': {}", selector_str, e));
                }
            }
        }
        
        if selectors.is_empty() {
            return Err(anyhow::anyhow!(
                "No valid selectors compiled. Errors: {}", 
                errors.join(", ")
            ));
        }
        
        if !errors.is_empty() {
            debug!("Some selectors failed to compile: {}", errors.join(", "));
        }
        
        Ok(selectors)
    }
}

impl ContextualParser for ProductListParser {
    type Output = Vec<Product>;
    type Context = ParseContext;
    
    /// Parse product list with comprehensive error handling and fallback strategies
    fn parse_with_context(&self, html: &Html, context: &Self::Context) -> ParsingResult<Self::Output> {
        debug!("Parsing product list for page {}", context.page_id);
        
        let mut products = Vec::new();
        let mut tried_selectors = Vec::new();
        
        // Try each container selector until we find products
        for (i, selector) in self.product_container_selectors.iter().enumerate() {
            let selector_str = format!("container_selector_{}", i);
            tried_selectors.push(selector_str.clone());
            
            let product_elements: Vec<ElementRef> = html.select(selector).collect();
            
            if !product_elements.is_empty() {
                debug!(
                    "Found {} product containers using selector {}", 
                    product_elements.len(), 
                    selector_str
                );
                
                // Extract data from each product element
                for (index, element) in product_elements.iter().enumerate() {
                    match self.extract_product_from_element(element, index as u32, context) {
                        Ok(product) => {
                            if self.validate_product(&product)? {
                                products.push(product);
                            } else {
                                warn!(
                                    "Skipping invalid product at index {} on page {}", 
                                    index, 
                                    context.page_id
                                );
                            }
                        },
                        Err(e) => {
                            error!(
                                "Failed to extract product at index {} on page {}: {}", 
                                index, 
                                context.page_id, 
                                e
                            );
                            // Continue processing other products
                        }
                    }
                }
                
                // If we found products, break out of selector loop
                if !products.is_empty() {
                    break;
                }
            }
        }
        
        if products.is_empty() {
            return Err(ParsingError::no_products_found(context.page_id, tried_selectors));
        }
        
        debug!(
            "Successfully extracted {} products from page {}", 
            products.len(), 
            context.page_id
        );
        
        Ok(products)
    }
}

impl ProductListParser {
    /// Extract individual product data from HTML element with fallback strategies
    fn extract_product_from_element(
        &self, 
        element: &ElementRef, 
        index: u32,
        context: &ParseContext
    ) -> ParsingResult<Product> {
        
        // Extract product URL with fallbacks
        let url = self.extract_product_url(element, &context.base_url)?;
        
        // Extract product title/model name with fallbacks
        let model_name = self.extract_text_with_fallbacks(element, &self.title_selectors)
            .ok_or_else(|| ParsingError::required_field_missing("model_name", Some("product listing")))?;
        
        // Extract brand with fallbacks (allow empty)
        let brand = self.extract_text_with_fallbacks(element, &self.brand_selectors)
            .unwrap_or_else(|| "Unknown".to_string());
        
        // Extract category/certificate_id with fallbacks (allow empty)
        let category = self.extract_text_with_fallbacks(element, &self.category_selectors)
            .unwrap_or_else(|| "Unknown".to_string());
        
        let now = chrono::Utc::now();
        
        Ok(Product {
            url,
            manufacturer: Some(brand.trim().to_string()).filter(|s| s != "Unknown" && !s.is_empty()),
            model: Some(model_name.trim().to_string()).filter(|s| !s.is_empty()),
            certificate_id: Some(category.trim().to_string()).filter(|s| s != "Unknown" && !s.is_empty()),
            device_type: None, // Not available in product list parsing
            certification_date: None, // Not available in product list parsing
            page_id: Some(context.page_id as i32),
            index_in_page: Some(index as i32),
            created_at: now,
            updated_at: now,
        })
    }
    
    /// Extract product URL with comprehensive fallback and validation
    fn extract_product_url(&self, element: &ElementRef, base_url: &str) -> ParsingResult<String> {
        let mut attempted_selectors = Vec::new();
        
        for (i, selector) in self.url_selectors.iter().enumerate() {
            let selector_name = format!("url_selector_{}", i);
            attempted_selectors.push(selector_name);
            
            if let Some(url_element) = element.select(selector).next() {
                if let Some(href) = url_element.value().attr("href") {
                    return self.resolve_url(href, base_url);
                }
            }
        }
        
        Err(ParsingError::matter_field_extraction_failed(
            "url",
            "No valid href attribute found",
            attempted_selectors,
        ))
    }
    
    /// Extract text content using multiple selectors as fallbacks
    fn extract_text_with_fallbacks(&self, element: &ElementRef, selectors: &[Selector]) -> Option<String> {
        for selector in selectors {
            if let Some(text) = self.extract_text_by_selector(element, selector) {
                return Some(text);
            }
        }
        None
    }
    
    /// Extract text content using a single CSS selector
    fn extract_text_by_selector(&self, element: &ElementRef, selector: &Selector) -> Option<String> {
        element
            .select(selector)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .filter(|text| !text.is_empty())
    }
    
    /// Resolve relative URLs to absolute URLs with comprehensive validation
    fn resolve_url(&self, href: &str, base_url: &str) -> ParsingResult<String> {
        let resolved_url = if href.starts_with("http") {
            href.to_string()
        } else if href.starts_with("/") {
            // Absolute path
            let base = Url::parse(base_url)
                .map_err(|e| ParsingError::UrlResolutionFailed {
                    url: base_url.to_string(),
                    reason: format!("Invalid base URL: {}", e),
                    base_url: None,
                })?;
            
            base.join(href)
                .map_err(|e| ParsingError::UrlResolutionFailed {
                    url: href.to_string(),
                    reason: format!("Failed to join URL: {}", e),
                    base_url: Some(base_url.to_string()),
                })?
                .to_string()
        } else {
            // Relative path
            format!("{}/{}", base_url.trim_end_matches('/'), href.trim_start_matches('/'))
        };
        
        // Validate the resolved URL
        if Url::parse(&resolved_url).is_err() {
            return Err(ParsingError::UrlResolutionFailed {
                url: resolved_url,
                reason: "Resolved URL is not valid".to_string(),
                base_url: Some(base_url.to_string()),
            });
        }
        
        Ok(resolved_url)
    }
    
    /// Validate extracted product data
    fn validate_product(&self, product: &Product) -> ParsingResult<bool> {
        let mut errors = Vec::new();
        
        // Check required fields
        if product.url.is_empty() {
            errors.push("URL is empty".to_string());
        }
        
        if product.model.as_ref().map_or(true, |m| m.is_empty()) {
            errors.push("Model/title is empty".to_string());
        }
        
        if !errors.is_empty() {
            return Err(ParsingError::ProductValidationFailed {
                reason: "Required fields are missing or empty".to_string(),
                field_errors: errors,
            });
        }
        
        Ok(true)
    }
    
    /// Check if there are more pages to crawl
    pub fn has_next_page(&self, html: &Html) -> bool {
        for selector in &self.pagination_selectors {
            if html.select(selector).any(|element| {
                let text = element.text().collect::<String>().to_lowercase();
                text.contains("next") || text.contains("→") || text.contains("»")
            }) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parser_creation() {
        let parser = ProductListParser::new();
        assert!(parser.is_ok());
    }
    
    #[test]
    fn test_url_resolution() {
        let parser = ProductListParser::new().unwrap();
        
        let result = parser.resolve_url("/product/123", "https://example.com");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "https://example.com/product/123");
        
        let result = parser.resolve_url("https://other.com/test", "https://example.com");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "https://other.com/test");
        
        let result = parser.resolve_url("relative/path", "https://example.com");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "https://example.com/relative/path");
    }
}
