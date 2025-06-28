//! Repository interfaces
//! 
//! Contains trait definitions for data access patterns.

use async_trait::async_trait;
use anyhow::Result;
use chrono::{DateTime, Utc};
use crate::domain::entities::{Vendor, Product, CrawlingSession};

#[async_trait]
pub trait VendorRepository: Send + Sync {
    async fn create(&self, vendor: &Vendor) -> Result<()>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Vendor>>;
    async fn find_all(&self) -> Result<Vec<Vendor>>;
    async fn find_active(&self) -> Result<Vec<Vendor>>;
    async fn update(&self, vendor: &Vendor) -> Result<()>;
    async fn update_last_crawled(&self, id: &str, timestamp: DateTime<Utc>) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<()>;
}

#[async_trait]
pub trait ProductRepository: Send + Sync {
    async fn create(&self, product: &Product) -> Result<()>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Product>>;
    async fn find_by_vendor(&self, vendor_id: &str) -> Result<Vec<Product>>;
    async fn find_all(&self) -> Result<Vec<Product>>;
    async fn find_in_stock(&self) -> Result<Vec<Product>>;
    async fn find_by_category(&self, category: &str) -> Result<Vec<Product>>;
    async fn search_by_name(&self, query: &str) -> Result<Vec<Product>>;
    async fn count_by_vendor(&self, vendor_id: &str) -> Result<i64>;
    async fn get_latest_by_vendor(&self, vendor_id: &str, limit: i64) -> Result<Vec<Product>>;
    async fn update(&self, product: &Product) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<()>;
    async fn delete_by_vendor(&self, vendor_id: &str) -> Result<()>;
}

#[async_trait]
pub trait CrawlingSessionRepository: Send + Sync {
    async fn create(&self, session: &CrawlingSession) -> Result<()>;
    async fn find_by_id(&self, id: &str) -> Result<Option<CrawlingSession>>;
    async fn find_by_vendor(&self, vendor_id: &str) -> Result<Vec<CrawlingSession>>;
    async fn find_active(&self) -> Result<Vec<CrawlingSession>>;
    async fn update(&self, session: &CrawlingSession) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<()>;
}
