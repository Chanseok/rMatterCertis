//! Product detail parser implementation following the guide's architecture
//! 
//! Advanced HTML parsing for product detail pages with multiple extraction strategies,
//! regex fallbacks, and comprehensive Matter certification data extraction.

#![allow(clippy::uninlined_format_args)]

use super::{ContextualParser, ParsingError, ParsingResult};
use super::context::DetailParseContext;
use crate::domain::product::ProductDetail;
use scraper::{Html, Selector, ElementRef};
use regex::Regex;
use std::collections::HashMap;
use anyhow::Result;
use tracing::{debug, warn};

/// Parser for extracting detailed product information from product detail pages
#[allow(dead_code)]
pub struct ProductDetailParser {
    /// Compiled selectors for basic product information
    title_selectors: Vec<Selector>,
    manufacturer_selectors: Vec<Selector>,
    model_selectors: Vec<Selector>,
    category_selectors: Vec<Selector>,
    description_selectors: Vec<Selector>,
    
    /// Compiled selectors for Matter-specific information
    vid_selectors: Vec<Selector>,
    pid_selectors: Vec<Selector>,
    certification_type_selectors: Vec<Selector>,
    certification_date_selectors: Vec<Selector>,
    specification_version_selectors: Vec<Selector>,
    transport_interface_selectors: Vec<Selector>,
    
    /// Compiled selectors for structured data containers
    info_table_selectors: Vec<Selector>,
    definition_list_selectors: Vec<Selector>,
    
    /// Compiled regex patterns for fallback extraction
    regex_patterns: HashMap<String, Regex>,
}

impl ProductDetailParser {
    /// Create a new product detail parser with default configuration
    pub fn new() -> Result<Self> {
        let config = super::config::ParsingConfig::default();
        Self::with_config(&config.product_detail_selectors)
    }
    
    /// Create parser with custom selector configuration
    pub fn with_config(selectors: &super::config::ProductDetailSelectors) -> Result<Self> {
        let mut regex_patterns = HashMap::new();
        
        // Compile regex patterns for fallback extraction
        Self::compile_regex_patterns(&mut regex_patterns)?;
        
        Ok(Self {
            title_selectors: Self::compile_selectors(&selectors.title)?,
            manufacturer_selectors: Self::compile_selectors(&selectors.manufacturer)?,
            model_selectors: Self::compile_selectors(&selectors.model)?,
            category_selectors: Self::compile_selectors(&selectors.category)?,
            description_selectors: Self::compile_selectors(&selectors.description)?,
            
            vid_selectors: Self::compile_selectors(&selectors.vid)?,
            pid_selectors: Self::compile_selectors(&selectors.pid)?,
            certification_type_selectors: Self::compile_selectors(&selectors.certification_type)?,
            certification_date_selectors: Self::compile_selectors(&selectors.certification_date)?,
            specification_version_selectors: Self::compile_selectors(&selectors.specification_version)?,
            transport_interface_selectors: Self::compile_selectors(&selectors.transport_interface)?,
            
            info_table_selectors: Self::compile_selectors(&selectors.info_table)?,
            definition_list_selectors: Self::compile_selectors(&selectors.definition_list)?,
            
            regex_patterns,
        })
    }
    
    /// Compile selector strings into Selector objects
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
        
        if selectors.is_empty() && !selector_strings.is_empty() {
            return Err(anyhow::anyhow!(
                "No valid selectors compiled from {} attempts. Errors: {}", 
                selector_strings.len(),
                errors.join(", ")
            ));
        }
        
        Ok(selectors)
    }
    
    /// Compile regex patterns for fallback data extraction
    fn compile_regex_patterns(patterns: &mut HashMap<String, Regex>) -> Result<()> {
        let pattern_definitions = vec![
            ("vid", r"(?i)(?:vid|vendor\s*id)[:\s]*([0-9a-fx]+)"),
            ("pid", r"(?i)(?:pid|product\s*id)[:\s]*([0-9a-fx]+)"),
            ("certification_id", r"([A-Z0-9\-]{6,})"),
            ("version", r"([0-9]+\.[0-9]+(?:\.[0-9]+)?)"),
            ("hex_number", r"0x([0-9a-fA-F]+)"),
            ("decimal_number", r"\b([0-9]+)\b"),
            ("date", r"(\d{4}[-/]\d{2}[-/]\d{2})|\b(\d{1,2}[-/]\d{1,2}[-/]\d{4})\b"),
        ];
        
        for (name, pattern_str) in pattern_definitions {
            match Regex::new(pattern_str) {
                Ok(regex) => {
                    patterns.insert(name.to_string(), regex);
                }
                Err(e) => {
                    warn!("Failed to compile regex pattern '{}': {}", name, e);
                }
            }
        }
        
        Ok(())
    }
}

