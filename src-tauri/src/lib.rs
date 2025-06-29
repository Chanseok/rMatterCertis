//! Matter Certis v2 - E-commerce Product Crawling Application
//! 
//! This application provides web crawling capabilities for e-commerce sites
//! with a modern desktop interface built with Tauri and SolidJS.

use crate::infrastructure::database_connection::DatabaseConnection;

// Module declarations
pub mod domain;
pub mod application; // Restored - previously disabled due to import conflicts
pub mod infrastructure; // Partially enabled for DatabaseConnection
// pub mod commands; // Temporarily disabled due to import conflicts
pub mod commands_simple;
pub mod commands_integrated; // Restored - previously disabled due to import conflicts

// Test utilities (only available during testing)
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

// Import command functions
// use commands_simple::{greet, test_database_connection, get_database_info};
// use commands_integrated::{
//     get_integrated_database_statistics,
//     search_integrated_products_simple,
//     get_integrated_products_without_details,
//     validate_integrated_database_integrity
// };

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize runtime for async operations
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    
    // Initialize database connection
    let db = rt.block_on(async {
        // Create data directory if it doesn't exist
        let data_dir = std::path::Path::new("./data");
        if !data_dir.exists() {
            std::fs::create_dir_all(data_dir).expect("Failed to create data directory");
        }

        // Initialize database with migrations
        let database_url = "sqlite:./data/matter_certis.db";
        let db = DatabaseConnection::new(database_url).await
            .expect("Failed to initialize database connection");
        
        db.migrate().await.expect("Failed to run database migrations");
        
        println!("✅ Database initialized successfully");
        db
    });

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(db)
        .invoke_handler(tauri::generate_handler![
            commands_simple::greet,
            commands_simple::test_database_connection,
            commands_simple::get_database_info,
            commands_integrated::get_integrated_database_statistics,
            commands_integrated::search_integrated_products_simple,
            commands_integrated::get_integrated_products_without_details,
            commands_integrated::validate_integrated_database_integrity
        ]);
    
    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
