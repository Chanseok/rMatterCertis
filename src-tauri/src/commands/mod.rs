//! Tauri 명령어 모듈
//! 프론트엔드와 백엔드 간의 API 인터페이스

pub mod advanced_engine_api;           // Advanced status/info endpoints
pub mod config_commands;               // Configuration & window state commands
pub mod data_queries;                  // Read/query endpoints
pub mod unified_crawling;              // Single crawling entrypoint (actor-based)
pub mod validation_commands;           // Validation command
pub mod sync_commands;                 // Partial sync command
pub mod db_diagnostics;                // DB pagination diagnostics
pub mod db_cleanup;                    // DB duplicate cleanup
pub mod db_repair;                     // DB repair/sync between products and product_details
pub mod debug_commands;                // UI debug logging helpers
// pub mod dashboard_commands;          // Archived while UI is disabled

// 모든 명령어를 한곳에서 export
pub use advanced_engine_api::{
    check_advanced_site_status,
    get_recent_products,
    get_database_stats,
};

pub use config_commands::{
    get_site_config,
    // ❌ REMOVED: get_frontend_config - 설정 전송 API 제거
    update_logging_settings,
    update_batch_settings,
    update_crawling_settings,
};

// 모든 크롤링 실행 엔트리포인트는 start_unified_crawling 하나만 유지

pub use data_queries::{
    get_products_page,
    get_latest_products,
    get_crawling_status_v2,
    get_system_status,
};

pub use unified_crawling::start_unified_crawling;

pub use validation_commands::start_validation;
pub use sync_commands::start_partial_sync;
pub use db_diagnostics::scan_db_pagination_mismatches;
pub use db_cleanup::cleanup_duplicate_urls;
pub use db_repair::sync_product_details_coordinates;
pub use debug_commands::ui_debug_log;
