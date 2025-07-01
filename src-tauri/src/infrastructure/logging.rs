//! Logging system configuration and initialization
//! 
//! This module provides a comprehensive logging setup with:
//! - File logging with automatic rotation
//! - Configuration file based log level control
//! - Structured JSON logging (optional)
//! - Console and file output support
//! - Log files stored relative to executable location

#![allow(clippy::uninlined_format_args)]
#![allow(clippy::needless_borrows_for_generic_args)]

use anyhow::{Result, anyhow};
use std::path::PathBuf;
use tracing::info;
use tracing_appender::{non_blocking, rolling};
use tracing_subscriber::{
    layer::SubscriberExt,
    util::SubscriberInitExt,
    fmt,
    EnvFilter,
    Registry,
};

// Re-export LoggingConfig from config module
pub use crate::infrastructure::config::LoggingConfig;

/// Get the log directory relative to the executable location
pub fn get_log_directory() -> PathBuf {
    // Get the directory where the executable is located
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    
    exe_dir.join("logs")
}

/// Initialize the logging system with default configuration
pub fn init_logging() -> Result<()> {
    let config = LoggingConfig::default();
    init_logging_with_config(config)
}

/// Initialize logging with custom configuration
pub fn init_logging_with_config(config: LoggingConfig) -> Result<()> {
    let log_dir = get_log_directory();
    
    // Create log directory if it doesn't exist
    std::fs::create_dir_all(&log_dir)
        .map_err(|e| anyhow!("Failed to create log directory {:?}: {}", log_dir, e))?;

    // Set up environment filter
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.level));

    // Build the subscriber registry
    let registry = Registry::default().with(env_filter);

    // Handle different combinations of output types
    match (config.file_output, config.console_output) {
        (true, true) => {
            // Both file and console output
            let app_name = "matter-certis-v2";
            let file_appender = rolling::daily(&log_dir, &format!("{}.log", app_name));
            let (file_writer, _guard) = non_blocking(file_appender);
            
            if config.json_format {
                let file_layer = fmt::Layer::new()
                    .json()
                    .with_writer(file_writer)
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_file(true)
                    .with_line_number(true);
                let console_layer = fmt::Layer::new()
                    .with_writer(std::io::stdout)
                    .with_target(false);
                
                registry.with(file_layer).with(console_layer).init();
            } else {
                let file_layer = fmt::Layer::new()
                    .with_writer(file_writer)
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_file(true)
                    .with_line_number(true);
                let console_layer = fmt::Layer::new()
                    .with_writer(std::io::stdout)
                    .with_target(false);
                
                registry.with(file_layer).with(console_layer).init();
            }
        },
        (true, false) => {
            // File output only
            let app_name = "matter-certis-v2";
            let file_appender = rolling::daily(&log_dir, &format!("{}.log", app_name));
            let (file_writer, _guard) = non_blocking(file_appender);
            
            if config.json_format {
                let file_layer = fmt::Layer::new()
                    .json()
                    .with_writer(file_writer)
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_file(true)
                    .with_line_number(true);
                
                registry.with(file_layer).init();
            } else {
                let file_layer = fmt::Layer::new()
                    .with_writer(file_writer)
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_file(true)
                    .with_line_number(true);
                
                registry.with(file_layer).init();
            }
        },
        (false, true) => {
            // Console output only
            let console_layer = fmt::Layer::new()
                .with_writer(std::io::stdout)
                .with_target(false);
            
            registry.with(console_layer).init();
        },
        (false, false) => {
            return Err(anyhow!("No logging output configured"));
        }
    }

    info!("Logging system initialized");
    info!("Log directory: {:?}", log_dir);
    info!("Log level: {}", config.level);
    info!("JSON format: {}", config.json_format);
    info!("Console output: {}", config.console_output);
    info!("File output: {}", config.file_output);

    Ok(())
}

/// Log system information for diagnostics
pub fn log_system_info() {
    info!("=== Matter Certis v2 System Information ===");
    info!("Application version: {}", env!("CARGO_PKG_VERSION"));
    info!("Operating system: {}", std::env::consts::OS);
    info!("Architecture: {}", std::env::consts::ARCH);
    
    if let Ok(current_dir) = std::env::current_dir() {
        info!("Working directory: {:?}", current_dir);
    }
    
    info!("Log directory: {:?}", get_log_directory());
    info!("============================================");
}

#[cfg(test)]
mod tests {
    use super::*;
    

    #[test]
    fn test_logging_config_default() {
        let config = LoggingConfig::default();
        assert!(!config.level.is_empty());
        assert!(config.console_output);
        assert!(config.file_output);
    }

    #[test]
    fn test_log_directory_creation() {
        let log_dir = get_log_directory();
        
        // The log directory should be deterministic
        assert!(log_dir.to_string_lossy().ends_with("logs"));
    }
}
