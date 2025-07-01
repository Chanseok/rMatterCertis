//! Domain module - Core business logic and entities
//! 
//! This module contains all domain-specific entities, value objects,
//! and domain services that represent the core business logic.
//! 
//! Modern Rust module organization (Rust 2018+ style):
//! - Each module is its own file in the domain/ directory
//! - Public exports are defined here for convenience

pub mod entities;
pub mod events;
pub mod repositories;
pub mod services;
pub mod session_manager;
pub mod product;
pub mod matter_product;
pub mod integrated_product;

// Re-export commonly used items for convenience
// Note: Be specific about re-exports to avoid ambiguous glob warnings
pub use entities::{CrawlingSession, Product};
pub use events::{
    CrawlingStage, CrawlingStatus, CrawlingEvent, CrawlingTaskStatus, 
    DatabaseStats, DatabaseHealth, CrawlingResult, PerformanceMetrics, TaskStatus,
    CrawlingProgress
};
pub use matter_product::MatterProduct;
pub use integrated_product::IntegratedProduct;
