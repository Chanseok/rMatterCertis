//! Matter Certis v2 - E-commerce Product Crawling Application
//! 
//! This application provides web crawling capabilities for e-commerce sites
//! with a modern desktop interface built with Tauri and SolidJS.

use crate::infrastructure::database_connection::DatabaseConnection;

// Module declarations
pub mod domain;
pub mod application;
pub mod infrastructure;
pub mod commands;

// Re-export commands for easier access
pub use commands::*;

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

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(db)  // Add database as managed state
        .invoke_handler(tauri::generate_handler![
            // Legacy/Example commands
            greet,
            
            // Database management
            test_database_connection,
            get_database_info,
            get_database_summary,
            
            // Vendor management
            create_vendor,
            get_all_vendors,
            get_vendor_by_id,
            search_vendors_by_name,
            update_vendor,
            delete_vendor,
            
            // Matter product management
            create_product,
            create_matter_product,
            search_matter_products,
            filter_matter_products,
            delete_product
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
