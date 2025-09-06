use std::sync::Arc;

use crate::crawl_engine::actors::types::{
    EnhancedStageItemResult, StageItemResult, StageItemType, StageResultData,
    StageType as ActorStageType,
};
use crate::crawl_engine::channels::types as ch;
use crate::crawl_engine::stages::traits::{StageInput, StageLogic, StageLogicError, StageOutput};
use crate::domain::services::crawling_services::ProductDetailCollector;

pub struct ProductDetailLogic;

#[async_trait::async_trait]
impl StageLogic for ProductDetailLogic {
    fn name(&self) -> &'static str { "ProductDetailLogic" }

    async fn execute(&self, input: StageInput) -> Result<StageOutput, StageLogicError> {
        let start = std::time::Instant::now();
        let StageInput { stage_type: st, item, config, deps, .. } = input;
        if !matches!(st, ActorStageType::ProductDetailCrawling) {
            return Err(StageLogicError::Unsupported(st));
        }
        let urls = match item {
            ch::StageItem::ProductUrls(u) => u,
            other => {
                return Err(StageLogicError::Internal(format!(
                    "ProductDetailLogic expected ProductUrls, got {:?}",
                    other
                )));
            }
        };
        let collector: Arc<dyn ProductDetailCollector> = if let Some(fake) = &deps.detail_collector {
            Arc::clone(fake)
        } else {
            Arc::new(
                crate::infrastructure::crawling_service_impls::ProductDetailCollectorImpl::new(
                    Arc::clone(&deps.http),
                    Arc::clone(&deps.extractor),
                    crate::infrastructure::crawling_service_impls::CollectorConfig {
                        max_concurrent: config.user.crawling.workers.product_detail_max_concurrent as u32,
                        concurrency: config.user.crawling.workers.product_detail_max_concurrent as u32,
                        delay_between_requests: std::time::Duration::from_millis(config.user.request_delay_ms),
                        delay_ms: config.user.request_delay_ms,
                        batch_size: config.user.batch.batch_size,
                        retry_attempts: config.user.crawling.workers.max_retries,
                        retry_max: config.user.crawling.workers.max_retries,
                    },
                ),
            )
        };
        let details = collector
            .collect_details(&urls.urls)
            .await
            .map_err(|e| StageLogicError::Internal(format!("Detail collect failed: {}", e)))?;
        let attempted = urls.urls.len() as u32;
        let successful = details.len() as u32;
        let failed = attempted.saturating_sub(successful);
        let duration_ms = start.elapsed().as_millis() as u64;
        // Emit typed StageResultData and bridge to legacy JSON at the boundary
        let enhanced = EnhancedStageItemResult {
            item_id: format!("product_urls_{}", attempted),
            item_type: StageItemType::ProductUrls { urls: urls
                .urls
                .iter()
                .map(|u| u.url.clone())
                .collect() },
            success: true,
            error: None,
            duration_ms,
            retry_count: 0,
            collected_data: Some(StageResultData::ProductDetails {
                details,
                successful_count: successful,
                failed_count: failed,
            }),
        };
        let result: StageItemResult = enhanced.into();
        Ok(StageOutput { result })
    }
}
