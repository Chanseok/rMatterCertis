use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tracing::{info, warn};
// chrono is not used directly in this module

use crate::commands::real_crawling_commands::{execute_real_crawling, RealCrawlingRequest};

/// 통합 크롤링 요청 구조체
#[derive(Debug, Deserialize)]
pub struct StartCrawlingRequest {
    pub engine_type: String, // "service" | "actor" | "simple"
    pub start_page: Option<u32>,
    pub end_page: Option<u32>,
    pub concurrency: Option<u32>,
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
            let start = request.start_page.unwrap_or(1);
            let end = request.end_page.unwrap_or(start);
            let actor_req = RealCrawlingRequest {
                start_page: start,
                end_page: end,
                concurrency_limit: request.concurrency,
                batch_size: None,
                perform_site_check: Some(true),
            };
            let result = execute_real_crawling(app, actor_req).await?;
            Ok(StartCrawlingResponse {
                success: result.success,
                message: format!(
                    "Actor 크롤링 완료: processed_pages={}, saved_products={}",
                    result.processed_pages, result.saved_products
                ),
                session_id: Some(result.session_id),
            })
        }
        _ => Err(format!("알 수 없는 엔진 타입: {}", request.engine_type)),
    }
}

// Deprecated helper paths removed to reduce warnings. Unified path is in start_unified_crawling.
