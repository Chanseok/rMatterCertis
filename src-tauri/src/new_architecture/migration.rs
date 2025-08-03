//! Migration Module
//! 
//! Phase 2 브릿지 - 기존 ServiceBased 시스템과 새로운 Actor 시스템 간 연결
//! Modern Rust 2024 준수: no mod.rs, 명시적 모듈 구조

pub mod service_bridge;

pub use service_bridge::*;
