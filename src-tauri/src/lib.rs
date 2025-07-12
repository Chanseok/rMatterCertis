//! Matter Certis v2 - E-commerce Product Crawling Application
//! 
//! This application provides web crawling capabilities for e-commerce sites
//! with a modern desktop interface built with Tauri and SolidJS.
//! 
//! Modern Rust module organization (Rust 2024+ style):
//! - Each module is defined in its own .rs file or directory
//! - No mod.rs files - clean, modern structure
//! - Direct module declarations following Rust 2024 conventions

#![allow(clippy::uninlined_format_args)]
#![allow(missing_docs)]
#![allow(clippy::unnecessary_qualification)]
#![allow(unused_must_use)]

use crate::infrastructure::{DatabaseConnection, init_logging_with_config};
use crate::infrastructure::config::{ConfigManager, AppConfig};
use tracing::{info, error, warn};
use tauri::Manager;

// Modern Rust 2024 module declarations - no mod.rs files needed
pub mod domain {
    //! Domain module - Core business logic and entities
    pub mod entities;
    pub mod events;
    pub mod repositories;
    pub mod value_objects;
    pub mod services {
        //! Domain services for business logic
        pub mod product_service;
        pub mod crawling_services;
        pub mod data_processing_services;
        
        // Re-export commonly used items
        pub use product_service::ProductService;
        pub use crawling_services::{
            StatusChecker, DatabaseAnalyzer, ProductListCollector, ProductDetailCollector,
            SiteStatus, DatabaseAnalysis, FieldAnalysis, DuplicateAnalysis, DuplicateGroup,
            DuplicateType, ProcessingStrategy
        };
        pub use data_processing_services::{
            DeduplicationService, ValidationService, ConflictResolver, BatchProgressTracker,
            BatchRecoveryService, RetryManager, ErrorClassifier, DuplicationAnalysis,
            ValidationResult, ConflictGroup, BatchProgress, RecoveryResult, ErrorClassification
        };
    }
    pub mod session_manager;
    pub mod product;
    pub mod matter_product;
    pub mod integrated_product;

    // Re-export commonly used items
    pub use entities::*;
    pub use events::*;
    pub use value_objects::*;
}

pub mod application {
    //! Application layer - Use cases and application services
    pub mod use_cases;
    pub mod crawling_use_cases;
    pub mod integrated_use_cases;
    pub mod dto;
    pub mod events;
    pub mod state;
    pub mod page_discovery_service;
    pub mod parsing_service;

    // Re-export commonly used items
    pub use events::EventEmitter;
    pub use state::AppState;
    pub use page_discovery_service::PageDiscoveryService;
}

pub mod infrastructure;

pub mod commands;

// Modern Rust 2024 - 명시적 모듈 선언
pub mod crawling;

