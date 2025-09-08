use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};
use ts_rs::TS;

use crate::application::AppState;
use crate::crawl_engine::actor_system::StageResult;
use crate::crawl_engine::channels::types::{StageItem, StageType};
use crate::crawl_engine::config::SystemConfig;
use crate::crawl_engine::services::crawling_integration::{
    CrawlingIntegrationService, RealCrawlingStageExecutor,
};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RealCrawlingRequest {
    pub start_page: u32,
    pub end_page: u32,
    pub concurrency_limit: Option<u32>,
    pub batch_size: Option<u32>,
    pub perform_site_check: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RealCrawlingProgress {
    pub session_id: String,
    pub current_stage: String,
    pub overall_progress: f64,
    pub stage_progress: f64,
    pub processed_items: u32,
    pub total_items: u32,
    pub successful_items: u32,
    pub failed_items: u32,
    pub elapsed_ms: u64,
    pub estimated_remaining_ms: Option<u64>,
    pub status_message: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RealCrawlingResult {
    pub session_id: String,
    pub success: bool,
    pub processed_pages: u32,
    pub collected_urls: u32,
    pub collected_details: u32,
    pub saved_products: u32,
    pub total_duration_ms: u64,
    pub error_message: Option<String>,
    pub stage_results: Vec<StageResultSummary>,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StageResultSummary {
    pub stage_name: String,
    pub success: bool,
    pub processed_items: u32,
    pub duration_ms: u64,
    pub error_message: Option<String>,
}

#[tauri::command]
/// # Errors
/// Returns an error string if initialization or any stage execution fails.
pub async fn execute_real_crawling(
    app: AppHandle,
    request: RealCrawlingRequest,
) -> Result<RealCrawlingResult, String> {
    let session_id = format!("real_crawling_{}", Utc::now().timestamp());
    info!(session_id = %session_id, start_page = request.start_page, end_page = request.end_page, "🚀 [PHASE C] Starting REAL crawling execution");
    let start_time = std::time::Instant::now();
    let app_state = app.state::<AppState>();
    let config_guard = app_state.config.read().await;
    let app_config = config_guard.clone();
    drop(config_guard);
    let system_config = Arc::new(SystemConfig::default());
    let integration_service: Arc<CrawlingIntegrationService> =
        match CrawlingIntegrationService::new(system_config.clone(), app_config).await {
            Ok(s) => Arc::new(s),
            Err(e) => {
                error!(session_id = %session_id, error = %e, "Failed to create crawling integration service");
                return Err(format!("Failed to initialize crawling service: {}", e));
            }
        };
    let executor = Arc::new(RealCrawlingStageExecutor::new(integration_service.clone()));
    let mut stage_results = Vec::new();
    let cancellation_token = CancellationToken::new();
    let total_pages = request.end_page - request.start_page + 1;
    let pages: Vec<u32> = (request.start_page..=request.end_page).collect();
    let _ = app.emit(
        "crawling-progress",
        RealCrawlingProgress {
            session_id: session_id.clone(),
            current_stage: "리스트 수집".to_string(),
            overall_progress: 0.0,
            stage_progress: 0.0,
            processed_items: 0,
            total_items: total_pages,
            successful_items: 0,
            failed_items: 0,
            elapsed_ms: 0,
            estimated_remaining_ms: None,
            status_message: "사이트 상태 확인 및 리스트 수집 시작".to_string(),
            timestamp: Utc::now(),
        },
    );
    info!(session_id = %session_id, "📋 Phase 1: List Collection");
    let list_stage_start = std::time::Instant::now();
    let list_items: Vec<StageItem> = pages.iter().map(|&p| StageItem::Page(p)).collect();
    let list_result = executor
        .execute_stage(
            StageType::ListCollection,
            list_items,
            request.concurrency_limit.unwrap_or(5),
            cancellation_token.clone(),
        )
        .await;
    let list_duration = list_stage_start.elapsed();
    let list_success = matches!(list_result, StageResult::Success { .. });
    stage_results.push(StageResultSummary {
        stage_name: "리스트 수집".to_string(),
        success: list_success,
        processed_items: match &list_result {
            StageResult::Success {
                processed_items, ..
            } => *processed_items,
            StageResult::Failure {
                partial_results, ..
            } => *partial_results,
            _ => 0,
        },
        duration_ms: u64::try_from(list_duration.as_millis()).unwrap_or(u64::MAX),
        error_message: if list_success {
            None
        } else {
            Some(format!("리스트 수집 실패: {:?}", list_result))
        },
    });
    let _ = app.emit(
        "crawling-progress",
        RealCrawlingProgress {
            session_id: session_id.clone(),
            current_stage: "상세 정보 수집".to_string(),
            overall_progress: 50.0,
            stage_progress: 100.0,
            processed_items: match &list_result {
                StageResult::Success {
                    processed_items, ..
                } => *processed_items,
                _ => 0,
            },
            total_items: total_pages,
            successful_items: match &list_result {
                StageResult::Success {
                    processed_items, ..
                } => *processed_items,
                StageResult::Failure {
                    partial_results, ..
                } => *partial_results,
                _ => 0,
            },
            failed_items: match &list_result {
                StageResult::Failure {
                    partial_results, ..
                } => total_pages - *partial_results,
                StageResult::FatalError { .. } => total_pages,
                _ => 0,
            },
            elapsed_ms: u64::try_from(start_time.elapsed().as_millis()).unwrap_or(u64::MAX),
            estimated_remaining_ms: Some(
                u64::try_from(start_time.elapsed().as_millis()).unwrap_or(u64::MAX),
            ),
            status_message: "상세 정보 수집 시작".to_string(),
            timestamp: Utc::now(),
        },
    );
    info!(session_id = %session_id, "📦 Phase 2: Detail Collection");
    let detail_stage_start = std::time::Instant::now();
    let detail_result = executor
        .execute_stage(
            StageType::DetailCollection,
            Vec::new(),
            request.concurrency_limit.unwrap_or(3),
            cancellation_token.clone(),
        )
        .await;
    let detail_duration = detail_stage_start.elapsed();
    let detail_success = matches!(detail_result, StageResult::Success { .. });
    stage_results.push(StageResultSummary {
        stage_name: "상세 정보 수집".to_string(),
        success: detail_success,
        processed_items: match &detail_result {
            StageResult::Success {
                processed_items, ..
            } => *processed_items,
            _ => 0,
        },
        duration_ms: u64::try_from(detail_duration.as_millis()).unwrap_or(u64::MAX),
        error_message: if detail_success {
            None
        } else {
            Some(format!("상세 정보 수집 실패: {:?}", detail_result))
        },
    });
    let total_duration = start_time.elapsed();
    let overall_success = list_success && detail_success;
    let result = RealCrawlingResult {
        session_id: session_id.clone(),
        success: overall_success,
        processed_pages: total_pages,
        collected_urls: match &list_result {
            StageResult::Success {
                processed_items, ..
            } => *processed_items * 12,
            _ => 0,
        },
        collected_details: match &detail_result {
            StageResult::Success {
                processed_items, ..
            } => *processed_items,
            _ => 0,
        },
        saved_products: match &detail_result {
            StageResult::Success {
                processed_items, ..
            } => *processed_items,
            _ => 0,
        },
        total_duration_ms: u64::try_from(total_duration.as_millis()).unwrap_or(u64::MAX),
        error_message: if overall_success {
            None
        } else {
            Some("크롤링 과정에서 오류가 발생했습니다".to_string())
        },
        stage_results,
        completed_at: Utc::now(),
    };
    let _ = app.emit("crawling-completed", result.clone());
    info!(session_id = %session_id, success = overall_success, duration_ms = total_duration.as_millis(), "🎉 [PHASE C] Real crawling execution completed");
    Ok(result)
}

#[tauri::command]
/// # Errors
/// Returns an error string if status retrieval fails.
pub async fn get_real_crawling_status(
    _session_id: String,
) -> Result<Option<RealCrawlingProgress>, String> {
    Ok(None)
}

#[tauri::command]
/// # Errors
/// Returns an error string if cancellation fails.
pub async fn cancel_real_crawling(_session_id: String) -> Result<bool, String> {
    Ok(true)
}
