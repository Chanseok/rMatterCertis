//! Matter Certis v2 - E-commerce Product Crawling Application
//! 
//! This application provides web crawling capabilities for e-commerce sites
//! with a modern desktop interface built with Tauri and SolidJS.

use crate::infrastructure::{DatabaseConnection, init_logging_with_config};
use crate::infrastructure::config::{ConfigManager, AppConfig};
use tracing::{info, error, warn};

pub mod domain;
pub mod application;
pub mod infrastructure;
pub mod commands; // Modern commands following the guide

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
    // Initialize runtime for async operations first
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    
    // Load configuration (with fallback to defaults)
    let config = rt.block_on(async {
        match ConfigManager::new() {
            Ok(manager) => {
                match manager.load_config().await {
                    Ok(config) => config,
                    Err(e) => {
                        eprintln!("⚠️ Failed to load configuration, using defaults: {}", e);
                        AppConfig::default()
                    }
                }
            },
            Err(e) => {
                eprintln!("⚠️ Failed to create config manager, using defaults: {}", e);
                AppConfig::default()
            }
        }
    });
    
    // Initialize logging system with config-based settings
    if let Err(e) = init_logging_with_config(config.user.logging.clone()) {
        eprintln!("❌ Failed to initialize logging system: {}", e);
        std::process::exit(1);
    }
    
    info!("🚀 Starting Matter Certis v2 application");
    info!("📋 Configuration loaded successfully");
    
    // Initialize runtime for async operations (already created above)
    info!("✅ Tokio runtime initialized successfully");
    
    // Initialize database connection
    let db = rt.block_on(async {
        info!("🔧 Initializing database connection...");
        
        // Create data directory if it doesn't exist
        let data_dir = std::path::Path::new("./data");
        if !data_dir.exists() {
            warn!("📁 Data directory does not exist, creating...");
            std::fs::create_dir_all(data_dir).expect("Failed to create data directory");
            info!("✅ Data directory created successfully");
        }

        // Initialize database with migrations
        let database_url = "sqlite:./data/matter_certis.db";
        info!("🗄️ Connecting to database: {}", database_url);
        
        let db = DatabaseConnection::new(database_url).await
            .expect("Failed to initialize database connection");
        
        info!("🔄 Running database migrations...");
        db.migrate().await.expect("Failed to run database migrations");
        
        info!("✅ Database initialized successfully");
        db
    });

    info!("🔧 Building Tauri application...");
    
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(db)
        .invoke_handler(tauri::generate_handler![
            // Modern parsing commands following the guide
            commands::crawl_product_list_page,
            commands::crawl_product_detail_page,
            commands::batch_crawl_product_lists,
            commands::batch_crawl_product_details,
            commands::check_has_next_page,
            commands::get_crawler_config,
            commands::crawler_health_check
        ]);
    
    info!("✅ Tauri application built successfully, starting...");
    
    builder
        .run(tauri::generate_context!())
        .map_err(|e| {
            error!("❌ Failed to run Tauri application: {}", e);
            e
        })
        .expect("error while running tauri application");
    
    info!("👋 Matter Certis v2 application ended");
}
