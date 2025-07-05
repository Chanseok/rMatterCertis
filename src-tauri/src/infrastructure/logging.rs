//! Logging system configuration and initialization
//! 
//! This module provides a comprehensive logging setup with:
//! - File logging with automatic rotation
//! - Configuration file based log level control
//! - Structured JSON logging (optional)
//! - Console and file output support
//! - Log files stored relative to executable location
//! - KST (Korea Standard Time) timezone support
//! - Separate backend and frontend log files

#![allow(clippy::uninlined_format_args)]
#![allow(clippy::needless_borrows_for_generic_args)]

use anyhow::{Result, anyhow};
use std::path::PathBuf;
use tracing::{info, warn};
use tracing_appender::{non_blocking, rolling};
use tracing_subscriber::{
    layer::SubscriberExt,
    util::SubscriberInitExt,
    fmt::{self, time::FormatTime},
    EnvFilter,
    Registry,
};
use chrono::{Utc, FixedOffset};
use lazy_static::lazy_static;
use std::sync::Mutex;

// Re-export LoggingConfig from config module
pub use crate::infrastructure::config::LoggingConfig;

// Global guard to keep the log file writer alive
lazy_static! {
    static ref LOG_GUARDS: Mutex<Vec<tracing_appender::non_blocking::WorkerGuard>> = Mutex::new(Vec::new());
}

/// Custom time formatter for KST (Korea Standard Time, UTC+9)
struct KstTimeFormatter;

impl FormatTime for KstTimeFormatter {
    fn format_time(&self, w: &mut fmt::format::Writer<'_>) -> std::fmt::Result {
        let now = Utc::now();
        let kst_offset = FixedOffset::east_opt(9 * 3600).unwrap(); // UTC+9
        let kst_time = now.with_timezone(&kst_offset);
        write!(w, "{}", kst_time.format("%Y-%m-%d %H:%M:%S%.3f %Z"))
    }
}

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

/// Rotate existing log file by renaming it with timestamp
fn rotate_existing_log_file(log_dir: &PathBuf, log_file_name: &str) -> Result<()> {
    let log_file_path = log_dir.join(log_file_name);
    
    // Check if the log file exists
    if log_file_path.exists() {
        // Get the creation time or modification time of the existing log file
        let metadata = std::fs::metadata(&log_file_path)
            .map_err(|e| anyhow!("Failed to get log file metadata: {}", e))?;
        
        let file_time = metadata.created()
            .or_else(|_| metadata.modified())
            .unwrap_or_else(|_| std::time::SystemTime::now());
        
        // Convert to KST datetime
        let datetime: chrono::DateTime<chrono::Utc> = file_time.into();
        let kst_datetime = datetime.with_timezone(&FixedOffset::east_opt(9 * 3600).unwrap());
        
        // Create new filename with timestamp (Seoul time format: YYYY-MMDDTHH:MM:SS)
        let file_stem = log_file_name.trim_end_matches(".log");
        let timestamped_name = format!("{}.{}.log", file_stem, kst_datetime.format("%Y%m%dT%H:%M:%S"));
        let timestamped_path = log_dir.join(&timestamped_name);
        
        // Rename the existing log file
        std::fs::rename(&log_file_path, &timestamped_path)
            .map_err(|e| anyhow!("Failed to rotate log file {} to {}: {}", 
                log_file_path.display(), timestamped_path.display(), e))?;
        
        info!("Rotated existing log file to: {}", timestamped_name);
    }
    
    Ok(())
}

/// Rotate all existing log files (including legacy ones) by renaming them with timestamp
fn rotate_all_existing_log_files(log_dir: &PathBuf) -> Result<()> {
    if !log_dir.exists() {
        return Ok(());
    }

    // List of possible log files to rotate
    let potential_log_files = vec![
        "matter-certis-v2.log",
        "back_front.log",
        "back.log",
        "front.log",
        "frontend.log",
        "backend.log"
    ];

    for log_file_name in potential_log_files {
        let log_file_path = log_dir.join(log_file_name);
        if log_file_path.exists() {
            rotate_existing_log_file(log_dir, log_file_name)?;
        }
    }

    Ok(())
}

