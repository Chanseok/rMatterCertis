//! Infrastructure layer for database connections, parsing, and external integrations
//! (Directory-style module replacing previous file-based infrastructure.rs)

pub mod config; // Configuration constants and helpers
pub mod crawler;
pub mod crawling_range_calculation_test;
pub mod crawling_service_impls; // Service implementations
pub mod data_processing_service_impls; // Data processing service implementations
pub mod database;
pub mod database_connection;
pub mod database_paths; // 중앙집중식 데이터베이스 경로 관리 (Modern Rust 2024)
pub mod features;
pub mod html_parser; // HTML parser with integrated tests
pub mod http_client;
pub mod integrated_product_repository;
pub mod logging; // Logging infrastructure
pub mod matter_product_repository;
pub mod parsing; // Modern parsing architecture following the guide
pub mod parsing_error; // Enhanced error types
pub mod product_repository;
pub mod product_schema_adapter;
pub mod retry_manager; // 재시도 관리자
pub mod simple_http_client;
pub mod system_broadcaster; // 실시간 시스템 상태 브로드캐스터
pub mod test_db;
pub mod write_lock_tracker; // Active write transaction tracker (diagnostics)

// Re-export commonly used items (mirroring previous infrastructure.rs)
pub use config::csa_iot;
pub use database_connection::DatabaseConnection;
pub use database_paths::{DatabasePathManager, get_main_database_url, initialize_database_paths};
pub use html_parser::MatterDataExtractor;
pub use integrated_product_repository::IntegratedProductRepository;
pub use simple_http_client::HttpClient;
pub use parsing::{ParsingConfig, ParsingError, ParsingResult, ProductDetailParser, ProductListParser};
pub use crawling_service_impls::{CollectorConfig, ProductListCollectorImpl, StatusCheckerImpl};
pub use data_processing_service_impls::{ConflictResolverImpl, DeduplicationServiceImpl, ValidationServiceImpl};
pub use logging::{get_log_directory, init_logging, init_logging_with_config};
pub use retry_manager::{ErrorClassification, RetryItem, RetryManager, RetryStats};
