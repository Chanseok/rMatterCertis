//! Matter Certis v2 - E-commerce Product Crawling Application
//! 
//! This application provides web crawling capabilities for e-commerce sites
//! with a modern desktop interface built with Tauri and SolidJS.
//! 
//! Modern Rust module organization (Rust 2024+ style):
//! - Each module is defined in its own .rs file or directory
//! - No mod.rs files - clean, modern structure
//! - Direct module declarations following Rust 2024 conventions

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::module_name_repetitions)] // 모듈명 중복은 도메인 명확성을 위해 허용
#![allow(clippy::similar_names)] // 유사한 변수명은 의미적 연관성이 있는 경우 허용
#![allow(clippy::unused_variables)] // 개발 중 임시 변수들 허용

#![allow(clippy::uninlined_format_args)]
#![allow(missing_docs)]
#![allow(clippy::unnecessary_operation)]
#![allow(unused_must_use)]

use crate::infrastructure::{DatabaseConnection, init_logging_with_config};
use crate::infrastructure::config::{ConfigManager, AppConfig};
use tracing::{info, error, warn};
use tauri::Manager;
use std::sync::{Arc, RwLock};

// Modern Rust 2024 module declarations - no mod.rs files needed

// 🎯 TypeScript 연동 타입 (ts-rs 기반)
pub mod types {
    //! TypeScript 연동을 위한 타입 정의
    pub mod frontend_api;
}

// 🚀 새로운 아키텍처 모듈 (Phase 1 구현 완료) - Modern Rust 2024
pub mod new_architecture {
    //! Modern architecture patterns and implementations
    pub mod actor_system;
    pub mod channel_types;
    pub mod system_config;
    pub mod retry_calculator;
    pub mod integrated_context;  // Phase 3: 통합 채널 컨텍스트 추가
    pub mod task_actor;          // Phase 3: TaskActor 계층 추가
    pub mod resilience_result;   // Phase 3: 회복탄력성 시스템 추가
    pub mod events;              // TaskLifecycleEvent 시스템
    
    // Services module with direct declarations
    pub mod services {
        //! Service layer implementations
        pub mod crawling_integration;
        pub mod real_crawling_integration;
        pub mod real_crawling_commands;
        pub mod crawling_planner;        // Phase 2: 지능형 계획 수립 시스템 추가
    }
}

pub mod domain {
    //! Domain module - Core business logic and entities
    pub mod entities;
    pub mod events;
    pub mod atomic_events; // 추가: 원자적 태스크 이벤트
    pub mod repositories;
    pub mod value_objects;
    pub mod constants; // 추가: 사이트 및 도메인 상수들
    pub mod product_url; // 추가: URL과 메타데이터를 함께 전달하는 구조체
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
    pub mod shared_state;  // 새로 추가된 공유 상태 관리
    pub mod crawling_profile;  // 크롤링 프로필 정의
    pub mod page_discovery_service;
    pub mod parsing_service;
    pub mod validated_crawling_config;  // 검증된 크롤링 설정

    // Re-export commonly used items
    pub use events::EventEmitter;
    pub use state::AppState;
    pub use shared_state::{SharedStateCache};
    pub use crawling_profile::{CrawlingProfile, CrawlingRequest};
    pub use page_discovery_service::PageDiscoveryService;
    pub use validated_crawling_config::ValidatedCrawlingConfig;
}

pub mod infrastructure;

// Re-export commonly used infrastructure items (database_paths 추가)
pub use infrastructure::database_paths;

// Events module - 실시간 이벤트 시스템
pub mod events;

