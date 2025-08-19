pub mod traits;

mod strategies {
    use super::traits::*;
    use std::sync::Arc;
    use crate::new_architecture::actors::types::{StageItemType, StageType as ActorStageType};
    use crate::domain::services::{ProductListCollector, StatusChecker};

    pub struct ListPageLogic;

    #[async_trait::async_trait]
    impl StageLogic for ListPageLogic {
        async fn execute(&self, input: StageInput) -> Result<StageOutput, StageLogicError> {
            let start = std::time::Instant::now();
            // Only supports ListPageCrawling
            let st = input.stage_type.clone();
            if !matches!(st, ActorStageType::ListPageCrawling) {
                return Err(StageLogicError::Unsupported(st));
            }
            // Expect a Page item; for other inputs, signal internal error
            let page_number = match &input.item {
                crate::new_architecture::channels::types::StageItem::Page(p) => *p,
                other => {
                    return Err(StageLogicError::Internal(format!(
                        "ListPageLogic received unexpected item: {:?}",
                        other
                    )));
                }
            };

            // Use product list collector via deps: derive URLs, reuse existing collectors via HttpClient/DataExtractor/Repo
            // For now, directly reuse the existing ProductListCollectorImpl via a minimal local instance mirroring StageActor logic.
            let status_checker = Arc::new(
                crate::infrastructure::crawling_service_impls::StatusCheckerImpl::with_product_repo(
                    (*input.deps.http).clone(),
                    (*input.deps.extractor).clone(),
                    input.config.clone(),
                    Arc::clone(&input.deps.repo),
                ),
            );
            let cfg = crate::infrastructure::crawling_service_impls::CollectorConfig {
                max_concurrent: input.config.user.crawling.workers.list_page_max_concurrent as u32,
                concurrency: input.config.user.crawling.workers.list_page_max_concurrent as u32,
                delay_between_requests: std::time::Duration::from_millis(input.config.user.request_delay_ms),
                delay_ms: input.config.user.request_delay_ms,
                batch_size: input.config.user.batch.batch_size,
                retry_attempts: input.config.user.crawling.workers.max_retries,
                retry_max: input.config.user.crawling.workers.max_retries,
            };
            let collector = crate::infrastructure::crawling_service_impls::ProductListCollectorImpl::new(
                Arc::clone(&input.deps.http),
                Arc::clone(&input.deps.extractor),
                cfg,
                Arc::clone(&status_checker),
            );

            // Determine pagination hints via StatusChecker for parity with StageActor
            let (total_pages, products_on_last_page) = match StatusChecker::check_site_status(&*status_checker).await {
                Ok(status) => (status.total_pages, status.products_on_last_page),
                Err(_) => (100u32, 10u32),
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
            let result = crate::new_architecture::actors::types::StageItemResult {
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
}

use std::sync::Arc;
use traits::{StageLogic, StageLogicFactory};

/// Placeholder factory; will be expanded as strategies are implemented.
pub struct DefaultStageLogicFactory;

impl StageLogicFactory for DefaultStageLogicFactory {
    fn logic_for(&self, stage_type: &crate::new_architecture::actors::types::StageType) -> Option<Arc<dyn StageLogic>> {
        match stage_type {
            crate::new_architecture::actors::types::StageType::ListPageCrawling => Some(Arc::new(strategies::ListPageLogic)),
            _ => None,
        }
    }
}
