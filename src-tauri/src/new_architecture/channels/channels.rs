//! 채널 모듈
//! 
//! Actor 간 통신을 위한 채널 시스템을 제공합니다.
//! Modern Rust 2024 모듈 구조를 따라 mod.rs 대신 같은 이름의 파일을 사용합니다.

// pub mod channel_types;  // 중복 타입 - types.rs로 통합됨
pub mod types;

// Re-export 주요 타입들 - 중복 제거
pub use types::*;
