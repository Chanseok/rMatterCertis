//! # Domain Value Objects
//!
//! Immutable value types that represent concepts in the crawling domain.
//! Value objects are defined by their attributes rather than identity.

#![allow(missing_docs)]
#![allow(clippy::unnecessary_operation)]
#![allow(unused_must_use)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Task identifier with strong typing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(Uuid);

impl TaskId {
    /// Creates a new unique task ID
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates a task ID from an existing UUID
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the inner UUID
    #[must_use]
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Validated URL value object
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ValidatedUrl {
    url: String,
    is_secure: bool,
    domain: String,
}

impl ValidatedUrl {
    /// Creates a new validated URL
    /// 
    /// # Errors
    /// Returns error if URL is invalid or malformed
    pub fn new(url: String) -> Result<Self, UrlError> {
        if url.trim().is_empty() {
            return Err(UrlError::Empty);
        }

        let parsed = url::Url::parse(&url).map_err(|_| UrlError::InvalidFormat)?;
        
        let domain = parsed.host_str()
            .ok_or(UrlError::NoDomain)?
            .to_string();

        let is_secure = parsed.scheme() == "https";

        Ok(Self {
            url,
            is_secure,
            domain,
        })
    }

    /// Returns the URL string
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.url
    }

    /// Returns the domain
    #[must_use]
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Returns true if the URL uses HTTPS
    #[must_use]
    pub const fn is_secure(&self) -> bool {
        self.is_secure
    }

    /// Returns true if the URL belongs to the specified domain
    #[must_use]
    pub fn is_from_domain(&self, domain: &str) -> bool {
        self.domain.contains(domain)
    }
}

/// URL validation errors
#[derive(Debug, thiserror::Error)]
pub enum UrlError {
    #[error("URL cannot be empty")]
    Empty,
    #[error("URL format is invalid")]
    InvalidFormat,
    #[error("URL must have a valid domain")]
    NoDomain,
}

/// Structured product data value object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductData {
    pub product_id: String,
    pub name: String,
    pub category: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub certification_number: Option<String>,
    pub certification_date: Option<chrono::DateTime<chrono::Utc>>,
    pub technical_details: HashMap<String, String>,
    pub compliance_details: HashMap<String, String>,
    pub source_url: ValidatedUrl,
    pub extracted_at: chrono::DateTime<chrono::Utc>,
    pub confidence_score: f64,
    // Product position coordinates calculated from pagination context
    pub page_id: Option<i32>,
    pub index_in_page: Option<i32>,
}

impl ProductData {
    /// Creates a new product data instance
    /// 
    /// # Errors
    /// Returns error if required fields are invalid
    pub fn new(
        product_id: String,
        name: String,
        source_url: ValidatedUrl,
    ) -> Result<Self, ProductDataError> {
        if product_id.trim().is_empty() {
            return Err(ProductDataError::InvalidProductId);
        }

        if name.trim().is_empty() {
            return Err(ProductDataError::InvalidName);
        }

        Ok(Self {
            product_id,
            name,
            category: None,
            manufacturer: None,
            model: None,
            certification_number: None,
            certification_date: None,
            technical_details: HashMap::new(),
            compliance_details: HashMap::new(),
            source_url,
            extracted_at: chrono::Utc::now(),
            confidence_score: 0.0,
            page_id: None,
            index_in_page: None,
        })
    }

    /// Builder pattern for optional fields
    #[must_use]
    pub fn with_category(mut self, category: Option<String>) -> Self {
        self.category = category;
        self
    }

    #[must_use]
    pub fn with_manufacturer(mut self, manufacturer: Option<String>) -> Self {
        self.manufacturer = manufacturer;
        self
    }

    #[must_use]
    pub fn with_model(mut self, model: Option<String>) -> Self {
        self.model = model;
        self
    }

