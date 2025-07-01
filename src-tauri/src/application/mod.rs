//! Application layer - Use cases and application services
//! 
//! This module contains application services, use cases, and DTOs
//! that coordinate domain logic for specific application workflows.

pub mod use_cases;
pub mod crawling_use_cases;
pub mod integrated_use_cases;
pub mod dto;
pub mod events;
pub mod state;
pub mod page_discovery_service;
pub mod parsing_service;

// Re-export commonly used items
pub use events::EventEmitter;
pub use state::AppState;
