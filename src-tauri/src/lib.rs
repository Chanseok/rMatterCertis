//! Matter Certis v2 - E-commerce Product Crawling Application
//!
//! This application provides web crawling capabilities for e-commerce sites
//! with a modern desktop interface built with Tauri and `SolidJS`.
//!
//! Modern Rust module organization (Rust 2024+ style):
//! - Each module is defined in its own .rs file or directory
//! - No mod.rs files - clean, modern structure
//! - Direct module declarations following Rust 2024 conventions

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::module_name_repetitions)] // 모듈명 중복은 도메인 명확성을 위해 허용
#![allow(clippy::similar_names)] // 유사한 변수명은 의미적 연관성이 있는 경우 허용
#![allow(unused_variables)] // 개발 중 임시 변수들 허용
#![allow(clippy::uninlined_format_args)]
#![allow(missing_docs)]
#![allow(clippy::unnecessary_operation)]
#![allow(unused_must_use)]
#![allow(ambiguous_glob_reexports)]
// High-noise lints we intentionally allow during active refactoring (no behavior change)
#![allow(clippy::unused_async)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::large_stack_frames)]
// Additional allowances to keep clippy green during migration; revisit and narrow later
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::too_long_first_doc_paragraph)]
#![allow(clippy::float_cmp)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::unused_self)]
#![allow(clippy::or_fun_call)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::comparison_chain)]
#![allow(clippy::significant_drop_tightening)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::struct_field_names)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::used_underscore_binding)]
#![allow(clippy::unnecessary_debug_formatting)]
// Newly added during Phase 0 stabilization to pass clippy -D warnings
#![allow(clippy::collection_is_never_read)]
#![allow(clippy::non_std_lazy_statics)]
#![allow(clippy::drain_collect)]
#![allow(clippy::iter_with_drain)]
#![allow(clippy::assertions_on_constants)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::assigning_clones)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::manual_clamp)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::manual_string_new)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::useless_let_if_seq)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::redundant_clone)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::module_inception)]
#![allow(clippy::unnested_or_patterns)]
#![allow(clippy::type_complexity)]
#![allow(clippy::items_after_test_module)]
#![allow(clippy::needless_return)]
#![allow(clippy::unchecked_duration_subtraction)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::missing_fields_in_debug)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::use_self)]
#![allow(clippy::match_wildcard_for_single_variants)]
#![allow(clippy::doc_lazy_continuation)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::format_push_string)]
#![allow(clippy::should_implement_trait)]
#![allow(clippy::branches_sharing_code)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::multiple_crate_versions)]

// moved: test_execution_plan_page_slots -> tests/ (integration test)
// moved: test_http_client_config -> tests/ (integration test)

use crate::infrastructure::config::{AppConfig, ConfigManager};
use crate::infrastructure::{DatabaseConnection, init_logging_with_config};
use std::sync::{Arc, RwLock};
use tauri::Manager;
use tracing::{debug, error, info, warn};

// Modern Rust 2024 module declarations - no mod.rs files needed

// 🎯 TypeScript 연동 타입 (ts-rs 기반)
#[path = "api/mod.rs"]
pub mod api; // renamed from `types`, pinned to directory module

// 🚀 새로운 아키텍처 모듈 (Phase 1 구현 완료) - Modern Rust 2024
pub mod crawl_engine; // renamed from new_architecture
// Note: legacy alias `new_architecture` has been removed after migration
// Use `crate::crawl_engine` going forward.

pub mod domain {
    //! Domain module - Core business logic and entities
    pub mod atomic_events; // 추가: 원자적 태스크 이벤트
    pub mod constants; // 추가: 사이트 및 도메인 상수들
    pub mod entities;
        // pub mod events; // removed: legacy event types (replaced by AppEvent + FE API types)
    pub mod product_url;
    // pub mod repositories; // removed: legacy repository traits (no active implementations)
    // pub mod value_objects; // removed: unused value objects (ValidatedUrl, ProductData, etc.)
    pub mod services {
        //! Domain services for business logic
        pub mod crawling_services;
        pub mod data_processing_services;
        pub mod product_service;

