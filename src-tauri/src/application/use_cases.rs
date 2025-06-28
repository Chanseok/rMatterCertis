//! Application use cases for Matter Certification crawling
//! 
//! Contains the application's use cases and business workflows specific to Matter domain.

use anyhow::Result;
use std::sync::Arc;
use std::collections::HashSet;
use uuid::Uuid;
use chrono::Utc;

use crate::domain::entities::{
    Vendor, Product, MatterProduct, CrawlingSession, CrawlingStatus, CrawlingStage,
    CrawlerConfig, ValidationResult, ValidationSummary, DatabaseSummary
};
use crate::domain::repositories::{VendorRepository, ProductRepository, CrawlingSessionRepository};

/// Use cases for starting and managing crawling operations
pub struct CrawlingUseCases {
    product_repo: Arc<dyn ProductRepository>,
    vendor_repo: Arc<dyn VendorRepository>,
    session_repo: Arc<dyn CrawlingSessionRepository>,
}

impl CrawlingUseCases {
    pub fn new(
        product_repo: Arc<dyn ProductRepository>,
        vendor_repo: Arc<dyn VendorRepository>,
        session_repo: Arc<dyn CrawlingSessionRepository>,
    ) -> Self {
        Self {
            product_repo,
            vendor_repo,
            session_repo,
        }
    }

    /// Start a new crawling session
    pub async fn start_crawling(&self, config: CrawlerConfig) -> Result<CrawlingSession> {
        let session = CrawlingSession {
            id: Uuid::new_v4().to_string(),
            status: CrawlingStatus::Initializing,
            current_stage: CrawlingStage::ProductList,
            total_pages: Some(config.page_range_limit),
            processed_pages: 0,
            products_found: 0,
            errors_count: 0,
            started_at: Utc::now(),
            completed_at: None,
            config_snapshot: serde_json::to_string(&config)?,
        };

        self.session_repo.create(&session).await?;
        Ok(session)
    }

    /// Update crawling session status
    pub async fn update_session_status(
        &self,
        session_id: &str,
        status: CrawlingStatus,
        stage: CrawlingStage,
        processed_pages: u32,
        products_found: u32,
        errors_count: u32,
    ) -> Result<()> {
        if let Some(mut session) = self.session_repo.find_by_id(session_id).await? {
            session.status = status.clone();
            session.current_stage = stage;
            session.processed_pages = processed_pages;
            session.products_found = products_found;
            session.errors_count = errors_count;

            if matches!(status, CrawlingStatus::Completed | CrawlingStatus::Error | CrawlingStatus::Stopped) {
                session.completed_at = Some(Utc::now());
            }

            self.session_repo.update(&session).await?;
        }

        Ok(())
    }

    /// Validate products against existing database (Stage 1.5)
    pub async fn validate_products(&self, products: Vec<Product>) -> Result<ValidationResult> {
        let start_time = std::time::Instant::now();
        
        // Get existing URLs from database
        let existing_urls = self.product_repo.get_existing_urls().await?;
        
        // Classify products
        let mut new_products = Vec::new();
        let mut existing_products = Vec::new();
        let mut duplicate_products = Vec::new();
        let mut seen_urls = HashSet::new();
        
        for product in products {
            // Check for duplicates within the current collection
            if seen_urls.contains(&product.url) {
                duplicate_products.push(product);
                continue;
            }
            seen_urls.insert(product.url.clone());
            
            // Check against database
            if existing_urls.contains(&product.url) {
                existing_products.push(product);
            } else {
                new_products.push(product);
            }
        }
        
        let validation_time_ms = start_time.elapsed().as_millis() as u64;
        
        Ok(ValidationResult {
            summary: ValidationSummary {
                total_products: seen_urls.len(),
                new_products: new_products.len(),
                existing_products: existing_products.len(),
                duplicate_products: duplicate_products.len(),
                validation_time_ms,
            },
            new_products,
            existing_products,
            duplicate_products,
        })
    }

    /// Save products to database (Stage 1 results)
    pub async fn save_products(&self, products: &[Product]) -> Result<()> {
        self.product_repo.save_products_batch(products).await
    }

    /// Save Matter products to database (Stage 2 results)
    pub async fn save_matter_products(&self, products: &[MatterProduct]) -> Result<()> {
        self.product_repo.save_matter_products_batch(products).await
    }

    /// Get recent crawling sessions
    pub async fn get_recent_sessions(&self, limit: u32) -> Result<Vec<CrawlingSession>> {
        self.session_repo.find_recent(limit).await
    }

    /// Clean up old completed sessions
    pub async fn cleanup_old_sessions(&self, days: u32) -> Result<u32> {
        self.session_repo.cleanup_old_sessions(days).await
    }
}

/// Use cases for product management and querying
pub struct ProductUseCases {
    product_repo: Arc<dyn ProductRepository>,
}

impl ProductUseCases {
    pub fn new(product_repo: Arc<dyn ProductRepository>) -> Self {
        Self { product_repo }
    }

    /// Get paginated products
    pub async fn get_products(&self, page: u32, limit: u32) -> Result<(Vec<Product>, u32)> {
        self.product_repo.get_products_paginated(page, limit).await
    }

