//! Matter Certis v2 - E-commerce Product Crawling Application
//! 
//! This application provides web crawling capabilities for e-commerce sites
//! with a modern desktop interface built with Tauri and SolidJS.

// Module declarations
pub mod domain;
pub mod application;
pub mod infrastructure;
pub mod commands;

// Re-export commands for easier access
pub use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            test_database_connection,
            get_database_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