        // Re-export commonly used items
        pub use crawling_services::{
            DatabaseAnalysis, DatabaseAnalyzer, DuplicateAnalysis, DuplicateGroup, DuplicateType,
            FieldAnalysis, ProcessingStrategy, ProductDetailCollector, ProductListCollector,
            SiteStatus, StatusChecker,
        };
        pub use data_processing_services::{
            BatchProgress, BatchProgressTracker, BatchRecoveryService, ConflictGroup,
            ConflictResolver, DeduplicationService, DuplicationAnalysis, ErrorClassification,
            ErrorClassifier, RecoveryResult, RetryManager, ValidationResult, ValidationService,
        };
        pub use product_service::ProductService;
    }
    pub mod integrated_product;
    pub mod matter_product;
    pub mod pagination;
    pub mod product;
    pub mod session_manager; // PHASE1: page_id/index_in_page 중앙 집중 모듈 (legacy -> canonical 전환용)

    // Re-export commonly used items
    pub use entities::*;
        // pub use events::*; // removed re-export of legacy event types
    pub use pagination::{CanonicalPageIdCalculator, PagePosition, PaginationCalculator};
}

pub mod application {
    //! Application layer - Use cases and application services
    // removed: unused dto module
    pub mod events;
    pub mod integrated_use_cases;
    // removed: unused parsing_service module
    pub mod shared_state; // 새로 추가된 공유 상태 관리
    pub mod state;
    pub mod validated_crawling_config; // 검증된 크롤링 설정
    #[path = "services.rs"]
    pub mod services; // application services via shim
    // pub mod crawler_manager;  // 🚧 임시 비활성화 - 컴파일 문제로 인해

    // Re-export commonly used items
        pub use events::EventEmitter;
        pub use shared_state::SharedStateCache;
        pub use state::AppState;
        // removed: dto::* re-export (module unused)
        pub use integrated_use_cases::IntegratedProductUseCases;
        // removed: parsing_service re-exports (module unused)
    // pub use crawler_manager::{CrawlerManager, CrawlingConfig, CrawlingEngineType}; // 임시 비활성화
}

pub mod infrastructure;

// Re-export commonly used infrastructure items (database_paths 추가)
pub use infrastructure::database_paths;

// Events module - 실시간 이벤트 시스템
pub mod events;

// moved: services -> application::services

// Integrated commands (DB stats / reset utilities)
pub mod commands_integrated;

// Modern Rust 2024 - Commands module with direct declarations
pub mod commands {
    //! Command handlers for Tauri frontend integration
    // Removed legacy modules: modern_crawling, crawling_v4, service_based_reference
    // legacy shim removed after migration
    // Grouped submodules (progressively migrated)
    pub mod crawling {
        pub mod actor_system; // moved here physically
        pub mod real_crawling_commands;
        pub mod smart_crawling;
    #[cfg(feature = "dev-tools")]
    pub mod simple_actor_test;
        pub mod unified_crawling;
    }
    pub mod advanced_engine_api; // 새로운 Advanced Engine API 추가
    pub mod config_commands;
    pub mod crawling_test_commands; // 🧪 Phase C: 크롤링 테스트 도구
    // Grouped domain modules
    pub mod database {
        pub mod data_queries; // Backend-Only CRUD commands (Modern Rust 2024)
        #[cfg(feature = "dev-tools")]
        pub mod db_cleanup;
        #[cfg(feature = "dev-tools")]
        pub mod db_repair; // 🔧 DB repair/sync between products and product_details
    }
    pub mod analysis {
        pub mod performance_commands; // 🔧 Phase C: 성능 최적화 도구
        pub mod system_analysis; // 시스템 분석 명령어
    }
    pub mod devtools {
        #[cfg(feature = "dev-tools")]
        pub mod actor_system_monitoring;
        #[cfg(any(feature = "dev-tools", debug_assertions))]
        pub mod db_diagnostics; // 🧪 DB pagination mismatch scan
        #[cfg(feature = "dev-tools")]
        pub mod debug_commands; // 🔎 UI debug logging helpers
        #[cfg(feature = "dev-tools")]
        pub mod product_details_analytics; // 📊 product_details analytics endpoints
    }
    pub mod legacy {
        #[cfg(feature = "legacy-ui")]
        pub mod dashboard_commands; // 🎨 Phase C: 실시간 대시보드 (legacy gated)
    }
    #[cfg(feature = "dev-tools")]
    pub mod real_actor_commands; // 🎭 진짜 Actor 시스템 명령어
    // moved under crawling/ with alias re-exports below
    // pub mod real_crawling_commands; // 🚀 Phase C: 실제 크롤링 기능
    // pub mod simple_actor_test;
    // pub mod smart_crawling;
    pub mod sync_commands;
    // pub mod unified_crawling; // 🎯 NEW: 통합 크롤링 명령어 (Actor 시스템 진입점)
    pub mod validation_commands; // ✅ Validation pass commands (page/index integrity) // 🔄 Partial Sync (recrawl + DB upsert) // 🧹 DB URL duplicate cleanup

