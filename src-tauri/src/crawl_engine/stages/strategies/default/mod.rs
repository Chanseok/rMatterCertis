// Default strategy implementations for each Stage

use crate::crawl_engine::actors::types::{StageItemType, StageType as ActorStageType};
use crate::crawl_engine::stages::traits::{StageInput, StageLogic, StageLogicError, StageOutput};
use std::sync::Arc;
// Bring trait methods into scope for collector impls
use crate::domain::services::StatusChecker;
use crate::domain::services::crawling_services::{ProductDetailCollector, ProductListCollector};

pub struct ListPageLogic;
pub struct StatusCheckLogic;
pub struct ProductDetailLogic;
pub struct DataValidationLogic;
pub struct DataSavingLogic;

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
            crate::crawl_engine::channels::types::StageItem::Page(p) => *p,
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
            delay_between_requests: std::time::Duration::from_millis(
                input.config.user.request_delay_ms,
            ),
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
                    // Collector still needs a StatusChecker for edge cases (e.g., Retry-After);
                    // reuse a lightweight checker instance but do NOT call site-level status here.
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

        // Use injected pagination hints (from StageActor/BatchActor) to avoid per-item site status calls
        let (total_pages, products_on_last_page) =
            match (input.total_pages_hint, input.products_on_last_page_hint) {
                (Some(tp), Some(plp)) => (tp, plp),
                // Fallbacks if hints are missing (should be rare): use conservative defaults
                _ => (
                    input.config.user.crawling.page_range_limit.max(1),
                    12u32.min(crate::domain::constants::site::PRODUCTS_PER_PAGE as u32),
                ),
            };

        let urls = collector
            .collect_single_page(page_number, total_pages, products_on_last_page)
            .await
            .map_err(|e| StageLogicError::Internal(format!("List page collect failed: {}", e)))?;
        // 빈 결과 또는 비마지막 페이지에서 기대 수량(12) 미만은 내부 Collector에서 재시도 처리됨.
        // 여기서는 최종적으로 0개인 경우만 실패로 처리.
        if urls.is_empty() {
            return Err(StageLogicError::Internal(
                "Empty result from list page".into(),
            ));
        }
        let json =
            serde_json::to_string(&urls).map_err(|e| StageLogicError::Internal(e.to_string()))?;
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
    fn name(&self) -> &'static str {
        "StatusCheckLogic"
    }
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
        let json =
            serde_json::to_string(&status).map_err(|e| StageLogicError::Internal(e.to_string()))?;
        let result = crate::crawl_engine::actors::types::StageItemResult {
            item_id: match &input.item {
                crate::crawl_engine::channels::types::StageItem::Page(n) => format!("page_{}", n),
                crate::crawl_engine::channels::types::StageItem::Url(u) => u.clone(),
                _ => "unknown".into(),
            },
            item_type: StageItemType::Url {
                url_type: "site_check".into(),
            },
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
    fn name(&self) -> &'static str {
        "ProductDetailLogic"
    }
    async fn execute(&self, input: StageInput) -> Result<StageOutput, StageLogicError> {
        let st = input.stage_type.clone();
        if !matches!(st, ActorStageType::ProductDetailCrawling) {
            return Err(StageLogicError::Unsupported(st));
        }
        use crate::crawl_engine::channels::types::{ExtractionStats, ProductDetails};
        let urls = match &input.item {
            crate::crawl_engine::channels::types::StageItem::ProductUrls(u) => u.clone(),
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
                        max_concurrent: input
                            .config
                            .user
                            .crawling
                            .workers
                            .product_detail_max_concurrent as u32,
                        concurrency: input
                            .config
                            .user
                            .crawling
                            .workers
                            .product_detail_max_concurrent as u32,
                        delay_between_requests: std::time::Duration::from_millis(
                            input.config.user.request_delay_ms,
                        ),
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
        let json = serde_json::to_string(&wrapper)
            .map_err(|e| StageLogicError::Internal(e.to_string()))?;
        let result = crate::crawl_engine::actors::types::StageItemResult {
            item_id: format!("product_urls_{}", wrapper.source_urls.len()),
            item_type: StageItemType::ProductUrls {
                urls: wrapper.source_urls.iter().map(|u| u.url.clone()).collect(),
            },
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
    fn name(&self) -> &'static str {
        "DataValidationLogic"
    }
    async fn execute(&self, input: StageInput) -> Result<StageOutput, StageLogicError> {
        let st = input.stage_type.clone();
        if !matches!(st, ActorStageType::DataValidation) {
            return Err(StageLogicError::Unsupported(st));
        }
        use crate::crawl_engine::services::data_quality_analyzer::DataQualityAnalyzer;
        let details_vec: Vec<crate::domain::product::ProductDetail> = match &input.item {
            crate::crawl_engine::channels::types::StageItem::ProductDetails(pd) => {
                pd.products.clone()
            }
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
        let json = serde_json::to_string(&validated)
            .map_err(|e| StageLogicError::Internal(e.to_string()))?;
        let result = crate::crawl_engine::actors::types::StageItemResult {
            item_id: format!("validated_products_{}", validated.len()),
            item_type: StageItemType::Url {
                url_type: "validated_products".into(),
            },
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
    fn name(&self) -> &'static str {
        "DataSavingLogic"
    }
    async fn execute(&self, input: StageInput) -> Result<StageOutput, StageLogicError> {
        let st = input.stage_type.clone();
        if !matches!(st, ActorStageType::DataSaving) {
            return Err(StageLogicError::Unsupported(st));
        }
        // Select products vector based on item type
        let (products, item_id, item_type) = match &input.item {
            crate::crawl_engine::channels::types::StageItem::ProductDetails(pd) => (
                &pd.products,
                format!("persist_product_details_{}", pd.products.len()),
                StageItemType::Url {
                    url_type: "data_saving:product_details".into(),
                },
            ),
            crate::crawl_engine::channels::types::StageItem::ValidatedProducts(vp) => (
                &vp.products,
                format!("persist_validated_{}", vp.products.len()),
                StageItemType::Url {
                    url_type: "data_saving:validated_products".into(),
                },
            ),
            other => {
                return Err(StageLogicError::Internal(format!(
                    "DataSaving expected ProductDetails|ValidatedProducts, got {:?}",
                    other
                )));
            }
        };

        // Persist each product detail; count inserts/updates using repository helpers
        let repo = input.deps.repo;
        let policy = input.deps.duplicate_policy.clone();
        let mut inserted: u32 = 0;
        let mut updated: u32 = 0;
        for detail in products {
            // create_or_update_product_detail internally upserts both product and product_details
            match repo.create_or_update_product_detail(detail).await {
                Ok((was_updated, was_created)) => {
                    if was_created {
                        inserted = inserted.saturating_add(1);
                    }
                    if was_updated {
                        updated = updated.saturating_add(1);
                    }
                    if !was_created && !was_updated
                        && policy == crate::crawl_engine::actors::types::DuplicatePersistencePolicy::UpdateIdIndexOnly {
                            if let (Some(pid), Some(idx)) = (detail.page_id, detail.index_in_page) {
                                // force-update positions for existing URL
                                let _ = repo
                                    .force_update_position_by_url(&detail.url, pid, idx)
                                    .await;
                            }
                        }
                }
                Err(e) => {
                    return Err(StageLogicError::Internal(format!(
                        "Persistence failed for URL {}: {}",
                        detail.url, e
                    )));
                }
            }
        }
        let attempted = products.len() as u32;
        let payload = serde_json::json!({
            "attempted": attempted,
            "products_inserted": inserted,
            "products_updated": updated
        });
        let result = crate::crawl_engine::actors::types::StageItemResult {
            item_id,
            item_type,
            success: true,
            error: None,
            duration_ms: 0,
            retry_count: 0,
            collected_data: Some(payload.to_string()),
        };
        Ok(StageOutput { result })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crawl_engine::channels::types as ch;
    use crate::crawl_engine::stages::traits::{Deps, StageInput};
    use std::sync::Arc;
    use crate::domain::services::crawling_services as svc;

    fn deps_stub() -> Deps {
        // Use default config paths; these dependencies may still do real work. For unit scope, we won't invoke network.
        let cfg = crate::infrastructure::config::AppConfig::default();
        let http = Arc::new(cfg.create_http_client().expect("http client"));
        let extractor = Arc::new(
            crate::infrastructure::MatterDataExtractor::new().expect("extractor"),
        );
        let pool = futures::executor::block_on(
            crate::infrastructure::database_connection::get_or_init_global_pool(),
        )
        .expect("pool");
        let repo = Arc::new(crate::infrastructure::IntegratedProductRepository::new(pool));
        Deps {
            http,
            extractor,
            repo,
            duplicate_policy:
                crate::crawl_engine::actors::types::DuplicatePersistencePolicy::Skip,
            list_collector: None,
            detail_collector: None,
        }
    }

    #[tokio::test]
    async fn list_page_logic_unsupported_item_errors() {
        let logic = ListPageLogic;
        let input = StageInput {
            stage_type: crate::crawl_engine::actors::types::StageType::ListPageCrawling,
            item: ch::StageItem::Url("https://example.com".into()),
            config: crate::infrastructure::config::AppConfig::default(),
            deps: deps_stub(),
            total_pages_hint: Some(1),
            products_on_last_page_hint: Some(12),
        };
        let res = logic.execute(input).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn product_detail_logic_rejects_wrong_item() {
        let logic = ProductDetailLogic;
        let input = StageInput {
            stage_type: crate::crawl_engine::actors::types::StageType::ProductDetailCrawling,
            item: ch::StageItem::Page(1),
            config: crate::infrastructure::config::AppConfig::default(),
            deps: deps_stub(),
            total_pages_hint: Some(1),
            products_on_last_page_hint: Some(12),
        };
        let res = logic.execute(input).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn data_saving_logic_rejects_wrong_item() {
        let logic = DataSavingLogic;
        let input = StageInput {
            stage_type: crate::crawl_engine::actors::types::StageType::DataSaving,
            item: ch::StageItem::Url("not_a_products_payload".into()),
            config: crate::infrastructure::config::AppConfig::default(),
            deps: deps_stub(),
            total_pages_hint: None,
            products_on_last_page_hint: None,
        };
        let res = logic.execute(input).await;
        assert!(res.is_err());
    }

    // Intentionally omit network/DB-hitting happy-path tests in unit scope.

    // --- Hermetic happy-path tests (no network/real DB) ---

    async fn memory_repo() -> Arc<crate::infrastructure::IntegratedProductRepository> {
        // In-memory sqlite (no filesystem I/O)
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory pool");
        Arc::new(crate::infrastructure::IntegratedProductRepository::new(pool))
    }

    fn deps_stub_memory_sync(repo: Arc<crate::infrastructure::IntegratedProductRepository>) -> Deps {
        let cfg = crate::infrastructure::config::AppConfig::default();
        let http = Arc::new(cfg.create_http_client().expect("http client"));
        let extractor = Arc::new(
            crate::infrastructure::MatterDataExtractor::new().expect("extractor"),
        );
        Deps {
            http,
            extractor,
            repo,
            duplicate_policy:
                crate::crawl_engine::actors::types::DuplicatePersistencePolicy::Skip,
            list_collector: None,
            detail_collector: None,
        }
    }

    #[tokio::test]
    async fn product_detail_logic_happy_path_with_empty_urls_is_success() {
        let logic = ProductDetailLogic;
        let repo = memory_repo().await;
        let deps = deps_stub_memory_sync(repo);
        let input = StageInput {
            stage_type: crate::crawl_engine::actors::types::StageType::ProductDetailCrawling,
            item: ch::StageItem::ProductUrls(ch::ProductUrls { urls: vec![], batch_id: None }),
            config: crate::infrastructure::config::AppConfig::default(),
            deps,
            total_pages_hint: Some(1),
            products_on_last_page_hint: Some(12),
        };
        let res = logic.execute(input).await.expect("happy path");
        assert!(res.result.success);
        // Collected data present (empty group serialized)
        assert!(res.result.collected_data.is_some());
    }

    #[tokio::test]
    async fn data_validation_logic_happy_path_validates_in_memory_details() {
        let logic = DataValidationLogic;
        let repo = memory_repo().await; // not used but required by Deps
        let deps = deps_stub_memory_sync(repo);
        // Minimal valid ProductDetail entries
        let now = chrono::Utc::now();
        let pd1 = crate::domain::product::ProductDetail {
            url: "https://example.com/p1".into(),
            page_id: Some(1),
            index_in_page: Some(1),
            id: None,
            manufacturer: Some("Acme".into()),
            model: Some("M1".into()),
            device_type: None,
            certificate_id: None,
            certification_date: None,
            software_version: None,
            hardware_version: None,
            vid: None,
            pid: None,
            family_sku: None,
            family_variant_sku: None,
            firmware_version: None,
            family_id: None,
            tis_trp_tested: None,
            specification_version: None,
            transport_interface: None,
            primary_device_type_id: None,
            application_categories: None,
            description: None,
            compliance_document_url: None,
            program_type: Some("Matter".into()),
            created_at: now,
            updated_at: now,
        };
        let pd2 = crate::domain::product::ProductDetail { url: "https://example.com/p2".into(), ..pd1.clone() };

        let input = StageInput {
            stage_type: crate::crawl_engine::actors::types::StageType::DataValidation,
            item: ch::StageItem::ProductDetails(ch::ProductDetails {
                products: vec![pd1, pd2],
                source_urls: vec![],
                extraction_stats: ch::ExtractionStats { attempted: 2, successful: 2, failed: 0, empty_responses: 0 },
            }),
            config: crate::infrastructure::config::AppConfig::default(),
            deps,
            total_pages_hint: None,
            products_on_last_page_hint: None,
        };
        let res = logic.execute(input).await.expect("happy path");
        assert!(res.result.success);
        // Should serialize validated list
        let json = res.result.collected_data.expect("validated json");
        let parsed: Vec<crate::domain::product::ProductDetail> = serde_json::from_str(&json).expect("parse");
        assert_eq!(parsed.len(), 2);
    }

    #[tokio::test]
    async fn data_saving_logic_happy_path_with_empty_validated_products_is_noop_success() {
        let logic = DataSavingLogic;
        let repo = memory_repo().await; // present but unused due to empty input
        let deps = deps_stub_memory_sync(repo);
        let input = StageInput {
            stage_type: crate::crawl_engine::actors::types::StageType::DataSaving,
            item: ch::StageItem::ValidatedProducts(ch::ValidatedProducts {
                products: vec![],
                validation_report: None,
                storage_recommendation: ch::StorageRecommendation::HighlyRecommended,
            }),
            config: crate::infrastructure::config::AppConfig::default(),
            deps,
            total_pages_hint: None,
            products_on_last_page_hint: None,
        };
        let res = logic.execute(input).await.expect("happy path");
        assert!(res.result.success);
        // JSON payload contains zero counts
        let json = res.result.collected_data.expect("payload json");
        let v: serde_json::Value = serde_json::from_str(&json).expect("json");
        assert_eq!(v.get("attempted").and_then(|n| n.as_u64()).unwrap_or_default(), 0);
        assert_eq!(v.get("products_inserted").and_then(|n| n.as_u64()).unwrap_or_default(), 0);
        assert_eq!(v.get("products_updated").and_then(|n| n.as_u64()).unwrap_or_default(), 0);
    }

    // --- Fakes for hermetic non-empty tests ---
    struct FakeListCollector {
        urls: Vec<crate::domain::product_url::ProductUrl>,
    }
    #[async_trait::async_trait]
    impl svc::ProductListCollector for FakeListCollector {
        async fn collect_all_pages(&self, _tp: u32, _plp: u32) -> anyhow::Result<Vec<crate::domain::product_url::ProductUrl>> { Ok(self.urls.clone()) }
        async fn collect_page_range(&self, _s: u32, _e: u32, _tp: u32, _plp: u32) -> anyhow::Result<Vec<crate::domain::product_url::ProductUrl>> { Ok(self.urls.clone()) }
        async fn collect_page_range_with_cancellation(&self, _s: u32, _e: u32, _tp: u32, _plp: u32, _ct: tokio_util::sync::CancellationToken) -> anyhow::Result<Vec<crate::domain::product_url::ProductUrl>> { Ok(self.urls.clone()) }
        async fn collect_single_page(&self, _p: u32, _tp: u32, _plp: u32) -> anyhow::Result<Vec<crate::domain::product_url::ProductUrl>> { Ok(self.urls.clone()) }
        async fn collect_page_batch(&self, _pages: &[u32], _tp: u32, _plp: u32) -> anyhow::Result<Vec<crate::domain::product_url::ProductUrl>> { Ok(self.urls.clone()) }
        fn as_any(&self) -> &dyn std::any::Any { self }
    }

    struct FakeDetailCollector {
        details: Vec<crate::domain::product::ProductDetail>,
    }
    #[async_trait::async_trait]
    impl svc::ProductDetailCollector for FakeDetailCollector {
        async fn collect_details(&self, _urls: &[crate::domain::product_url::ProductUrl]) -> anyhow::Result<Vec<crate::domain::product::ProductDetail>> { Ok(self.details.clone()) }
        async fn collect_details_with_cancellation(&self, _urls: &[crate::domain::product_url::ProductUrl], _ct: tokio_util::sync::CancellationToken) -> anyhow::Result<Vec<crate::domain::product::ProductDetail>> { Ok(self.details.clone()) }
        async fn collect_single_product(&self, _url: &crate::domain::product_url::ProductUrl) -> anyhow::Result<crate::domain::product::ProductDetail> { Ok(self.details.first().cloned().unwrap_or_else(|| crate::domain::product::ProductDetail{ url: "u".into(), page_id: Some(1), index_in_page: Some(1), id: None, manufacturer: None, model: None, device_type: None, certificate_id: None, certification_date: None, software_version: None, hardware_version: None, vid: None, pid: None, family_sku: None, family_variant_sku: None, firmware_version: None, family_id: None, tis_trp_tested: None, specification_version: None, transport_interface: None, primary_device_type_id: None, application_categories: None, description: None, compliance_document_url: None, program_type: None, created_at: chrono::Utc::now(), updated_at: chrono::Utc::now() })) }
        async fn collect_product_batch(&self, _urls: &[crate::domain::product_url::ProductUrl]) -> anyhow::Result<Vec<crate::domain::product::ProductDetail>> { Ok(self.details.clone()) }
        fn as_any(&self) -> &dyn std::any::Any { self }
    }

    #[tokio::test]
    async fn list_page_logic_happy_path_with_fake_collector_is_success() {
        let logic = ListPageLogic;
        let repo = memory_repo().await;
        let mut deps = deps_stub_memory_sync(repo);
        deps.list_collector = Some(Arc::new(FakeListCollector { urls: vec![
            crate::domain::product_url::ProductUrl { url: "https://e/p1".into(), page_id: 1, index_in_page: 1 },
            crate::domain::product_url::ProductUrl { url: "https://e/p2".into(), page_id: 1, index_in_page: 2 },
        ] }));
        let input = StageInput {
            stage_type: crate::crawl_engine::actors::types::StageType::ListPageCrawling,
            item: ch::StageItem::Page(1),
            config: crate::infrastructure::config::AppConfig::default(),
            deps,
            total_pages_hint: Some(1),
            products_on_last_page_hint: Some(12),
        };
        let out = logic.execute(input).await.expect("ok");
        assert!(out.result.success);
        let json = out.result.collected_data.expect("urls json");
        let urls: Vec<crate::domain::product_url::ProductUrl> = serde_json::from_str(&json).expect("parse");
        assert_eq!(urls.len(), 2);
    }

    #[tokio::test]
    async fn product_detail_logic_happy_path_with_fake_collector_is_success() {
        let logic = ProductDetailLogic;
        let repo = memory_repo().await;
        let mut deps = deps_stub_memory_sync(repo);
        let now = chrono::Utc::now();
        let pd = crate::domain::product::ProductDetail {
            url: "https://e/p1".into(), page_id: Some(1), index_in_page: Some(1), id: None,
            manufacturer: Some("Acme".into()), model: Some("M1".into()), device_type: None,
            certificate_id: None, certification_date: None, software_version: None, hardware_version: None, vid: None, pid: None, family_sku: None, family_variant_sku: None, firmware_version: None, family_id: None, tis_trp_tested: None, specification_version: None, transport_interface: None, primary_device_type_id: None, application_categories: None, description: None, compliance_document_url: None, program_type: Some("Matter".into()), created_at: now, updated_at: now };
        deps.detail_collector = Some(Arc::new(FakeDetailCollector { details: vec![pd.clone()] }));
        let input = StageInput {
            stage_type: crate::crawl_engine::actors::types::StageType::ProductDetailCrawling,
            item: ch::StageItem::ProductUrls(ch::ProductUrls { urls: vec![
                crate::domain::product_url::ProductUrl { url: "https://e/p1".into(), page_id: 1, index_in_page: 1 }
            ], batch_id: None }),
            config: crate::infrastructure::config::AppConfig::default(),
            deps,
            total_pages_hint: Some(1),
            products_on_last_page_hint: Some(12),
        };
        let out = logic.execute(input).await.expect("ok");
        assert!(out.result.success);
        let json = out.result.collected_data.expect("details json");
        let wrapper: ch::ProductDetails = serde_json::from_str(&json).expect("parse");
        assert_eq!(wrapper.products.len(), 1);
        assert_eq!(wrapper.products[0].url, "https://e/p1");
    }
}
