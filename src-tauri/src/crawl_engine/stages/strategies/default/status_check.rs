use std::sync::Arc;

use crate::crawl_engine::actors::types::{StageItemResult, StageItemType, StageType as ActorStageType};
use crate::crawl_engine::channels::types::StageItem;
use crate::crawl_engine::stages::traits::{StageInput, StageLogic, StageLogicError, StageOutput};
use crate::domain::services::crawling_services::StatusChecker;

pub struct StatusCheckLogic;

#[async_trait::async_trait]
impl StageLogic for StatusCheckLogic {
    fn name(&self) -> &'static str { "StatusCheckLogic" }

    async fn execute(&self, input: StageInput) -> Result<StageOutput, StageLogicError> {
        let st = input.stage_type.clone();
        if !matches!(st, ActorStageType::StatusCheck) {
            return Err(StageLogicError::Unsupported(st));
        }
        let status_checker = Arc::new(
            crate::infrastructure::crawling_service_impls::StatusCheckerImpl::with_product_repo(
                (*input.deps.http).clone(),
                (*input.deps.extractor).clone(),
                input.config.clone(),
                Arc::clone(&input.deps.repo),
            ),
        );
        let status = status_checker
            .check_site_status()
            .await
            .map_err(|e| StageLogicError::Internal(format!("Status check failed: {}", e)))?;
        let json = serde_json::to_string(&status).map_err(|e| StageLogicError::Internal(e.to_string()))?;
        let result = StageItemResult {
            item_id: match &input.item {
                StageItem::Page(n) => format!("page_{}", n),
                StageItem::Url(u) => u.clone(),
                _ => "unknown".into(),
            },
            item_type: StageItemType::Url { url_type: "site_check".into() },
            success: true,
            error: None,
            duration_ms: 0,
            retry_count: 0,
            collected_data: Some(json),
        };
        Ok(StageOutput { result })
    }
}
