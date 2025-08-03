//! Events 모듈
//! 
//! Actor 시스템을 위한 이벤트 시스템을 제공합니다.
//! Modern Rust 2024 모듈 구조를 따라 mod.rs 대신 같은 이름의 파일을 사용합니다.

pub mod task_lifecycle;

// Re-export 주요 타입들
pub use task_lifecycle::*;
