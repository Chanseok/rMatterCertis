// Default strategy implementations for each Stage are split into dedicated files
pub mod list_page;
pub mod status_check;
pub mod product_detail;
pub mod data_validation;
pub mod data_saving;

// Re-exports for factory binding and tests
pub use data_saving::DataSavingLogic;
pub use data_validation::DataValidationLogic;
pub use list_page::ListPageLogic;
pub use product_detail::ProductDetailLogic;
pub use status_check::StatusCheckLogic;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crawl_engine::channels::types as ch;
    use crate::crawl_engine::stages::traits::{Deps, StageInput, StageLogic};
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
