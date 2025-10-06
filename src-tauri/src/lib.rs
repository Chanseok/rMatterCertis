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

// 🎯 TypeScript 연동 타입 (ts-rs 기반) - file-based gate (disambiguate from legacy mod.rs)
#[path = "api.rs"]
pub mod api;

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
    pub mod app_handle_access; // global access helpers
    // File-based gate (resolved by services.rs) - path relative to this module
    #[path = "services.rs"]
    pub mod services;
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

// Structured crawl events (new) - flat module naming without mod.rs pattern
pub mod crawl_events;
pub mod crawl_events_mapping;
pub mod metrics; // Prometheus metrics module

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
        pub mod shallow_sync_commands; // 🏃 얕은 크롤링 + 진단 기반 보완
        #[cfg(feature = "dev-tools")]
        pub mod simple_actor_test;
        pub mod smart_crawling;
        pub mod unified_crawling;
    }
    pub mod advanced_engine_api; // 새로운 Advanced Engine API 추가
    pub mod config_commands;
    // Moved under devtools; keep alias re-exports below
    // moved under devtools/crawling_test_commands.rs; don't declare as module here
    // Grouped domain modules
    pub mod database {
        pub mod data_queries; // Backend-Only CRUD commands (Modern Rust 2024)
        #[cfg(feature = "dev-tools")]
        pub mod db_cleanup;
        #[cfg(feature = "dev-tools")]
        pub mod db_repair;
        pub mod vendor_sync; // CSA DCL vendor sync // 🔧 DB repair/sync between products and product_details
    pub mod export_import; // Phase 2: export/import commands
    pub mod device_types_editor; // Phase 5: device types JSON editor
    pub mod core_queries; // Extracted core DB query functions for testability
    }
    pub mod analysis {
        pub mod performance_commands; // 🔧 Phase C: 성능 최적화 도구
        pub mod system_analysis; // 시스템 분석 명령어
    }
    pub mod devtools {
        #[cfg(feature = "dev-tools")]
        pub mod actor_system_monitoring;
    pub mod db_diagnostics; // 🧪 DB pagination mismatch scan (enabled in release)
    pub mod lock_detector; // 🔒 Active writer lock detector (debug/build)
        #[cfg(feature = "dev-tools")]
        pub mod debug_commands; // 🔎 UI debug logging helpers
        #[cfg(feature = "dev-tools")]
        pub mod product_details_analytics; // 📊 product_details analytics endpoints
        // Relocated from commands::crawling_test_commands (pruned from public build)
        // pub mod crawling_test_commands; // 🧪 Crawling test utilities
        // pub mod real_actor_commands; // 🎭 Real Actor 시스템 명령어 (moved here)
    }
    pub mod legacy {
        // dashboard UI removed (Option B); legacy module deleted
    }
    // moved under devtools::real_actor_commands
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
    // Dev-only crawling test utilities (pruned from public surface)
    // pub use self::devtools::crawling_test_commands;
    // pub use self::devtools::crawling_test_commands::*;
    // pub use self::devtools::real_actor_commands as real_actor_commands;
    // pub use self::devtools::real_actor_commands::*;
    // Preserve original path commands::actor_system via alias re-export
    pub use crawling::actor_system;
    pub use crawling::actor_system::*; // prefer role-based alias (star)
    // Preserve original paths for other crawling commands via alias re-exports
    pub use crawling::real_crawling_commands;
    pub use crawling::real_crawling_commands::*;
    #[cfg(feature = "dev-tools")]
    pub use crawling::simple_actor_test;
    #[cfg(feature = "dev-tools")]
    pub use crawling::simple_actor_test::*;
    pub use crawling::smart_crawling;
    pub use crawling::smart_crawling::*;
    pub use crawling::unified_crawling;
    pub use crawling::unified_crawling::*;
    // Database exports (preserve commands::data_queries path)
    pub use database::data_queries;
    pub use database::data_queries::*;
    #[cfg(feature = "dev-tools")]
    pub use database::db_cleanup;
    #[cfg(feature = "dev-tools")]
    pub use database::db_cleanup::*;
    #[cfg(feature = "dev-tools")]
    pub use database::db_repair;
    #[cfg(feature = "dev-tools")]
    pub use database::db_repair::*;
    pub use database::vendor_sync;
    pub use database::vendor_sync::*; // DB repair/sync 명령어 export
    // Analysis exports
    pub use analysis::performance_commands;
    pub use analysis::performance_commands::*; // Phase C 성능 최적화 명령어 export
    pub use analysis::system_analysis;
    pub use analysis::system_analysis::*; // 시스템 분석 명령어 export
    // Devtools exports
    #[cfg(feature = "dev-tools")]
    pub use devtools::actor_system_monitoring;
    #[cfg(any(feature = "dev-tools", debug_assertions))]
    pub use devtools::db_diagnostics;
    #[cfg(any(feature = "dev-tools", debug_assertions))]
    pub use devtools::db_diagnostics::*; // DB diagnostics 명령어 export
    #[cfg(any(feature = "dev-tools", debug_assertions))]
    pub use devtools::lock_detector;
    #[cfg(any(feature = "dev-tools", debug_assertions))]
    pub use devtools::lock_detector::*; // Active writer lock detector export
    #[cfg(feature = "dev-tools")]
    pub use devtools::debug_commands;
    #[cfg(feature = "dev-tools")]
    pub use devtools::debug_commands::*; // UI debug logger export
    #[cfg(feature = "dev-tools")]
    pub use devtools::product_details_analytics;
    #[cfg(feature = "dev-tools")]
    pub use devtools::product_details_analytics::*;
    // Legacy exports
    // dashboard legacy exports removed
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
    // Initialize primary DatabaseConnection (will be injected into Tauri managed state)
    let database_connection = rt.block_on(async {
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

        // Cleanup: if legacy bridge table was removed (post-022), drop obsolete triggers that reference it.
        {
            if let Ok(pool) = crate::infrastructure::database_connection::get_or_init_global_pool().await {
                if let Ok(absent) = sqlx::query_scalar::<_, i64>("SELECT 1 FROM sqlite_master WHERE type='table' AND name='product_primary_device_types' LIMIT 1;")
                    .fetch_optional(&pool).await { if absent.is_none() {
                        let _ = sqlx::query("DROP TRIGGER IF EXISTS trg_ppt_after_insert;").execute(&pool).await;
                        let _ = sqlx::query("DROP TRIGGER IF EXISTS trg_ppt_after_update;").execute(&pool).await;
                        tracing::debug!("dropped_legacy_bridge_triggers");
                    }}
            }
        }

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

    // Create application state (owned). Also store Arc clone for global accessor.
    let app_state = application::AppState::new(config);
    crate::application::app_handle_access::set_app_state(Arc::new(app_state.clone()));

    // Create shared state cache for stateful backend operations
    let shared_state = crate::application::shared_state::SharedStateCache::new();

    // Create crawling session manager for Actor system integration
    let session_manager: Arc<RwLock<()>> = Arc::new(RwLock::new(()));
    // crate::commands::crawling_session_manager::CrawlingSessionManager::new()

    info!("🔧 Building Tauri application...");

    // Start Prometheus metrics server (non-blocking). Port can be overridden via MC_METRICS_PORT
    let metrics_port: u16 = std::env::var("MC_METRICS_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9898);
    {
        let port = metrics_port;
        rt.spawn(async move {
            crate::metrics::start_metrics_server(port).await; // detached task
        });
        info!(port, "📊 Metrics server initialized (Prometheus exporter)");
    }

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        // Provide direct access to the primary database connection for dashboard commands
        .manage(database_connection)
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

                // 5. Auto-seed reference tables & vendors when empty (idempotent)
                let skip_auto = std::env::var("MC_SKIP_AUTO_SEED")
                    .ok()
                    .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
                if skip_auto {
                    info!("⏭️ Skipping auto-seed (MC_SKIP_AUTO_SEED=1)");
                } else {
                    let pool_res = state.get_database_pool().await;
                    let http_res = state.get_http_client().await;
                    if let (Ok(pool), Ok(http)) = (pool_res, http_res) {
                        // Device Types (guard for table existence; migrations might not have added yet in some packaged edge cases)
                        match sqlx::query_scalar::<_, i64>(
                            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='device_types'"
                        ).fetch_one(&pool).await {
                            Ok(master_flag) if master_flag > 0 => {
                                                                                     // Defensive inline schema upgrade (in case migration 011/012 didn't run yet)
                                                                if let Ok(col_present) = sqlx::query_scalar::<_, i64>(
                                                                        "SELECT 1 FROM pragma_table_info('device_types') WHERE name='category' LIMIT 1;"
                                                                ).fetch_optional(&pool).await {
                                                                        if col_present.is_none() {
                                                                                info!("🛠️ Upgrading device_types schema inline (add category,introduced_in; drop description)");
                                                                                let t0 = std::time::Instant::now();
                                                                                info!("⏱️ device_types inline upgrade (tx) start");
                                                                                match pool.begin().await {
                                                                                    Ok(mut tx) => {
                                                                                        let steps = [
                                                                                            "CREATE TABLE IF NOT EXISTS device_types_new (\n    id INTEGER PRIMARY KEY,\n    code_hex TEXT,\n    name TEXT NOT NULL,\n    category TEXT,\n    introduced_in TEXT,\n    type_id INTEGER,\n    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,\n    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP\n);",
                                                                                            "INSERT OR IGNORE INTO device_types_new (id, code_hex, name, created_at, updated_at) SELECT id, code_hex, name, created_at, updated_at FROM device_types;",
                                                                                            "DROP TABLE device_types;",
                                                                                            "ALTER TABLE device_types_new RENAME TO device_types;",
                                                                                            "CREATE UNIQUE INDEX IF NOT EXISTS ux_device_types_name ON device_types(name);",
                                                                                            "CREATE UNIQUE INDEX IF NOT EXISTS ux_device_types_code_hex ON device_types(code_hex);",
                                                                                            "CREATE UNIQUE INDEX IF NOT EXISTS ux_device_types_type_id ON device_types(type_id);",
                                                                                            "CREATE TRIGGER IF NOT EXISTS device_types_updated_at\nAFTER UPDATE ON device_types\nFOR EACH ROW BEGIN\n    UPDATE device_types SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;\nEND;",
                                                                                        ];
                                                                                        let mut step_ok = true;
                                                                                        for (i, s) in steps.iter().enumerate() {
                                                                                            if let Err(e) = sqlx::query(s).execute(&mut *tx).await { warn!(step=i, err=%e, "device_types inline upgrade step failed"); step_ok = false; break; }
                                                                                        }
                                                                                        if step_ok {
                                                                                            if let Err(e) = tx.commit().await { warn!("device_types upgrade commit failed: {}", e); } else {
                                                                                                let elapsed = t0.elapsed().as_millis();
                                                                                                info!(elapsed_ms = elapsed, "✅ Inline device_types schema upgrade applied (tx)");
                                                                                                // Backfill type_id from legacy id if still NULL
                                                                                                let t1 = std::time::Instant::now();
                                                                                                if let Err(e) = sqlx::query("UPDATE device_types SET type_id = id WHERE type_id IS NULL;").execute(&pool).await { warn!("Failed to backfill type_id inline: {}", e); } else { debug!(elapsed_ms = t1.elapsed().as_millis(), "Inline type_id backfill completed"); }
                                                                                            }
                                                                                        } else { let _ = tx.rollback().await; }
                                                                                    }
                                                                                    Err(e) => warn!("device_types inline upgrade tx begin failed: {}", e)
                                                                                }
                                                                        }
                                                                }
                                          // Ensure type_id column exists (if added via migrations but index missing)
                                          if let Ok(has_type_id) = sqlx::query_scalar::<_, i64>("SELECT 1 FROM pragma_table_info('device_types') WHERE name='type_id' LIMIT 1;").fetch_optional(&pool).await { if has_type_id.is_some() { let _ = sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS ux_device_types_type_id ON device_types(type_id);").execute(&pool).await; } }
                                if let Ok(count) = sqlx::query_scalar::<_, i64>(
                                    "SELECT COUNT(*) FROM device_types"
                                ).fetch_one(&pool).await {
                                    if count == 0 {
                                        info!("📥 Seeding device_types from data/matter_device_types.json (empty table)");
                                        // Try multiple fallback paths for robustness (dev, prod, packaged)
                                        let dt_paths = [
                                            "data/matter_device_types.json",
                                            "./data/matter_device_types.json",
                                            "../data/matter_device_types.json",
                                        ];
                                        let mut dt_json: Option<String> = None;
                                        for p in dt_paths.iter() {
                                            if let Ok(s) = tokio::fs::read_to_string(p).await { dt_json = Some(s); info!("📄 Loaded device_types JSON from {}", p); break; }
                                        }
                                        if dt_json.is_none() {
                                            if let Ok(exec) = std::env::current_exe() { if let Some(parent) = exec.parent() { let alt = parent.join("data/matter_device_types.json"); if let Ok(s) = tokio::fs::read_to_string(&alt).await { dt_json = Some(s); info!("📄 Loaded device_types JSON from {:?}", alt); } } }
                                        }
                                        match dt_json.map(|j| serde_json::from_str::<serde_json::Value>(&j)) {
                                            Some(Ok(serde_json::Value::Array(items))) => {
                                                let mut inserted = 0u32;
                                                let has_type_id: Option<i64> = sqlx::query_scalar("SELECT 1 FROM pragma_table_info('device_types') WHERE name='type_id' LIMIT 1;").fetch_optional(&pool).await.ok().flatten();
                                                for item in items {
                                                    if let (Some(id), Some(name)) = (item.get("id").and_then(|v| v.as_i64()), item.get("name").and_then(|v| v.as_str())) {
                                                        let id_i64 = id; // satisfy lint: explicit binding
                                                        let code_hex = item.get("hex").and_then(|v| v.as_str()).unwrap_or("");
                                                        let category = item.get("category").and_then(|v| v.as_str()).unwrap_or("");
                                                        let introduced_in = item.get("introduced_in").and_then(|v| v.as_str()).unwrap_or("");
                                                        let query = if has_type_id.is_some() {
                                                            r#"INSERT INTO device_types (type_id, code_hex, name, category, introduced_in) VALUES (?1, ?2, ?3, ?4, ?5)
                                                                ON CONFLICT(type_id) DO UPDATE SET code_hex=excluded.code_hex, name=excluded.name, category=excluded.category, introduced_in=excluded.introduced_in"#
                                                        } else {
                                                            r#"INSERT INTO device_types (id, code_hex, name, category, introduced_in) VALUES (?1, ?2, ?3, ?4, ?5)
                                                                ON CONFLICT(id) DO UPDATE SET code_hex=excluded.code_hex, name=excluded.name, category=excluded.category, introduced_in=excluded.introduced_in"#
                                                        };
                                                        let q = sqlx::query(query)
                                                            .bind(id_i64)
                                                            .bind(code_hex)
                                                            .bind(name)
                                                            .bind(category)
                                                            .bind(introduced_in);
                                                        if let Err(e) = q.execute(&pool).await {
                                                            warn!("device_type upsert failed id={} name={} err={}", id, name, e);
                                                        } else { inserted += 1; }
                                                    }
                                                }
                                                info!("✅ Seeded {} device_types", inserted);
                                            }
                                            Some(Ok(_)) => warn!("device_types JSON root is not an array"),
                                            Some(Err(e)) => warn!("Failed to parse device_types JSON: {}", e),
                                            None => warn!("device_types JSON file not found in fallback paths"),
                                        }
                                        // Runtime backfill of product_primary_device_types (after device_types seeded)
                                        if let Ok(has_ppt) = sqlx::query_scalar::<_, i64>("SELECT 1 FROM sqlite_master WHERE type='table' AND name='product_primary_device_types' LIMIT 1;").fetch_optional(&pool).await { if has_ppt.is_some() {
                                            if let Ok(ppt_count) = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM product_primary_device_types").fetch_one(&pool).await { if ppt_count == 0 { info!("🔄 Backfilling product_primary_device_types (post device_types seed)");
                                                let backfill_sql = r#"INSERT OR IGNORE INTO product_primary_device_types (product_detail_id, device_type_id)
SELECT pd.url, CAST(json_each.value AS INTEGER) AS device_type_id
FROM product_details pd
JOIN json_each(pd.primary_device_type_ids)
JOIN device_types dt ON dt.type_id = CAST(json_each.value AS INTEGER)
WHERE pd.primary_device_type_ids IS NOT NULL
  AND json_valid(pd.primary_device_type_ids)
  AND json_each.value GLOB '[0-9]*';"#;
                                                match sqlx::query(backfill_sql).execute(&pool).await { Ok(res) => info!("✅ Backfill complete: inserted={}", res.rows_affected()), Err(e) => warn!("Backfill product_primary_device_types failed: {}", e)};
                                            } } } }
                                    }
                                }
                            }
                            Ok(_) => {
                                // Fallback: create table inline (defensive) then seed
                                info!("🧩 device_types table missing; creating inline fallback then seeding");
                                if let Err(e) = sqlx::query(
                                    r#"CREATE TABLE IF NOT EXISTS device_types (
                                        id INTEGER PRIMARY KEY,
                                        code_hex TEXT,
                                        name TEXT NOT NULL,
                                        category TEXT,
                                        introduced_in TEXT,
                                        type_id INTEGER,
                                        created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                                        updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
                                    );"#
                                ).execute(&pool).await { warn!("Failed to create device_types inline: {}", e); }
                                if let Err(e) = sqlx::query(
                                    "CREATE UNIQUE INDEX IF NOT EXISTS ux_device_types_name ON device_types(name);"
                                ).execute(&pool).await { warn!("Failed to create ux_device_types_name: {}", e); }
                                if let Err(e) = sqlx::query(
                                    "CREATE UNIQUE INDEX IF NOT EXISTS ux_device_types_code_hex ON device_types(code_hex);"
                                ).execute(&pool).await { warn!("Failed to create ux_device_types_code_hex: {}", e); }
                                if let Err(e) = sqlx::query(
                                    "CREATE UNIQUE INDEX IF NOT EXISTS ux_device_types_type_id ON device_types(type_id);"
                                ).execute(&pool).await { warn!("Failed to create ux_device_types_type_id: {}", e); }
                                if let Err(e) = sqlx::query(
                                    r#"CREATE TRIGGER IF NOT EXISTS device_types_updated_at
                                        AFTER UPDATE ON device_types
                                        FOR EACH ROW BEGIN
                                            UPDATE device_types SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
                                        END;"#
                                ).execute(&pool).await { warn!("Failed to create device_types_updated_at trigger: {}", e); }
                                // After creating, attempt seeding (recursive style via master_flag path)
                                if let Ok(count) = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM device_types").fetch_one(&pool).await {
                                    if count == 0 {
                                        info!("📥 Seeding device_types from data/matter_device_types.json (post-inline creation)");
                                        // Fallback seeding using same multi-path logic (post-inline creation)
                                        let dt_paths = [
                                            "data/matter_device_types.json",
                                            "./data/matter_device_types.json",
                                            "../data/matter_device_types.json",
                                        ];
                                        let mut dt_json: Option<String> = None;
                                        for p in dt_paths.iter() {
                                            if let Ok(s) = tokio::fs::read_to_string(p).await { dt_json = Some(s); info!("📄 Loaded device_types JSON from {}", p); break; }
                                        }
                                        if dt_json.is_none() {
                                            if let Ok(exec) = std::env::current_exe() { if let Some(parent) = exec.parent() { let alt = parent.join("data/matter_device_types.json"); if let Ok(s) = tokio::fs::read_to_string(&alt).await { dt_json = Some(s); info!("📄 Loaded device_types JSON from {:?}", alt); } } }
                                        }
                                        match dt_json.map(|j| serde_json::from_str::<serde_json::Value>(&j)) {
                                            Some(Ok(serde_json::Value::Array(items))) => {
                                                let mut inserted = 0u32;
                                                let has_type_id: Option<i64> = sqlx::query_scalar("SELECT 1 FROM pragma_table_info('device_types') WHERE name='type_id' LIMIT 1;").fetch_optional(&pool).await.ok().flatten();
                                                let mut batch_counter = 0usize;
                                                let loop_start = std::time::Instant::now();
                                                for item in items {
                                                    if let (Some(id), Some(name)) = (item.get("id").and_then(|v| v.as_i64()), item.get("name").and_then(|v| v.as_str())) {
                                                        let id_i64 = id;
                                                        let code_hex = item.get("hex").and_then(|v| v.as_str()).unwrap_or("");
                                                        let category = item.get("category").and_then(|v| v.as_str()).unwrap_or("");
                                                        let introduced_in = item.get("introduced_in").and_then(|v| v.as_str()).unwrap_or("");
                                                        let query = if has_type_id.is_some() {
                                                            r#"INSERT INTO device_types (type_id, code_hex, name, category, introduced_in) VALUES (?1, ?2, ?3, ?4, ?5)
                                                                ON CONFLICT(type_id) DO UPDATE SET code_hex=excluded.code_hex, name=excluded.name, category=excluded.category, introduced_in=excluded.introduced_in"#
                                                        } else {
                                                            r#"INSERT INTO device_types (id, code_hex, name, category, introduced_in) VALUES (?1, ?2, ?3, ?4, ?5)
                                                                ON CONFLICT(id) DO UPDATE SET code_hex=excluded.code_hex, name=excluded.name, category=excluded.category, introduced_in=excluded.introduced_in"#
                                                        };
                                                        let q = sqlx::query(query)
                                                            .bind(id_i64)
                                                            .bind(code_hex)
                                                            .bind(name)
                                                            .bind(category)
                                                            .bind(introduced_in);
                                                        if let Err(e) = q.execute(&pool).await {
                                                            warn!("device_type upsert failed id={} name={} err={}", id, name, e);
                                                        } else { inserted += 1; }
                                                    }
                                                    batch_counter += 1;
                                                    if batch_counter % 100 == 0 { // yield every 100 rows
                                                        let elapsed = loop_start.elapsed().as_millis();
                                                        debug!(inserted_rows = inserted, batch_counter, elapsed_ms = elapsed, "device_types seeding progress");
                                                        tokio::task::yield_now().await;
                                                    }
                                                }
                                                info!("✅ Seeded {} device_types", inserted);
                                            }
                                            Some(Ok(_)) => warn!("device_types JSON root is not an array"),
                                            Some(Err(e)) => warn!("Failed to parse device_types JSON: {}", e),
                                            None => warn!("device_types JSON file not found in fallback paths"),
                                        }
                                                                                // Runtime backfill after inline creation + seeding
                                                                                if let Ok(has_ppt) = sqlx::query_scalar::<_, i64>("SELECT 1 FROM sqlite_master WHERE type='table' AND name='product_primary_device_types' LIMIT 1;").fetch_optional(&pool).await { if has_ppt.is_some() { if let Ok(ppt_count) = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM product_primary_device_types").fetch_one(&pool).await { if ppt_count == 0 { info!("🔄 Backfilling product_primary_device_types (post inline seed)");
                                                                                                let backfill_sql = r#"INSERT OR IGNORE INTO product_primary_device_types (product_detail_id, device_type_id)
SELECT pd.url, CAST(json_each.value AS INTEGER) AS device_type_id
FROM product_details pd
JOIN json_each(pd.primary_device_type_ids)
JOIN device_types dt ON dt.type_id = CAST(json_each.value AS INTEGER)
WHERE pd.primary_device_type_ids IS NOT NULL
    AND json_valid(pd.primary_device_type_ids)
    AND json_each.value GLOB '[0-9]*';"#;
                                                                                                match sqlx::query(backfill_sql).execute(&pool).await { Ok(res) => info!("✅ Backfill complete: inserted={}", res.rows_affected()), Err(e) => warn!("Backfill product_primary_device_types failed: {}", e)}; } } } }
                                    }
                                }
                            },
                            Err(e) => warn!("device_types existence check failed: {}", e),
                        };

                        // Vendors (only if empty)
                        if let Ok(vendor_count) = sqlx::query_scalar::<_, i64>(
                            "SELECT COUNT(*) FROM vendors"
                        )
                        .fetch_one(&pool)
                        .await
                        {
                            if vendor_count == 0 {
                                info!("📥 Auto-syncing vendors from CSA (empty vendors table)");
                                match crate::commands::database::vendor_sync::sync_vendors_internal(&pool, &http).await {
                                    Ok(res) => info!("✅ Vendors sync complete: inserted={} updated={} skipped={} final={}", res.inserted, res.updated, res.skipped, res.final_count),
                                    Err(e) => warn!("Vendor auto-sync failed: {}", e),
                                }
                            }
                        }
                    } else {
                        warn!("Skipping auto-seed: pool or http client not available");
                    }
                }

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

            // 🏃 Shallow Sync commands (얕은 크롤링 + 진단 기반 보완)
            commands::crawling::shallow_sync_commands::start_shallow_sync,
            commands::crawling::shallow_sync_commands::analyze_missing_details,
            commands::crawling::shallow_sync_commands::start_smart_sync,
            commands::crawling::shallow_sync_commands::start_complement_crawl,

            // Legacy v4 commands removed (init/start/stop/etc.) – replaced by unified_crawling + real_crawling_commands

            // Advanced Crawling Engine commands (status/info only)
            commands::advanced_engine_api::check_advanced_site_status,
            // (start_advanced_crawling 완전 제거)
            commands::advanced_engine_api::get_recent_products,
            commands::advanced_engine_api::get_database_stats,
            // System Analysis commands (proposal6.md Phase 3)
            commands::analysis::system_analysis::analyze_system_status,
            commands::analysis::system_analysis::diagnose_and_repair_data,
            #[cfg(any(feature = "dev-tools", debug_assertions))]
            commands::analysis::system_analysis::get_analysis_cache_status,
            #[cfg(any(feature = "dev-tools", debug_assertions))]
            commands::analysis::system_analysis::clear_analysis_cache,
            // Smart crawling commands
            commands::crawling::smart_crawling::calculate_crawling_range,
            #[cfg(any(feature = "dev-tools", debug_assertions))]
            commands::crawling::smart_crawling::get_crawling_progress,
            #[cfg(any(feature = "dev-tools", debug_assertions))]
            commands::crawling::smart_crawling::get_database_state_for_range_calculation,
            #[cfg(any(feature = "dev-tools", debug_assertions))]
            commands::crawling::smart_crawling::demo_prompts6_calculation,
            // Simple crawling commands (Phase 1 - 즉시 안정화)
            // Removed start_smart_crawling (use start_unified_crawling)

            // Backend-Only CRUD commands (Modern Rust 2024 Architecture)
            #[cfg(any(feature = "dev-tools", debug_assertions))]
            commands::database::data_queries::get_products_page,
            #[cfg(any(feature = "dev-tools", debug_assertions))]
            commands::database::data_queries::get_latest_products,
            #[cfg(any(feature = "dev-tools", debug_assertions))]
            commands::database::data_queries::get_crawling_status_v2,
            #[cfg(any(feature = "dev-tools", debug_assertions))]
            commands::database::data_queries::get_system_status,
            // Local DB Dashboard Phase 1
            commands::database::data_queries::get_db_summary,
            commands::database::data_queries::analytics_query,
            commands::database::data_queries::diagnostics_analytics_mapping,
            // Filter-aware analytics
            commands::database::data_queries::get_available_filter_options,
            commands::database::data_queries::get_filtered_analytics_summary,
            // commands::database::data_queries::diagnose_db_mapping, // TODO: Fix command registration
            // Vendor sync command (CSA DCL)
            commands::database::vendor_sync::update_vendors_from_csa,
            commands::database::vendor_sync::dashboard_vendor_sync,
            // Phase 5 device types JSON editor
            commands::database::device_types_editor::get_device_types_json,
            commands::database::device_types_editor::save_device_types_json,
            // Phase 2 export/import
            commands::database::export_import::export_data,
            commands::database::export_import::import_data,
            // Phase 3 delete range
            commands::database::export_import::preview_delete_range,
            commands::database::export_import::delete_range,
            // Window Management commands (이미 config_commands에 구현됨)
            commands::config_commands::save_window_state,
            commands::config_commands::load_window_state,
            commands::config_commands::set_window_position,
            commands::config_commands::set_window_size,
            commands::config_commands::maximize_window,
            commands::config_commands::show_window,
            commands::config_commands::write_frontend_log,
            // Read-only StageBatcher settings exposure
            commands::config_commands::get_stage_batcher_settings,
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
            #[cfg(any(feature = "dev-tools", debug_assertions))]
            commands::actor_system::test_session_actor_basic,
            commands::actor_system::list_actor_sessions,
            commands::actor_system::check_page_index_consistency,
            // Real Crawling Integration commands (Option B implementation)
            // Note: These commands are temporarily disabled due to module restructuring
            // They will be re-enabled after Phase 2 completion

            // Actor System Monitoring commands (Phase C: UI 개선)
            // Removed start_crawling_session (unified entrypoint)

            // 🚀 Phase C: Real Crawling Commands (temporarily gated; not used by FE)
            #[cfg(feature = "dev-tools")]
            commands::crawling::real_crawling_commands::execute_real_crawling,
            #[cfg(feature = "dev-tools")]
            commands::crawling::real_crawling_commands::get_real_crawling_status,
            #[cfg(feature = "dev-tools")]
            commands::crawling::real_crawling_commands::cancel_real_crawling,
            // 🧪 Phase C: Crawling Test & Development Tools (not registered to FE)
            // 🔧 Phase C: Performance Optimization Tools
            #[cfg(feature = "dev-tools")]
            commands::analysis::performance_commands::init_performance_optimizer,
            #[cfg(feature = "dev-tools")]
            commands::analysis::performance_commands::get_current_performance_metrics,
            #[cfg(feature = "dev-tools")]
            commands::analysis::performance_commands::get_optimization_recommendation,
            #[cfg(feature = "dev-tools")]
            commands::analysis::performance_commands::get_performance_history,
            #[cfg(feature = "dev-tools")]
            commands::analysis::performance_commands::clear_performance_history,
            #[cfg(feature = "dev-tools")]
            commands::analysis::performance_commands::start_performance_session,
            #[cfg(feature = "dev-tools")]
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
            // Legacy invoke compatibility wrappers removed (FE migrated to actor_system)
            commands::devtools::db_diagnostics::scan_db_pagination_mismatches,
            commands::devtools::db_diagnostics::diagnose_database_connection,
            commands::devtools::db_diagnostics::get_products_without_coordinates,
            commands::devtools::db_diagnostics::delete_products_without_coordinates,
            commands::devtools::lock_detector::debug_active_writer_lock,
            #[cfg(feature = "dev-tools")]
            commands::devtools::debug_commands::ui_debug_log,
            #[cfg(feature = "dev-tools")]
            commands::database::db_repair::sync_product_details_coordinates,
            #[cfg(feature = "dev-tools")]
            commands::database::db_cleanup::cleanup_duplicate_urls,
            // Coordinate repair & targeted recrawl utilities (always available)
            commands::database::data_queries::repair_product_coordinates_cmd,
            commands::database::data_queries::list_page_zero_urls,
            commands::database::data_queries::recrawl_physical_pages,
            commands::database::data_queries::coord_mismatch_breakdown,
            commands::database::data_queries::rehydrate_list_pages,
            commands::database::data_queries::get_lock_error_count,
            commands::database::data_queries::reset_lock_error_count,
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
