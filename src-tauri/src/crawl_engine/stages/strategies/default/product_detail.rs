use std::sync::Arc;

use crate::crawl_engine::actors::types::{
    StageItemResult, StageItemType, StageResultData, StageType as ActorStageType,
};
use crate::crawl_engine::channels::types as ch;
use crate::crawl_engine::stages::traits::{StageInput, StageLogic, StageLogicError, StageOutput};
use crate::domain::services::crawling_services::ProductDetailCollector;

pub struct ProductDetailLogic;

#[async_trait::async_trait]
impl StageLogic for ProductDetailLogic {
    fn name(&self) -> &'static str {
        "ProductDetailLogic"
    }

    async fn execute(&self, input: StageInput) -> Result<StageOutput, StageLogicError> {
        let start = std::time::Instant::now();
        // We need access to product_detail_event_emitter; avoid destructuring it away
        let st = input.stage_type.clone();
        let urls_item = input.item.clone();
        let config = input.config.clone();
        let deps = input.deps.clone();
        let product_detail_event_emitter = input.product_detail_event_emitter.clone();
        if !matches!(st, ActorStageType::ProductDetailCrawling) {
            return Err(StageLogicError::Unsupported(st));
        }
        let urls = match urls_item {
            ch::StageItem::ProductUrls(u) => u,
            ch::StageItem::ProductUrl(single_url) => {
                // 개별 ProductUrl을 ProductUrls 번들로 래핑
                ch::ProductUrls {
                    urls: vec![single_url],
                    batch_id: None,
                }
            },
            other => {
                return Err(StageLogicError::Internal(format!(
                    "ProductDetailLogic expected ProductUrls or ProductUrl, got {:?}",
                    other
                )));
            }
        };
        let collector: Arc<dyn ProductDetailCollector> = if let Some(fake) = &deps.detail_collector
        {
            Arc::clone(fake)
        } else {
            let base = crate::infrastructure::crawling_service_impls::ProductDetailCollectorImpl::new(
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
            );
            let base = if let Some(ref em) = product_detail_event_emitter {
                base.with_event_emitter(Arc::clone(em))
            } else { base };
            Arc::new(base)
        };
        // Refactored: call collector once with all URLs; collector internally emits keyed events (fetch/parse/persist) via injected emitter
        let attempted = urls.urls.len() as u32;
        let mut details: Vec<crate::domain::product::ProductDetail> = Vec::new();
        let mut successful = 0u32;
        match collector.collect_details(&urls.urls).await {
            Ok(mut v) => {
                successful = v.len() as u32;
                details.append(&mut v);
            }
            Err(_e) => {
                // On aggregate failure we leave successful=0; keyed per-item failures already emitted by collector
            }
        }
        let failed = attempted.saturating_sub(successful);
        let duration_ms = start.elapsed().as_millis() as u64;
        // Emit typed StageResultData and bridge to legacy JSON at the boundary
        let (item_id, item_type) = if urls.urls.len() == 1 {
            // 개별 ProductUrl의 경우
            let url = &urls.urls[0];
            (
                url.url.clone(),
                StageItemType::ProductDetail {
                    url: url.url.clone(),
                    page_id: url.page_id,
                    index_in_page: url.index_in_page,
                }
            )
        } else {
            // 기존 ProductUrls 번들의 경우
            (
                format!("product_urls_{}", attempted),
                StageItemType::ProductUrls {
                    urls: urls.urls.iter().map(|u| u.url.clone()).collect(),
                }
            )
        };
        
        let enhanced = StageItemResult {
            item_id,
            item_type,
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
        Ok(StageOutput { result: enhanced })
    }
}
