use crate::crawl_engine::actors::types::{
    DuplicatePersistencePolicy, StageItemResult, StageItemType, StageResultData,
    StageType as ActorStageType,
};
use crate::crawl_engine::channels::types as ch;
use crate::crawl_engine::stages::traits::{StageInput, StageLogic, StageLogicError, StageOutput};

pub struct DataSavingLogic;

#[async_trait::async_trait]
impl StageLogic for DataSavingLogic {
    fn name(&self) -> &'static str { "DataSavingLogic" }

    async fn execute(&self, input: StageInput) -> Result<StageOutput, StageLogicError> {
    let start = std::time::Instant::now();
    let StageInput { stage_type: st, item, deps, .. } = input;
        if !matches!(st, ActorStageType::DataSaving) {
            return Err(StageLogicError::Unsupported(st));
        }
        // Select products vector based on item type
        let (products, item_id, item_type) = match &item {
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
    let repo = deps.repo;
    let policy = deps.duplicate_policy.clone();
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
        let duration_ms = start.elapsed().as_millis() as u64;
    let enhanced = StageItemResult {
            item_id,
            item_type,
            success: true,
            error: None,
            duration_ms,
            retry_count: 0,
            collected_data: Some(StageResultData::SavingResult {
                saved_count: inserted + updated,
                duplicates_found: attempted.saturating_sub(inserted + updated),
                database_id_range: None,
            }),
        };
    Ok(StageOutput { result: enhanced })
    }
}
