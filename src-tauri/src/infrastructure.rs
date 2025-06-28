//! Infrastructure layer module
//! 
//! This module contains implementations for external concerns
//! such as databases, HTTP clients, and configuration.

pub mod database_connection;
pub mod repositories;
pub mod config;
pub mod database;
pub mod http;

// Re-export commonly used items
pub use database_connection::DatabaseConnection;
pub use repositories::{SqliteVendorRepository, SqliteProductRepository, SqliteCrawlingSessionRepository};