impl ContextualParser for ProductDetailParser {
    type Output = ProductDetail;
    type Context = DetailParseContext;
    
    /// Parse detailed product information with comprehensive extraction strategies
    fn parse_with_context(&self, html: &Html, context: &Self::Context) -> ParsingResult<Self::Output> {
        debug!("Parsing product detail from: {}", context.url);
        
        // Extract basic product information with fallbacks
        let model = self.extract_basic_info(html, "title", &self.title_selectors)
            .or_else(|| self.extract_basic_info(html, "model", &self.model_selectors))
            .ok_or_else(|| ParsingError::required_field_missing("model", Some("product detail page")))?;
        
        let manufacturer = self.extract_basic_info(html, "manufacturer", &self.manufacturer_selectors);
        let device_type = self.extract_basic_info(html, "category", &self.category_selectors);
        
        // Extract Matter-specific certification data using multiple strategies
        let certification_data = self.extract_matter_certification_data(html)?;
        
        // Extract additional fields
        let description = self.extract_product_description(html);
        
        let now = chrono::Utc::now();
        
        let product_detail = ProductDetail {
            url: context.url.clone(),
            page_id: context.source_page_id.map(|p| p as i32),
            index_in_page: context.source_index.map(|i| i as i32),
            id: certification_data.get("certification_id").cloned(),
            manufacturer,
            model: Some(model),
            device_type,
            certification_id: certification_data.get("certification_id").cloned(),
            certification_date: certification_data.get("certification_date").cloned(),
            software_version: certification_data.get("software_version").cloned(),
            hardware_version: certification_data.get("hardware_version").cloned(),
            vid: self.parse_numeric_field(certification_data.get("vid")),
            pid: self.parse_numeric_field(certification_data.get("pid")),
            family_sku: None,
            family_variant_sku: None,
            firmware_version: None,
            family_id: None,
            tis_trp_tested: None,
            specification_version: certification_data.get("specification_version").cloned(),
            transport_interface: certification_data.get("transport_interface").cloned(),
            primary_device_type_id: None,
            application_categories: None,
            description,
            compliance_document_url: None,
            program_type: certification_data.get("certification_type").cloned(),
            created_at: now,
            updated_at: now,
        };
        
        debug!("Extracted Matter product details for: {}", product_detail.model.as_ref().unwrap_or(&"Unknown".to_string()));
        Ok(product_detail)
    }
}

impl ProductDetailParser {
    /// Extract basic product information with fallback selectors
    fn extract_basic_info(&self, html: &Html, field_name: &str, selectors: &[Selector]) -> Option<String> {
        for (i, selector) in selectors.iter().enumerate() {
            if let Some(element) = html.select(selector).next() {
                let text = element.text().collect::<String>().trim().to_string();
                if !text.is_empty() {
                    debug!("Extracted {} using selector {}: {}", field_name, i, text);
                    return Some(text);
                }
            }
        }
        
        debug!("Failed to extract {} using {} selectors", field_name, selectors.len());
        None
    }
    
