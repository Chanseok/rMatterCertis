//! StageLogic trait and data contracts (Phase 3 scaffold)

use std::sync::Arc;
use async_trait::async_trait;

use crate::infrastructure::config::AppConfig;
use crate::infrastructure::{HttpClient, IntegratedProductRepository, MatterDataExtractor};
use crate::crawl_engine::channels::types::StageItem;
use crate::crawl_engine::actors::types::StageType;
use crate::crawl_engine::actors::types::StageItemResult;

/// Dependencies passed into strategies (DI-friendly, Arc'ed at the boundary)
#[derive(Clone)]
pub struct Deps {
    pub http: Arc<HttpClient>,
    pub extractor: Arc<MatterDataExtractor>,
    pub repo: Arc<IntegratedProductRepository>,
}

/// Input to a StageLogic strategy
pub struct StageInput {
    pub stage_type: StageType,
    pub item: StageItem,
    pub config: AppConfig,
    pub deps: Deps,
    pub total_pages_hint: Option<u32>,
    pub products_on_last_page_hint: Option<u32>,
}

/// Output from a StageLogic strategy
pub struct StageOutput {
    pub result: StageItemResult,
}

/// Error returned by strategies; bridged to Actor's StageError
#[derive(thiserror::Error, Debug)]
pub enum StageLogicError {
    #[error("Unsupported stage type: {0:?}")]
    Unsupported(StageType),

    #[error("Strategy internal error: {0}")]
    Internal(String),
}

// No direct conversion here; StageActor maps StageLogicError into its own error type.

#[async_trait]
pub trait StageLogic: Send + Sync {
    /// Human-friendly strategy identifier for logging/metrics
    fn name(&self) -> &'static str { "UnnamedStrategy" }
    async fn execute(&self, input: StageInput) -> Result<StageOutput, StageLogicError>;
}

/// Factory dispatch to obtain logic for a stage type
pub trait StageLogicFactory: Send + Sync {
    fn logic_for(&self, stage_type: &StageType) -> Option<Arc<dyn StageLogic>>;
}
