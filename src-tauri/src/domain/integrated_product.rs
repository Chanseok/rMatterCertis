use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Product basic information from listing pages (products table)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub url: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub certificate_id: Option<String>,
    pub page_id: Option<i32>,
    pub index_in_page: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Detailed product specifications (product_details table)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductDetail {
    pub url: String,
    pub page_id: Option<i32>,
    pub index_in_page: Option<i32>,
    
    // Core identification
    pub id: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub device_type: Option<String>,
    pub certificate_id: Option<String>,
    
    // Certification details
    pub certification_date: Option<String>,
    pub software_version: Option<String>,
    pub hardware_version: Option<String>,
    pub firmware_version: Option<String>,
    pub specification_version: Option<String>,
    
    // Technical identifiers (optimized as INTEGER)
    pub vid: Option<i32>,
    pub pid: Option<i32>,
    
    // Product family information
    pub family_sku: Option<String>,
    pub family_variant_sku: Option<String>,
    pub family_id: Option<String>,
    
    // Testing and compliance
    pub tis_trp_tested: Option<String>, // "Yes"/"No"
    pub transport_interface: Option<String>,
    pub primary_device_type_id: Option<String>,
    pub application_categories: Option<String>, // JSON array
    
    // Enhanced fields
    pub description: Option<String>,
    pub compliance_document_url: Option<String>,
    pub program_type: Option<String>, // Default: "Matter"
    
    // Audit fields
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Combined product with details for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductWithDetails {
    pub product: Product,
    pub details: Option<ProductDetail>,
}

/// Enhanced vendor with audit fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vendor {
    pub vendor_id: i32,
    pub vendor_number: Option<i32>,
    pub vendor_name: String,
    pub company_legal_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Crawling session results (persistent storage)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingResult {
    pub session_id: String,
    pub status: String, // 'Completed', 'Failed', 'Stopped'
    pub stage: String,  // 'ProductList', 'ProductDetails', 'Completed'
    pub total_pages: i32,
    pub products_found: i32,
    pub details_fetched: i32,
    pub errors_count: i32,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub execution_time_seconds: Option<i32>,
    pub config_snapshot: Option<String>, // JSON
    pub error_details: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Search and filter criteria
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProductSearchCriteria {
    pub manufacturer: Option<String>,
    pub device_type: Option<String>,
    pub certificate_id: Option<String>,
    pub specification_version: Option<String>,
    pub program_type: Option<String>,
    pub certification_date_from: Option<String>,
    pub certification_date_to: Option<String>,
    pub page: Option<i32>,
    pub limit: Option<i32>,
}

/// Search results with pagination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductSearchResult {
    pub products: Vec<ProductWithDetails>,
    pub total_count: i32,
    pub page: i32,
    pub limit: i32,
    pub total_pages: i32,
}

/// Database statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStatistics {
    pub total_products: i32,
    pub total_details: i32,
    pub unique_manufacturers: i32,
    pub unique_device_types: i32,
    pub latest_crawl_date: Option<String>,
    pub matter_products_count: i32,
    pub completion_rate: f32, // percentage of products with details
}

impl Product {
    /// Create a new Product with current timestamps
    pub fn new(
        url: String,
        manufacturer: Option<String>,
        model: Option<String>,
        certificate_id: Option<String>,
        page_id: Option<i32>,
        index_in_page: Option<i32>,
    ) -> Self {
        let now = Utc::now();
        Self {
            url,
            manufacturer,
            model,
            certificate_id,
            page_id,
            index_in_page,
            created_at: now,
            updated_at: now,
        }
    }
}

impl ProductDetail {
    /// Create a new ProductDetail with current timestamps
    pub fn new(url: String) -> Self {
        let now = Utc::now();
        Self {
            url,
            page_id: None,
            index_in_page: None,
            id: None,
            manufacturer: None,
            model: None,
            device_type: None,
            certificate_id: None,
            certification_date: None,
            software_version: None,
            hardware_version: None,
            firmware_version: None,
            specification_version: None,
            vid: None,
            pid: None,
            family_sku: None,
            family_variant_sku: None,
            family_id: None,
            tis_trp_tested: None,
            transport_interface: None,
            primary_device_type_id: None,
            application_categories: None,
            description: None,
            compliance_document_url: None,
            program_type: Some("Matter".to_string()),
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if this is a Matter product
    pub fn is_matter_product(&self) -> bool {
        self.program_type.as_ref().map_or(true, |pt| pt == "Matter")
    }

    /// Parse hexadecimal vendor/product IDs from string
    pub fn parse_hex_id(hex_str: &str) -> Option<i32> {
        let cleaned = hex_str.trim_start_matches("0x").trim_start_matches("0X");
        i32::from_str_radix(cleaned, 16).ok()
    }

    /// Format integer ID as hexadecimal string
    pub fn format_hex_id(id: i32) -> String {
        format!("0x{:X}", id)
    }
}

impl CrawlingResult {
    /// Create a new crawling result
    pub fn new(session_id: String, config_snapshot: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            session_id,
            status: "Running".to_string(),
            stage: "Starting".to_string(),
            total_pages: 0,
            products_found: 0,
            details_fetched: 0,
            errors_count: 0,
            started_at: now,
            completed_at: None,
            execution_time_seconds: None,
            config_snapshot,
            error_details: None,
            created_at: now,
        }
    }

    /// Mark the session as completed
    pub fn complete(&mut self) {
        self.status = "Completed".to_string();
        self.stage = "Completed".to_string();
        self.completed_at = Some(Utc::now());
        self.execution_time_seconds = Some(
            (self.completed_at.unwrap() - self.started_at).num_seconds() as i32
        );
    }

    /// Mark the session as failed
    pub fn fail(&mut self, error: String) {
        self.status = "Failed".to_string();
        self.completed_at = Some(Utc::now());
        self.execution_time_seconds = Some(
            (self.completed_at.unwrap() - self.started_at).num_seconds() as i32
        );
        self.error_details = Some(error);
        self.errors_count += 1;
    }
}