/// Initialize logging with custom configuration
/// 
/// This function sets up optimized logging filters to reduce verbose output from dependencies.
/// 
/// # Log Level Optimization
/// - When level != "trace": SQL queries, HTTP details, and framework internals are suppressed
/// - When level == "trace": All logs including verbose dependencies are shown
/// 
/// # Environment Variable Override
/// You can override the filtering using RUST_LOG environment variable:
/// ```bash
/// # Show all SQL queries even on DEBUG level
/// RUST_LOG="debug,sqlx::query=debug" cargo run
/// 
/// # Show only errors from all dependencies
/// RUST_LOG="info,sqlx=error,reqwest=error,tokio=error" cargo run
/// 
/// # Show detailed HTTP logs
/// RUST_LOG="debug,reqwest=debug,hyper=debug" cargo run
/// ```
/// 
/// # Optimized Targets (suppressed unless TRACE):
/// - `sqlx::query`: Database query execution details
/// - `sqlx::migrate`: Database migration logs  
/// - `reqwest`: HTTP client request/response details
/// - `tokio`: Async runtime task scheduling
/// - `tauri`: Desktop framework internals
pub fn init_logging_with_config(config: LoggingConfig) -> Result<()> {
    let log_dir = get_log_directory();
    
    // Create log directory if it doesn't exist
    std::fs::create_dir_all(&log_dir)
        .map_err(|e| anyhow!("Failed to create log directory {:?}: {}", log_dir, e))?;

    // Determine log file name based on configuration
    let log_file_name = match config.file_naming_strategy.as_str() {
        "separated" => {
            if config.separate_frontend_backend {
                "back.log" // Backend log file, frontend will have its own file
            } else {
                "back_front.log" // Unified backend + frontend log
            }
        },
        "timestamped" => {
            let now = chrono::Utc::now().with_timezone(&FixedOffset::east_opt(9 * 3600).unwrap());
            if config.separate_frontend_backend {
                &format!("back-{}.log", now.format("%Y%m%d"))
            } else {
                &format!("back_front-{}.log", now.format("%Y%m%d"))
            }
        },
        _ => {
            if config.separate_frontend_backend {
                "back.log" // Backend log file
            } else {
                "back_front.log" // Default unified log
            }
        }
    };

    // Rotate all existing log files before creating new ones
    rotate_all_existing_log_files(&log_dir)?;

    // Perform log cleanup if enabled
    if config.auto_cleanup_logs {
        cleanup_old_logs(&log_dir, &config)?;
    }

    // Set up environment filter with optimized SQL log filtering
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            // Create base filter with application log level
            let mut filter = EnvFilter::new(&config.level);
            
            // Suppress verbose SQL and database logs unless TRACE level is specifically requested
            if !config.level.to_lowercase().contains("trace") {
                filter = filter
                    // SQLx query logs (migrations, prepared statements) - only show on TRACE
                    .add_directive("sqlx::query=warn".parse().unwrap())
                    .add_directive("sqlx::migrate=info".parse().unwrap())
                    .add_directive("sqlx::postgres=warn".parse().unwrap())
                    .add_directive("sqlx::sqlite=warn".parse().unwrap())
                    
                    // HTTP client detailed logs - only show on TRACE
                    .add_directive("reqwest=info".parse().unwrap())
                    .add_directive("hyper=warn".parse().unwrap())
                    .add_directive("h2=warn".parse().unwrap())
                    
                    // Tokio runtime details - only show on TRACE
                    .add_directive("tokio=info".parse().unwrap())
                    .add_directive("runtime=warn".parse().unwrap())
                    
                    // Tauri internals - only show on TRACE
                    .add_directive("tauri=info".parse().unwrap())
                    .add_directive("wry=warn".parse().unwrap())
                    
                    // Keep our application logs at the requested level
                    .add_directive(format!("matter_certis_v2={}", config.level).parse().unwrap());
            }
            
            filter
        });

    // Build the subscriber registry
    let registry = Registry::default().with(env_filter);

    // Determine log file name based on configuration
    let log_file_name = match config.file_naming_strategy.as_str() {
        "separated" => {
            if config.separate_frontend_backend {
                "back.log" // Backend log file, frontend will have its own file
            } else {
                "back_front.log" // Unified backend + frontend log
            }
        },
        "timestamped" => {
            let now = chrono::Utc::now().with_timezone(&FixedOffset::east_opt(9 * 3600).unwrap());
            if config.separate_frontend_backend {
                &format!("back-{}.log", now.format("%Y%m%d"))
            } else {
                &format!("back_front-{}.log", now.format("%Y%m%d"))
            }
        },
        _ => {
            if config.separate_frontend_backend {
                "back.log" // Backend log file
            } else {
                "back_front.log" // Default unified log
            }
        }
    };

    // Handle different combinations of output types
    match (config.file_output, config.console_output) {
        (true, true) => {
            // Both file and console output
            let file_appender = rolling::never(&log_dir, log_file_name);
            let (file_writer, file_guard) = non_blocking(file_appender);
            
            // Store the guard globally to prevent it from being dropped
            LOG_GUARDS.lock().unwrap().push(file_guard);
            
            if config.json_format {
                let file_layer = fmt::Layer::new()
                    .json()
                    .with_writer(file_writer)
                    .with_timer(KstTimeFormatter)
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_ansi(false);       // No ANSI color codes for file output
                let console_layer = fmt::Layer::new()
                    .with_writer(std::io::stdout)
                    .with_timer(KstTimeFormatter)
                    .with_target(false);
                
                registry.with(file_layer).with(console_layer).init();
            } else {
                // File layer with minimal formatting (time + level + message only)
                let file_layer = fmt::Layer::new()
                    .with_writer(file_writer)
                    .with_timer(KstTimeFormatter)
                    .with_target(false)      // No target module info
                    .with_thread_ids(false) // No thread IDs
                    .with_file(false)       // No file names
                    .with_line_number(false) // No line numbers
                    .with_ansi(false);       // No ANSI color codes for file output
                // Console layer with detailed info for development
                let console_layer = fmt::Layer::new()
                    .with_writer(std::io::stdout)
                    .with_timer(KstTimeFormatter)
                    .with_target(false);
                
                registry.with(file_layer).with(console_layer).init();
            }
        },
        (true, false) => {
            // File output only
            let file_appender = rolling::never(&log_dir, log_file_name);
            let (file_writer, file_guard) = non_blocking(file_appender);
            
            // Store the guard globally to prevent it from being dropped
            LOG_GUARDS.lock().unwrap().push(file_guard);
            
            if config.json_format {
                let file_layer = fmt::Layer::new()
                    .json()
                    .with_writer(file_writer)
                    .with_timer(KstTimeFormatter)
                    .with_target(true)
                    .with_thread_ids(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_ansi(false);       // No ANSI color codes for file output
                
                registry.with(file_layer).init();
            } else {
                // File-only layer with minimal formatting (time + level + message only)
                let file_layer = fmt::Layer::new()
                    .with_writer(file_writer)
                    .with_timer(KstTimeFormatter)
                    .with_target(false)      // No target module info
                    .with_thread_ids(false) // No thread IDs
                    .with_file(false)       // No file names
                    .with_line_number(false) // No line numbers
                    .with_ansi(false);       // No ANSI color codes for file output
                
                registry.with(file_layer).init();
            }
        },
        (false, true) => {
            // Console output only with KST time
            let console_layer = fmt::Layer::new()
                .with_writer(std::io::stdout)
                .with_timer(KstTimeFormatter)
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
    
    // Log filter optimization info
    if !config.level.to_lowercase().contains("trace") {
        info!("SQL and verbose logs suppressed (use TRACE level to see all logs)");
        info!("Optimized filters: sqlx=warn, reqwest=info, tokio=info, tauri=info");
    } else {
        info!("TRACE level active - all logs including SQL queries will be shown");
    }
    info!("File output: {}", config.file_output);
    info!("Separate frontend/backend logs: {}", config.separate_frontend_backend);
    info!("File naming strategy: {}", config.file_naming_strategy);
    info!("Auto cleanup: {}", config.auto_cleanup_logs);
    info!("Keep only latest: {}", config.keep_only_latest);

    // Handle frontend logging setup
    if config.file_output {
        setup_frontend_logging(&log_dir, &config)?;
    }

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

/// Setup frontend logging based on configuration
fn setup_frontend_logging(log_dir: &PathBuf, config: &LoggingConfig) -> Result<()> {
    if config.separate_frontend_backend {
        // Only create separate frontend log file if we're using separate logs
        let frontend_log_path = log_dir.join("front.log");
        
        if !frontend_log_path.exists() {
            let now = chrono::Utc::now().with_timezone(&FixedOffset::east_opt(9 * 3600).unwrap());
            let initial_content = format!(
                "{} [INFO] Frontend log file initialized - Matter Certis v2\n",
                now.format("%Y-%m-%d %H:%M:%S%.3f %Z")
            );
            std::fs::write(&frontend_log_path, initial_content)
                .map_err(|e| anyhow!("Failed to create frontend log file: {}", e))?;
        }
        
        info!("Frontend logging configured: {:?}", frontend_log_path);
    } else {
        // For unified logs, frontend writes to the same file as backend
        let unified_log_path = match config.file_naming_strategy.as_str() {
            "timestamped" => {
                let now = chrono::Utc::now().with_timezone(&FixedOffset::east_opt(9 * 3600).unwrap());
                log_dir.join(format!("back_front-{}.log", now.format("%Y%m%d")))
            },
            _ => log_dir.join("back_front.log"),
        };
        
        info!("Frontend logging configured (unified): {:?}", unified_log_path);
    }
    
    Ok(())
}

/// Clean up old log files based on configuration
fn cleanup_old_logs(log_dir: &PathBuf, config: &LoggingConfig) -> Result<()> {
    if !log_dir.exists() {
        return Ok(());
    }
    
    let mut log_files = Vec::new();
    
    // Read all log files in the directory
    for entry in std::fs::read_dir(log_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_file() {
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if filename.ends_with(".log") {
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            log_files.push((path, modified));
                        }
                    }
                }
            }
        }
    }
    
    // Sort by modification time (newest first)
    log_files.sort_by(|a, b| b.1.cmp(&a.1));
    
    info!("Found {} log files", log_files.len());
    
    // Handle keep_only_latest option
    if config.keep_only_latest && log_files.len() > 1 {
        info!("Keeping only the latest log file");
        for (path, _) in log_files.iter().skip(1) {
            if let Err(e) = std::fs::remove_file(path) {
                warn!("Failed to remove old log file {:?}: {}", path, e);
            } else {
                info!("Removed old log file: {:?}", path);
            }
        }
        return Ok(());
    }
    
    // Handle max_files limit
    if log_files.len() > config.max_files as usize {
        let files_to_remove = log_files.len() - config.max_files as usize;
        info!("Removing {} old log files (keeping {})", files_to_remove, config.max_files);
        
        for (path, _) in log_files.iter().skip(config.max_files as usize) {
            if let Err(e) = std::fs::remove_file(path) {
                warn!("Failed to remove old log file {:?}: {}", path, e);
            } else {
                info!("Removed old log file: {:?}", path);
            }
        }
    }
    
    Ok(())
}