    /// Extract Matter certification data using multiple strategies
    fn extract_matter_certification_data(&self, html: &Html) -> ParsingResult<HashMap<String, String>> {
        let mut certification_data = HashMap::new();
        
        // Strategy 1: Extract from structured tables (most reliable)
        if let Some(table_data) = self.extract_from_tables(html) {
            debug!("Extracted {} fields from tables", table_data.len());
            certification_data.extend(table_data);
        }
        
        // Strategy 2: Extract from definition lists
        if let Some(dl_data) = self.extract_from_definition_lists(html) {
            debug!("Extracted {} fields from definition lists", dl_data.len());
            certification_data.extend(dl_data);
        }
        
        // Strategy 3: Extract from labeled paragraphs/divs
        if let Some(labeled_data) = self.extract_from_labeled_elements(html) {
            debug!("Extracted {} fields from labeled elements", labeled_data.len());
            certification_data.extend(labeled_data);
        }
        
        // Strategy 4: Extract using regex patterns (fallback)
        let html_text = html.html();
        if let Some(regex_data) = self.extract_using_regex(&html_text) {
            debug!("Extracted {} fields using regex patterns", regex_data.len());
            certification_data.extend(regex_data);
        }
        
        if certification_data.is_empty() {
            warn!("No Matter certification data found in product detail page");
        } else {
            debug!("Total extracted fields: {}", certification_data.len());
        }
        
        Ok(certification_data)
    }
    
    /// Extract data from HTML tables - Most reliable method for Matter data
    fn extract_from_tables(&self, html: &Html) -> Option<HashMap<String, String>> {
        let mut data = HashMap::new();
        
        for table in self.select_from_multiple(&self.info_table_selectors, html) {
            let row_selector = Selector::parse("tr").ok()?;
            
            for row in table.select(&row_selector) {
                let cell_selector = Selector::parse("td, th").ok()?;
                let cells: Vec<_> = row.select(&cell_selector).collect();
                
                if cells.len() >= 2 {
                    let key = cells[0].text().collect::<String>().trim().to_lowercase();
                    let value = cells[1].text().collect::<String>().trim().to_string();
                    
                    if !value.is_empty() && value != "-" && value != "N/A" && value != "TBD" {
                        self.classify_and_store_field(&key, &value, &mut data);
                    }
                }
            }
        }
        
        if data.is_empty() { None } else { Some(data) }
    }
    
    /// Extract data from definition lists
    fn extract_from_definition_lists(&self, html: &Html) -> Option<HashMap<String, String>> {
        let mut data = HashMap::new();
        
        for dl in self.select_from_multiple(&self.definition_list_selectors, html) {
            let dt_selector = Selector::parse("dt").ok()?;
            let dd_selector = Selector::parse("dd").ok()?;
            
            let terms: Vec<_> = dl.select(&dt_selector).collect();
            let definitions: Vec<_> = dl.select(&dd_selector).collect();
            
            for (term, definition) in terms.iter().zip(definitions.iter()) {
                let key = term.text().collect::<String>().trim().to_lowercase();
                let value = definition.text().collect::<String>().trim().to_string();
                
                if !value.is_empty() && value != "-" && value != "N/A" {
                    self.classify_and_store_field(&key, &value, &mut data);
                }
            }
        }
        
        if data.is_empty() { None } else { Some(data) }
    }
    
    /// Extract data from labeled paragraphs and divs
    fn extract_from_labeled_elements(&self, html: &Html) -> Option<HashMap<String, String>> {
        let mut data = HashMap::new();
        
        // Look for patterns like "Label: Value" in text content
        let text_content = html.root_element().text().collect::<String>();
        
        for line in text_content.lines() {
            if let Some(colon_pos) = line.find(':') {
                let key = line[..colon_pos].trim().to_lowercase();
                let value = line[colon_pos + 1..].trim().to_string();
                
                if !value.is_empty() && value.len() > 1 {
                    self.classify_and_store_field(&key, &value, &mut data);
                }
            }
        }
        
        if data.is_empty() { None } else { Some(data) }
    }
    
