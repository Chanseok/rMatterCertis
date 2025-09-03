use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tracing::info;

use crate::commands::actor_system::{
	ActorCrawlingRequest, CrawlingMode, start_actor_system_crawling,
};

#[derive(Debug, Deserialize)]
pub struct StartCrawlingRequest {
	pub mode: Option<String>,
	pub override_batch_size: Option<u32>,
	pub override_concurrency: Option<u32>,
	pub delay_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct StartCrawlingResponse {
	pub success: bool,
	pub message: String,
	pub session_id: Option<String>,
}

#[tauri::command]
/// # Errors
/// Returns an error string if the actor system fails to start.
pub async fn start_unified_crawling(
	app: AppHandle,
	request: StartCrawlingRequest,
) -> Result<StartCrawlingResponse, String> {
	info!("🚀 통합 크롤링 요청 수신: {:?}", request);

	let crawling_mode = match request.mode.as_deref() {
		Some("advanced") => Some(CrawlingMode::AdvancedEngine),
		Some("live") => Some(CrawlingMode::LiveProduction),
		_ => None,
	};
	let actor_req = ActorCrawlingRequest {
		site_url: None,
		start_page: None,
		end_page: None,
		page_count: None,
		concurrency: request.override_concurrency,
		batch_size: request.override_batch_size,
		delay_ms: request.delay_ms,
		mode: crawling_mode,
	};
	let result = start_actor_system_crawling(app.clone(), actor_req)
		.await
		.map_err(|e| format!("failed to start actor crawling: {}", e))?;
	Ok(StartCrawlingResponse {
		success: result.success,
		message: result.message,
		session_id: result.session_id,
	})
}