/// Manually clean up all log files except the latest one
pub fn cleanup_logs_keep_latest() -> Result<String> {
    let log_dir = get_log_directory();
    
    if !log_dir.exists() {
        return Ok("Log directory does not exist".to_string());
    }
    
    let mut log_files = Vec::new();
    let mut removed_count = 0;
    
    // Read all log files in the directory
    for entry in std::fs::read_dir(&log_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_file() {
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if filename.ends_with(".log") {
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            log_files.push((path, modified));
                        }
                    }
                }
            }
        }
    }
    
    if log_files.is_empty() {
        return Ok("No log files found".to_string());
    }
    
    // Sort by modification time (newest first)
    log_files.sort_by(|a, b| b.1.cmp(&a.1));
    
    // Remove all but the newest file
    for (path, _) in log_files.iter().skip(1) {
        if let Err(e) = std::fs::remove_file(path) {
            warn!("Failed to remove log file {:?}: {}", path, e);
        } else {
            info!("Removed log file: {:?}", path);
            removed_count += 1;
        }
    }
    
    Ok(format!(
        "Cleanup completed. Removed {} log files, kept 1 latest file: {:?}", 
        removed_count, 
        log_files.first().map(|(p, _)| p.file_name().unwrap_or_default()).unwrap_or_default()
    ))
}
