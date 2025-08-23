use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Product basic information from listing pages
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Product {
    pub id: Option<String>, // Generated ID: "p" + 4-digit page_id + "i" + 2-digit index_in_page
    pub url: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    #[serde(rename = "certificateId")]
    pub certificate_id: Option<String>,
    #[serde(rename = "pageId")]
    pub page_id: Option<i32>,
    #[serde(rename = "indexInPage")]
    pub index_in_page: Option<i32>,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
}

/// Detailed product specifications from detail pages
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProductDetail {
    pub url: String,
    pub page_id: Option<i32>,
    pub index_in_page: Option<i32>,
    pub id: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub device_type: Option<String>,
    pub certificate_id: Option<String>,
    pub certification_date: Option<String>,
    pub software_version: Option<String>,
    pub hardware_version: Option<String>,
    pub vid: Option<i32>,
    pub pid: Option<i32>,
    pub family_sku: Option<String>,
    pub family_variant_sku: Option<String>,
    pub firmware_version: Option<String>,
    pub family_id: Option<String>,
    pub tis_trp_tested: Option<String>,
    pub specification_version: Option<String>,
    pub transport_interface: Option<String>,
    pub primary_device_type_id: Option<String>,
    pub application_categories: Option<String>,
    pub description: Option<String>,
    pub compliance_document_url: Option<String>,
    pub program_type: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Combined product with details for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductWithDetails {
    pub product: Product,
    pub details: Option<ProductDetail>,
}

/// Vendor information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vendor {
    pub vendor_id: i32,
    pub vendor_name: Option<String>,
    pub company_legal_name: Option<String>,
    pub vendor_number: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Search and filter criteria
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductSearchCriteria {
    pub manufacturer: Option<String>,
    pub device_type: Option<String>,
    pub certificate_id: Option<String>,
    pub specification_version: Option<String>,
    pub program_type: Option<String>,
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

impl Product {
    /// Generate unique ID from page_id and index_in_page
    /// Format: "p" + 4-digit page_id + "i" + 2-digit index_in_page
    /// Example: p0485i01 for page 485, index 1
    pub fn generate_id(&mut self) {
        if let (Some(page_id), Some(index_in_page)) = (self.page_id, self.index_in_page) {
            self.id = Some(format!("p{:04}i{:02}", page_id, index_in_page));
        }
    }

    /// Generate ID and return the generated value
    pub fn with_generated_id(mut self) -> Self {
        self.generate_id();
        self
    }
}

impl ProductDetail {
    /// Generate unique ID from page_id and index_in_page
    /// Format: "p" + 4-digit page_id + "i" + 2-digit index_in_page
    /// Example: p0485i01 for page 485, index 1
    pub fn generate_id(&mut self) {
        if let (Some(page_id), Some(index_in_page)) = (self.page_id, self.index_in_page) {
            self.id = Some(format!("p{:04}i{:02}", page_id, index_in_page));
        }
    }

    /// Generate ID and return the generated value
    pub fn with_generated_id(mut self) -> Self {
        self.generate_id();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_generate_id_zero_padded() {
        let now = Utc::now();
        let mut p = Product {
            id: None,
            url: "https://example.test/item".to_string(),
            manufacturer: None,
            model: None,
            certificate_id: None,
            page_id: Some(7),
            index_in_page: Some(3),
            created_at: now,
            updated_at: now,
        };
        p.generate_id();
        assert_eq!(p.id.as_deref(), Some("p0007i03"));
    }

    #[test]
    fn product_detail_generate_id_zero_padded() {
        let now = Utc::now();
        let mut d = ProductDetail {
            url: "https://example.test/item".to_string(),
            page_id: Some(85),
            index_in_page: Some(12),
            id: None,
            manufacturer: None,
            model: None,
            device_type: None,
            certificate_id: None,
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
            created_at: now,
            updated_at: now,
        };
        d.generate_id();
        assert_eq!(d.id.as_deref(), Some("p0085i12"));
    }
}
