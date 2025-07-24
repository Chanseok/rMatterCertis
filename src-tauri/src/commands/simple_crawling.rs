use tauri::State;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use tracing::info;

use crate::infrastructure::config::ConfigManager;
use crate::infrastructure::crawling_service_impls::CrawlingRangeCalculator;
use crate::infrastructure::integrated_product_repository::IntegratedProductRepository;
use crate::application::AppState;

/// 크롤링 세션 정보 (간소화)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CrawlingSession {
    pub session_id: String,
    pub started_at: String,
    pub status: String,
}

/// Smart Crawling 시작 - 설정 파일 기반 자동 실행
#[tauri::command]
pub async fn start_smart_crawling(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>
) -> Result<CrawlingSession, String> {
    // 1. 설정 파일 자동 로딩
    let config_manager = ConfigManager::new()
        .map_err(|e| format!("Failed to initialize config manager: {}", e))?;
    
    let config = config_manager.load_config().await
        .map_err(|e| format!("Failed to load config from file: {}", e))?;

    info!("🚀 Starting smart crawling with {} max pages, {}ms delay", 
          config.user.crawling.page_range_limit, config.user.request_delay_ms);

    info!("✅ Config loaded successfully: max_pages={}, request_delay={}ms", 
          config.user.crawling.page_range_limit, config.user.request_delay_ms);

    // 2. 공유 데이터베이스 Pool 사용 (Modern Rust 2024 - Backend-Only CRUD)
    let pool = state.get_database_pool().await?;
    
    let product_repo = IntegratedProductRepository::new(pool);

    // 3. 크롤링 범위 계산
    let range_calculator = CrawlingRangeCalculator::new(
        std::sync::Arc::new(product_repo),
        config.clone(),
    );

    // 임시로 총 페이지 수와 마지막 페이지 제품 수를 하드코딩 (나중에 사이트 분석으로 대체)
    let total_pages = 485u32;
    let products_on_last_page = 11u32;

    let range_result = range_calculator.calculate_next_crawling_range(
        total_pages,
        products_on_last_page,
    ).await
    .map_err(|e| format!("Failed to calculate crawling range: {}", e))?;

    match range_result {
        Some((start_page, end_page)) => {
            let session_id = format!("session_{}", chrono::Utc::now().timestamp());
            let started_at = chrono::Utc::now().to_rfc3339();
            
            info!("✅ Smart crawling session created: {} (pages {} → {})", 
                  session_id, start_page, end_page);

            // 실제 크롤링 시작 - ServiceBasedBatchCrawlingEngine 사용
            use crate::commands::crawling_v4::{CrawlingEngineState, StartCrawlingRequest, start_crawling, init_crawling_engine};
            use crate::application::SharedStateCache;
            use tauri::Manager;
            
            // Engine state를 가져와서 크롤링 실행
            if let Some(engine_state) = app_handle.try_state::<CrawlingEngineState>() {
                if let Some(shared_cache) = app_handle.try_state::<SharedStateCache>() {
                    
                    // 엔진이 초기화되지 않았다면 먼저 초기화
                    {
                        let engine_guard = engine_state.engine.read().await;
                        if engine_guard.is_none() {
                            drop(engine_guard); // 읽기 락 해제
                            info!("🔧 Crawling engine not initialized, initializing now...");
                            
                            // 엔진 초기화
                            match init_crawling_engine(app_handle.clone(), engine_state.clone()).await {
                                Ok(response) => {
                                    if response.success {
                                        info!("✅ Crawling engine initialized successfully");
                                    } else {
                                        return Err(format!("Failed to initialize crawling engine: {}", response.message));
                                    }
                                }
                                Err(e) => {
                                    return Err(format!("Failed to initialize crawling engine: {}", e));
                                }
                            }
                        }
                    }
                    
                    info!("🚀 Starting actual crawling with ServiceBasedBatchCrawlingEngine");
                    
                    let request = StartCrawlingRequest {
                        start_page,
                        end_page,
                        max_products_per_page: Some(12), // 페이지당 최대 제품 수
                        concurrent_requests: Some(3),    // 동시 요청 수
                        request_timeout_seconds: Some(30), // 요청 타임아웃 (초)
                    };
                    
                    // 크롤링 시작
                    match start_crawling(
                        app_handle.clone(),
                        engine_state,
                        shared_cache,
                        request
                    ).await {
                        Ok(response) => info!("✅ Crawling started successfully: {}", response.message),
                        Err(e) => {
                            tracing::error!("❌ Crawling failed to start: {}", e);
                            return Err(format!("Failed to start crawling: {}", e));
                        }
                    }
                    
                    info!("🎯 Crawling task initiated for session: {}", session_id);
                } else {
                    tracing::warn!("⚠️ SharedStateCache not found - falling back to session-only mode");
                }
            } else {
                tracing::warn!("⚠️ CrawlingEngineState not found - falling back to session-only mode");
            }
            
            Ok(CrawlingSession {
                session_id,
                started_at,
                status: "started".to_string(),
            })
        }
        None => {
            info!("🏁 No more pages to crawl");
            Err("모든 페이지가 이미 크롤링되었습니다.".to_string())
        }
    }
}
