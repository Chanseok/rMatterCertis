// Rust 2024 gate file for `crawl_engine::actors`
// Replaces the need for a directory-level mod.rs and pins paths explicitly.

#[path = "actors/batch_actor.rs"]
pub mod batch_actor;
#[path = "actors/contract.rs"]
pub mod contract;
// Details phase actor removed (DetailTask* retired); keep path reserved if reintroduced later.
// #[path = "actors/details/mod.rs"]
// pub mod details;
#[path = "actors/session_actor.rs"]
pub mod session_actor;
#[path = "actors/stage_actor.rs"]
pub mod stage_actor;
#[path = "actors/traits.rs"]
pub mod traits;
#[path = "actors/types.rs"]
pub mod types;

// Re-exports matching prior public surface
pub use batch_actor::{BatchActor, BatchError};
pub use contract::ACTOR_CONTRACT_VERSION;
pub use session_actor::{SessionActor, SessionError};
pub use stage_actor::StageActor;
pub use traits::*;
pub use types::{
    ActorCommand, ActorError, BatchConfig, CrawlingConfig, StageItem, StageItemResult,
    StageItemType, StageResult, StageType,
};
