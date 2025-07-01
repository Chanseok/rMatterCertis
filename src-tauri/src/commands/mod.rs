//! Modern command module for real-time crawling operations
//! 
//! This module contains all Tauri commands that support real-time
//! event emission and modern state management patterns.

pub mod modern_crawling;
pub mod config_commands;

// Re-export all commands
pub use modern_crawling::*;
pub use config_commands::*;
