//! Infrastructure layer module
//! 
//! This module contains implementations for external concerns
//! such as databases, HTTP clients, crawling engines, and configuration.

pub mod database_connection;
pub mod config;
pub mod database;
pub mod integrated_product_repository;
// Temporarily disabled - working on schema compatibility
// pub mod product_repository;
// pub mod matter_product_repository; 
// pub mod crawling_result_repository;
pub mod repositories_adapter;
pub mod http;
pub mod http_client;
// pub mod crawler;  // Temporarily disabled - will be enabled after repositories are stable

// Re-export commonly used items
pub use database_connection::DatabaseConnection;
pub use integrated_product_repository::IntegratedProductRepository;
// Legacy compatibility through adapters
pub use repositories_adapter::{SqliteVendorRepository, SqliteProductRepository};
pub use http_client::{HttpClient, HttpClientConfig};
// pub use crawler::{WebCrawler, CrawlingConfig, CrawledPage};  // Temporarily disabled
