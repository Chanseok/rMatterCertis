use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tracing::{info, error};
use chrono;

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
        "service" => execute_service_based_crawling(app, request).await,
        "actor" => execute_actor_crawling(app, request).await,
        "simple" => execute_simple_crawling(app, request).await,
        _ => Err(format!("알 수 없는 엔진 타입: {}", request.engine_type)),
    }
}

/// 서비스 기반 크롤링 실행 (기존 검증된 엔진 활용)
async fn execute_service_based_crawling(
    app: AppHandle,
    request: StartCrawlingRequest,
) -> Result<StartCrawlingResponse, String> {
    info!("🔧 서비스 기반 크롤링 시작");
    
    // TODO: ServiceBasedBatchCrawlingEngine 연결
    // 현재는 Mock 응답으로 기본 통신 경로 검증
    
    Ok(StartCrawlingResponse {
        success: true,
        message: format!("서비스 크롤링 완료 (페이지: {}-{})", 
                        request.start_page.unwrap_or(1), 
                        request.end_page.unwrap_or(100)),
        session_id: Some(format!("service_{}", chrono::Utc::now().timestamp())),
    })
}

/// Actor 기반 크롤링 실행 (새로운 Actor 시스템)
async fn execute_actor_crawling(
    app: AppHandle,
    request: StartCrawlingRequest,
) -> Result<StartCrawlingResponse, String> {
    info!("🎭 Actor 기반 크롤링 시작");
    
    // TODO: SessionActor에게 명령 전송
    // 현재는 Mock 응답으로 기본 통신 경로 검증
    
    Ok(StartCrawlingResponse {
        success: true,
        message: format!("Actor 크롤링 완료 (페이지: {}-{})", 
                        request.start_page.unwrap_or(1), 
                        request.end_page.unwrap_or(100)),
        session_id: Some(format!("actor_{}", chrono::Utc::now().timestamp())),
    })
}

/// 단순 크롤링 실행 (기존 simple_crawling 연결)
async fn execute_simple_crawling(
    app: AppHandle,
    request: StartCrawlingRequest,
) -> Result<StartCrawlingResponse, String> {
    info!("🔧 단순 크롤링 시작");
    
    // TODO: simple_crawling::start_smart_crawling 연결
    // 현재는 Mock 응답으로 기본 통신 경로 검증
    
    Ok(StartCrawlingResponse {
        success: true,
        message: "단순 크롤링 완료".to_string(),
        session_id: Some(format!("simple_{}", chrono::Utc::now().timestamp())),
    })
}
