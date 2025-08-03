// Config module - Phase 2 legacy compatibility bridge
// 기존 설정 로직을 새로운 Actor 시스템과 연결하는 브릿지 모듈

pub mod system_config;

// Re-export for backward compatibility
pub use system_config::*;
