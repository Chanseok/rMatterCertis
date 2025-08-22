//! 새로운 Actor 아키텍처 모듈
//! 
//! Modern Rust 2024 원칙을 따라 설계된 Actor 시스템입니다.
//! mod.rs 대신 같은 이름의 파일을 사용하여 모듈을 정의합니다.

// 🎯 Phase 1: 핵심 인프라 (새로 구축된 Actor 기반)
pub mod context;
pub mod channels;
pub mod actors;

// 📋 Phase 2: 브릿지 및 검증 (새로 구축됨)
pub mod migration;
pub mod services;
pub mod config;
pub mod events;

// 🔄 기존 모듈들 (Phase 3에서 점진적 마이그레이션 예정)
pub mod actor_system;
pub mod channel_types;
pub mod system_config;
pub mod retry_calculator;
pub mod integrated_context;
pub mod task_actor;
pub mod resilience_result;

// ✅ Phase 1 핵심 컴포넌트 re-exports
pub use context::*;
pub use channels::*;
pub use actors::*;

// ⚠️ Phase 2 브릿지 re-exports (기존 코드 호환성 보장)
pub use migration::*;
pub use services::*;
pub use config::*;
pub use events::*;

// 🔄 기존 모듈 re-exports (actor_system 포함)
pub use actor_system::*;
