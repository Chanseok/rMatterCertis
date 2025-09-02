use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tracing::info;
// chrono is not used directly in this module

use crate::commands::actor_system::{
    ActorCrawlingRequest, CrawlingMode, start_actor_system_crawling,
};

/// 통합 크롤링 요청 구조체
#[derive(Debug, Deserialize)]
pub struct StartCrawlingRequest {
    pub mode: Option<String>, // "advanced" | "live" (UI 두 탭)
    pub override_batch_size: Option<u32>,
    pub override_concurrency: Option<u32>,
    pub delay_ms: Option<u64>,
}

/// 통합 크롤링 응답 구조체
#[derive(Debug, Serialize)]
pub struct StartCrawlingResponse {
    pub success: bool,
    pub message: String,
    pub session_id: Option<String>,
}

/// 통합 크롤링 명령어 (Actor 시스템 진입점)
///
/// # Errors
/// Returns `Err(String)` when the actor system fails to start crawling
/// (for example, invalid parameters, internal initialization errors,
/// or runtime failures reported by the actor entrypoint).
#[tauri::command]
pub async fn start_unified_crawling(
    app: AppHandle,
    request: StartCrawlingRequest,
) -> Result<StartCrawlingResponse, String> {
    info!("🚀 통합 크롤링 요청 수신: {:?}", request);

    // 단일 경로: Actor 기반
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

// NOTE: engine_type 제거로 호출 단순화됨. FE는 mode + override_* 만 전달.
