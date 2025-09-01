use std::sync::Arc;

use crate::crawl_engine::actors::types::{StageItemResult, StageItemType, StageType as ActorStageType};
use crate::crawl_engine::channels::types::StageItem;
use crate::crawl_engine::stages::traits::{StageInput, StageLogic, StageLogicError, StageOutput};
use crate::domain::services::crawling_services::ProductListCollector;

pub struct ListPageLogic;

#[async_trait::async_trait]
impl StageLogic for ListPageLogic {
    fn name(&self) -> &'static str {
        "ListPageLogic"
    }

    async fn execute(&self, input: StageInput) -> Result<StageOutput, StageLogicError> {
        let start = std::time::Instant::now();
        let st = input.stage_type.clone();
        if !matches!(st, ActorStageType::ListPageCrawling) {
            return Err(StageLogicError::Unsupported(st));
        }
        let page_number = match &input.item {
            StageItem::Page(p) => *p,
            other => {
                return Err(StageLogicError::Internal(format!(
                    "ListPageLogic received unexpected item: {:?}",
                    other
                )));
            }
        };

        let cfg = crate::infrastructure::crawling_service_impls::CollectorConfig {
            max_concurrent: input.config.user.crawling.workers.list_page_max_concurrent as u32,
            concurrency: input.config.user.crawling.workers.list_page_max_concurrent as u32,
            delay_between_requests: std::time::Duration::from_millis(input.config.user.request_delay_ms),
            delay_ms: input.config.user.request_delay_ms,
            batch_size: input.config.user.batch.batch_size,
            retry_attempts: input.config.user.crawling.workers.max_retries,
            retry_max: input.config.user.crawling.workers.max_retries,
        };
        let collector: Arc<dyn ProductListCollector> = if let Some(fake) = &input.deps.list_collector {
            Arc::clone(fake)
        } else {
            Arc::new(
                crate::infrastructure::crawling_service_impls::ProductListCollectorImpl::new(
                    Arc::clone(&input.deps.http),
                    Arc::clone(&input.deps.extractor),
                    cfg,
                    // Provide a lightweight StatusChecker if needed internally by collector
                    Arc::new(
                        crate::infrastructure::crawling_service_impls::StatusCheckerImpl::with_product_repo(
                            (*input.deps.http).clone(),
                            (*input.deps.extractor).clone(),
                            input.config.clone(),
                            Arc::clone(&input.deps.repo),
                        ),
                    ),
                ),
            )
        };

        // Prefer hints to avoid extra status calls
        let (total_pages, products_on_last_page) = match (input.total_pages_hint, input.products_on_last_page_hint) {
            (Some(tp), Some(plp)) => (tp, plp),
            _ => (
                input.config.user.crawling.page_range_limit.max(1),
                12u32.min(crate::domain::constants::site::PRODUCTS_PER_PAGE as u32),
            ),
        };

        let urls = collector
            .collect_single_page(page_number, total_pages, products_on_last_page)
            .await
            .map_err(|e| StageLogicError::Internal(format!("List page collect failed: {}", e)))?;
        if urls.is_empty() {
            return Err(StageLogicError::Internal("Empty result from list page".into()));
        }
        let json = serde_json::to_string(&urls).map_err(|e| StageLogicError::Internal(e.to_string()))?;
        let duration_ms = start.elapsed().as_millis() as u64;
        let result = StageItemResult {
            item_id: format!("page_{}", page_number),
            item_type: StageItemType::Page { page_number },
            success: true,
            error: None,
            duration_ms,
            retry_count: 0,
            collected_data: Some(json),
        };
        Ok(StageOutput { result })
    }
}
