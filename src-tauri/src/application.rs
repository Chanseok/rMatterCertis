//! Application layer - Use cases and application services
//! 
//! This module contains application services, use cases, and DTOs
//! that coordinate domain logic for specific application workflows.
//! 
//! Modern Rust module organization (Rust 2018+ style):
//! - Each module is its own file in the application/ directory
//! - Public exports are defined here for convenience

pub mod use_cases;
pub mod crawling_use_cases;
pub mod integrated_use_cases;
pub mod dto;
pub mod events;
pub mod state;
pub mod page_discovery_service;
pub mod parsing_service;
pub mod crawler_manager;  // 새로 추가된 통합 크롤러 매니저

// Re-export commonly used items for convenience
pub use events::EventEmitter;
pub use state::AppState;
pub use dto::*;
pub use integrated_use_cases::IntegratedProductUseCases;
pub use crawling_use_cases::CrawlingUseCases;
pub use parsing_service::{ParsingService, CrawlerService};
pub use page_discovery_service::PageDiscoveryService;

// Re-export crawler manager types
pub use crawler_manager::{
    CrawlerManager, 
    BatchProcessor, 
    CrawlingConfig, 
    CrawlingEngineType,
    TaskResult,
    RetryManager,
    PerformanceMonitor
};
