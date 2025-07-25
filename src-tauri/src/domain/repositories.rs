//! Repository interfaces for Matter Certification crawling
//! 
//! Contains trait definitions for data access patterns specific to Matter domain.

use async_trait::async_trait;
use anyhow::Result;
use std::collections::HashSet;
use crate::domain::entities::{
    Vendor, Product, MatterProduct, 
    DatabaseSummary
};

#[async_trait]
pub trait VendorRepository: Send + Sync {
    async fn create(&self, vendor: &Vendor) -> Result<()>;
    async fn save(&self, vendor: &Vendor) -> Result<Vendor>;
    async fn find_by_id(&self, vendor_id: &str) -> Result<Option<Vendor>>;
    async fn find_by_number(&self, vendor_number: u32) -> Result<Option<Vendor>>;
    async fn find_all(&self) -> Result<Vec<Vendor>>;
    async fn find_all_paginated(&self, page: u32, limit: u32) -> Result<(Vec<Vendor>, u32)>;
    async fn search_by_name(&self, name: &str) -> Result<Vec<Vendor>>;
    async fn update(&self, vendor: &Vendor) -> Result<()>;
    async fn delete(&self, vendor_id: &str) -> Result<()>;
}

#[async_trait]
pub trait ProductRepository: Send + Sync {
    // Basic product operations (Stage 1 collection)
    async fn save_product(&self, product: &Product) -> Result<()>;
    async fn save_products_batch(&self, products: &[Product]) -> Result<()>;
    async fn find_product_by_url(&self, url: &str) -> Result<Option<Product>>;
    async fn get_existing_urls(&self) -> Result<HashSet<String>>;
    async fn get_products_paginated(&self, page: u32, limit: u32) -> Result<(Vec<Product>, u32)>;
    
    // Matter product operations (Stage 2 collection)
    async fn save_matter_product(&self, product: &MatterProduct) -> Result<()>;
    async fn save_matter_products_batch(&self, products: &[MatterProduct]) -> Result<()>;
    async fn find_matter_product_by_url(&self, url: &str) -> Result<Option<MatterProduct>>;
    async fn get_matter_products_paginated(&self, page: u32, limit: u32) -> Result<(Vec<MatterProduct>, u32)>;
    
    // Search and filtering
    async fn search_products(&self, query: &str) -> Result<Vec<MatterProduct>>;
    async fn find_by_manufacturer(&self, manufacturer: &str) -> Result<Vec<MatterProduct>>;
    async fn find_by_device_type(&self, device_type: &str) -> Result<Vec<MatterProduct>>;
    async fn find_by_vid(&self, vid: &str) -> Result<Vec<MatterProduct>>;
    async fn find_by_certification_date_range(&self, start: &str, end: &str) -> Result<Vec<MatterProduct>>;
    
    // Statistics and summary
    async fn get_database_summary(&self) -> Result<DatabaseSummary>;
    async fn count_products(&self) -> Result<u32>;
    async fn count_matter_products(&self) -> Result<u32>;
    
    // Cleanup operations
    async fn delete_product(&self, url: &str) -> Result<()>;
    async fn delete_matter_product(&self, url: &str) -> Result<()>;
    
    // Additional query methods
    async fn get_all_products(&self) -> Result<Vec<Product>>;
    async fn get_all_matter_products(&self) -> Result<Vec<MatterProduct>>;
    async fn get_recent_products(&self, limit: u32) -> Result<Vec<Product>>;
    async fn get_recent_matter_products(&self, limit: u32) -> Result<Vec<MatterProduct>>;
    
    // Advanced filtering and utilities
    async fn filter_matter_products(
        &self,
        manufacturer: Option<&str>,
        device_type: Option<&str>,
        vid: Option<&str>,
        certification_date_start: Option<&str>,
        certification_date_end: Option<&str>,
    ) -> Result<Vec<MatterProduct>>;
    async fn get_unique_manufacturers(&self) -> Result<Vec<String>>;
    async fn get_unique_device_types(&self) -> Result<Vec<String>>;
    
    // Database analysis methods
    async fn get_latest_updated_product(&self) -> Result<Option<Product>>;
}
