//! Domain services
//! 
//! Contains business logic that doesn't naturally fit within entities.

// Re-export from services directory
pub use crate::domain::services::product_service::ProductService;
pub use crate::domain::services::crawling_services::{
    StatusChecker, DatabaseAnalyzer, ProductListCollector, ProductDetailCollector,
    SiteStatus, DatabaseAnalysis, FieldAnalysis, DuplicateAnalysis, DuplicateGroup, 
    DuplicateType, ProcessingStrategy
};
