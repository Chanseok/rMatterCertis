use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tracing::{info, error};
use std::sync::Arc;
use crate::infrastructure::service_based_crawling_engine::{ServiceBasedBatchCrawlingEngine, BatchCrawlingConfig};
use crate::infrastructure::simple_http_client::HttpClient;
use crate::infrastructure::html_parser::MatterDataExtractor;
use crate::infrastructure::integrated_product_repository::IntegratedProductRepository;
use crate::infrastructure::config::{AppConfig, ConfigManager};
use crate::application::{AppState, EventEmitter};

/// ServiceBased 크롤링 요청 구조체 (참조용)
#[derive(Debug, Deserialize)]
pub struct ServiceBasedCrawlingRequest {
    pub start_page: Option<u32>,
    pub end_page: Option<u32>,
    pub concurrency: Option<u32>,
}

/// ServiceBased 크롤링 응답 구조체 (참조용)
#[derive(Debug, Serialize)]
pub struct ServiceBasedCrawlingResponse {
    pub success: bool,
    pub message: String,
    pub session_id: Option<String>,
    pub pages_processed: Option<u32>,
}

/// [참조용] ServiceBasedBatchCrawlingEngine 직접 실행 명령어
/// 
/// 이 명령어는 Actor 시스템 구현 완료 후 삭제될 예정입니다.
/// 현재는 작동하는 구현의 참조를 위해 유지됩니다.
#[tauri::command]
pub async fn start_service_based_crawling_reference(
    app: AppHandle,
    request: ServiceBasedCrawlingRequest,
) -> Result<ServiceBasedCrawlingResponse, String> {
    info!("🔧 [REFERENCE] Starting ServiceBasedBatchCrawlingEngine for comparison");
    info!("📊 Request: start_page={:?}, end_page={:?}, concurrency={:?}", 
          request.start_page, request.end_page, request.concurrency);
    
    // AppState 가져오기
    let app_state = app.state::<AppState>();
    
    // 데이터베이스 풀 가져오기
    let database_pool = {
        let pool_guard = app_state.database_pool.read().await;
        pool_guard.as_ref()
            .ok_or("Database pool not initialized")?
            .clone()
    };
    
    // 설정 파일 로드
    let config_manager = ConfigManager::new().map_err(|e| format!("Config manager creation failed: {}", e))?;
    let app_config = config_manager.load_config().await.map_err(|e| format!("Config loading failed: {}", e))?;
    
    // ServiceBasedBatchCrawlingEngine 설정 생성
    let config = BatchCrawlingConfig {
        start_page: request.start_page.unwrap_or(294),
        end_page: request.end_page.unwrap_or(298),
        concurrency: request.concurrency.unwrap_or(5),
        list_page_concurrency: request.concurrency.unwrap_or(5),
        product_detail_concurrency: request.concurrency.unwrap_or(5),
        delay_ms: 1000,
        batch_size: 20,
        retry_max: 3,
        timeout_ms: 300000,
        disable_intelligent_range: false,
        cancellation_token: None,
    };
    
    info!("⚙️ [REFERENCE] ServiceBased config: {:?}", config);
    
    // 필요한 컴포넌트들 생성
    let http_client = HttpClient::create_from_global_config()
        .map_err(|e| format!("HttpClient creation failed: {}", e))?;
    let data_extractor = MatterDataExtractor::new()
        .map_err(|e| format!("MatterDataExtractor creation failed: {}", e))?;
    let product_repo = Arc::new(IntegratedProductRepository::new(database_pool));
    let event_emitter = Arc::new(None::<EventEmitter>);
    
    let session_id = format!("service_ref_{}", chrono::Utc::now().timestamp());
    
    // ServiceBasedBatchCrawlingEngine 생성
    let mut engine = ServiceBasedBatchCrawlingEngine::new(
        http_client,
        data_extractor,
        product_repo,
        event_emitter,
        config.clone(),
        session_id.clone(),
        app_config,
    );
    
    info!("🚀 [REFERENCE] ServiceBased engine created, session_id: {}", session_id);
    
    // 백그라운드에서 실행
    let pages_count = config.end_page - config.start_page + 1;
    tokio::spawn(async move {
        match engine.execute().await {
            Ok(()) => {
                info!("✅ [REFERENCE] ServiceBased crawling completed successfully");
            },
            Err(e) => {
                error!("❌ [REFERENCE] ServiceBased crawling failed: {}", e);
            }
        }
    });
    
    // 즉시 응답 반환
    Ok(ServiceBasedCrawlingResponse {
        success: true,
        message: "ServiceBased 참조 크롤링 시작됨".to_string(),
        session_id: Some(session_id),
        pages_processed: Some(pages_count),
    })
}
