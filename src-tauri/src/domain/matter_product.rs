use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Matter product information extracted from CSA-IoT website
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MatterProduct {
    pub id: Option<i64>,

    // Basic Information
    pub certificate_id: String,
    pub company_name: String,
    pub product_name: String,
    pub description: Option<String>,

    // Technical Specifications
    pub firmware_version: Option<String>,
    pub hardware_version: Option<String>,
    pub specification_version: Option<String>,
    pub product_id: Option<String>,             // Hexadecimal ID
    pub vendor_id: Option<String>,              // Hexadecimal ID
    pub primary_device_type_id: Option<String>, // Hexadecimal ID
    pub transport_interface: Option<String>,    // Comma-separated

    // Certification Details
    pub certified_date: Option<DateTime<Utc>>,
    pub tis_trp_tested: Option<bool>,
    pub compliance_document_url: Option<String>,

    // Program Classification
    pub program_type: String, // Default: "Matter"
    pub device_type: Option<String>,

    // URLs and Metadata
    pub detail_url: String,
    pub listing_url: Option<String>,
    pub page_number: Option<i32>,
    pub position_in_page: Option<i32>,

    // Timestamps
    pub crawled_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl MatterProduct {
    /// Create a new MatterProduct with basic information
    pub fn new(
        certificate_id: String,
        company_name: String,
        product_name: String,
        detail_url: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: None,
            certificate_id,
            company_name,
            product_name,
            description: None,
            firmware_version: None,
            hardware_version: None,
            specification_version: None,
            product_id: None,
            vendor_id: None,
            primary_device_type_id: None,
            transport_interface: None,
            certified_date: None,
            tis_trp_tested: None,
            compliance_document_url: None,
            program_type: "Matter".to_string(),
            device_type: None,
            detail_url,
            listing_url: None,
            page_number: None,
            position_in_page: None,
            crawled_at: now,
            updated_at: now,
        }
    }

    /// Set page metadata (page number and position)
    pub fn with_page_metadata(mut self, page_number: i32, position: i32) -> Self {
        self.page_number = Some(page_number);
        self.position_in_page = Some(position);
        self
    }

    /// Set listing URL where this product was found
    pub fn with_listing_url(mut self, listing_url: String) -> Self {
        self.listing_url = Some(listing_url);
        self
    }

    /// Update technical specifications from detail page
    #[allow(clippy::too_many_arguments)]
    pub fn update_technical_specs(
        &mut self,
        firmware_version: Option<String>,
        hardware_version: Option<String>,
        specification_version: Option<String>,
        product_id: Option<String>,
        vendor_id: Option<String>,
        primary_device_type_id: Option<String>,
        transport_interface: Option<String>,
    ) {
        self.firmware_version = firmware_version;
        self.hardware_version = hardware_version;
        self.specification_version = specification_version;
        self.product_id = product_id;
        self.vendor_id = vendor_id;
        self.primary_device_type_id = primary_device_type_id;
        self.transport_interface = transport_interface;
        self.updated_at = Utc::now();
    }

    /// Update certification details from detail page
    pub fn update_certification_details(
        &mut self,
        certified_date: Option<DateTime<Utc>>,
        tis_trp_tested: Option<bool>,
        compliance_document_url: Option<String>,
    ) {
        self.certified_date = certified_date;
        self.tis_trp_tested = tis_trp_tested;
        self.compliance_document_url = compliance_document_url;
        self.updated_at = Utc::now();
    }
}

/// Configuration for Matter products crawler
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatterCrawlerConfig {
    // Base URLs
    pub base_url: String,
    pub matter_filter_url: String,

    // Rate limiting and performance
    pub rate_limit_ms: u64,
    pub max_concurrent_requests: usize,
    pub request_timeout_seconds: u64,

    // Retry configuration
    pub max_retries: u32,
    pub retry_delay_ms: u64,

    // Crawling limits
    pub max_pages: Option<u32>,
    pub start_page: u32,

    // User agent
    pub user_agent: Option<String>,
}

impl Default for MatterCrawlerConfig {
    fn default() -> Self {
        use crate::infrastructure::config::csa_iot;
        Self {
            base_url: csa_iot::PRODUCTS_PAGE_GENERAL.to_string(),
            matter_filter_url: csa_iot::PRODUCTS_PAGE_MATTER_ONLY.to_string(),
            rate_limit_ms: 1000,        // 1 second between requests
            max_concurrent_requests: 3, // Conservative default
            request_timeout_seconds: 30,
            max_retries: 3,
            retry_delay_ms: 2000,
            max_pages: Some(10), // Start with 10 pages for testing
            start_page: 1,
            user_agent: Some("MatterCertis/1.0 (Research Tool)".to_string()),
        }
    }
}

/// Crawling session state for Matter products
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatterCrawlingSession {
    pub session_id: String,
    pub config: MatterCrawlerConfig,
    pub start_time: DateTime<Utc>,
    pub stage: CrawlingStage,
    pub current_page: u32,
    pub total_pages_to_crawl: Option<u32>,
    pub products_found: u32,
    pub products_detailed: u32,
    pub errors: Vec<String>,
    pub current_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrawlingStage {
    ProductList,
    ProductDetails,
    Completed,
    Failed,
}

impl MatterCrawlingSession {
    pub fn new(session_id: String, config: MatterCrawlerConfig) -> Self {
        Self {
            session_id,
            config,
            start_time: Utc::now(),
            stage: CrawlingStage::ProductList,
            current_page: 1,
            total_pages_to_crawl: None,
            products_found: 0,
            products_detailed: 0,
            errors: Vec::new(),
            current_url: None,
        }
    }

    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
    }

    pub fn update_progress(&mut self, current_page: u32, products_found: u32) {
        self.current_page = current_page;
        self.products_found = products_found;
    }

    pub fn move_to_details_stage(&mut self) {
        self.stage = CrawlingStage::ProductDetails;
    }

    pub fn complete(&mut self) {
        self.stage = CrawlingStage::Completed;
    }

    pub fn fail(&mut self, error: String) {
        self.stage = CrawlingStage::Failed;
        self.add_error(error);
    }
}
