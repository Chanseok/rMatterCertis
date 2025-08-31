use std::sync::Arc;

use crate::crawl_engine::actors::types::{StageItemResult, StageItemType, StageType as ActorStageType};
use crate::crawl_engine::channels::types as ch;
use crate::crawl_engine::stages::traits::{StageInput, StageLogic, StageLogicError, StageOutput};
use crate::domain::services::crawling_services::ProductDetailCollector;

pub struct ProductDetailLogic;

#[async_trait::async_trait]
impl StageLogic for ProductDetailLogic {
    fn name(&self) -> &'static str { "ProductDetailLogic" }

    async fn execute(&self, input: StageInput) -> Result<StageOutput, StageLogicError> {
        let st = input.stage_type.clone();
        if !matches!(st, ActorStageType::ProductDetailCrawling) {
            return Err(StageLogicError::Unsupported(st));
        }
        let urls = match &input.item {
            ch::StageItem::ProductUrls(u) => u.clone(),
            other => {
                return Err(StageLogicError::Internal(format!(
                    "ProductDetailLogic expected ProductUrls, got {:?}",
                    other
                )));
            }
        };
        let collector: Arc<dyn ProductDetailCollector> = if let Some(fake) = &input.deps.detail_collector {
            Arc::clone(fake)
        } else {
            Arc::new(
                crate::infrastructure::crawling_service_impls::ProductDetailCollectorImpl::new(
                    Arc::clone(&input.deps.http),
                    Arc::clone(&input.deps.extractor),
                    crate::infrastructure::crawling_service_impls::CollectorConfig {
                        max_concurrent: input.config.user.crawling.workers.product_detail_max_concurrent as u32,
                        concurrency: input.config.user.crawling.workers.product_detail_max_concurrent as u32,
                        delay_between_requests: std::time::Duration::from_millis(input.config.user.request_delay_ms),
                        delay_ms: input.config.user.request_delay_ms,
                        batch_size: input.config.user.batch.batch_size,
                        retry_attempts: input.config.user.crawling.workers.max_retries,
                        retry_max: input.config.user.crawling.workers.max_retries,
                    },
                ),
            )
        };
        let details = collector
            .collect_details(&urls.urls)
            .await
            .map_err(|e| StageLogicError::Internal(format!("Detail collect failed: {}", e)))?;
        let wrapper = ch::ProductDetails {
            products: details.clone(),
            source_urls: urls.urls.clone(),
            extraction_stats: ch::ExtractionStats {
                attempted: urls.urls.len() as u32,
                successful: details.len() as u32,
                failed: (urls.urls.len().saturating_sub(details.len())) as u32,
                empty_responses: 0,
            },
        };
        let json = serde_json::to_string(&wrapper).map_err(|e| StageLogicError::Internal(e.to_string()))?;
        let result = StageItemResult {
            item_id: format!("product_urls_{}", wrapper.source_urls.len()),
            item_type: StageItemType::ProductUrls { urls: wrapper.source_urls.iter().map(|u| u.url.clone()).collect() },
            success: true,
            error: None,
            duration_ms: 0,
            retry_count: 0,
            collected_data: Some(json),
        };
        Ok(StageOutput { result })
    }
}
