//! Config 모듈
//! 
//! Actor 시스템을 위한 설정 관리 모듈입니다.
//! Modern Rust 2024 모듈 구조를 따라 mod.rs 대신 같은 이름의 파일을 사용합니다.

pub mod system_config;

// Re-export 주요 타입들 - Actor 시스템 호환성 보장
pub use system_config::*;
