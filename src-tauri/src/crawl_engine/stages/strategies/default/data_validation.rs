use crate::crawl_engine::actors::types::{StageItemResult, StageItemType, StageType as ActorStageType};
use crate::crawl_engine::channels::types as ch;
use crate::crawl_engine::stages::traits::{StageInput, StageLogic, StageLogicError, StageOutput};

pub struct DataValidationLogic;

#[async_trait::async_trait]
impl StageLogic for DataValidationLogic {
    fn name(&self) -> &'static str { "DataValidationLogic" }

    async fn execute(&self, input: StageInput) -> Result<StageOutput, StageLogicError> {
        let StageInput { stage_type: st, item, .. } = input;
        if !matches!(st, ActorStageType::DataValidation) {
            return Err(StageLogicError::Unsupported(st));
        }
        use crate::crawl_engine::services::data_quality_analyzer::DataQualityAnalyzer;
        let details_vec: Vec<crate::domain::product::ProductDetail> = match item {
            ch::StageItem::ProductDetails(pd) => pd.products,
            other => {
                return Err(StageLogicError::Internal(format!(
                    "DataValidation expected ProductDetails, got {:?}",
                    other
                )));
            }
        };
        let analyzer = DataQualityAnalyzer::new();
        let validated = analyzer
            .validate_before_storage(&details_vec)
            .map_err(|e| StageLogicError::Internal(format!("Validation failed: {}", e)))?;
        let json = serde_json::to_string(&validated).map_err(|e| StageLogicError::Internal(e.to_string()))?;
        let result = StageItemResult {
            item_id: format!("validated_products_{}", validated.len()),
            item_type: StageItemType::Url { url_type: "validated_products".into() },
            success: true,
            error: None,
            duration_ms: 0,
            retry_count: 0,
            collected_data: Some(json),
        };
        Ok(StageOutput { result })
    }
}
