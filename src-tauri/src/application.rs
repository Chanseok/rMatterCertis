//! Application layer - Use cases and application services
//! 
//! This module contains application services, use cases, and DTOs
//! that coordinate domain logic for specific application workflows.
//! 
//! Modern Rust module organization (Rust 2018+ style):
//! - Each module is its own file in the application/ directory
//! - Public exports are defined here for convenience

pub mod integrated_use_cases;
pub mod dto;
pub mod events;
pub mod state;
pub mod shared_state;  // 새로 추가된 공유 상태 관리
pub mod crawling_profile;  // 크롤링 프로필 정의
pub mod parsing_service;

// Re-export commonly used items for convenience
pub use shared_state::{SharedState, SharedStateCache, create_shared_state};
pub use crawling_profile::{CrawlingProfile, CrawlingRequest};
pub use events::EventEmitter;
pub use state::AppState;
pub use dto::*;
pub use integrated_use_cases::IntegratedProductUseCases;
pub use parsing_service::{ParsingService, CrawlerService};
