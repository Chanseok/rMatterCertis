//! 설정 모듈 재익스포트
//! Modern Rust 2024: mod.rs 금지, lib.rs를 통한 명시적 재익스포트만 허용

pub use system_config::*;

// 개별 모듈들 (mod.rs 대신 직접 선언)
pub mod system_config;
