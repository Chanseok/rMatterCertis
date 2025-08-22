#![cfg(any())]
//! ARCHIVED: infrastructure/crawler.rs (legacy crawler engine)
// Historical snapshot preserved for reference. Not part of active build.

// Full original content follows (guarded out of compilation):

use std::sync::Arc;
use std::collections::HashSet;
use anyhow::{Result, Context};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::infrastructure::http_client::{HttpClient, HttpClientConfig};
// use crate::infrastructure::repositories::{SqliteProductRepository, SqliteVendorRepository};  // Temporarily disabled
use crate::infrastructure::integrated_product_repository::IntegratedProductRepository;
use crate::domain::session_manager::{SessionManager, SessionStatus, CrawlingStage};
use crate::application::dto::StartCrawlingDto;

#[derive(Debug, Clone, serde::Serialize)]
pub struct CrawlingConfig {
    pub session_id: String,
    pub start_url: String,
    pub target_domains: Vec<String>,
    pub max_pages: u32,
    pub concurrent_requests: u32,
    pub delay_ms: u64,
    pub http_config: HttpClientConfig,
}

impl From<StartCrawlingDto> for CrawlingConfig {
    fn from(dto: StartCrawlingDto) -> Self {
        Self {
            session_id: Uuid::new_v4().to_string(),
            start_url: dto.start_url,
            target_domains: dto.target_domains,
            max_pages: dto.max_pages.unwrap_or(100),
            concurrent_requests: dto.concurrent_requests.unwrap_or(3),
            delay_ms: dto.delay_ms.unwrap_or(1000),
            http_config: HttpClientConfig { max_requests_per_second: 2, ..Default::default() },
        }
    }
}

#[derive(Debug, Clone)]
pub struct CrawledPage {
    pub url: String,
    pub content: String,
    pub title: Option<String>,
    pub links: Vec<String>,
    pub status_code: u16,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExtractedProduct {
    pub url: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub certificate_id: Option<String>,
    pub page_id: u32,
    pub index_in_page: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExtractedMatterProduct {
    pub url: String,
    pub page_id: u32,
    pub index_in_page: u32,
    pub id: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub device_type: Option<String>,
    pub certificate_id: Option<String>,
    pub certification_date: Option<String>,
    pub software_version: Option<String>,
    pub hardware_version: Option<String>,
    pub vid: Option<String>,
    pub pid: Option<String>,
    pub family_sku: Option<String>,
    pub family_variant_sku: Option<String>,
    pub firmware_version: Option<String>,
    pub family_id: Option<String>,
    pub tis_trp_tested: Option<String>,
    pub specification_version: Option<String>,
    pub transport_interface: Option<String>,
    pub primary_device_type_id: Option<String>,
    pub application_categories: Option<Vec<String>>,
}

pub struct WebCrawler {
    http_client: HttpClient,
    session_manager: Arc<SessionManager>,
    visited_urls: Arc<Mutex<HashSet<String>>>,
    product_repo: Arc<SqliteProductRepository>,
    vendor_repo: Arc<SqliteVendorRepository>,
}

impl WebCrawler {
    pub fn new(
        http_config: HttpClientConfig,
        session_manager: Arc<SessionManager>,
        product_repo: Arc<SqliteProductRepository>,
        vendor_repo: Arc<SqliteVendorRepository>,
    ) -> Result<Self> {
        let http_client = HttpClient::new(http_config)?;
        Ok(Self { http_client, session_manager, visited_urls: Arc::new(Mutex::new(HashSet::new())), product_repo, vendor_repo })
    }
}