    #[must_use]
    pub fn with_certification(
        mut self,
        number: Option<String>,
        date: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Self {
        self.certification_number = number;
        self.certification_date = date;
        self
    }

    #[must_use]
    pub fn with_confidence_score(mut self, score: f64) -> Self {
        self.confidence_score = score.clamp(0.0, 1.0);
        self
    }

    /// Sets pagination coordinates (page_id and index_in_page)
    #[must_use]
    pub fn with_pagination_coordinates(mut self, page_id: i32, index_in_page: i32) -> Self {
        self.page_id = Some(page_id);
        self.index_in_page = Some(index_in_page);
        self
    }

    /// Adds a technical detail
    pub fn add_technical_detail(&mut self, key: String, value: String) {
        self.technical_details.insert(key, value);
    }

    /// Adds a compliance detail
    pub fn add_compliance_detail(&mut self, key: String, value: String) {
        self.compliance_details.insert(key, value);
    }

    /// Returns true if the product data has high confidence
    #[must_use]
    pub fn is_high_confidence(&self) -> bool {
        self.confidence_score >= 0.8
    }

    /// Returns true if the product has minimum required information
    #[must_use]
    pub fn is_complete(&self) -> bool {
        !self.product_id.trim().is_empty()
            && !self.name.trim().is_empty()
            && self.manufacturer.is_some()
            && self.category.is_some()
    }

    /// Validates the product data
    pub fn validate(&self) -> Result<(), Vec<ProductDataError>> {
        let mut errors = Vec::new();

        if self.product_id.trim().is_empty() {
            errors.push(ProductDataError::InvalidProductId);
        }

        if self.name.trim().is_empty() {
            errors.push(ProductDataError::InvalidName);
        }

        if self.confidence_score < 0.0 || self.confidence_score > 1.0 {
            errors.push(ProductDataError::InvalidConfidenceScore);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Product data validation errors
#[derive(Debug, thiserror::Error)]
pub enum ProductDataError {
    #[error("Product ID cannot be empty")]
    InvalidProductId,
    #[error("Product name cannot be empty")]
    InvalidName,
    #[error("Confidence score must be between 0.0 and 1.0")]
    InvalidConfidenceScore,
}

/// Crawling progress value object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingProgress {
    pub total_pages: u32,
    pub processed_pages: u32,
    pub successful_pages: u32,
    pub failed_pages: u32,
    pub products_found: u32,
    pub products_saved: u32,
    pub current_page: Option<u32>,
    pub estimated_completion: Option<chrono::DateTime<chrono::Utc>>,
    pub processing_rate: f64, // pages per minute
}

impl CrawlingProgress {
    /// Creates a new crawling progress tracker
    #[must_use]
    pub fn new(total_pages: u32) -> Self {
        Self {
            total_pages,
            processed_pages: 0,
            successful_pages: 0,
            failed_pages: 0,
            products_found: 0,
            products_saved: 0,
            current_page: None,
            estimated_completion: None,
            processing_rate: 0.0,
        }
    }

    /// Returns the completion percentage (0.0 to 1.0)
    #[must_use]
    pub fn completion_percentage(&self) -> f64 {
        if self.total_pages == 0 {
            return 0.0;
        }
        self.processed_pages as f64 / self.total_pages as f64
    }

    /// Returns the success rate (0.0 to 1.0)
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        if self.processed_pages == 0 {
            return 0.0;
        }
        self.successful_pages as f64 / self.processed_pages as f64
    }

    /// Returns true if crawling is complete
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.processed_pages >= self.total_pages
    }

    /// Updates progress with new page processing
    pub fn update_page_processed(&mut self, success: bool, products_found: u32) {
        self.processed_pages += 1;
        self.products_found += products_found;
        
        if success {
            self.successful_pages += 1;
        } else {
            self.failed_pages += 1;
        }

        // Update estimated completion time based on current rate
        if self.processing_rate > 0.0 {
            let remaining_pages = self.total_pages.saturating_sub(self.processed_pages);
            let remaining_minutes = remaining_pages as f64 / self.processing_rate;
            self.estimated_completion = Some(
                chrono::Utc::now() + chrono::Duration::minutes(remaining_minutes as i64)
            );
        }
    }

    /// Updates the processing rate
    pub fn update_processing_rate(&mut self, rate: f64) {
        self.processing_rate = rate.max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_id_uniqueness() {
        let id1 = TaskId::new();
        let id2 = TaskId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn validated_url_creation() {
        let url = ValidatedUrl::new("https://example.com/page".to_string()).unwrap();
        assert!(url.is_secure());
        assert_eq!(url.domain(), "example.com");
        assert!(url.is_from_domain("example.com"));
    }

    #[test]
    fn product_data_validation() {
        let url = ValidatedUrl::new("https://example.com".to_string()).unwrap();
        let product = ProductData::new(
            "test_id".to_string(),
            "Test Product".to_string(),
            url,
        ).unwrap();

        assert!(product.validate().is_ok());
        assert!(!product.is_complete()); // Missing manufacturer and category
    }

    #[test]
    fn crawling_progress_calculation() {
        let mut progress = CrawlingProgress::new(100);
        assert_eq!(progress.completion_percentage(), 0.0);
        assert!(!progress.is_complete());

        progress.update_page_processed(true, 5);
        assert_eq!(progress.completion_percentage(), 0.01);
        assert_eq!(progress.success_rate(), 1.0);
    }
}
