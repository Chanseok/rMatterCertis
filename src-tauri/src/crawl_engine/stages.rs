// Rust 2024 gate file for `crawl_engine::stages`
// Replaces directory-level mod.rs and pins submodules explicitly.

#[path = "stages/traits.rs"]
pub mod traits;

// Strategies gate (default family and future ones)
pub mod strategies;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crawl_engine::actors::types::StageType;

    #[test]
    fn test_factory_returns_all_strategies() {
        let f = DefaultStageLogicFactory;
        let cases = vec![
            (StageType::StatusCheck, "StatusCheckLogic"),
            (StageType::ListPageCrawling, "ListPageLogic"),
            (StageType::ProductDetailCrawling, "ProductDetailLogic"),
            (StageType::DataValidation, "DataValidationLogic"),
            (StageType::DataSaving, "DataSavingLogic"),
        ];
        for (st, expected_name) in cases {
            let logic = f.logic_for(&st).expect("strategy must exist");
            assert_eq!(logic.name(), expected_name);
        }
    }
}