    /// Get paginated Matter products with full details
    pub async fn get_matter_products(&self, page: u32, limit: u32) -> Result<(Vec<MatterProduct>, u32)> {
        self.product_repo.get_matter_products_paginated(page, limit).await
    }

    /// Search products by text query
    pub async fn search_products(&self, query: &str) -> Result<Vec<MatterProduct>> {
        self.product_repo.search_products(query).await
    }

    /// Find products by manufacturer
    pub async fn find_by_manufacturer(&self, manufacturer: &str) -> Result<Vec<MatterProduct>> {
        self.product_repo.find_by_manufacturer(manufacturer).await
    }

    /// Find products by device type
    pub async fn find_by_device_type(&self, device_type: &str) -> Result<Vec<MatterProduct>> {
        self.product_repo.find_by_device_type(device_type).await
    }

    /// Find products by Vendor ID
    pub async fn find_by_vid(&self, vid: &str) -> Result<Vec<MatterProduct>> {
        self.product_repo.find_by_vid(vid).await
    }

    /// Get database summary statistics
    pub async fn get_database_summary(&self) -> Result<DatabaseSummary> {
        self.product_repo.get_database_summary().await
    }

    /// Delete a product by URL
    pub async fn delete_product(&self, url: &str) -> Result<()> {
        self.product_repo.delete_product(url).await
    }
}

/// Use cases for vendor management
pub struct VendorUseCases {
    vendor_repo: Arc<dyn VendorRepository>,
}

impl VendorUseCases {
    pub fn new(vendor_repo: Arc<dyn VendorRepository>) -> Self {
        Self { vendor_repo }
    }

    /// Create a new vendor
    pub async fn create_vendor(&self, vendor: Vendor) -> Result<()> {
        self.vendor_repo.create(&vendor).await
    }

    /// Get all vendors
    pub async fn get_all_vendors(&self) -> Result<Vec<Vendor>> {
        self.vendor_repo.find_all().await
    }

    /// Search vendors by name
    pub async fn search_vendors(&self, name: &str) -> Result<Vec<Vendor>> {
        self.vendor_repo.search_by_name(name).await
    }

    /// Find vendor by ID
    pub async fn find_vendor_by_id(&self, vendor_id: &str) -> Result<Option<Vendor>> {
        self.vendor_repo.find_by_id(vendor_id).await
    }

    /// Find vendor by number
    pub async fn find_vendor_by_number(&self, vendor_number: u32) -> Result<Option<Vendor>> {
        self.vendor_repo.find_by_number(vendor_number).await
    }

    /// Update vendor information
    pub async fn update_vendor(&self, vendor: Vendor) -> Result<()> {
        self.vendor_repo.update(&vendor).await
    }

    /// Delete vendor
    pub async fn delete_vendor(&self, vendor_id: &str) -> Result<()> {
        self.vendor_repo.delete(vendor_id).await
    }
}
   

pub struct ProductUseCases {
    product_repo: Arc<dyn ProductRepository>,
}

impl ProductUseCases {
    pub fn new(product_repo: Arc<dyn ProductRepository>) -> Self {
        Self { product_repo }
    }

    pub async fn create_product(&self, product: Product) -> Result<()> {
        self.product_repo.create(&product).await
    }

    pub async fn get_product(&self, id: &str) -> Result<Option<Product>> {
        self.product_repo.find_by_id(id).await
    }

    pub async fn list_products(&self) -> Result<Vec<Product>> {
        self.product_repo.find_all().await
    }

    pub async fn list_products_by_vendor(&self, vendor_id: &str) -> Result<Vec<Product>> {
        self.product_repo.find_by_vendor(vendor_id).await
    }

    pub async fn update_product(&self, product: Product) -> Result<()> {
        self.product_repo.update(&product).await
    }

    pub async fn delete_product(&self, id: &str) -> Result<()> {
        self.product_repo.delete(id).await
    }
}

pub struct CrawlingSessionUseCases {
    session_repo: Arc<dyn CrawlingSessionRepository>,
}

impl CrawlingSessionUseCases {
    pub fn new(session_repo: Arc<dyn CrawlingSessionRepository>) -> Self {
        Self { session_repo }
    }

    pub async fn start_crawling_session(&self, vendor_id: String) -> Result<CrawlingSession> {
        let session = CrawlingSession {
            id: Uuid::new_v4().to_string(),
            vendor_id,
            status: CrawlingStatus::Pending,
            total_pages: None,
            processed_pages: 0,
            products_found: 0,
            errors_count: 0,
            started_at: Utc::now(),
            completed_at: None,
        };

        self.session_repo.create(&session).await?;
        Ok(session)
    }

    pub async fn get_session(&self, id: &str) -> Result<Option<CrawlingSession>> {
        self.session_repo.find_by_id(id).await
    }

    pub async fn list_sessions_by_vendor(&self, vendor_id: &str) -> Result<Vec<CrawlingSession>> {
        self.session_repo.find_by_vendor(vendor_id).await
    }

    pub async fn list_active_sessions(&self) -> Result<Vec<CrawlingSession>> {
        self.session_repo.find_active().await
    }

    pub async fn update_session(&self, session: CrawlingSession) -> Result<()> {
        self.session_repo.update(&session).await
    }
}
