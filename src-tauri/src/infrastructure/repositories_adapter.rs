#![cfg(any())]
//! ARCHIVED: legacy repositories adapters (see `_archive/infrastructure/repositories_adapter.rs`)
// Not part of the active build.

//! Repository adapters for integrated schema compatibility
//! 
//! Provides adapters to make the integrated schema work with existing interfaces

// use async_trait::async_trait;
// use sqlx::SqlitePool;
// use anyhow::{Result, anyhow};
// use std::sync::Arc;
// use std::collections::HashSet;

// use crate::domain::entities::{Vendor, Product, MatterProduct, DatabaseSummary};
// use crate::domain::repositories::{VendorRepository, ProductRepository};
// use crate::infrastructure::integrated_product_repository::IntegratedProductRepository;

// ============================================================================
// Vendor Repository Adapter (Placeholder implementation)
// ============================================================================

pub struct SqliteVendorRepository {
    #[allow(dead_code)]
    integrated_repo: Arc<IntegratedProductRepository>,
}

impl SqliteVendorRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            integrated_repo: Arc::new(IntegratedProductRepository::new(pool)),
        }
    }
}

#[async_trait]
impl VendorRepository for SqliteVendorRepository {
    async fn create(&self, _vendor: &Vendor) -> Result<()> {
        Err(anyhow!("Not implemented yet"))
    }

    async fn save(&self, _vendor: &Vendor) -> Result<Vendor> {
        Err(anyhow!("Not implemented yet"))
    }

    async fn find_by_id(&self, _id: &str) -> Result<Option<Vendor>> {
        Ok(None)
    }

    async fn find_by_number(&self, _vendor_number: u32) -> Result<Option<Vendor>> {
        Ok(None)
    }

    async fn find_all(&self) -> Result<Vec<Vendor>> {
        Ok(vec![])
    }

    async fn find_all_paginated(&self, _page: u32, _limit: u32) -> Result<(Vec<Vendor>, u32)> {
        Ok((vec![], 0))
    }

    async fn search_by_name(&self, _name: &str) -> Result<Vec<Vendor>> {
        Ok(vec![])
    }

    async fn update(&self, _vendor: &Vendor) -> Result<()> {
        Err(anyhow!("Not implemented yet"))
    }

    async fn delete(&self, _id: &str) -> Result<()> {
        Err(anyhow!("Not implemented yet"))
    }
}

// ============================================================================
// Product Repository Adapter (Placeholder implementation)
// ============================================================================

pub struct SqliteProductRepository {
    integrated_repo: Arc<IntegratedProductRepository>,
}

impl SqliteProductRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            integrated_repo: Arc::new(IntegratedProductRepository::new(pool)),
        }
    }
}

#[async_trait]
impl ProductRepository for SqliteProductRepository {
    async fn save_product(&self, _product: &Product) -> Result<()> {
        Err(anyhow!("Not implemented yet"))
    }

    async fn save_products_batch(&self, _products: &[Product]) -> Result<()> {
        Err(anyhow!("Not implemented yet"))
    }

    async fn find_product_by_url(&self, _url: &str) -> Result<Option<Product>> {
        Ok(None)
    }

    async fn get_existing_urls(&self) -> Result<HashSet<String>> {
        Ok(HashSet::new())
    }

    async fn get_products_paginated(&self, _page: u32, _limit: u32) -> Result<(Vec<Product>, u32)> {
        Ok((vec![], 0))
    }
    
    async fn save_matter_product(&self, _product: &MatterProduct) -> Result<()> {
        Err(anyhow!("Not implemented yet"))
    }

    async fn save_matter_products_batch(&self, _products: &[MatterProduct]) -> Result<()> {
        Err(anyhow!("Not implemented yet"))
    }

    async fn find_matter_product_by_url(&self, _url: &str) -> Result<Option<MatterProduct>> {
        Ok(None)
    }

    async fn get_matter_products_paginated(&self, _page: u32, _limit: u32) -> Result<(Vec<MatterProduct>, u32)> {
        Ok((vec![], 0))
    }
    
    async fn search_products(&self, _query: &str) -> Result<Vec<MatterProduct>> {
        Ok(vec![])
    }

    async fn find_by_manufacturer(&self, _manufacturer: &str) -> Result<Vec<MatterProduct>> {
        Ok(vec![])
    }

    async fn find_by_device_type(&self, _device_type: &str) -> Result<Vec<MatterProduct>> {
        Ok(vec![])
    }

    async fn find_by_vid(&self, _vid: &str) -> Result<Vec<MatterProduct>> {
        Ok(vec![])
    }

    async fn find_by_certification_date_range(&self, _start: &str, _end: &str) -> Result<Vec<MatterProduct>> {
        Ok(vec![])
    }
    
    async fn get_database_summary(&self) -> Result<DatabaseSummary> {
        let stats = self.integrated_repo.get_database_statistics().await?;
        
        Ok(DatabaseSummary {
            total_products: stats.total_products as u32,
            total_matter_products: stats.matter_products_count as u32,
            total_vendors: 0, // TODO: Add vendor count to integrated schema
            last_crawling_date: None, // TODO: Parse string to DateTime
            database_size_mb: 0.0, // TODO: Add database size calculation
        })
    }

    async fn count_products(&self) -> Result<u32> {
        let stats = self.integrated_repo.get_database_statistics().await?;
        Ok(stats.total_products as u32)
    }

    async fn count_matter_products(&self) -> Result<u32> {
        let stats = self.integrated_repo.get_database_statistics().await?;
        Ok(stats.matter_products_count as u32)
    }
    
    async fn delete_product(&self, _url: &str) -> Result<()> {
        Err(anyhow!("Not implemented yet"))
    }

    async fn delete_matter_product(&self, _url: &str) -> Result<()> {
        Err(anyhow!("Not implemented yet"))
    }
    
    async fn get_all_products(&self) -> Result<Vec<Product>> {
        Ok(vec![])
    }

    async fn get_all_matter_products(&self) -> Result<Vec<MatterProduct>> {
        Ok(vec![])
    }

    async fn get_recent_products(&self, _limit: u32) -> Result<Vec<Product>> {
        Ok(vec![])
    }

    async fn get_recent_matter_products(&self, _limit: u32) -> Result<Vec<MatterProduct>> {
        Ok(vec![])
    }
    
    async fn filter_matter_products(
        &self,
        _manufacturer: Option<&str>,
        _device_type: Option<&str>,
        _vid: Option<&str>,
        _certification_date_start: Option<&str>,
        _certification_date_end: Option<&str>,
    ) -> Result<Vec<MatterProduct>> {
        Ok(vec![])
    }

    async fn get_unique_manufacturers(&self) -> Result<Vec<String>> {
        Ok(vec![])
    }

    async fn get_unique_device_types(&self) -> Result<Vec<String>> {
        Ok(vec![])
    }
    
    async fn get_latest_updated_product(&self) -> Result<Option<Product>> {
        Ok(None)
    }
}
