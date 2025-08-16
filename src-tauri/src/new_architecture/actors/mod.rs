pub mod batch_actor;
pub mod contract;
pub mod details; // product details phase actors
pub mod session_actor;
pub mod stage_actor;
pub mod traits;
pub mod types;

pub use batch_actor::{BatchActor, BatchError};
pub use contract::ACTOR_CONTRACT_VERSION;
pub use session_actor::{SessionActor, SessionError};
pub use stage_actor::StageActor;
pub use traits::*;
pub use types::{
    ActorCommand, ActorError, BatchConfig, CrawlingConfig, StageItem, StageItemResult,
    StageItemType, StageResult, StageType,
};
