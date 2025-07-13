//! # Crawling Task Definitions
//!
//! This module defines the core task types for the event-driven crawling system.
//! Following Clean Code principles, each task is self-contained and immutable.

#![allow(missing_docs)]
#![allow(clippy::unnecessary_qualification)]
#![allow(unused_must_use)]

use std::fmt;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::domain::{ProductData, ValidatedUrl};

/// Unique identifier for crawling tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(Uuid);

impl TaskId {
    /// Creates a new unique task ID
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
    
    /// Get the inner UUID
    #[must_use]
    pub fn inner(&self) -> Uuid {
        self.0
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<TaskId> for Uuid {
    fn from(task_id: TaskId) -> Self {
        task_id.0
    }
}

/// Core crawling task types following the event-driven architecture
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrawlingTask {
    /// Fetch HTML content from a product list page
    FetchListPage {
        task_id: TaskId,
        page_number: u32,
        url: String,
    },
    
    /// Parse HTML content to extract product URLs
    ParseListPage {
        task_id: TaskId,
        page_number: u32,
        html_content: String,
        source_url: String,
    },
    
    /// Fetch HTML content from a product detail page
    FetchProductDetail {
        task_id: TaskId,
        product_url: String,
    },
    
    /// Parse product detail HTML to extract structured data
    ParseProductDetail {
        task_id: TaskId,
        product_url: String,
        html_content: String,
    },
    
    /// Save extracted product data to database
    SaveProduct {
        task_id: TaskId,
        product_data: TaskProductData,
    },
}

impl CrawlingTask {
    /// Returns the unique task identifier
    #[must_use]
    pub const fn task_id(&self) -> TaskId {
        match self {
            Self::FetchListPage { task_id, .. }
            | Self::ParseListPage { task_id, .. }
            | Self::FetchProductDetail { task_id, .. }
            | Self::ParseProductDetail { task_id, .. }
            | Self::SaveProduct { task_id, .. } => *task_id,
        }
    }
    
    /// Returns the task type as a string for telemetry
    #[must_use]
    pub const fn task_type(&self) -> &'static str {
        match self {
            Self::FetchListPage { .. } => "fetch_list_page",
            Self::ParseListPage { .. } => "parse_list_page",
            Self::FetchProductDetail { .. } => "fetch_product_detail",
            Self::ParseProductDetail { .. } => "parse_product_detail",
            Self::SaveProduct { .. } => "save_product",
        }
    }

    /// Increment retry count for task retries
    pub fn increment_retry_count(&mut self) {
        // Note: This is a simplified implementation
        // In a real implementation, you would add retry_count fields to each task variant
        // For now, we'll just clone the task with a new task_id
        match self {
            Self::FetchListPage { task_id, .. } => {
                *task_id = TaskId::new();
            }
            Self::ParseListPage { task_id, .. } => {
                *task_id = TaskId::new();
            }
            Self::FetchProductDetail { task_id, .. } => {
                *task_id = TaskId::new();
            }
            Self::ParseProductDetail { task_id, .. } => {
                *task_id = TaskId::new();
            }
            Self::SaveProduct { task_id, .. } => {
                *task_id = TaskId::new();
            }
        }
    }
}

/// Structured product data extracted from HTML  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskProductData {
    pub product_id: String,
    pub name: String,
    pub category: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub certification_number: Option<String>,
    pub certification_date: Option<String>,
    pub details: std::collections::HashMap<String, String>,
    pub extracted_at: chrono::DateTime<chrono::Utc>,
    pub source_url: String,
}

impl TaskProductData {
    /// Creates a new product data instance
    #[must_use]
    pub fn new(product_id: String, name: String, source_url: String) -> Self {
        Self {
            product_id,
            name,
            category: None,
            manufacturer: None,
            model: None,
            certification_number: None,
            certification_date: None,
            details: std::collections::HashMap::new(),
            extracted_at: chrono::Utc::now(),
            source_url,
        }
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
    pub fn with_certification_number(mut self, cert_number: Option<String>) -> Self {
        self.certification_number = cert_number;
        self
    }
    
    #[must_use]
    pub fn with_certification_date(mut self, cert_date: Option<String>) -> Self {
        self.certification_date = cert_date;
        self
    }
    
    /// Adds a key-value pair to the details
    pub fn add_detail(&mut self, key: String, value: String) {
        self.details.insert(key, value);
    }

    /// Converts TaskProductData to domain ProductData
    pub fn to_product_data(self) -> Result<ProductData, String> {
        let certification_date = self.certification_date
            .as_ref()
            .and_then(|date_str| chrono::DateTime::parse_from_rfc3339(date_str).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));
            
        Ok(ProductData {
            product_id: self.product_id,
            name: self.name,
            category: self.category,
            manufacturer: self.manufacturer,
            model: self.model,
            certification_number: self.certification_number,
            certification_date,
            technical_details: self.details,
            compliance_details: HashMap::new(), // Default empty
            confidence_score: 1.0, // Default confidence
            extracted_at: self.extracted_at,
            source_url: ValidatedUrl::new(self.source_url)
                .map_err(|e| format!("Invalid URL: {}", e))?,
        })
    }
}

impl From<TaskProductData> for ProductData {
    fn from(task_data: TaskProductData) -> Self {
        task_data.to_product_data().expect("Failed to convert TaskProductData to ProductData")
    }
}

/// Task execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskResult {
    /// Task completed successfully
    Success {
        task_id: TaskId,
        output: TaskOutput,
        duration: std::time::Duration,
    },
    
    /// Task failed with error
    Failure {
        task_id: TaskId,
        error: String,
        duration: std::time::Duration,
        retry_count: u32,
    },
}

/// Output produced by successful task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskOutput {
    /// HTML content fetched from a page
    HtmlContent(String),
    
    /// Product URLs extracted from list page
    ProductUrls(Vec<String>),
    
    /// Product detail HTML content
    ProductDetailHtml {
        product_id: String,
        html_content: String,
        source_url: String,
    },
    
    /// Structured product data
    ProductData(TaskProductData),
    
    /// Database save confirmation
    SaveConfirmation {
        product_id: String,
        saved_at: chrono::DateTime<chrono::Utc>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_id_is_unique() {
        let id1 = TaskId::new();
        let id2 = TaskId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn task_type_returns_correct_string() {
        let task = CrawlingTask::FetchListPage {
            task_id: TaskId::new(),
            page_number: 1,
            url: "https://example.com".to_string(),
        };
        assert_eq!(task.task_type(), "fetch_list_page");
    }

    #[test]
    fn product_data_builder_pattern() {
        let product = ProductData::new(
            "test_id".to_string(),
            "Test Product".to_string(),
            "https://example.com/product".to_string(),
        )
        .with_category(Some("Electronics".to_string()))
        .with_manufacturer(Some("Test Corp".to_string()));
        
        assert_eq!(product.product_id, "test_id");
        assert_eq!(product.name, "Test Product");
        assert_eq!(product.category, Some("Electronics".to_string()));
        assert_eq!(product.manufacturer, Some("Test Corp".to_string()));
    }
}
