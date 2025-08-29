#![cfg(any())]
// ARCHIVED: Replaced by `crawl_engine::channels` gate file (channels.rs) in Rust 2024 layout.
// The previous contents are preserved for reference but excluded from the build.

// Channels module - Phase 2 legacy compatibility bridge
// 삼중 채널 시스템 (Control/Data/Event)

// pub mod channel_types;  // 중복 타입 - types.rs로 통합됨
pub mod types;

// Re-export for backward compatibility - 중복 제거
pub use types::*;
