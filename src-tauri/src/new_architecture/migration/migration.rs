//! Migration 모듈
//! 
//! 기존 ServiceBased 로직을 새로운 Actor 시스템으로 점진적으로 마이그레이션하기 위한 모듈입니다.
//! Modern Rust 2024 원칙을 따라 mod.rs 대신 같은 이름의 파일을 사용합니다.

pub mod service_bridge;

// Re-export 주요 타입들
pub use service_bridge::*;

pub mod service_bridge;

// Re-export 주요 타입들
pub use service_bridge::*;
