//! Infrastructure layer module
//! 
//! This module contains implementations for external concerns
//! such as databases, HTTP clients, crawling engines, and configuration.

pub mod database_connection;
pub mod repositories;
pub mod crawling_result_repository;
pub mod config;
pub mod database;
pub mod http;
pub mod http_client;
pub mod crawler;

// Re-export commonly used items
pub use database_connection::DatabaseConnection;
pub use repositories::{SqliteVendorRepository, SqliteProductRepository};
pub use crawling_result_repository::{CrawlingResultRepository, SqliteCrawlingResultRepository};
pub use http_client::{HttpClient, HttpClientConfig};
pub use crawler::{WebCrawler, CrawlingConfig, CrawledPage};
