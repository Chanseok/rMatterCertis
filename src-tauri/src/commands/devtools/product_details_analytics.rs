#![allow(clippy::missing_errors_doc, clippy::unused_async)]
// Product details analytics endpoints (dev-tools only)

use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct ProductDetailsAnalyticsSummary {
	pub total_details: u32,
	pub missing_core_fields: u32,
}

/// 간단한 product_details 통계 요약을 반환합니다 (dev-tools 전용).
#[cfg_attr(feature = "dev-tools", tauri::command)]
pub async fn get_product_details_analytics(
	state: State<'_, crate::application::AppState>,
) -> Result<ProductDetailsAnalyticsSummary, String> {
	let pool = state.get_database_pool().await?;

	let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_details")
		.fetch_one(&pool)
		.await
		.unwrap_or(0);
	let missing_core: i64 = sqlx::query_scalar(
		r#"SELECT COUNT(*) FROM product_details
		   WHERE manufacturer IS NULL OR model IS NULL OR certificate_id IS NULL"#,
	)
	.fetch_one(&pool)
	.await
	.unwrap_or(0);

	Ok(ProductDetailsAnalyticsSummary {
		total_details: u32::try_from(total).unwrap_or(u32::MAX),
		missing_core_fields: u32::try_from(missing_core).unwrap_or(u32::MAX),
	})
}

