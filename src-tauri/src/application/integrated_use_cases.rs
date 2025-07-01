//! Integrated use cases combining products and product details
//!
//! This module provides high-level business operations that work with both
//! products and their details, implementing complex queries and operations
//! across multiple data sources.

#![allow(clippy::uninlined_format_args)]

use anyhow::{Result, anyhow};
use std::sync::Arc;
use chrono::Utc;

use crate::domain::product::{
    Product, ProductDetail, ProductWithDetails, ProductSearchCriteria, 
    ProductSearchResult, Vendor
};
use crate::domain::session_manager::CrawlingResult;
use crate::domain::integrated_product::DatabaseStatistics;
use crate::infrastructure::integrated_product_repository::IntegratedProductRepository;

/// Integrated use cases for the new unified schema
pub struct IntegratedProductUseCases {
    repo: Arc<IntegratedProductRepository>,
}

impl IntegratedProductUseCases {
    pub fn new(repo: Arc<IntegratedProductRepository>) -> Self {
        Self { repo }
    }

    // ===============================
    // PRODUCT OPERATIONS
    // ===============================

    /// Create or update basic product information (Stage 1 crawling)
    pub async fn create_or_update_product(&self, product: Product) -> Result<()> {
        // Validation
        if product.url.trim().is_empty() {
            return Err(anyhow!("Product URL cannot be empty"));
        }

        self.repo.create_or_update_product(&product).await
    }

    /// Create or update detailed product information (Stage 2 crawling)
    pub async fn create_or_update_product_detail(&self, detail: ProductDetail) -> Result<()> {
        // Validation
        if detail.url.trim().is_empty() {
            return Err(anyhow!("Product URL cannot be empty"));
        }

        // Ensure the product exists first
        if self.repo.get_product_by_url(&detail.url).await?.is_none() {
            return Err(anyhow!("Product must exist before adding details"));
        }

        self.repo.create_or_update_product_detail(&detail).await
    }

    /// Get products with pagination
    pub async fn get_products_paginated(&self, page: i32, limit: i32) -> Result<Vec<Product>> {
        // Validation
        if page < 1 {
            return Err(anyhow!("Page must be >= 1"));
        }
        if !(1..=100).contains(&limit) {
            return Err(anyhow!("Limit must be between 1 and 100"));
        }

        self.repo.get_products_paginated(page, limit).await
    }

    /// Get product with details by URL
    pub async fn get_product_with_details(&self, url: &str) -> Result<Option<ProductWithDetails>> {
        if url.trim().is_empty() {
            return Err(anyhow!("URL cannot be empty"));
        }

        self.repo.get_product_with_details(url).await
    }

    /// Search products with filters
    pub async fn search_products(&self, criteria: ProductSearchCriteria) -> Result<ProductSearchResult> {
        self.repo.search_products(&criteria).await
    }

    /// Get products without details (for crawling prioritization)
    pub async fn get_products_without_details(&self, limit: i32) -> Result<Vec<Product>> {
        if !(1..=1000).contains(&limit) {
            return Err(anyhow!("Limit must be between 1 and 1000"));
        }

        self.repo.get_products_without_details(limit).await
    }

    // ===============================
    // VENDOR OPERATIONS
    // ===============================

    /// Create a new vendor
    pub async fn create_vendor(&self, vendor_name: String, company_legal_name: Option<String>, _vendor_number: Option<i32>) -> Result<i32> {
        if vendor_name.trim().is_empty() {
            return Err(anyhow!("Vendor name cannot be empty"));
        }

        let _now = Utc::now();
        let vendor = Vendor {
            vendor_id: 0, // Will be auto-generated
            vendor_name: Some(vendor_name),
            company_legal_name,
            vendor_number: _vendor_number,
            created_at: _now,
            updated_at: _now,
        };

        self.repo.create_vendor(&vendor).await
    }

    /// Get all vendors
    pub async fn get_vendors(&self) -> Result<Vec<Vendor>> {
        self.repo.get_vendors().await
    }

    // ===============================
    // CRAWLING SESSION MANAGEMENT
    // ===============================

    /// Save crawling result
    pub async fn save_crawling_result(&self, result: CrawlingResult) -> Result<()> {
        if result.session_id.trim().is_empty() {
            return Err(anyhow!("Session ID cannot be empty"));
        }

        self.repo.save_crawling_result(&result).await
    }

