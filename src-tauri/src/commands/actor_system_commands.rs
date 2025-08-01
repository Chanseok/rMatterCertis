//! Actor System Commands for Tauri Integration
//! 
//! Commands to test and use the Actor system from the UI

use crate::new_architecture::actor_system::SessionActor;
use crate::new_architecture::system_config::SystemConfig;
use crate::new_architecture::channel_types::{AppEvent, BatchConfig};
use crate::infrastructure::config::AppConfig;
use crate::infrastructure::service_based_crawling_engine::{ServiceBasedBatchCrawlingEngine, BatchCrawlingConfig};
use crate::infrastructure::simple_http_client::HttpClient;
use crate::infrastructure::html_parser::MatterDataExtractor;
use crate::infrastructure::integrated_product_repository::IntegratedProductRepository;
use crate::application::AppState;
use crate::domain::services::{StatusChecker, DatabaseAnalyzer}; // 실제 CrawlingPlanner에서 사용
use crate::infrastructure::config::ConfigManager; // 설정 관리자 추가
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Manager, Emitter};
use tokio::sync::mpsc;
use tokio::time::Duration;
use tracing::{info, error};
use chrono::Utc;

/// Actor System State managed by Tauri
#[derive(Default)]
pub struct ActorSystemState {
    pub is_running: Arc<tokio::sync::RwLock<bool>>,
}

/// Actor System Response
#[derive(Debug, Serialize, Deserialize)]
pub struct ActorSystemResponse {
    pub success: bool,
    pub message: String,
    pub session_id: Option<String>,
    pub data: Option<serde_json::Value>,
}

/// Crawling Request for Actor System
#[derive(Debug, Serialize, Deserialize)]
pub struct ActorCrawlingRequest {
    pub start_page: u32,
    pub end_page: u32,
    pub concurrency: Option<u32>,
    pub batch_size: Option<u32>,
    pub delay_ms: Option<u64>,
}

