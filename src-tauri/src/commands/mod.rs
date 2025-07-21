//! Tauri 명령어 모듈
//! 프론트엔드와 백엔드 간의 API 인터페이스

pub mod advanced_engine_api;
pub mod config_commands;
pub mod simple_crawling;

// 모든 명령어를 한곳에서 export
pub use advanced_engine_api::{
    check_advanced_site_status,
    start_advanced_crawling,
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

pub use simple_crawling::{
    start_smart_crawling,
};
