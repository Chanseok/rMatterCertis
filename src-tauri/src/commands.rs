//! Modern Tauri commands for real-time crawling operations
//! 
//! This module contains all Tauri commands that support real-time
//! event emission and modern state management patterns.
//! 
//! Modern Rust module organization (Rust 2018+ style):
//! - Each command module is its own file in the commands/ directory
//! - Public exports are defined here for convenience

pub mod modern_crawling;
pub mod parsing_commands;

// Re-export all commands for easy access
pub use modern_crawling::*;
pub use parsing_commands::*;