    /// Extract data using regex patterns as ultimate fallback
    fn extract_using_regex(&self, html: &str) -> Option<HashMap<String, String>> {
        let mut data = HashMap::new();
        
        // Extract VID/PID patterns
        if let Some(vid_regex) = self.regex_patterns.get("vid") {
            if let Some(captures) = vid_regex.captures(html) {
                if let Some(value) = captures.get(1) {
                    data.insert("vid".to_string(), value.as_str().to_string());
                }
            }
        }
        
        if let Some(pid_regex) = self.regex_patterns.get("pid") {
            if let Some(captures) = pid_regex.captures(html) {
                if let Some(value) = captures.get(1) {
                    data.insert("pid".to_string(), value.as_str().to_string());
                }
            }
        }
        
        // Extract certification ID patterns
        if let Some(cert_regex) = self.regex_patterns.get("certification_id") {
            for captures in cert_regex.captures_iter(html) {
                if let Some(value) = captures.get(1) {
                    let cert_id = value.as_str().to_string();
                    if cert_id.len() >= 6 {  // Minimum reasonable length
                        data.insert("certification_id".to_string(), cert_id);
                        break; // Take the first reasonable match
                    }
                }
            }
        }
        
        // Extract version patterns
        if let Some(version_regex) = self.regex_patterns.get("version") {
            if let Some(captures) = version_regex.captures(html) {
                if let Some(value) = captures.get(1) {
                    data.insert("specification_version".to_string(), value.as_str().to_string());
                }
            }
        }
        
        // Extract date patterns
        if let Some(date_regex) = self.regex_patterns.get("date") {
            if let Some(captures) = date_regex.captures(html) {
                if let Some(value) = captures.get(1).or_else(|| captures.get(2)) {
                    data.insert("certification_date".to_string(), value.as_str().to_string());
                }
            }
        }
        
        if data.is_empty() { None } else { Some(data) }
    }
    
    /// Classify a field and store it in the appropriate category
    fn classify_and_store_field(&self, key: &str, value: &str, data: &mut HashMap<String, String>) {
        if key.contains("vid") || key.contains("vendor id") {
            data.insert("vid".to_string(), value.to_string());
        } else if key.contains("pid") || key.contains("product id") {
            data.insert("pid".to_string(), value.to_string());
        } else if key.contains("certification") && !key.contains("date") {
            data.insert("certification_type".to_string(), value.to_string());
        } else if key.contains("certification") && key.contains("id") {
            data.insert("certification_id".to_string(), value.to_string());
        } else if (key.contains("date") || key.contains("certified")) && key.contains("cert") {
            data.insert("certification_date".to_string(), value.to_string());
        } else if key.contains("specification") || key.contains("spec") {
            data.insert("specification_version".to_string(), value.to_string());
        } else if key.contains("transport") || key.contains("interface") || key.contains("connectivity") {
            data.insert("transport_interface".to_string(), value.to_string());
        } else if key.contains("software") && key.contains("version") {
            data.insert("software_version".to_string(), value.to_string());
        } else if key.contains("hardware") && key.contains("version") {
            data.insert("hardware_version".to_string(), value.to_string());
        }
    }
    
    /// Select elements using multiple selectors, returning the first that matches
    fn select_from_multiple<'a>(&self, selectors: &[Selector], html: &'a Html) -> Vec<ElementRef<'a>> {
        for selector in selectors {
            let elements: Vec<_> = html.select(selector).collect();
            if !elements.is_empty() {
                return elements;
            }
        }
        Vec::new()
    }
    
    /// Parse numeric field (VID/PID) from string
    fn parse_numeric_field(&self, value: Option<&String>) -> Option<i32> {
        value.and_then(|text| {
            // Try hex format first
            if text.starts_with("0x") || text.starts_with("0X") {
                i32::from_str_radix(&text[2..], 16).ok()
            } else if text.chars().all(|c| c.is_ascii_hexdigit()) && text.len() <= 8 {
                // Try as hex without prefix
                i32::from_str_radix(text, 16).ok()
            } else {
                // Try as decimal
                text.parse::<i32>().ok()
            }
        })
    }
    
    /// Extract product description with comprehensive fallbacks
    fn extract_product_description(&self, html: &Html) -> Option<String> {
        for selector in &self.description_selectors {
            if let Some(element) = html.select(selector).next() {
                let text = element.text().collect::<String>().trim().to_string();
                if !text.is_empty() && text.len() > 10 {  // Filter out very short descriptions
                    return Some(text);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parser_creation() {
        let parser = ProductDetailParser::new();
        assert!(parser.is_ok());
    }
    
    #[test]
    fn test_numeric_field_parsing() {
        let parser = ProductDetailParser::new().unwrap();
        
        assert_eq!(parser.parse_numeric_field(Some(&"0x1234".to_string())), Some(0x1234));
        assert_eq!(parser.parse_numeric_field(Some(&"1234".to_string())), Some(1234));
        assert_eq!(parser.parse_numeric_field(Some(&"ABCD".to_string())), Some(0xABCD));
        assert_eq!(parser.parse_numeric_field(Some(&"invalid".to_string())), None);
    }
}