// Test utilities (only available during testing)
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize runtime for async operations first
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    
    // Load configuration with automatic initialization on first run
    let config = rt.block_on(async {
        match ConfigManager::new() {
            Ok(manager) => {
                match manager.initialize_on_first_run().await {
                    Ok(config) => {
                        info!("✅ Configuration initialized successfully");
                        config
                    },
                    Err(e) => {
                        eprintln!("⚠️ Failed to initialize configuration, using defaults: {}", e);
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
    
    // Initialize database connection with proper data directory
    let db = rt.block_on(async {
        info!("🔧 Initializing database connection...");
        
        // Use the same data directory structure as config
        let data_dir = match crate::infrastructure::config::ConfigManager::get_app_data_dir() {
            Ok(dir) => dir.join("database"),
            Err(_) => {
                warn!("📁 Using fallback data directory");
                std::path::PathBuf::from("./data")
            }
        };
        
        if !data_dir.exists() {
            warn!("📁 Database directory does not exist, creating...");
            std::fs::create_dir_all(&data_dir).expect("Failed to create database directory");
            info!("✅ Database directory created successfully");
        }

        // Initialize database with proper path
        let database_url = format!("sqlite:{}/matter_certis.db", data_dir.to_string_lossy());
        info!("🗄️ Connecting to database: {}", database_url);
        
        let db = DatabaseConnection::new(&database_url).await
            .expect("Failed to initialize database connection");
        
        info!("🔄 Running database migrations...");
        db.migrate().await.expect("Failed to run database migrations");
        
        info!("✅ Database initialized successfully");
        db
    });

    // Create application state
    let app_state = application::AppState::new(config);
    
    info!("🔧 Building Tauri application...");
    
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(db)
        .manage(app_state)
        .manage(commands::crawling_v4::CrawlingEngineState {
            engine: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
            database: commands::crawling_v4::MockDatabase {
                connection_status: "Mock Connected".to_string(),
            },
        })
        .setup(|app| {
            let app_handle = app.handle().clone();
            
            // Initialize event emitter in background
            tauri::async_runtime::spawn(async move {
                let state: tauri::State<application::AppState> = app_handle.state();
                let emitter = application::EventEmitter::new(app_handle.clone());
                
                if let Err(e) = state.initialize_event_emitter(emitter).await {
                    error!("Failed to initialize event emitter: {}", e);
                } else {
                    info!("✅ Event emitter initialized successfully");
                }
            });
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Modern real-time commands (Legacy v3.x)
            commands::start_crawling_v3,
            commands::pause_crawling,
            commands::resume_crawling,
            commands::stop_crawling_v3,
            
            // Backend v4.0 commands
            commands::crawling_v4::init_crawling_engine,
            commands::crawling_v4::start_crawling,
            commands::crawling_v4::stop_crawling,
            commands::crawling_v4::get_crawling_stats,
            commands::crawling_v4::get_system_health,
            commands::crawling_v4::update_crawling_config,
            commands::crawling_v4::get_crawling_config,
            commands::crawling_v4::emergency_stop,
            commands::get_crawling_status,
            commands::get_active_sessions,
            commands::get_database_stats,
            commands::backup_database,
            commands::optimize_database,
            commands::export_database_data,
            commands::clear_crawling_errors,
            commands::export_crawling_results,
            commands::check_site_status,
            commands::get_retry_stats, // 재시도 통계 - INTEGRATED_PHASE2_PLAN Week 1 Day 5
            commands::update_user_crawling_preferences, // User settings management
            
            // New database commands for real data
            commands::get_products,
            commands::get_local_db_stats,
            commands::get_analysis_data,
            
            // CrawlerManager 기반 통합 크롤링 명령어들 (임시 비활성화)
            // commands::start_integrated_crawling,
            // commands::stop_integrated_crawling,
            // commands::pause_integrated_crawling,
            // commands::resume_integrated_crawling,
            // commands::get_integrated_crawling_progress,
            
            // Configuration management commands
            commands::get_frontend_config,
            commands::get_site_config,
            commands::get_default_crawling_config,
            commands::get_comprehensive_crawler_config,
            commands::initialize_app_config,
            commands::reset_config_to_defaults,
            commands::get_app_directories,
            commands::is_first_run,
            commands::update_crawling_settings,
            commands::get_crawling_status_check,
            commands::build_page_url,
            commands::resolve_url,
            
            // Frontend logging commands
            commands::write_frontend_log,
            commands::get_log_directory_path,
            commands::cleanup_logs,
            commands::update_logging_settings,
            commands::update_batch_settings,
            
            // Window state management commands
            commands::save_window_state,
            commands::load_window_state,
            commands::set_window_position,
            commands::set_window_size,
            commands::maximize_window,
            
            // Legacy parsing commands (temporarily disabled for Modern Rust 2024 migration)
            // commands::crawl_product_list_page,
            // commands::crawl_product_detail_page,
            // commands::batch_crawl_product_lists,
            // commands::batch_crawl_product_details,
            // commands::check_has_next_page,
            // commands::get_crawler_config,
            // commands::crawler_health_check,
            
            // Smart crawling commands (temporarily disabled for Modern Rust 2024 migration)
            // commands::calculate_crawling_range,
            // commands::get_crawling_progress,
            // commands::get_database_state_for_range_calculation,
            // commands::demo_prompts6_calculation
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
