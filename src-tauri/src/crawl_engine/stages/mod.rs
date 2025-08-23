pub mod traits;

// Externalized strategies: default family lives under strategies/default
pub mod strategies {
    pub mod default;
}

use std::sync::Arc;
use traits::{StageLogic, StageLogicFactory};

/// Placeholder factory; will be expanded as strategies are implemented.
pub struct DefaultStageLogicFactory;

impl StageLogicFactory for DefaultStageLogicFactory {
    fn logic_for(
        &self,
        stage_type: &crate::crawl_engine::actors::types::StageType,
    ) -> Option<Arc<dyn StageLogic>> {
        use crate::crawl_engine::stages::strategies::default::{
            DataSavingLogic, DataValidationLogic, ListPageLogic, ProductDetailLogic,
            StatusCheckLogic,
        };
        match stage_type {
            crate::crawl_engine::actors::types::StageType::StatusCheck => {
                Some(Arc::new(StatusCheckLogic))
            }
            crate::crawl_engine::actors::types::StageType::ListPageCrawling => {
                Some(Arc::new(ListPageLogic))
            }
            crate::crawl_engine::actors::types::StageType::ProductDetailCrawling => {
                Some(Arc::new(ProductDetailLogic))
            }
            crate::crawl_engine::actors::types::StageType::DataValidation => {
                Some(Arc::new(DataValidationLogic))
            }
            crate::crawl_engine::actors::types::StageType::DataSaving => {
                Some(Arc::new(DataSavingLogic))
            }
        }
    }
}