    /// Get crawling results with pagination
    pub async fn get_crawling_results(&self, page: i32, limit: i32) -> Result<Vec<CrawlingResult>> {
        if page < 1 {
            return Err(anyhow!("Page must be >= 1"));
        }
        if !(1..=100).contains(&limit) {
            return Err(anyhow!("Limit must be between 1 and 100"));
        }

        self.repo.get_crawling_results(page, limit).await
    }

    // ===============================
    // ANALYTICS AND STATISTICS
    // ===============================

    /// Get comprehensive database statistics
    pub async fn get_database_statistics(&self) -> Result<DatabaseStatistics> {
        self.repo.get_database_statistics().await
    }

    // ===============================
    // DATA MIGRATION HELPERS
    // ===============================

    /// Convert hex string to integer for vid/pid fields
    pub fn convert_hex_to_int(hex_str: &str) -> Option<i32> {
        if hex_str.starts_with("0x") || hex_str.starts_with("0X") {
            i32::from_str_radix(&hex_str[2..], 16).ok()
        } else {
            hex_str.parse::<i32>().ok()
        }
    }

    /// Convert integer back to hex string for display
    pub fn convert_int_to_hex(value: i32) -> String {
        format!("0x{:X}", value)
    }

    /// Batch create products from crawling data
    pub async fn batch_create_products(&self, products: Vec<Product>) -> Result<usize> {
        let mut successful = 0;
        for product in products {
            match self.create_or_update_product(product).await {
                Ok(_) => successful += 1,
                Err(e) => {
                    eprintln!("Failed to create product: {}", e);
                }
            }
        }
        Ok(successful)
    }

    /// Batch create product details from crawling data
    pub async fn batch_create_product_details(&self, details: Vec<ProductDetail>) -> Result<usize> {
        let mut successful = 0;
        for detail in details {
            match self.create_or_update_product_detail(detail).await {
                Ok(_) => successful += 1,
                Err(e) => {
                    eprintln!("Failed to create product detail: {}", e);
                }
            }
        }
        Ok(successful)
    }

    // ===============================
    // VALIDATION AND HEALTH CHECKS
    // ===============================

    /// Validate database integrity
    pub async fn validate_database_integrity(&self) -> Result<DatabaseStatistics> {
        let stats = self.get_database_statistics().await?;
        
        // Log some useful information for debugging
        println!("Database Statistics:");
        println!("  Total Products: {}", stats.total_products);
        println!("  Total Details: {}", stats.total_details);
        println!("  Completion Rate: {:.1}%", stats.completion_rate);
        println!("  Unique Manufacturers: {}", stats.unique_manufacturers);
        println!("  Unique Device Types: {}", stats.unique_device_types);
        println!("  Matter Products: {}", stats.matter_products_count);
        
        if let Some(latest_crawl) = &stats.latest_crawl_date {
            println!("  Latest Crawl: {}", latest_crawl);
        }

        Ok(stats)
    }
}

/// Helper functions for data conversion and validation
impl IntegratedProductUseCases {
    /// Create product from basic crawling data
    pub fn create_product_from_crawl_data(
        url: String,
        manufacturer: Option<String>,
        model: Option<String>,
        certificate_id: Option<String>,
        page_id: Option<i32>,
        index_in_page: Option<i32>,
    ) -> Product {
        Product {
            url,
            manufacturer,
            model,
            certificate_id,
            page_id,
            index_in_page,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Create product detail from detailed crawling data
    pub fn create_product_detail_from_crawl_data(url: String) -> ProductDetail {
        ProductDetail {
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
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Validate product URL format
    pub fn validate_product_url(url: &str) -> Result<()> {
        if url.trim().is_empty() {
            return Err(anyhow!("URL cannot be empty"));
        }
        
        if !url.starts_with("http") {
            return Err(anyhow!("URL must start with http or https"));
        }

        // Additional URL validation can be added here
        Ok(())
    }

    /// Validate manufacturer name
    pub fn validate_manufacturer(manufacturer: &str) -> Result<()> {
        if manufacturer.trim().is_empty() {
            return Err(anyhow!("Manufacturer name cannot be empty"));
        }
        
        if manufacturer.len() > 255 {
            return Err(anyhow!("Manufacturer name too long (max 255 characters)"));
        }

        Ok(())
    }
}
