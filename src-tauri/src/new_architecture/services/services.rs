//! Services 모듈
//! 
//! Actor 시스템에서 사용되는 서비스 레이어를 제공합니다.
//! Modern Rust 2024 모듈 구조를 따라 mod.rs 대신 같은 이름의 파일을 사용합니다.

pub mod crawling_planner;

// 기존 모듈들 (Phase 2에서 점진적 마이그레이션 예정)
pub mod crawling_integration;
pub mod real_crawling_integration;
pub mod real_crawling_commands;

// Re-export 주요 타입들
pub use crawling_planner::*;