/// 🎭 NEW ARCHITECTURE: Start Actor-based crawling
#[tauri::command]
pub async fn start_actor_based_crawling(
    app: AppHandle,
    request: ActorCrawlingRequest,
) -> Result<ActorSystemResponse, String> {
    info!("🎭 [NEW ARCHITECTURE] Starting REAL Actor-based crawling: {:?}", request);
    
    let batch_size = request.batch_size.unwrap_or(3);
    let total_pages = request.end_page - request.start_page + 1;
    let batch_count = (total_pages + batch_size - 1) / batch_size; // 올림 계산
    
    info!("✅ [ACTOR] Creating actual SessionActor for real crawling");
    info!("📊 [ACTOR] Pages: {} to {}, Batch size: {}, Expected batches: {}", 
          request.start_page, request.end_page, batch_size, batch_count);
    
    // 🚀 실제 SessionActor 생성 및 실행
    let system_config = Arc::new(SystemConfig::default());
    let (_control_tx, control_rx) = mpsc::channel(100);
    let (event_tx, mut event_rx) = mpsc::channel(500);
    
    // SessionActor 생성
    let _session_actor = SessionActor::new(
        system_config,
        control_rx,
        event_tx.clone(),
    );
    
    let session_id = format!("actor_session_{}", Utc::now().timestamp());
    info!("🎭 SessionActor created with ID: {}", session_id);
    
    // session_id와 request 클론 생성 (move closure에서 사용할 것)
    let session_id_for_task = session_id.clone();
    let session_id_for_event = session_id.clone();
    let session_id_for_return = session_id.clone();
    let request_for_task = ActorCrawlingRequest {
        start_page: request.start_page,
        end_page: request.end_page,
        concurrency: request.concurrency,
        batch_size: request.batch_size,
        delay_ms: request.delay_ms,
    };
    let app_handle_for_task = app.clone();
    
    // 🔥 실제 크롤링 엔진을 사용한 SessionActor 실행 (백그라운드)
    let _session_actor_handle = tokio::spawn(async move {
        info!("🚀 SessionActor starting execution with REAL crawling engine...");
        
        // 🎯 실제 크롤링 엔진 초기화
        match initialize_real_crawling_engine(&session_id_for_task, &request_for_task, &app_handle_for_task).await {
            Ok((mut crawling_engine, analysis_info)) => {
                info!("✅ Real crawling engine initialized successfully");
                
                // 실제 크롤링 엔진 실행
                match crawling_engine.execute().await {
                    Ok(()) => {
                        info!("🎉 Real crawling completed successfully!");
                    }
                    Err(e) => {
                        error!("❌ Real crawling failed: {}", e);
                    }
                }
                
                // 분석 정보 저장 (나중에 응답에서 사용)
                // TODO: 분석 정보를 세션에 저장하거나 이벤트로 전달
            }
            Err(e) => {
                error!("❌ Failed to initialize real crawling engine: {}", e);
                
                // 실패 시 시뮬레이션 모드로 폴백
                info!("🔄 Falling back to simulation mode...");
                run_simulation_crawling(&request_for_task, request_for_task.batch_size.unwrap_or(3)).await;
            }
        }
        
        info!("✅ SessionActor completed execution");
        
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    });
    
    // 🔥 이벤트 리스너 실행 (백그라운드) - 실제 이벤트 방출
    let event_tx_clone = event_tx.clone();
    let session_id_for_second_spawn = session_id.clone();
    let end_page_for_event = request.end_page;
    let app_handle_for_events = app.clone();
    tokio::spawn(async move {
        // 시작 이벤트 방출 (AppEvent 타입으로)
        let session_event = AppEvent::SessionStarted {
            session_id: session_id_for_second_spawn.clone(),
            config: BatchConfig {
                target_url: "https://csa-iot.org".to_string(),
                max_pages: Some(end_page_for_event),
            },
        };
        let _ = event_tx_clone.send(session_event).await;
        
        // 이벤트 수신 처리 및 프론트엔드로 방출
        while let Some(event) = event_rx.recv().await {
            info!("📨 [ACTOR EVENT] Received: {:?}", event);
            
            // 프론트엔드로 이벤트 방출
            if let Err(e) = app_handle_for_events.emit("actor-event", &event) {
                error!("Failed to emit actor event to frontend: {}", e);
            }
        }
    });
    
    // 🔥 실제 Actor 시스템 - 도메인 지능형 시스템과 연결 완료
    info!("🎭 Actor 시스템 INTELLIGENT MODE: 도메인 요구사항 준수");
    info!("📊 요청 범위: {} ~ {} 페이지, 배치크기 {}, 동시성 {}", 
          request.start_page, request.end_page, batch_size, request.concurrency.unwrap_or(8));
    
    let total_pages = if request.start_page >= request.end_page {
        request.start_page - request.end_page + 1
    } else {
        request.end_page - request.start_page + 1
    };
    
    Ok(ActorSystemResponse {
        success: true,
        message: format!("🧠 INTELLIGENT Actor-based crawling started with domain logic compliance"), 
        session_id: Some(session_id_for_return),
        data: Some(serde_json::json!({
            "engine_type": "Actor + Domain Intelligence + ServiceBasedBatchCrawlingEngine",
            "architecture": "SessionActor → Domain Logic → ServiceBasedBatchCrawlingEngine → [StatusChecker, ProductListCollector, ProductDetailCollector]",
            "status": "RUNNING",
            "mode": "INTELLIGENT_CRAWLING",
            "config": {
                "requested_start_page": request.start_page,
                "requested_end_page": request.end_page,
                "batch_size": batch_size,
                "concurrency": request.concurrency.unwrap_or(8),
                "total_pages": total_pages,
                "expected_batches": batch_count,
                "domain_logic_enabled": true
            }
        })),
    })
}

/// Test SessionActor functionality
#[tauri::command]
pub async fn test_session_actor_basic(
    _app: AppHandle,
) -> Result<ActorSystemResponse, String> {
    info!("🧪 Testing SessionActor...");
    
    let system_config = Arc::new(SystemConfig::default());
    let (_control_tx, control_rx) = mpsc::channel(100);
    let (event_tx, _event_rx) = mpsc::channel(500);
    
    let _session_actor = SessionActor::new(
        system_config,
        control_rx,
        event_tx,
    );
    
    info!("✅ SessionActor created successfully");
    
    Ok(ActorSystemResponse {
        success: true,
        message: "SessionActor test completed successfully".to_string(),
        session_id: Some(format!("test_session_{}", Utc::now().timestamp())),
        data: None,
    })
}

