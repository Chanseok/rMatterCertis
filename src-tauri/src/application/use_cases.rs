//! Application use cases
//! 
//! Contains the application's use cases and business workflows.

use anyhow::Result;
use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;

use crate::domain::entities::{Vendor, Product, CrawlingSession, CrawlingStatus};
use crate::domain::repositories::{VendorRepository, ProductRepository, CrawlingSessionRepository};

pub struct VendorUseCases {
    vendor_repo: Arc<dyn VendorRepository>,
}

impl VendorUseCases {
    pub fn new(vendor_repo: Arc<dyn VendorRepository>) -> Self {
        Self { vendor_repo }
    }

    pub async fn create_vendor(&self, vendor: Vendor) -> Result<()> {
        self.vendor_repo.create(&vendor).await
    }

    pub async fn get_vendor(&self, id: &str) -> Result<Option<Vendor>> {
        self.vendor_repo.find_by_id(id).await
    }

    pub async fn list_vendors(&self) -> Result<Vec<Vendor>> {
        self.vendor_repo.find_all().await
    }

    pub async fn list_active_vendors(&self) -> Result<Vec<Vendor>> {
        self.vendor_repo.find_active().await
    }

    pub async fn update_vendor(&self, vendor: Vendor) -> Result<()> {
        self.vendor_repo.update(&vendor).await
    }

    pub async fn delete_vendor(&self, id: &str) -> Result<()> {
        self.vendor_repo.delete(id).await
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
