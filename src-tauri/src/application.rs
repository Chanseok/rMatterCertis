//! Application layer module
//! 
//! This module contains use cases, services, and data transfer objects
//! that orchestrate the domain logic for Matter Certification following the guide's architecture.

pub mod dto;
pub mod use_cases;
pub mod integrated_use_cases;
pub mod crawling_use_cases;
pub mod parsing_service;  // Modern parsing service following the guide
pub mod page_discovery_service;  // Page discovery service for finding last page

// Re-export for easier access
pub use dto::*;
pub use integrated_use_cases::IntegratedProductUseCases;
pub use crawling_use_cases::{CrawlingUseCases, CrawlingConfig};
pub use parsing_service::{ParsingService, CrawlerService};
pub use page_discovery_service::PageDiscoveryService;
