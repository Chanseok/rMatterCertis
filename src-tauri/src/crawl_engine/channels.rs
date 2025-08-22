// Rust 2024 gate file for `crawl_engine::channels`
// Replaces directory-level mod.rs and pins submodules explicitly.

#[path = "channels/types.rs"]
pub mod types;

// Maintain prior public surface: re-export common channel types and aliases
pub use types::*;
