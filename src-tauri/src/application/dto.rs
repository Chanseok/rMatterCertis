//! Data Transfer Objects for Matter Certification domain
//! 
//! Contains DTOs for data exchange between Use Cases and Tauri Commands.

#![allow(missing_docs)]
#![allow(clippy::unnecessary_operation)]
#![allow(unused_must_use)]

use serde::{Deserialize, Serialize};
use crate::domain::entities::{Vendor, Product, MatterProduct, DatabaseSummary};
use crate::domain::session_manager::{CrawlingSessionState};

// ============================================================================
// Vendor DTOs
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateVendorDto {
    pub vendor_number: u32,           // Matter 인증 벤더 번호 (숫자)
    pub vendor_name: String,          // 벤더명
    pub company_legal_name: String,   // 법인명 (Matter 인증 필수)
    pub vendor_url: Option<String>,   // 벤더 웹사이트 URL
    pub csa_assigned_number: Option<String>, // CSA 할당 번호
}

#[derive(Debug, Deserialize)]
pub struct UpdateVendorDto {
    pub vendor_name: Option<String>,
    pub company_legal_name: Option<String>,
    pub vendor_url: Option<String>,
    pub csa_assigned_number: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct VendorResponseDto {
    pub vendor_id: String,
    pub vendor_number: u32,
    pub vendor_name: String,
    pub company_legal_name: String,
    pub created_at: String,
}

impl From<Vendor> for VendorResponseDto {
    fn from(vendor: Vendor) -> Self {
        Self {
            vendor_id: vendor.id,
            vendor_number: vendor.vendor_number,
            vendor_name: vendor.vendor_name,
            company_legal_name: vendor.company_legal_name,
            created_at: vendor.created_at.to_rfc3339(),
        }
    }
}

// ============================================================================
// Product DTOs
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateProductDto {
    pub url: String,                  // 제품 상세 페이지 URL
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub certificate_id: Option<String>,
    pub page_id: Option<u32>,
    pub index_in_page: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProductDto {
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub certificate_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProductResponseDto {
    pub url: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub certificate_id: Option<String>,
    pub page_id: Option<u32>,
    pub index_in_page: Option<u32>,
    pub created_at: String,
}

impl From<Product> for ProductResponseDto {
    fn from(product: Product) -> Self {
        Self {
            url: product.url,
            manufacturer: product.manufacturer,
            model: product.model,
            certificate_id: product.certificate_id,
            page_id: product.page_id.map(|id| id as u32),
            index_in_page: product.index_in_page.map(|idx| idx as u32),
            created_at: product.created_at.to_rfc3339(),
        }
    }
}

// ============================================================================
// MatterProduct DTOs (Matter 인증 특화)
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateMatterProductDto {
    pub url: String,                  // Product와 연결되는 URL
    pub page_id: Option<u32>,
    pub json_data: Option<String>,    // Raw JSON data from crawling
    pub vid: Option<String>,          // Vendor ID (Matter 특화)
    pub pid: Option<String>,          // Product ID (Matter 특화)
    pub device_name: Option<String>,  // Device name
    pub device_type: Option<String>,  // Device type
    pub manufacturer: Option<String>,
    pub certification_date: Option<String>,
    pub commissioning_method: Option<String>,
    pub transport_protocol: Option<String>,
    pub application_categories: Option<String>, // JSON string
    pub clusters_client: Option<String>,        // JSON string
    pub clusters_server: Option<String>,        // JSON string
}

#[derive(Debug, Serialize, Clone)]
pub struct MatterProductResponseDto {
    pub url: String,
    pub page_id: Option<u32>,
    pub index_in_page: Option<u32>,
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
    pub application_categories: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<MatterProduct> for MatterProductResponseDto {
    fn from(product: MatterProduct) -> Self {
        Self {
            url: product.url,
            page_id: product.page_id.map(|id| id),
            index_in_page: product.index_in_page.map(|idx| idx),
            id: product.id,
            manufacturer: product.manufacturer,
            model: product.model,
            device_type: product.device_type,
            certificate_id: product.certificate_id,
            certification_date: product.certification_date,
            software_version: product.software_version,
            hardware_version: product.hardware_version,
            vid: product.vid,
            pid: product.pid,
            family_sku: product.family_sku,
            family_variant_sku: product.family_variant_sku,
            firmware_version: product.firmware_version,
            family_id: product.family_id,
            tis_trp_tested: product.tis_trp_tested,
            specification_version: product.specification_version,
            transport_interface: product.transport_interface,
            primary_device_type_id: product.primary_device_type_id,
            application_categories: product.application_categories,
            created_at: product.created_at.to_rfc3339(),
            updated_at: product.updated_at.to_rfc3339(),
        }
    }
}

// ============================================================================
// Session Management DTOs (Memory-based)
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct StartSessionDto {
    pub session_id: String,
    pub start_url: String,
    pub target_domains: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SessionStatusDto {
    pub session_id: String,
    pub status: String,
    pub progress: u32,
    pub current_step: String,
    pub started_at: String,
    pub last_updated: String,
}

impl From<CrawlingSessionState> for SessionStatusDto {
    fn from(state: CrawlingSessionState) -> Self {
        Self {
            session_id: state.session_id,
            status: format!("{:?}", state.status),
            progress: state.products_found, // Use products_found as progress indicator
            current_step: state.current_url.unwrap_or("Unknown".to_string()),
            started_at: state.started_at.to_rfc3339(),
            last_updated: state.last_updated_at.to_rfc3339(),
        }
    }
}

// ============================================================================
// Search and Filter DTOs
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct ProductSearchDto {
    pub query: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct MatterProductFilterDto {
    pub manufacturer: Option<String>,
    pub device_type: Option<String>,
    pub vid: Option<String>,
    pub certification_date_start: Option<String>,
    pub certification_date_end: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct ProductSearchResultDto {
    pub products: Vec<MatterProductResponseDto>,
    pub total_count: u32,
    pub page: u32,
    pub page_size: u32,
    pub total_pages: u32,
}

// ============================================================================
// Database Summary DTOs
// ============================================================================

#[derive(Debug, Serialize)]
pub struct DatabaseSummaryDto {
    pub total_vendors: u32,
    pub total_products: u32,
    pub total_matter_products: u32,
    pub database_size_mb: f64,
    pub last_crawling_date: Option<String>,
}

impl From<DatabaseSummary> for DatabaseSummaryDto {
    fn from(summary: DatabaseSummary) -> Self {
        Self {
            total_vendors: summary.total_vendors,
            total_products: summary.total_products,
            total_matter_products: summary.total_matter_products,
            database_size_mb: summary.database_size_mb,
            last_crawling_date: summary.last_crawling_date.map(|dt| dt.to_rfc3339()),
        }
    }
}

// ============================================================================
// Crawling Engine DTOs (for Phase 3 implementation)
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct StartCrawlingDto {
    pub start_url: String,
    pub target_domains: Vec<String>,
    pub max_pages: Option<u32>,
    pub concurrent_requests: Option<u32>,
    pub delay_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct CrawlingConfigDto {
    pub max_concurrent_requests: u32,
    pub request_delay_ms: u64,
    pub timeout_seconds: u64,
    pub retry_attempts: u32,
    pub user_agent: String,
    pub respect_robots_txt: bool,
}

#[derive(Debug, Serialize)]
pub struct CrawlingResultDto {
    pub session_id: String,
    pub status: String,
    pub total_pages_crawled: u32,
    pub products_found: u32,
    pub errors_count: u32,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub execution_time_seconds: Option<u32>,
    pub error_details: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct CrawlingProgressDto {
    pub session_id: String,
    pub current_page: u32,
    pub total_pages: u32,
    pub progress_percentage: f32,
    pub current_url: Option<String>,
    pub products_found: u32,
    pub last_updated: String,
    pub estimated_completion: Option<String>,
}