// Modern Rust 2024 - Commands module with direct declarations
pub mod commands {
    //! Command handlers for Tauri frontend integration
    pub mod modern_crawling;
    pub mod config_commands;
    pub mod crawling_v4;
    pub mod smart_crawling;
    pub mod simple_crawling;      // Phase 1: 설정 파일 기반 간단한 크롤링  
    pub mod simple_actor_test;
    pub mod actor_system_monitoring;
    pub mod system_analysis;      // 시스템 분석 명령어
    pub mod advanced_engine_api;  // 새로운 Advanced Engine API 추가
    pub mod data_queries;         // Backend-Only CRUD commands (Modern Rust 2024)
    
    // Re-export commonly used commands
    pub use crawling_v4::*;
    pub use simple_crawling::*;   // Phase 1 명령어 export
    pub use advanced_engine_api::*;  // Advanced Engine 명령어 export
    pub use data_queries::*;      // Backend-Only CRUD 명령어 export
    pub use config_commands::*;   // Config and window management 명령어 export
}

// Modern Rust 2024 - 명시적 모듈 선언
pub mod crawling;

// Utilities module
pub mod utils;

// Test utilities (only available during testing)
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

// PageIdCalculator 테스트 모듈 추가
#[cfg(test)]
pub mod test_page_id_calculator;

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
    
    // Initialize database paths using the new centralized manager (Modern Rust 2024)
    rt.block_on(async {
        info!("🔧 Initializing centralized database path management...");
        
        match crate::infrastructure::initialize_database_paths().await {
            Ok(()) => {
                info!("✅ Database paths initialized successfully");
                let main_url = crate::infrastructure::get_main_database_url();
                info!("🗄️ Using database: {}", main_url);
            },
            Err(e) => {
                error!("❌ Failed to initialize database paths: {}", e);
                eprintln!("Critical error: Database path initialization failed");
                std::process::exit(1);
            }
        }
    });

    // Initialize database connection with centralized path
    let db = rt.block_on(async {
        info!("🔧 Establishing database connection...");
        
        let database_url = crate::infrastructure::get_main_database_url();
        info!("� Connecting to: {}", database_url);
        
        let db = DatabaseConnection::new(&database_url).await
            .expect("Failed to initialize database connection");
        
        info!("🔄 Running database migrations...");
        db.migrate().await.expect("Failed to run database migrations");
        
        info!("✅ Database connection established successfully");
        db
    });

    // Create application state
    let app_state = application::AppState::new(config);
    
    // Create shared state cache for stateful backend operations
    let shared_state = crate::application::shared_state::SharedStateCache::new();
    
    // Create crawling session manager for Actor system integration
    let session_manager: Arc<RwLock<()>> = Arc::new(RwLock::new(()));
    // crate::commands::crawling_session_manager::CrawlingSessionManager::new()
    
    info!("🔧 Building Tauri application...");
    
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(db)
        .manage(app_state)
        .manage(shared_state)  // SharedState 추가
        .manage(session_manager)  // CrawlingSessionManager 추가
        .manage(commands::crawling_v4::CrawlingEngineState {
            engine: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
            database: commands::crawling_v4::MockDatabase {
                connection_status: "Mock Connected".to_string(),
            },
        })
        .manage(commands::simple_actor_test::ActorSystemState::default())
        .setup(|app| {
            let app_handle = app.handle().clone();
            let broadcaster_handle = app_handle.clone();
            let database_handle = app_handle.clone();
            
            // Initialize shared database connection pool (Modern Rust 2024 - Backend-Only CRUD)
            tauri::async_runtime::spawn(async move {
                let state: tauri::State<application::AppState> = database_handle.state();
                
                if let Err(e) = state.initialize_database_pool().await {
                    error!("❌ Failed to initialize database pool: {}", e);
                } else {
                    info!("✅ Shared database connection pool initialized successfully");
                }
            });
            
            // Initialize event emitter in background
            tauri::async_runtime::spawn(async move {
                let state: tauri::State<application::AppState> = app_handle.state();
                let emitter = application::EventEmitter::new(app_handle.clone());
                
                if let Err(e) = state.initialize_event_emitter(emitter).await {
                    error!("Failed to initialize event emitter: {}", e);
                } else {
                    info!("✅ Event emitter initialized successfully");
                }
                
                // Initialize shared database connection pool (Modern Rust 2024 - Backend-Only CRUD)
                if let Err(e) = state.initialize_database_pool().await {
                    error!("Failed to initialize database pool: {}", e);
                } else {
                    info!("✅ Shared database connection pool initialized successfully");
                }
            });
            
            // Initialize system state broadcaster
            tauri::async_runtime::spawn(async move {
                info!("🚀 Starting system state broadcaster...");
                crate::infrastructure::system_broadcaster::start_system_broadcaster(broadcaster_handle);
            });
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Core v4.0 commands - keeping only the implemented ones
            commands::crawling_v4::init_crawling_engine,
            commands::crawling_v4::start_crawling,
            commands::crawling_v4::start_crawling_with_profile,
            commands::crawling_v4::get_cache_status,
            commands::crawling_v4::clear_cache,
            commands::crawling_v4::stop_crawling,
            commands::crawling_v4::get_crawling_stats,
            commands::crawling_v4::get_system_health,
            commands::crawling_v4::update_crawling_config,
            commands::crawling_v4::get_crawling_config,
            commands::crawling_v4::emergency_stop,
            commands::crawling_v4::ping_backend,
            commands::crawling_v4::get_app_settings,
            commands::crawling_v4::save_app_settings,
            
            // Advanced Crawling Engine commands (Phase 4B)
            commands::advanced_engine_api::check_advanced_site_status,
            commands::advanced_engine_api::start_advanced_crawling,
            commands::advanced_engine_api::get_recent_products,
            commands::advanced_engine_api::get_database_stats,
            
            // System Analysis commands (proposal6.md Phase 3)
            commands::system_analysis::analyze_system_status,
            commands::system_analysis::get_analysis_cache_status,
            commands::system_analysis::clear_analysis_cache,
            
            // Smart crawling commands 
            commands::smart_crawling::calculate_crawling_range,
            commands::smart_crawling::get_crawling_progress,
            commands::smart_crawling::get_database_state_for_range_calculation,
            commands::smart_crawling::demo_prompts6_calculation,
            
            // Simple crawling commands (Phase 1 - 즉시 안정화)
            commands::simple_crawling::start_smart_crawling,
            
            // Backend-Only CRUD commands (Modern Rust 2024 Architecture)
            commands::data_queries::get_products_page,
            commands::data_queries::get_latest_products,
            commands::data_queries::get_crawling_status_v2,
            commands::data_queries::get_system_status,
            
            // Window Management commands (이미 config_commands에 구현됨)
            commands::config_commands::save_window_state,
            commands::config_commands::load_window_state,
            commands::config_commands::set_window_position,
            commands::config_commands::set_window_size,
            commands::config_commands::maximize_window,
            commands::config_commands::write_frontend_log,
            
            // New Architecture Actor System commands (OneShot integration 완료)
            commands::simple_actor_test::test_new_arch_session_actor,
            commands::simple_actor_test::test_new_arch_batch_actor,
            commands::simple_actor_test::test_new_arch_integration,
            commands::simple_actor_test::test_new_arch_channels,
            commands::simple_actor_test::test_new_arch_performance,
            
            // Real Crawling Integration commands (Option B implementation)
            new_architecture::services::real_crawling_commands::test_real_crawling_init,
            new_architecture::services::real_crawling_commands::test_real_site_status,
            new_architecture::services::real_crawling_commands::test_real_crawling_analysis,
            new_architecture::services::real_crawling_commands::test_real_page_crawling,
            new_architecture::services::real_crawling_commands::test_real_oneshot_integration,
            
            // Actor System Monitoring commands (Phase C: UI 개선)
            commands::actor_system_monitoring::start_crawling_session
            
            
            // TODO: Add other commands as they are implemented
            // Most commands are temporarily disabled for compilation
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
