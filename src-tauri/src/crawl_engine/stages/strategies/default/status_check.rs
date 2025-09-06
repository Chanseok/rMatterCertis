use std::sync::Arc;

use crate::crawl_engine::actors::types::{
    StageItemResult, StageItemType, StageResultData, StageType as ActorStageType,
};
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
    let start = std::time::Instant::now();
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
        let duration_ms = start.elapsed().as_millis() as u64;
        let (item_id, item_type) = match item {
            StageItem::Page(n) => (format!("page_{}", n), StageItemType::Page { page_number: n }),
            StageItem::Url(u) => (u, StageItemType::Url { url_type: "site_check".into() }),
            _ => ("unknown".into(), StageItemType::SiteCheck),
        };
    let enhanced = StageItemResult {
            item_id,
            item_type,
            success: true,
            error: None,
            duration_ms,
            retry_count: 0,
            collected_data: Some(StageResultData::StatusCheck {
                site_available: status.is_accessible,
                total_pages: Some(status.total_pages),
                last_page_products: Some(status.products_on_last_page),
                response_time_ms: status.response_time_ms,
            }),
        };
    Ok(StageOutput { result: enhanced })
    }
}