    // Re-export commonly used commands
    // simple_crawling removed
    pub use advanced_engine_api::*; // Advanced Engine 명령어 export
    pub use config_commands::*; // Config and window management 명령어 export
    pub use crawling_test_commands::*; // Phase C 테스트 명령어 export
    // Preserve original path commands::actor_system via alias re-export
    pub use crawling::actor_system as actor_system;
    pub use crawling::actor_system::*; // prefer role-based alias (star)
    // Preserve original paths for other crawling commands via alias re-exports
    pub use crawling::real_crawling_commands as real_crawling_commands;
    pub use crawling::real_crawling_commands::*;
    pub use crawling::smart_crawling as smart_crawling;
    pub use crawling::smart_crawling::*;
    #[cfg(feature = "dev-tools")]
    pub use crawling::simple_actor_test as simple_actor_test;
    #[cfg(feature = "dev-tools")]
    pub use crawling::simple_actor_test::*;
    pub use crawling::unified_crawling as unified_crawling;
    pub use crawling::unified_crawling::*;
    // Database exports (preserve commands::data_queries path)
    pub use database::data_queries as data_queries;
    pub use database::data_queries::*;
    #[cfg(feature = "dev-tools")]
    pub use database::db_cleanup as db_cleanup;
    #[cfg(feature = "dev-tools")]
    pub use database::db_cleanup::*;
    #[cfg(feature = "dev-tools")]
    pub use database::db_repair as db_repair;
    #[cfg(feature = "dev-tools")]
    pub use database::db_repair::*; // DB repair/sync 명령어 export
    // Analysis exports
    pub use analysis::performance_commands as performance_commands;
    pub use analysis::performance_commands::*; // Phase C 성능 최적화 명령어 export
    pub use analysis::system_analysis as system_analysis;
    pub use analysis::system_analysis::*; // 시스템 분석 명령어 export
    // Devtools exports
    #[cfg(feature = "dev-tools")]
    pub use devtools::actor_system_monitoring as actor_system_monitoring;
    #[cfg(any(feature = "dev-tools", debug_assertions))]
    pub use devtools::db_diagnostics as db_diagnostics;
    #[cfg(any(feature = "dev-tools", debug_assertions))]
    pub use devtools::db_diagnostics::*; // DB diagnostics 명령어 export
    #[cfg(feature = "dev-tools")]
    pub use devtools::debug_commands as debug_commands;
    #[cfg(feature = "dev-tools")]
    pub use devtools::debug_commands::*; // UI debug logger export
    #[cfg(feature = "dev-tools")]
    pub use devtools::product_details_analytics as product_details_analytics;
    #[cfg(feature = "dev-tools")]
    pub use devtools::product_details_analytics::*;
    // Legacy exports
    #[cfg(feature = "legacy-ui")]
    pub use legacy::dashboard_commands as dashboard_commands;
    #[cfg(feature = "legacy-ui")]
    pub use legacy::dashboard_commands::*; // Phase C 대시보드 명령어 export
    // real_crawling_commands re-exported above via crawling:: alias
    pub use sync_commands::*; // Partial Sync 명령어 export // DB cleanup 명령어 export // Export analytics command
} // Modern Rust 2024 - 명시적 모듈 선언
// Deprecated legacy crawling engine module (disabled). Historical snapshots were under _archive; see scripts/_backups now.
// pub mod crawling;

