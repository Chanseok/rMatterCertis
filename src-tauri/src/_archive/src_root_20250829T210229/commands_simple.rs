//! Simplified Tauri commands for integrated schema testing
//! 
//! This module contains minimal commands for testing the integrated schema

use crate::infrastructure::database_connection::DatabaseConnection;
use crate::infrastructure::database_paths::get_main_database_url;

// ============================================================================
// Database Management Commands
// ============================================================================

#[tauri::command(async)]
pub async fn test_database_connection() -> Result<String, String> {
    println!("🔄 Starting database connection test...");
    
    // Create data directory if it doesn't exist
    let data_dir = std::path::Path::new("./data");
    if !data_dir.exists() {
        std::fs::create_dir_all(data_dir)
            .map_err(|e| format!("Failed to create data directory: {e}"))?;
    }
    
    // Use centralized database path management
    let database_url = get_main_database_url();
    println!("📊 Database URL: {database_url}");
    
    match DatabaseConnection::new(&database_url).await {
        Ok(db) => {
            println!("✅ Database connection successful!");
            match db.migrate().await {
                Ok(_) => {
                    println!("✅ Migration successful!");
                    Ok("Database connection and migration successful!".to_string())
                },
                Err(e) => {
                    println!("❌ Migration failed: {e}");
                    Err(format!("Migration failed: {e}"))
                }
            }
        },
        Err(e) => {
            println!("❌ Database connection failed: {e}");
            Err(format!("Database connection failed: {e}"))
        }
    }
}

#[tauri::command(async)]
pub async fn get_database_info() -> Result<String, String> {
    let database_url = get_main_database_url();
    let location = database_url.strip_prefix("sqlite:").unwrap_or(&database_url);
    let info = format!("Database: SQLite\nLocation: {}\nStatus: Available\nSchema: Integrated v3", location);
    
    Ok(info)
}

// ============================================================================
// Legacy/Example Commands
// ============================================================================

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust with integrated schema!", name)
}
