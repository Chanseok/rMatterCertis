#![cfg(any())]
// ARCHIVED: superseded by `src-tauri/src/crawl_engine/actors.rs` gate file.

// Rust 2024 gate file for `crawl_engine::actors`
// This replaces the need for a directory-level mod.rs.

pub mod batch_actor;
pub mod contract;
// pub mod details; // removed
pub mod session_actor;
pub mod stage_actor;
pub mod traits;
pub mod types;

// Re-exports matching prior public surface
pub use batch_actor::{BatchActor, BatchError};
pub use contract::ACTOR_CONTRACT_VERSION;
pub use session_actor::{SessionActor, SessionError};
pub use stage_actor::StageActor;
pub use traits::*;
pub use types::{
    ActorCommand, ActorError, BatchConfig, CrawlingConfig, StageItem, StageItemResult,
    StageItemType, StageResult, StageType,
};
