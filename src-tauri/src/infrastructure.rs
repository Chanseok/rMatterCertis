//! Infrastructure layer for database connections, parsing, and external integrations
//! 
//! This module provides database connections, session management, HTML parsing,
//! web crawling, and external service integrations following the guide's architecture.

pub mod database_connection;
pub mod integrated_product_repository;
pub mod html_parser;  // HTML parser with integrated tests
pub mod simple_http_client;
pub mod parsing_error;  // Enhanced error types
pub mod parsing;  // Modern parsing architecture following the guide
pub mod crawling;  // Web crawler implementation
pub mod crawling_engine;  // 4-stage batch crawling engine
pub mod service_based_crawling_engine;  // New service-based crawling engine
pub mod advanced_crawling_engine;  // Phase 2 advanced crawling engine with data pipeline
pub mod crawling_service_impls;  // Service implementations
pub mod data_processing_service_impls;  // Data processing service implementations
pub mod config;  // Configuration constants and helpers
pub mod logging;  // Logging infrastructure
pub mod retry_manager;  // 재시도 관리자 - INTEGRATED_PHASE2_PLAN Week 1 Day 3-4

// Temporarily disabled - working on schema compatibility
// pub mod product_repository;
// pub mod matter_product_repository; 
// pub mod crawling_result_repository;
// pub mod repositories_adapter;
// pub mod http;
// pub mod http_client;
// pub mod crawler;  // Temporarily disabled - will be enabled after repositories are stable

// Re-export commonly used items
pub use database_connection::DatabaseConnection;
pub use integrated_product_repository::IntegratedProductRepository;
pub use html_parser::MatterDataExtractor;  // HTML parser with integrated tests
pub use simple_http_client::HttpClient;
pub use config::csa_iot;  // CSA-IoT configuration constants

// Modern parsing and crawling exports following the guide
pub use parsing::{ParsingConfig, ParsingError, ParsingResult, ProductListParser, ProductDetailParser};
pub use crawling::WebCrawler;
pub use crawling_engine::{BatchCrawlingEngine, BatchCrawlingConfig};
pub use service_based_crawling_engine::ServiceBasedBatchCrawlingEngine;
pub use advanced_crawling_engine::AdvancedBatchCrawlingEngine;
pub use crawling_service_impls::{StatusCheckerImpl, DatabaseAnalyzerImpl, ProductListCollectorImpl, ProductDetailCollectorImpl, CollectorConfig};
pub use data_processing_service_impls::{DeduplicationServiceImpl, ValidationServiceImpl, ConflictResolverImpl};
pub use logging::{init_logging, init_logging_with_config, get_log_directory};
pub use retry_manager::{RetryManager, RetryItem, RetryStats, ErrorClassification};

// Legacy compatibility through adapters
// pub use repositories_adapter::{SqliteVendorRepository, SqliteProductRepository};
// pub use http_client::{HttpClient, HttpClientConfig};
// pub use crawler::{WebCrawler, CrawlingConfig, CrawledPage};  // Temporarily disabled
