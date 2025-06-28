//! Tauri commands module
//! 
//! This module contains all Tauri commands that expose
//! backend functionality to the frontend.

use crate::infrastructure::database_connection::DatabaseConnection;

// Example command - remove this when implementing real commands
#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
pub async fn test_database_connection() -> Result<String, String> {
    println!("🔄 Starting database connection test...");
    
    // Create data directory if it doesn't exist
    let data_dir = std::path::Path::new("./data");
    if !data_dir.exists() {
        std::fs::create_dir_all(data_dir)
            .map_err(|e| format!("Failed to create data directory: {}", e))?;
    }
    
    // Use relative path for database
    let database_url = "sqlite:./data/matter_certis.db";
    println!("📊 Database URL: {}", database_url);
    
    match DatabaseConnection::new(database_url).await {
        Ok(db) => {
            println!("✅ Database connection successful!");
            match db.migrate().await {
                Ok(_) => {
                    println!("✅ Migration successful!");
                    Ok("Database connection and migration successful!".to_string())
                },
                Err(e) => {
                    println!("❌ Migration failed: {}", e);
                    Err(format!("Migration failed: {}", e))
                }
            }
        },
        Err(e) => {
            println!("❌ Database connection failed: {}", e);
            Err(format!("Database connection failed: {}", e))
        }
    }
}

#[tauri::command]
pub async fn get_database_info() -> Result<String, String> {
    // Create data directory if it doesn't exist
    let data_dir = std::path::Path::new("./data");
    if !data_dir.exists() {
        std::fs::create_dir_all(data_dir)
            .map_err(|e| format!("Failed to create data directory: {}", e))?;
    }
    
    let database_url = "sqlite:./data/matter_certis.db";
    
    match DatabaseConnection::new(database_url).await {
        Ok(db) => {
            match sqlx::query("SELECT name FROM sqlite_master WHERE type='table'")
                .fetch_all(db.pool())
                .await 
            {
                Ok(tables) => {
                    let table_names: Vec<String> = tables.iter()
                        .map(|row| {
                            use sqlx::Row;
                            row.get::<String, _>("name")
                        })
                        .collect();
                    Ok(format!("Available tables: {}", table_names.join(", ")))
                },
                Err(e) => Err(format!("Query failed: {}", e)),
            }
        },
        Err(e) => Err(format!("Database connection failed: {}", e)),
    }
}
