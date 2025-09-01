use crate::crawl_engine::actors::types::{DuplicatePersistencePolicy, StageItemResult, StageItemType, StageType as ActorStageType};
use crate::crawl_engine::channels::types as ch;
use crate::crawl_engine::stages::traits::{StageInput, StageLogic, StageLogicError, StageOutput};

pub struct DataSavingLogic;

#[async_trait::async_trait]
impl StageLogic for DataSavingLogic {
    fn name(&self) -> &'static str { "DataSavingLogic" }

    async fn execute(&self, input: StageInput) -> Result<StageOutput, StageLogicError> {
        let st = input.stage_type.clone();
        if !matches!(st, ActorStageType::DataSaving) {
            return Err(StageLogicError::Unsupported(st));
        }
        // Select products vector based on item type
        let (products, item_id, item_type) = match &input.item {
            ch::StageItem::ProductDetails(pd) => (
                &pd.products,
                format!("persist_product_details_{}", pd.products.len()),
                StageItemType::Url { url_type: "data_saving:product_details".into() },
            ),
            ch::StageItem::ValidatedProducts(vp) => (
                &vp.products,
                format!("persist_validated_{}", vp.products.len()),
                StageItemType::Url { url_type: "data_saving:validated_products".into() },
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
            match repo.create_or_update_product_detail(detail).await {
                Ok((was_updated, was_created)) => {
                    if was_created { inserted = inserted.saturating_add(1); }
                    if was_updated { updated = updated.saturating_add(1); }
                    if !was_created && !was_updated && policy == DuplicatePersistencePolicy::UpdateIdIndexOnly {
                        if let (Some(pid), Some(idx)) = (detail.page_id, detail.index_in_page) {
                            let _ = repo.force_update_position_by_url(&detail.url, pid, idx).await;
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
        let result = StageItemResult {
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
