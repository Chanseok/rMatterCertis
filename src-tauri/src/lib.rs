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

#[cfg(test)]
mod test_http_client_config;

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
    pub mod dashboard_types;  // 🎨 Phase C: 대시보드 타입
}

// 🚀 새로운 아키텍처 모듈 (Phase 1 구현 완료) - Modern Rust 2024
pub mod new_architecture;

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
    // pub mod crawler_manager;  // 🚧 임시 비활성화 - 컴파일 문제로 인해

    // Re-export commonly used items
    pub use events::EventEmitter;
    pub use state::AppState;
    pub use shared_state::{SharedStateCache};
    pub use crawling_profile::{CrawlingProfile, CrawlingRequest};
    pub use page_discovery_service::PageDiscoveryService;
    pub use validated_crawling_config::ValidatedCrawlingConfig;
    // pub use crawler_manager::{CrawlerManager, CrawlingConfig, CrawlingEngineType}; // 임시 비활성화
}

pub mod infrastructure;

// Re-export commonly used infrastructure items (database_paths 추가)
pub use infrastructure::database_paths;

// Events module - 실시간 이벤트 시스템
pub mod events;

// Services module - 실시간 대시보드 등
pub mod services;

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
    pub mod actor_system_commands;    // 🎭 NEW: Actor System commands
    pub mod real_crawling_commands;   // 🚀 Phase C: 실제 크롤링 기능
    pub mod crawling_test_commands;   // 🧪 Phase C: 크롤링 테스트 도구
    pub mod performance_commands;     // 🔧 Phase C: 성능 최적화 도구
    pub mod dashboard_commands;       // 🎨 Phase C: 실시간 대시보드
    pub mod system_analysis;      // 시스템 분석 명령어
    pub mod advanced_engine_api;  // 새로운 Advanced Engine API 추가
    pub mod data_queries;         // Backend-Only CRUD commands (Modern Rust 2024)
    pub mod unified_crawling;     // 🎯 NEW: 통합 크롤링 명령어 (Actor 시스템 진입점)
    pub mod service_based_reference;  // 🔧 참조용: ServiceBased 크롤링 (구현 완료 후 삭제 예정)
    pub mod real_actor_commands;  // 🎭 진짜 Actor 시스템 명령어
    
    // Re-export commonly used commands
    pub use crawling_v4::*;
    pub use simple_crawling::*;   // Phase 1 명령어 export
    pub use advanced_engine_api::*;  // Advanced Engine 명령어 export
    pub use data_queries::*;      // Backend-Only CRUD 명령어 export
    pub use config_commands::*;   // Config and window management 명령어 export
    pub use real_crawling_commands::*;  // Phase C 실제 크롤링 명령어 export
    pub use crawling_test_commands::*;  // Phase C 테스트 명령어 export
    pub use performance_commands::*;    // Phase C 성능 최적화 명령어 export
    pub use dashboard_commands::*;      // Phase C 대시보드 명령어 export
}// Modern Rust 2024 - 명시적 모듈 선언
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
    
    // 🦀 Modern Rust 2024: Single Responsibility Database Initialization
    rt.block_on(async {
        info!("🔧 Initializing centralized database system (paths + pool + migrations)...");
        
        // 1. Initialize database paths using centralized manager
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
        
        // 2. Initialize database connection with migrations
        let database_url = crate::infrastructure::get_main_database_url();
        info!("🔧 Establishing database connection...");
        info!("� Connecting to: {}", database_url);
        
        let db = DatabaseConnection::new(&database_url).await
            .expect("Failed to initialize database connection");
        
        info!("🔄 Verifying database schema...");
        db.migrate().await.expect("Failed to verify database schema");
        
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
        .manage(commands::performance_commands::PerformanceOptimizerState::default())
        .manage(commands::dashboard_commands::DashboardServiceState::default())
        .setup(|app| {
            let app_handle = app.handle().clone();
            
            // 🚀 Single Backend Initialization (Modern Rust 2024 - Backend-Only CRUD)
            tauri::async_runtime::spawn(async move {
                let state: tauri::State<application::AppState> = app_handle.state();
                
                info!("🔧 Initializing unified backend services...");
                
                // 1. Initialize database pool (single source of truth)
                if let Err(e) = state.initialize_database_pool().await {
                    error!("❌ Failed to initialize database pool: {}", e);
                    return;
                }
                info!("✅ Database connection pool initialized");
                
                // 2. Initialize event emitter
                let emitter = application::EventEmitter::new(app_handle.clone());
                if let Err(e) = state.initialize_event_emitter(emitter).await {
                    error!("❌ Failed to initialize event emitter: {}", e);
                    return;
                }
                info!("✅ Event emitter initialized");
                
                // 3. Start system state broadcaster (10s intervals)
                info!("� Starting system state broadcaster...");
                crate::infrastructure::system_broadcaster::start_system_broadcaster(app_handle.clone());
                
                info!("🎯 Unified backend services initialization complete");
            });
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 🎯 NEW: 통합 크롤링 명령어 (Actor 시스템 진입점)
            commands::unified_crawling::start_unified_crawling,
            
            // 🔧 참조용: ServiceBased 크롤링 명령어 (구현 완료 후 삭제 예정)
            commands::service_based_reference::start_service_based_crawling_reference,
            
            // 🎭 Legacy Service-Based 크롤링 (참고용 보존, 추후 제거 예정)
            commands::real_actor_commands::start_legacy_service_based_crawling,
            
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
            commands::config_commands::show_window,
            commands::config_commands::write_frontend_log,
            
            // New Architecture Actor System commands (OneShot integration 완료)
            commands::simple_actor_test::test_new_arch_channels,
            commands::simple_actor_test::test_new_arch_performance,
            
            // 🎭 Actor System 크롤링 (메인 개발 브랜치)
            commands::actor_system_commands::start_actor_system_crawling,
            commands::actor_system_commands::test_session_actor_basic,
            commands::actor_system_commands::test_actor_integration_basic,
            
            // Real Crawling Integration commands (Option B implementation)
            // Note: These commands are temporarily disabled due to module restructuring
            // They will be re-enabled after Phase 2 completion
            
            // Actor System Monitoring commands (Phase C: UI 개선)
            commands::actor_system_monitoring::start_crawling_session,
            
            // 🚀 Phase C: Real Crawling Commands (PRODUCTION-READY)
            commands::real_crawling_commands::execute_real_crawling,
            commands::real_crawling_commands::get_real_crawling_status,
            commands::real_crawling_commands::cancel_real_crawling,
            
            // 🧪 Phase C: Crawling Test & Development Tools
            commands::crawling_test_commands::quick_crawling_test,
            commands::crawling_test_commands::check_site_status_only,
            commands::crawling_test_commands::crawling_performance_benchmark,
            
            // 🔧 Phase C: Performance Optimization Tools
            commands::performance_commands::init_performance_optimizer,
            commands::performance_commands::get_current_performance_metrics,
            commands::performance_commands::get_optimization_recommendation,
            commands::performance_commands::get_performance_history,
            commands::performance_commands::clear_performance_history,
            commands::performance_commands::start_performance_session,
            commands::performance_commands::end_performance_session,
            
            // 🎨 Phase C: Realtime Dashboard Tools
            commands::dashboard_commands::init_dashboard_service,
            commands::dashboard_commands::get_dashboard_state,
            commands::dashboard_commands::get_chart_data,
            commands::dashboard_commands::start_dashboard_crawling_session,
            commands::dashboard_commands::update_dashboard_progress,
            commands::dashboard_commands::complete_dashboard_crawling_session,
            commands::dashboard_commands::test_dashboard_integration,
            commands::dashboard_commands::run_dashboard_demo
            
            
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

// Priority 1 검증 테스트 모듈
#[cfg(test)]
mod priority1_verification_tests;
