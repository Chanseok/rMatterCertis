//! 새로운 Actor 아키텍처 
//! Modern Rust 2024 준수된 Actor 시스템입니다.

// 🎯 Phase 1: 핵심 인프라 (새로 구축된 Actor 기반)
pub mod context;
pub mod integrated_context; // AppContext와 EventEmitter 제공
pub mod system_config; // 🔧 SystemConfig 중앙 관리
pub mod channels;
pub mod actors;
pub mod actor_system; // 호환성을 위해 활성화
pub mod actor_event_bridge; // Actor 이벤트 프론트엔드 브릿지

// 📋 Phase 2: 브릿지 및 검증 (새로 구축됨)
pub mod migration;
pub mod services;
pub mod config;
pub mod events;

// 🔄 Phase 4: 타입 동기화 및 ts-rs 통합 (새로 추가)
pub mod ts_gen;

// Re-exports for compatibility
pub use context::*;
pub use integrated_context::*;
pub use system_config::*; // 🔧 SystemConfig 중앙 export
pub use channels::*;
pub use actors::*;
pub use actor_system::*;
pub use actor_event_bridge::*; // Actor Event Bridge export
pub use migration::*;
pub use services::*;
pub use config::*;
pub use events::*;