// moved: legacy utils consolidated into domain modules

// moved: test_utils -> tests/ (integration test utilities)
// moved: test_page_id_calculator -> tests/ (integration test)

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Start the application runtime and initialize subsystems.
///
/// # Panics
/// Panics if a Tokio runtime cannot be created.
pub fn run() {
    // Initialize runtime for async operations first
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    // Load configuration with automatic initialization on first run
    let config = rt.block_on(async {
        match ConfigManager::new() {
            Ok(manager) => match manager.initialize_on_first_run().await {
                Ok(config) => {
                    info!("✅ Configuration initialized successfully");
                    config
                }
                Err(e) => {
                    eprintln!(
                        "⚠️ Failed to initialize configuration, using defaults: {}",
                        e
                    );
                    AppConfig::default()
                }
            },
            Err(e) => {
                eprintln!("⚠️ Failed to create config manager, using defaults: {}", e);
                AppConfig::default()
            }
        }
    });

    // Initialize logging system with config-based settings
    if let Err(e) = init_logging_with_config(&config.user.logging) {
        eprintln!("❌ Failed to initialize logging system: {}", e);
        std::process::exit(1);
    }

    info!("🚀 Starting Matter Certis v2 application");
    info!("📋 Configuration loaded successfully");

    // Phase 0: Log feature toggles for visibility (no behavior changes yet)
    {
        let http_unified = crate::infrastructure::features::feature_http_client_unified();
        let stage_exec = crate::infrastructure::features::feature_stage_executor_template();
        info!(
            "⚙️ Features -> http_unified={}, stage_executor_template={}",
            http_unified, stage_exec
        );
    }

    // Initialize runtime for async operations (already created above)
    info!("✅ Tokio runtime initialized successfully");

    // 🦀 Modern Rust 2024: Single Responsibility Database Initialization
    rt.block_on(async {
        let concise_all = std::env::var("MC_CONCISE_ALL")
            .ok()
            .is_none_or(|v| !(v == "0" || v.eq_ignore_ascii_case("false")));
        let concise = concise_all
            || std::env::var("MC_CONCISE_STARTUP")
                .ok()
                .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
        if concise {
            debug!("🔧 Initializing centralized database system (paths + pool + migrations)...");
        } else {
            info!("🔧 Initializing centralized database system (paths + pool + migrations)...");
        }

        // 1. Initialize database paths using centralized manager
        match crate::infrastructure::initialize_database_paths().await {
            Ok(()) => {
                if concise {
                    debug!("✅ Database paths initialized successfully");
                } else {
                    info!("✅ Database paths initialized successfully");
                }
                let main_url = crate::infrastructure::get_main_database_url();
                if concise {
                    debug!("🗄️ Using database: {}", main_url);
                } else {
                    info!("🗄️ Using database: {}", main_url);
                }
            }
            Err(e) => {
                error!("❌ Failed to initialize database paths: {}", e);
                eprintln!("Critical error: Database path initialization failed");
                std::process::exit(1);
            }
        }

        // 2. Initialize database connection with migrations
        let database_url = crate::infrastructure::get_main_database_url();
        if concise {
            debug!("🔧 Establishing database connection...");
        } else {
            info!("🔧 Establishing database connection...");
        }
        if concise {
            debug!("🔌 Connecting to: {}", database_url);
        } else {
            info!("🔌 Connecting to: {}", database_url);
        }

        let db = DatabaseConnection::new(&database_url)
            .await
            .expect("Failed to initialize database connection");

        if concise {
            debug!("🔄 Verifying database schema...");
        } else {
            info!("🔄 Verifying database schema...");
        }
        db.migrate()
            .await
            .expect("Failed to verify database schema");

        if concise {
            debug!("✅ Database connection established successfully");
        } else {
            info!("✅ Database connection established successfully");
        }
        db
    });

    // 2. Eagerly initialize the global Sqlite pool at application startup (managed state)
    //    This ensures all subsequent code paths reuse a single pool via OnceLock
    rt.block_on(async {
        match crate::infrastructure::database_connection::get_or_init_global_pool().await {
            Ok(_pool) => {
                let concise_all = std::env::var("MC_CONCISE_ALL")
                    .ok()
                    .is_none_or(|v| !(v == "0" || v.eq_ignore_ascii_case("false")));
                let concise = concise_all
                    || std::env::var("MC_CONCISE_STARTUP")
                        .ok()
                        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
                if concise {
                    debug!("🧩 Global DB pool initialized at startup (OnceLock)");
                } else {
                    info!("🧩 Global DB pool initialized at startup (OnceLock)");
                }
            }
            Err(e) => {
                error!("❌ Failed to initialize global DB pool at startup: {}", e);
                eprintln!("Critical error: Global DB pool init failed: {}", e);
                std::process::exit(1);
            }
        }
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
        .manage(shared_state) // SharedState 추가
        .manage(session_manager) // CrawlingSessionManager 추가
        // Legacy CrawlingEngineState (crawling_v4) removed – unified actor-based path only
        .manage(commands::analysis::performance_commands::PerformanceOptimizerState::default());

    // Conditionally add dev-only simple_actor_test state
    let builder = {
        #[cfg(feature = "dev-tools")]
        {
            builder.manage(commands::crawling::simple_actor_test::ActorSystemState::default())
        }
        #[cfg(not(feature = "dev-tools"))]
        {
            builder
        }
    };

    // Conditionally add legacy dashboard state without shadowing the outer builder
    let builder = {
        #[cfg(feature = "legacy-ui")]
        {
            builder.manage(commands::legacy::dashboard_commands::DashboardServiceState::default())
        }
        #[cfg(not(feature = "legacy-ui"))]
        {
            builder
        }
    };

    let builder = builder
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

                // 3. Initialize unified HTTP client (shared)
                if let Err(e) = state.initialize_http_client().await {
                    error!("❌ Failed to initialize HTTP client: {}", e);
                    return;
                }
                info!("✅ HTTP client initialized (shared)");

                // 4. Start system state broadcaster (10s intervals)
                info!("� Starting system state broadcaster...");
                crate::infrastructure::system_broadcaster::start_system_broadcaster(
                    app_handle.clone(),
                );

                info!("🎯 Unified backend services initialization complete");
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 🎯 NEW: 통합 크롤링 명령어 (Actor 시스템 진입점)
            commands::crawling::unified_crawling::start_unified_crawling,
            // 🔧 참조/레거시 ServiceBased 명령어는 노출 중단 (엔트리포인트 통일)
            // commands::service_based_reference::start_service_based_crawling_reference,
            // commands::real_actor_commands::start_legacy_service_based_crawling,

            // Legacy v4 commands removed (init/start/stop/etc.) – replaced by unified_crawling + real_crawling_commands

            // Advanced Crawling Engine commands (status/info only)
            commands::advanced_engine_api::check_advanced_site_status,
            // (start_advanced_crawling 완전 제거)
            commands::advanced_engine_api::get_recent_products,
            commands::advanced_engine_api::get_database_stats,
            // System Analysis commands (proposal6.md Phase 3)
            commands::analysis::system_analysis::analyze_system_status,
            commands::analysis::system_analysis::diagnose_and_repair_data,
            commands::analysis::system_analysis::get_analysis_cache_status,
            commands::analysis::system_analysis::clear_analysis_cache,
            // Smart crawling commands
            commands::crawling::smart_crawling::calculate_crawling_range,
            commands::crawling::smart_crawling::get_crawling_progress,
            commands::crawling::smart_crawling::get_database_state_for_range_calculation,
            commands::crawling::smart_crawling::demo_prompts6_calculation,
            // Simple crawling commands (Phase 1 - 즉시 안정화)
            // Removed start_smart_crawling (use start_unified_crawling)

            // Backend-Only CRUD commands (Modern Rust 2024 Architecture)
            commands::database::data_queries::get_products_page,
            commands::database::data_queries::get_latest_products,
            commands::database::data_queries::get_crawling_status_v2,
            commands::database::data_queries::get_system_status,
            // Window Management commands (이미 config_commands에 구현됨)
            commands::config_commands::save_window_state,
            commands::config_commands::load_window_state,
            commands::config_commands::set_window_position,
            commands::config_commands::set_window_size,
            commands::config_commands::maximize_window,
            commands::config_commands::show_window,
            commands::config_commands::write_frontend_log,
            // New Architecture Actor System commands (OneShot integration 완료)
            #[cfg(feature = "dev-tools")]
            commands::crawling::simple_actor_test::test_new_arch_channels,
            #[cfg(feature = "dev-tools")]
            commands::crawling::simple_actor_test::test_new_arch_performance,
            // 🎭 Actor System 크롤링 (직접 호출 허용: FE 통합)
            commands::actor_system::start_actor_system_crawling,
            commands::actor_system::pause_session,
            commands::actor_system::resume_session,
            commands::actor_system::get_session_status,
            commands::actor_system::request_graceful_shutdown,
            commands::actor_system::test_session_actor_basic,
            commands::actor_system::list_actor_sessions,
            commands::actor_system::check_page_index_consistency,
            // Real Crawling Integration commands (Option B implementation)
            // Note: These commands are temporarily disabled due to module restructuring
            // They will be re-enabled after Phase 2 completion

            // Actor System Monitoring commands (Phase C: UI 개선)
            // Removed start_crawling_session (unified entrypoint)

            // 🚀 Phase C: Real Crawling Commands (PRODUCTION-READY)
            commands::crawling::real_crawling_commands::execute_real_crawling,
            commands::crawling::real_crawling_commands::get_real_crawling_status,
            commands::crawling::real_crawling_commands::cancel_real_crawling,
            // 🧪 Phase C: Crawling Test & Development Tools
            commands::crawling_test_commands::quick_crawling_test,
            commands::crawling_test_commands::check_site_status_only,
            commands::crawling_test_commands::crawling_performance_benchmark,
            // 🔧 Phase C: Performance Optimization Tools
            commands::analysis::performance_commands::init_performance_optimizer,
            commands::analysis::performance_commands::get_current_performance_metrics,
            commands::analysis::performance_commands::get_optimization_recommendation,
            commands::analysis::performance_commands::get_performance_history,
            commands::analysis::performance_commands::clear_performance_history,
            commands::analysis::performance_commands::start_performance_session,
            commands::analysis::performance_commands::end_performance_session,
            // 🎨 Phase C: Realtime Dashboard Tools (temporarily disabled while UI is archived)
            // commands::dashboard_commands::init_dashboard_service,
            // commands::dashboard_commands::get_dashboard_state,
            // commands::dashboard_commands::get_chart_data,
            // commands::dashboard_commands::update_dashboard_progress,
            // commands::dashboard_commands::complete_dashboard_crawling_session,
            // commands::dashboard_commands::test_dashboard_integration,
            // commands::dashboard_commands::run_dashboard_demo,
            // Settings store commands
            commands::config_commands::get_app_settings,
            commands::config_commands::save_app_settings,
            crate::commands_integrated::reset_product_storage,
            commands::validation_commands::start_validation,
            commands::sync_commands::start_partial_sync, // TODO: Add other commands as they are implemented
            commands::sync_commands::start_batched_sync,
            commands::sync_commands::start_repair_sync,
            commands::sync_commands::start_sync_pages,
            commands::sync_commands::start_basic_sync_pages,
            commands::sync_commands::retry_failed_details,
            commands::sync_commands::start_diagnostic_sync,
            commands::actor_system::start_manual_crawl_pages_actor,
            #[cfg(any(feature = "dev-tools", debug_assertions))]
            commands::devtools::db_diagnostics::scan_db_pagination_mismatches,
            #[cfg(feature = "dev-tools")]
            commands::devtools::debug_commands::ui_debug_log,
            #[cfg(feature = "dev-tools")]
            commands::database::db_repair::sync_product_details_coordinates,
            #[cfg(feature = "dev-tools")]
            commands::database::db_cleanup::cleanup_duplicate_urls,
            #[cfg(feature = "dev-tools")]
            commands::devtools::product_details_analytics::get_product_details_analytics // dev-tools only
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

// moved: priority1_verification_tests -> tests/ (integration test)
