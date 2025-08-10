use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tracing::{info, warn};
// chrono is not used directly in this module

use crate::commands::actor_system_commands::{start_actor_system_crawling, ActorCrawlingRequest, CrawlingMode};

/// 통합 크롤링 요청 구조체
#[derive(Debug, Deserialize)]
pub struct StartCrawlingRequest {
    pub engine_type: String, // "actor" (others deprecated)
    pub mode: Option<String>, // "advanced" | "live" (UI two tabs)
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
#[tauri::command]
pub async fn start_unified_crawling(
    app: AppHandle,
    request: StartCrawlingRequest,
) -> Result<StartCrawlingResponse, String> {
    info!("🚀 통합 크롤링 요청 수신: {:?}", request);
    
    match request.engine_type.as_str() {
        // Deprecated paths kept for compatibility; guide callers to the actor flow
        "service" | "simple" => {
            let msg = "Deprecated engine_type. Use engine_type=\"actor\" via start_unified_crawling".to_string();
            warn!("{}", msg);
            Ok(StartCrawlingResponse {
                success: false,
                message: msg,
                session_id: None,
            })
        }
        // Primary path: Actor-based real crawling
        "actor" | "" => {
            // Map mode string to CrawlingMode
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
            let result = start_actor_system_crawling(app.clone(), actor_req).await.map_err(|e| format!("failed to start actor crawling: {}", e))?;
            Ok(StartCrawlingResponse { success: result.success, message: result.message, session_id: result.session_id })
        }
        _ => Err(format!("알 수 없는 엔진 타입: {}", request.engine_type)),
    }
}

// Deprecated helper paths removed to reduce warnings. Unified path is in start_unified_crawling.
