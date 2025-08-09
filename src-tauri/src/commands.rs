//! # Modern Command Module v2.0
//! 
//! Modern Rust 2024 + Clean Architecture 명령어 모듈
//! - 명시적 모듈 구조 (mod.rs 비사용)
//! - Real-time 이벤트 방출 지원
//! - Modern 상태 관리 패턴
//! - Clean Architecture 어댑터 레이어

// Modern Rust 2024 - 명시적 모듈 선언
pub mod config_commands; // legacy modules removed
pub mod smart_crawling;
pub mod simple_crawling;        // Phase 1: 설정 파일 기반 간단한 크롤링
pub mod actor_system_commands;  // 새로운 Actor 시스템 명령어
pub mod system_analysis;        // 시스템 분석 명령어

// Re-export all commands
pub use config_commands::*; // Only non-legacy exports retained
pub use smart_crawling::*;
pub use simple_crawling::*;
pub use actor_system_commands::*;
pub use system_analysis::*;
