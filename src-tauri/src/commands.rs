//! # Modern Command Module v2.0
//! 
//! Modern Rust 2024 + Clean Architecture 명령어 모듈
//! - 명시적 모듈 구조 (mod.rs 비사용)
//! - Real-time 이벤트 방출 지원
//! - Modern 상태 관리 패턴
//! - Clean Architecture 어댑터 레이어

// Modern Rust 2024 - 명시적 모듈 선언
pub mod modern_crawling;
pub mod config_commands;
pub mod crawling_v4;

// Re-export all commands
pub use modern_crawling::*;
pub use config_commands::*;
pub use crawling_v4::*;
