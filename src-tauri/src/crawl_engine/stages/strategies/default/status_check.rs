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
        let StageInput { stage_type: st, item, config, deps, .. } = input;
        if !matches!(st, ActorStageType::StatusCheck) {
            return Err(StageLogicError::Unsupported(st));
        }
        let status_checker = Arc::new(
            crate::infrastructure::crawling_service_impls::StatusCheckerImpl::with_product_repo(
                (*deps.http).clone(),
                (*deps.extractor).clone(),
                config.clone(),
                Arc::clone(&deps.repo),
            ),
        );
        let status = status_checker
            .check_site_status()
            .await
            .map_err(|e| StageLogicError::Internal(format!("Status check failed: {}", e)))?;
        let json = serde_json::to_string(&status).map_err(|e| StageLogicError::Internal(e.to_string()))?;
        let result = StageItemResult {
            item_id: match item {
                StageItem::Page(n) => format!("page_{}", n),
                StageItem::Url(u) => u,
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