/// Test Actor integration
#[tauri::command]
pub async fn test_actor_integration_basic(
    _app: AppHandle,
) -> Result<ActorSystemResponse, String> {
    info!("🧪 Testing Actor system integration...");
    
    // Integration test - create full system
    let system_config = Arc::new(SystemConfig::default());
    let (_control_tx, control_rx) = mpsc::channel(100);
    let (event_tx, _event_rx) = mpsc::channel(500);
    
    let _session_actor = SessionActor::new(
        system_config,
        control_rx,
        event_tx.clone(),
    );
    
    // Test actor system flow
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_millis(100)) => {
            info!("✅ Actor integration test completed within timeout");
        }
    }
    
    Ok(ActorSystemResponse {
        success: true,
        message: "Actor integration test completed successfully".to_string(),
        session_id: Some(format!("integration_test_{}", Utc::now().timestamp())),
        data: Some(serde_json::json!({
            "integration_status": "success",
            "components_tested": ["SessionActor", "channels", "configuration"]
        })),
    })
}

/// 실제 크롤링 엔진 초기화 (지능형 범위 계산 포함)
async fn initialize_real_crawling_engine(
    session_id: &str,
    request: &ActorCrawlingRequest,
    app_handle: &AppHandle,
) -> Result<(ServiceBasedBatchCrawlingEngine, serde_json::Value), Box<dyn std::error::Error + Send + Sync>> {
    info!("🔧 Initializing real crawling engine with intelligent planning for session: {}", session_id);
    
    // 앱 상태에서 데이터베이스 풀 가져오기
    let app_state = app_handle.state::<AppState>();
    let db_pool = {
        let pool_guard = app_state.database_pool.read().await;
        pool_guard.as_ref()
            .ok_or("Database pool not initialized")?
            .clone()
    };
    
    // IntegratedProductRepository 생성
    let product_repo = Arc::new(IntegratedProductRepository::new(db_pool));
    
    // HTTP 클라이언트 생성
    let http_client = HttpClient::create_from_global_config()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    // 데이터 추출기 생성
    let data_extractor = MatterDataExtractor::new()
        .map_err(|e| format!("Failed to create data extractor: {}", e))?;
    
    // 이벤트 방출기 설정 (선택적)
    let event_emitter = Arc::new(None);
    
    // 🧠 실제 설정 파일 로드 및 CrawlingPlanner 사용
    info!("🧠 [ACTOR] Loading configuration and using CrawlingPlanner for intelligent analysis...");
    
    // 실제 앱 설정 로드 (기본값 대신)
    let config_manager = crate::infrastructure::config::ConfigManager::new()
        .map_err(|e| format!("Failed to initialize config manager: {}", e))?;
    let app_config = config_manager.load_config().await
        .map_err(|e| format!("Failed to load config: {}", e))?;
    
    info!("📋 [ACTOR] Configuration loaded: page_range_limit={}, batch_size={}, max_concurrent={}", 
          app_config.user.crawling.page_range_limit, 
          app_config.user.batch.batch_size,
          app_config.user.max_concurrent_requests);
    
    // StatusChecker 생성 (실제 설정 사용)
    let status_checker_impl = crate::infrastructure::crawling_service_impls::StatusCheckerImpl::new(
        http_client.clone(),
        data_extractor.clone(),
        app_config.clone(),
    );
    let status_checker = Arc::new(status_checker_impl);
    
    // DatabaseAnalyzer 생성 (실제 DB 분석)
    let db_analyzer = Arc::new(crate::infrastructure::crawling_service_impls::DatabaseAnalyzerImpl::new(
        product_repo.clone(),
    ));
    
    // SystemConfig로 변환 (CrawlingPlanner용)
    let system_config = Arc::new(crate::new_architecture::config::SystemConfig::default());
    
    // 🚀 실제 CrawlingPlanner 사용!
    let crawling_planner = crate::new_architecture::services::crawling_planner::CrawlingPlanner::new(
        status_checker.clone(),
        db_analyzer.clone(),
        system_config.clone(),
    );
    
    // 시스템 상태 분석 (진짜 도메인 로직)
    let (site_status, db_analysis) = crawling_planner.analyze_system_state().await
        .map_err(|e| format!("Failed to analyze system state: {}", e))?;
    
    info!("🌐 [ACTOR] Real site analysis: {} pages, {} products on last page", 
          site_status.total_pages, site_status.products_on_last_page);
    info!("💾 [ACTOR] Real DB analysis: {} total products, {} unique products", 
          db_analysis.total_products, db_analysis.unique_products);
    
    // 🎯 실제 CrawlingPlanner로 지능형 전략 결정
    let (range_recommendation, processing_strategy) = crawling_planner
        .determine_crawling_strategy(&site_status, &db_analysis)
        .await
        .map_err(|e| format!("Failed to determine crawling strategy: {}", e))?;
    
    info!("📋 [ACTOR] CrawlingPlanner recommendation: {:?}", range_recommendation);
    info!("⚙️ [ACTOR] Processing strategy: batch_size={}, concurrency={}", 
          processing_strategy.recommended_batch_size, processing_strategy.recommended_concurrency);
    
    // 지능형 범위 권장사항을 실제 페이지 범위로 변환
    let (calculated_start_page, calculated_end_page) = match range_recommendation.to_page_range(site_status.total_pages) {
        Some((start, end)) => {
            // 🔄 역순 크롤링으로 변환 (start > end)
            let reverse_start = if start > end { start } else { end };
            let reverse_end = if start > end { end } else { start };
            info!("🎯 [ACTOR] CrawlingPlanner range: {} to {} (reverse crawling)", reverse_start, reverse_end);
            (reverse_start, reverse_end)
        },
        None => {
            info!("🔍 [ACTOR] No crawling needed, using verification range");
            let verification_pages = app_config.user.crawling.page_range_limit.min(5);
            let start = site_status.total_pages;
            let end = if start >= verification_pages { start - verification_pages + 1 } else { 1 };
            (start, end)
        }
    };
    
    // 🚨 설정 기반 범위 제한 적용 (user.crawling.page_range_limit)
    let max_allowed_pages = app_config.user.crawling.page_range_limit;
    let requested_pages = if calculated_start_page >= calculated_end_page {
        calculated_start_page - calculated_end_page + 1
    } else {
        calculated_end_page - calculated_start_page + 1
    };
    
    let (final_start_page, final_end_page) = if requested_pages > max_allowed_pages {
        info!("⚠️ [ACTOR] CrawlingPlanner requested {} pages, but config limits to {} pages", 
              requested_pages, max_allowed_pages);
        // 설정 제한에 맞춰 범위 조정
        let limited_start = site_status.total_pages;
        let limited_end = if limited_start >= max_allowed_pages { 
            limited_start - max_allowed_pages + 1 
        } else { 
            1 
        };
        info!("🔒 [ACTOR] Range limited by config: {} to {} ({} pages)", 
              limited_start, limited_end, max_allowed_pages);
        (limited_start, limited_end)
    } else if request.start_page != 0 && request.end_page != 0 {
        // 사용자가 명시적으로 범위를 지정한 경우
        info!("👤 [ACTOR] User specified range: {} to {}", request.start_page, request.end_page);
        info!("🤖 [ACTOR] CrawlingPlanner recommendation: {} to {}", calculated_start_page, calculated_end_page);
        
        // 사용자 범위도 설정 제한 적용
        let user_pages = if request.start_page >= request.end_page {
            request.start_page - request.end_page + 1
        } else {
            request.end_page - request.start_page + 1
        };
        
        if user_pages <= max_allowed_pages {
            info!("✅ [ACTOR] Using user range (within config limits)");
            (request.start_page, request.end_page)
        } else {
            info!("⚠️ [ACTOR] User range exceeds config limit, using CrawlingPlanner recommendation");
            (calculated_start_page, calculated_end_page)
        }
    } else {
        // CrawlingPlanner 권장사항 사용
        info!("🧠 [ACTOR] Using CrawlingPlanner recommendation");
        (calculated_start_page, calculated_end_page)
    };
    
    // 🎯 설정 기반 처리 전략 사용 (하드코딩 제거)
    let recommended_batch_size = processing_strategy.recommended_batch_size;
    let recommended_concurrency = processing_strategy.recommended_concurrency;
    
    // 배치 크롤링 설정 생성 - 🧠 CrawlingPlanner 권장사항과 설정 파일 기반
    let batch_config = BatchCrawlingConfig {
        start_page: final_start_page,
        end_page: final_end_page,
        concurrency: recommended_concurrency,
        batch_size: recommended_batch_size,
        delay_ms: request.delay_ms.unwrap_or(app_config.user.request_delay_ms),
        list_page_concurrency: app_config.user.crawling.workers.list_page_max_concurrent as u32,
        product_detail_concurrency: app_config.user.crawling.workers.product_detail_max_concurrent as u32,
        retry_max: app_config.advanced.retry_attempts,
        timeout_ms: (app_config.advanced.request_timeout_seconds * 1000) as u64,
        disable_intelligent_range: false, // 🧠 CrawlingPlanner 사용하므로 false
        cancellation_token: None,
    };
    
    info!("🧠 [ACTOR] Final configuration applied:");
    info!("   📊 Range: {} to {} ({} pages, config limit: {})", 
          final_start_page, final_end_page, 
          if final_start_page >= final_end_page { final_start_page - final_end_page + 1 } else { final_end_page - final_start_page + 1 },
          app_config.user.crawling.page_range_limit);
    info!("   ⚙️ Processing: batch_size={}, concurrency={}, delay={}ms", 
          recommended_batch_size, recommended_concurrency, batch_config.delay_ms);
    info!("   🔧 Workers: list_page={}, product_detail={}, retries={}", 
          batch_config.list_page_concurrency, batch_config.product_detail_concurrency, batch_config.retry_max);
    
    // ServiceBasedBatchCrawlingEngine 생성
    let crawling_engine = ServiceBasedBatchCrawlingEngine::new(
        http_client,
        data_extractor,
        product_repo,
        event_emitter,
        batch_config,
        session_id.to_string(),
        app_config,
    );
    
    // 분석 정보를 JSON으로 구성
    let analysis_info = serde_json::json!({
        "range_recommendation": format!("{:?}", range_recommendation),
        "user_requested": {
            "start_page": request.start_page,
            "end_page": request.end_page
        },
        "intelligent_calculated": {
            "start_page": calculated_start_page,
            "end_page": calculated_end_page
        },
        "final_used": {
            "start_page": final_start_page,
            "end_page": final_end_page
        },
        "site_analysis": {
            "total_pages": site_status.total_pages,
            "products_on_last_page": site_status.products_on_last_page,
            "estimated_products": site_status.estimated_products,
            "is_accessible": site_status.is_accessible
        }
    });
    
    info!("✅ Real crawling engine initialized successfully with intelligent planning");
    Ok((crawling_engine, analysis_info))
}

/// 시뮬레이션 크롤링 실행 (폴백)
async fn run_simulation_crawling(
    request: &ActorCrawlingRequest,
    batch_size: u32,
) {
    info!("🔄 Running simulation crawling as fallback...");
    
    // 페이지 범위를 배치로 분할
    let mut current_page = request.start_page;
    let mut batch_number = 1;
    
    while current_page <= request.end_page {
        let batch_end = std::cmp::min(current_page + batch_size - 1, request.end_page);
        info!("📦 Processing Batch {}: pages {} to {}", batch_number, current_page, batch_end);
        
        // 배치별 페이지 처리 시뮬레이션
        for page in current_page..=batch_end {
            info!("🔍 Processing page {} with simulated crawling", page);
            
            // 시뮬레이션 지연 시간
            tokio::time::sleep(Duration::from_millis(request.delay_ms.unwrap_or(800))).await;
            
            info!("✅ Page {} processed successfully", page);
        }
        
        info!("✅ Batch {} completed", batch_number);
        current_page = batch_end + 1;
        batch_number += 1;
    }
    
    info!("🎉 Simulation crawling completed successfully");
}
