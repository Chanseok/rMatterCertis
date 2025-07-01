//! Domain module - Core business logic and entities
//! 
//! This module contains all domain-specific entities, value objects,
//! and domain services that represent the core business logic.

pub mod entities;
pub mod events;
pub mod repositories;
pub mod services;
pub mod session_manager;
pub mod product;
pub mod matter_product;
pub mod integrated_product;

// Re-export commonly used items
pub use entities::*;
pub use events::*;
