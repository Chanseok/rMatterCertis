// Default strategy implementations for each Stage

use std::sync::Arc;
use crate::crawl_engine::stages::traits::{StageLogic, StageLogicError, StageInput, StageOutput};
use crate::crawl_engine::actors::types::{StageItemType, StageType as ActorStageType};
// Bring trait methods into scope for collector impls
use crate::domain::services::crawling_services::{ProductListCollector, ProductDetailCollector};
use crate::domain::services::StatusChecker;

pub struct ListPageLogic;
pub struct StatusCheckLogic;
pub struct ProductDetailLogic;
pub struct DataValidationLogic;
pub struct DataSavingLogic;

#[async_trait::async_trait]
impl StageLogic for ListPageLogic {
    fn name(&self) -> &'static str { "ListPageLogic" }
    async fn execute(&self, input: StageInput) -> Result<StageOutput, StageLogicError> {
        let start = std::time::Instant::now();
        let st = input.stage_type.clone();
        if !matches!(st, ActorStageType::ListPageCrawling) {
            return Err(StageLogicError::Unsupported(st));
        }
        let page_number = match &input.item {
            crate::crawl_engine::channels::types::StageItem::Page(p) => *p,
            other => return Err(StageLogicError::Internal(format!("ListPageLogic received unexpected item: {:?}", other))),
        };

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

    let (total_pages, products_on_last_page) = match status_checker.check_site_status().await {
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
    let result = crate::crawl_engine::actors::types::StageItemResult {
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
    let result = crate::crawl_engine::actors::types::StageItemResult {
            item_id: match &input.item { crate::crawl_engine::channels::types::StageItem::Page(n) => format!("page_{}", n), crate::crawl_engine::channels::types::StageItem::Url(u) => u.clone(), _ => "unknown".into() },
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

#[async_trait::async_trait]
impl StageLogic for ProductDetailLogic {
    fn name(&self) -> &'static str { "ProductDetailLogic" }
    async fn execute(&self, input: StageInput) -> Result<StageOutput, StageLogicError> {
        let st = input.stage_type.clone();
        if !matches!(st, ActorStageType::ProductDetailCrawling) {
            return Err(StageLogicError::Unsupported(st));
        }
    use crate::crawl_engine::channels::types::{ProductDetails, ExtractionStats};
        let urls = match &input.item {
            crate::crawl_engine::channels::types::StageItem::ProductUrls(u) => u.clone(),
            other => return Err(StageLogicError::Internal(format!("ProductDetailLogic expected ProductUrls, got {:?}", other))),
        };
        let collector = crate::infrastructure::crawling_service_impls::ProductDetailCollectorImpl::new(
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
        );
        let details = collector
            .collect_details(&urls.urls)
            .await
            .map_err(|e| StageLogicError::Internal(format!("Detail collect failed: {}", e)))?;
        let wrapper = ProductDetails {
            products: details.clone(),
            source_urls: urls.urls.clone(),
            extraction_stats: ExtractionStats {
                attempted: urls.urls.len() as u32,
                successful: details.len() as u32,
                failed: (urls.urls.len().saturating_sub(details.len())) as u32,
                empty_responses: 0,
            },
        };
        let json = serde_json::to_string(&wrapper).map_err(|e| StageLogicError::Internal(e.to_string()))?;
    let result = crate::crawl_engine::actors::types::StageItemResult {
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

#[async_trait::async_trait]
impl StageLogic for DataValidationLogic {
    fn name(&self) -> &'static str { "DataValidationLogic" }
    async fn execute(&self, input: StageInput) -> Result<StageOutput, StageLogicError> {
        let st = input.stage_type.clone();
        if !matches!(st, ActorStageType::DataValidation) {
            return Err(StageLogicError::Unsupported(st));
        }
    use crate::crawl_engine::services::data_quality_analyzer::DataQualityAnalyzer;
        let details_vec: Vec<crate::domain::product::ProductDetail> = match &input.item {
            crate::crawl_engine::channels::types::StageItem::ProductDetails(pd) => pd.products.clone(),
            other => return Err(StageLogicError::Internal(format!("DataValidation expected ProductDetails, got {:?}", other))),
        };
        let analyzer = DataQualityAnalyzer::new();
        let validated = analyzer
            .validate_before_storage(&details_vec)
            .map_err(|e| StageLogicError::Internal(format!("Validation failed: {}", e)))?;
        let json = serde_json::to_string(&validated).map_err(|e| StageLogicError::Internal(e.to_string()))?;
    let result = crate::crawl_engine::actors::types::StageItemResult {
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

#[async_trait::async_trait]
impl StageLogic for DataSavingLogic {
    fn name(&self) -> &'static str { "DataSavingLogic" }
    async fn execute(&self, input: StageInput) -> Result<StageOutput, StageLogicError> {
        let st = input.stage_type.clone();
        if !matches!(st, ActorStageType::DataSaving) {
            return Err(StageLogicError::Unsupported(st));
        }
        let (attempted, item_id, item_type) = match &input.item {
            crate::crawl_engine::channels::types::StageItem::ProductDetails(pd) => {
                let count = pd.products.len();
                (count as u32, format!("persist_product_details_{}", count), StageItemType::Url { url_type: "data_saving:product_details".into() })
            }
            crate::crawl_engine::channels::types::StageItem::ValidatedProducts(vp) => {
                let count = vp.products.len();
                (count as u32, format!("persist_validated_{}", count), StageItemType::Url { url_type: "data_saving:validated_products".into() })
            }
            other => return Err(StageLogicError::Internal(format!("DataSaving expected ProductDetails|ValidatedProducts, got {:?}", other))),
        };
    let result = crate::crawl_engine::actors::types::StageItemResult {
            item_id,
            item_type,
            success: true,
            error: None,
            duration_ms: 0,
            retry_count: 0,
            collected_data: Some(format!("{{\"attempted\":{}}}", attempted)),
        };
        Ok(StageOutput { result })
    }
}
