use tauri::State;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use tracing::info;
use std::sync::Arc;

use crate::infrastructure::config::ConfigManager;
use crate::application::AppState;
use crate::infrastructure::crawling_service_impls::StatusCheckerImpl;
use crate::infrastructure::{HttpClient, MatterDataExtractor};
use crate::new_architecture::services::crawling_planner::CrawlingPlanner;
use crate::new_architecture::context::SystemConfig;
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
    _state: State<'_, AppState>
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
    info!("⚙️ [NEW ARCHITECTURE] SystemConfig initialized with intelligent defaults");
    
    // CrawlingPlanner 초기화 (설정 파일 기반)
    let planner = CrawlingPlanner::new(
        status_checker,
        database_analyzer,
        system_config,
    );
    info!("🧠 [NEW ARCHITECTURE] CrawlingPlanner initialized - replacing hardcoded logic");
    
    // 3. 지능형 시스템 상태 분석 및 계획 수립
    info!("🔍 [NEW ARCHITECTURE] Analyzing system state with intelligent CrawlingPlanner...");
    
    let (site_status, db_analysis) = planner.analyze_system_state().await
        .map_err(|e| format!("System analysis failed: {}", e))?;
    
    let (range_recommendation, processing_strategy) = planner.determine_crawling_strategy(&site_status, &db_analysis).await
        .map_err(|e| format!("Strategy determination failed: {}", e))?;

    info!("✅ [NEW ARCHITECTURE] Analysis complete - Range: {:?}, Processing: {:?}", range_recommendation, processing_strategy);    // 4. 계산된 범위로 크롤링 실행 (설정 파일 고정값 대신 지능형 계산 결과 사용)
    // Legacy crawling_v4 path removed. For now, delegate to unified actor-based crawling if range is recommended.
    if let Some((start_page, end_page)) = range_recommendation.to_page_range(site_status.total_pages) {
        info!("📊 지능형 분석 기반 크롤링 범위: {}-{} 페이지 (총 {} 페이지 중)", start_page, end_page, site_status.total_pages);
        // Fire unified crawling command (actor-based real crawling)
        use crate::commands::unified_crawling::{start_unified_crawling, StartCrawlingRequest};
        let req = StartCrawlingRequest {
            engine_type: "actor".to_string(),
            start_page: Some(start_page),
            end_page: Some(end_page),
            concurrency: None,
        };
        if let Err(e) = start_unified_crawling(app_handle.clone(), req).await {
            return Err(format!("Unified crawling execution failed: {}", e));
        }
    } else {
        info!("🛑 분석 결과: 크롤링이 필요하지 않습니다 (CrawlingRangeRecommendation::None)");
    }
    
    Ok(CrawlingSession {
        session_id,
        started_at,
        status: "started".to_string(),
    })
}
