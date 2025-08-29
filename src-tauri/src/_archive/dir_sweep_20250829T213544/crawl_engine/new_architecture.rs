#![cfg(any())]
// ARCHIVED: Legacy aggregator. The active entry is `src-tauri/src/crawl_engine.rs` (gate file).

//! 새로운 Actor 아키텍처 
//! Modern Rust 2024 계된 Actor 시스템입니다.
//! mod.rs 대신 같은 이름의 파일을 사용하여 모듈을 정의합니다.

// 🎯 Phase 1: 핵심 인프라 (새로 구축된 Actor 기반)
pub mod context;
pub mod channels;
pub mod actors;
pub mod actor_system; // 호환성을 위해 활성화

// 📋 Phase 2: 브릿지 및 검증 (새로 구축됨)
pub mod migration;
pub mod services;
pub mod config;
pub mod events;

// 🔄 Phase 4: 타입 동기화 및 ts-rs 통합 (새로 추가)
pub mod ts_gen;

// Re-exports for compatibility
pub use context::*;
pub use channels::*;
pub use actors::*;
pub use actor_system::*;
pub use migration::*;
pub use services::*;
pub use config::*;
pub use events::*;

// 🔄 기존 모듈들 (Phase 3에서 점진적 마이그레이션 예정) - Phase B 목표를 위해 임시 비활성화
// pub mod actor_system;           // 실험적 - 에러 많음
// pub mod channel_types;          // 중복 타입 - channels/types.rs로 통합됨
// pub mod system_config;          // 중복 설정 - context.rs로 통합됨
// pub mod retry_calculator;       // 미사용 - services로 이동 예정
// pub mod integrated_context;     // 중복 컨텍스트 - context.rs로 통합됨
// pub mod task_actor;             // 실험적 - 설계에 포함되지 않음
// pub mod resilience_result;      // 실험적 - 사용하지 않음

// 🎯 Phase 1: 핵심 인프라 (새로 구축된 Actor 기반)
pub mod context;
pub mod channels;
pub mod actors;

// 📋 Phase 2: 브릿지 및 검증 (새로 구축됨)
pub mod migration;
pub mod services;
pub mod config;
pub mod events;

// � Phase 4: 타입 동기화 및 ts-rs 통합 (새로 추가)
pub mod ts_gen;

// �🔄 기존 모듈들 (Phase 3에서 점진적 마이그레이션 예정)
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

// 🔄 기존 모듈 re-exports - Phase B 목표를 위해 임시 비활성화
// pub use actor_system::*;          // 중복 타입으로 에러 발생
