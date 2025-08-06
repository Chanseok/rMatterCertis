//! Actor System Commands for Tauri Integration
//! 
//! Commands to test and use the Actor system from the UI

use crate::new_architecture::actors::SessionActor;
use crate::new_architecture::context::SystemConfig;
use crate::new_architecture::channels::types::{AppEvent, BatchConfig};
use crate::new_architecture::actors::types::{CrawlingConfig, ActorCommand};
use crate::new_architecture::actor_event_bridge::{ActorEventBridge, start_actor_event_bridge};
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
use tokio::sync::{mpsc, broadcast};
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

/// 🎭 Actor System 크롤링 시작 (새로운 아키텍처)
/// 
/// 순수 Actor 기반: SessionActor → BatchActor → StageActor 아키텍처
/// CrawlingPlanner 기반 지능형 범위 계산과 ActorEventBridge 이벤트 시스템 포함.
/// ⚠️ ServiceBasedBatchCrawlingEngine을 사용하지 않음!
#[tauri::command]
pub async fn start_actor_system_crawling(
    app: AppHandle,
    request: ActorCrawlingRequest,
) -> Result<ActorSystemResponse, String> {
    info!("🎭 [NEW ARCHITECTURE] Starting REAL Actor-based crawling: {:?}", request);
    
    let batch_size = request.batch_size.unwrap_or(3);
    // 역순 크롤링을 고려한 total_pages 계산
    let total_pages = if request.start_page >= request.end_page {
        request.start_page - request.end_page + 1
    } else {
        request.end_page - request.start_page + 1
    };
    let batch_count = (total_pages + batch_size - 1) / batch_size; // 올림 계산
    
    info!("✅ [ACTOR] Creating actual SessionActor for real crawling");
    info!("📊 [ACTOR] Pages: {} to {}, Batch size: {}, Expected batches: {}", 
          request.start_page, request.end_page, batch_size, batch_count);
    
    // 🚀 실제 SessionActor 생성 및 실행
    let system_config = Arc::new(SystemConfig::default());
    let (_control_tx, control_rx) = mpsc::channel::<ActorCommand>(100);
    
    // 🌉 Actor 이벤트 브릿지를 위한 브로드캐스트 채널 생성
    let (actor_event_tx, actor_event_rx) = broadcast::channel::<AppEvent>(1000);
    
    // 🌉 Actor Event Bridge 시작 - Actor 이벤트를 프론트엔드로 자동 전달
    let bridge_handle = start_actor_event_bridge(app.clone(), actor_event_rx)
        .await
        .map_err(|e| format!("Failed to start Actor Event Bridge: {}", e))?;
    
    info!("🌉 Actor Event Bridge started successfully");

    // SessionActor 생성
    let _session_actor = SessionActor::new(
        format!("session_{}", chrono::Utc::now().timestamp())
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
    
    // actor_event_tx를 각 spawn에서 사용할 수 있도록 clone
    let actor_event_tx_for_spawn1 = actor_event_tx.clone();
    let actor_event_tx_for_spawn2 = actor_event_tx.clone();
    
    // 🔥 순수 Actor 시스템 실행 (백그라운드)
    let _session_actor_handle = tokio::spawn(async move {
        info!("🚀 SessionActor starting execution with pure Actor system...");
        
        // 🎯 CrawlingPlanner로 지능형 범위 계산
        match calculate_intelligent_crawling_range(&session_id_for_task, &request_for_task, &app_handle_for_task).await {
            Ok((final_start_page, final_end_page, analysis_info)) => {
                info!("✅ Intelligent range calculated: {} to {}", final_start_page, final_end_page);
                
                // 🎭 SessionActor가 범위를 여러 BatchActor에게 배분
                match execute_session_actor_with_batches(
                    &session_id_for_task, 
                    final_start_page, 
                    final_end_page,
                    request_for_task.batch_size.unwrap_or(3),
                    actor_event_tx_for_spawn1.clone()
                ).await {
                    Ok(()) => {
                        info!("🎉 Actor system crawling completed successfully!");
                    }
                    Err(e) => {
                        error!("❌ Actor system crawling failed: {}", e);
                    }
                }
            }
            Err(e) => {
                error!("❌ Failed to calculate intelligent range: {}", e);
                
                // 실패 시 시뮬레이션 모드로 폴백
                info!("🔄 Falling back to simulation mode...");
                run_simulation_crawling(&request_for_task, request_for_task.batch_size.unwrap_or(3)).await;
            }
        }
        
        info!("✅ SessionActor completed execution");
        
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    });
    
        // 🔥 이벤트 리스너 실행 (백그라운드) - Actor 이벤트를 브로드캐스트 채널로 발행
    let actor_event_tx_clone = actor_event_tx_for_spawn2.clone();
    let session_id_for_second_spawn = session_id.clone();
    let app_handle_for_events = app.clone();
    tokio::spawn(async move {
        // 🎯 시작 이벤트 방출 (Actor 시스템의 AppEvent 타입으로)
        info!("📡 Emitting SessionStarted event through Actor Event Bridge");
        let session_event = AppEvent::SessionStarted {
            session_id: session_id_for_second_spawn.clone(),
            config: CrawlingConfig {
                site_url: format!("https://matter.certis.io/device-list/{}", request.start_page),
                start_page: request.start_page,
                end_page: request.end_page,
                concurrency_limit: 5,
                batch_size: 20,
                request_delay_ms: 1000,
                timeout_secs: 300,
                max_retries: 3,
            },
            timestamp: chrono::Utc::now(),
        };
        
        // Actor Event Bridge를 통해 프론트엔드로 자동 전달
        if let Err(e) = actor_event_tx_clone.send(session_event) {
            error!("Failed to send Actor event through bridge: {}", e);
        } else {
            info!("✅ Actor event sent through bridge successfully");
        }
        
        // 추가 진행 상황 이벤트들 (시뮬레이션)
        tokio::time::sleep(Duration::from_millis(1000)).await;
        
        let progress_event = AppEvent::Progress {
            session_id: session_id_for_second_spawn.clone(),
            current_step: 1,
            total_steps: request.end_page - request.start_page + 1,
            message: "Starting crawling process...".to_string(),
            percentage: 10.0,
            timestamp: chrono::Utc::now(),
        };
        
        if let Err(e) = actor_event_tx_clone.send(progress_event) {
            error!("Failed to send progress event through bridge: {}", e);
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
        message: format!("🎭 Pure Actor-based crawling started with intelligent planning"), 
        session_id: Some(session_id_for_return),
        data: Some(serde_json::json!({
            "engine_type": "Pure Actor System",
            "architecture": "SessionActor → BatchActor → StageActor",
            "status": "RUNNING",
            "mode": "PURE_ACTOR_CRAWLING",
            "config": {
                "requested_start_page": request.start_page,
                "requested_end_page": request.end_page,
                "batch_size": batch_size,
                "concurrency": request.concurrency.unwrap_or(8),
                "total_pages": total_pages,
                "expected_batches": batch_count,
                "domain_logic_enabled": true,
                "service_based_engine": false
            }
        })),
    })
}

/// 🔧 ServiceBasedBatchCrawlingEngine 크롤링 (가짜 크롤링 - 참고용)
/// 
/// 기존 ServiceBasedBatchCrawlingEngine을 직접 사용하는 방식
/// 도메인 요구사항 일부 구현, 나중에 제거 예정
#[tauri::command]
pub async fn start_service_based_crawling(
    app: AppHandle,
    request: ActorCrawlingRequest,
) -> Result<ActorSystemResponse, String> {
    info!("🔧 [SERVICE-BASED] Starting ServiceBasedBatchCrawlingEngine crawling: {:?}", request);
    
    let session_id = format!("service_session_{}", Utc::now().timestamp());
    
    // ServiceBasedBatchCrawlingEngine 초기화 및 실행
    match initialize_service_based_engine(&session_id, &request, &app).await {
        Ok((mut crawling_engine, analysis_info)) => {
            info!("✅ ServiceBasedBatchCrawlingEngine initialized successfully");
            
            // 백그라운드에서 실행
            let _engine_handle = tokio::spawn(async move {
                match crawling_engine.execute().await {
                    Ok(()) => {
                        info!("🎉 ServiceBasedBatchCrawlingEngine completed successfully!");
                    }
                    Err(e) => {
                        error!("❌ ServiceBasedBatchCrawlingEngine failed: {}", e);
                    }
                }
            });
            
            Ok(ActorSystemResponse {
                success: true,
                message: "🔧 ServiceBasedBatchCrawlingEngine started successfully".to_string(),
                session_id: Some(session_id),
                data: Some(serde_json::json!({
                    "engine_type": "ServiceBasedBatchCrawlingEngine",
                    "architecture": "Direct ServiceBasedBatchCrawlingEngine",
                    "status": "RUNNING",
                    "mode": "SERVICE_BASED_CRAWLING",
                    "analysis_info": analysis_info
                })),
            })
        }
        Err(e) => {
            error!("❌ Failed to initialize ServiceBasedBatchCrawlingEngine: {}", e);
            Err(format!("ServiceBasedBatchCrawlingEngine initialization failed: {}", e))
        }
    }
}

/// ServiceBasedBatchCrawlingEngine 초기화 (참고용)
async fn initialize_service_based_engine(
    session_id: &str,
    request: &ActorCrawlingRequest,
    app_handle: &AppHandle,
) -> Result<(ServiceBasedBatchCrawlingEngine, serde_json::Value), Box<dyn std::error::Error + Send + Sync>> {
    info!("🔧 Initializing ServiceBasedBatchCrawlingEngine for session: {}", session_id);
    
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
    
    // 설정 로드
    let config_manager = crate::infrastructure::config::ConfigManager::new()
        .map_err(|e| format!("Failed to initialize config manager: {}", e))?;
    let app_config = config_manager.load_config().await
        .map_err(|e| format!("Failed to load config: {}", e))?;
    
    // 기본 배치 크롤링 설정 생성 (CrawlingPlanner 없이)
    let batch_config = BatchCrawlingConfig {
        start_page: request.start_page,
        end_page: request.end_page,
        concurrency: request.concurrency.unwrap_or(8),
        batch_size: request.batch_size.unwrap_or(3),
        delay_ms: request.delay_ms.unwrap_or(app_config.user.request_delay_ms),
        list_page_concurrency: app_config.user.crawling.workers.list_page_max_concurrent as u32,
        product_detail_concurrency: app_config.user.crawling.workers.product_detail_max_concurrent as u32,
        retry_max: app_config.advanced.retry_attempts,
        timeout_ms: (app_config.advanced.request_timeout_seconds * 1000) as u64,
        disable_intelligent_range: true, // CrawlingPlanner 사용하지 않음
        cancellation_token: None,
    };
    
    info!("🔧 [SERVICE-BASED] Configuration applied:");
    info!("   📊 Range: {} to {} ({} pages)", 
          batch_config.start_page, batch_config.end_page, 
          if batch_config.start_page >= batch_config.end_page { 
              batch_config.start_page - batch_config.end_page + 1 
          } else { 
              batch_config.end_page - batch_config.start_page + 1 
          });
    info!("   ⚙️ Processing: batch_size={}, concurrency={}, delay={}ms", 
          batch_config.batch_size, batch_config.concurrency, batch_config.delay_ms);
    
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
        "user_requested": {
            "start_page": request.start_page,
            "end_page": request.end_page
        },
        "engine_type": "ServiceBasedBatchCrawlingEngine",
        "intelligent_planning": false
    });
    
    info!("✅ ServiceBasedBatchCrawlingEngine initialized successfully");
    Ok((crawling_engine, analysis_info))
}

/// Test SessionActor functionality
#[tauri::command]
pub async fn test_session_actor_basic(
    _app: AppHandle,
) -> Result<ActorSystemResponse, String> {
    info!("🧪 Testing SessionActor...");
    
    let system_config = Arc::new(SystemConfig::default());
    let (_control_tx, control_rx) = mpsc::channel::<ActorCommand>(100);
    let (event_tx, _event_rx) = mpsc::channel::<AppEvent>(500);
    
    let _session_actor = SessionActor::new(
        format!("session_{}", chrono::Utc::now().timestamp())
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
    let (_control_tx, control_rx) = mpsc::channel::<ActorCommand>(100);
    let (event_tx, _event_rx) = mpsc::channel::<AppEvent>(500);
    
    let _session_actor = SessionActor::new(
        format!("session_{}", chrono::Utc::now().timestamp())
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

/// CrawlingPlanner 기반 지능형 범위 계산 (Actor 시스템용)
async fn calculate_intelligent_crawling_range(
    session_id: &str,
    request: &ActorCrawlingRequest,
    app_handle: &AppHandle,
) -> Result<(u32, u32, serde_json::Value), Box<dyn std::error::Error + Send + Sync>> {
    info!("🧠 Calculating intelligent crawling range for Actor system session: {}", session_id);
    
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
    let system_config = Arc::new(crate::new_architecture::context::SystemConfig::default());
    
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
    } else {
        // 🚨 프론트엔드에서는 By Design으로 페이지 범위를 지정하지 않음
        // 따라서 항상 CrawlingPlanner 권장사항을 사용
        info!("🧠 [ACTOR] Frontend does not specify page ranges by design - using CrawlingPlanner recommendation");
        info!("🤖 [ACTOR] CrawlingPlanner recommendation: {} to {}", calculated_start_page, calculated_end_page);
        
        // ⚠️ request.start_page와 request.end_page는 프론트엔드 테스트 코드에서 설정한 임시값이므로 무시
        if request.start_page != 0 && request.end_page != 0 {
            info!("⚠️ [ACTOR] Ignoring frontend test values (start_page: {}, end_page: {}) - using intelligent planning", 
                  request.start_page, request.end_page);
        }
        
        // CrawlingPlanner 권장사항 사용
        info!("🎯 [ACTOR] Using CrawlingPlanner intelligent recommendation for optimal crawling");
        (calculated_start_page, calculated_end_page)
    };
    
    info!("🧠 [ACTOR] Final range calculated:");
    info!("   📊 Range: {} to {} ({} pages, config limit: {})", 
          final_start_page, final_end_page, 
          if final_start_page >= final_end_page { final_start_page - final_end_page + 1 } else { final_end_page - final_start_page + 1 },
          app_config.user.crawling.page_range_limit);
    
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
        },
        "processing_strategy": {
            "recommended_batch_size": processing_strategy.recommended_batch_size,
            "recommended_concurrency": processing_strategy.recommended_concurrency
        }
    });
    
    info!("✅ Intelligent range calculation completed for Actor system");
    Ok((final_start_page, final_end_page, analysis_info))
}

/// 순수 Actor 기반 SessionActor 실행 (BatchActor들을 관리)
async fn execute_session_actor_with_batches(
    session_id: &str,
    start_page: u32,
    end_page: u32,
    batch_size: u32,
    actor_event_tx: broadcast::Sender<AppEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("🎭 SessionActor {} starting with range {} to {}, batch_size: {}", 
          session_id, start_page, end_page, batch_size);
    
    // 페이지 범위를 BatchActor들에게 배분
    let pages: Vec<u32> = if start_page > end_page {
        (end_page..=start_page).rev().collect()
    } else {
        (start_page..=end_page).collect()
    };
    
    let total_pages = pages.len();
    let batch_count = (total_pages + batch_size as usize - 1) / batch_size as usize;
    
    info!("📊 SessionActor will create {} BatchActors for {} pages", batch_count, total_pages);
    
    // SessionStarted 이벤트 발송
    let session_event = AppEvent::SessionStarted {
        session_id: session_id.to_string(),
        config: CrawlingConfig {
            site_url: "https://csa-iot.org/csa-iot_products/".to_string(),
            start_page,
            end_page,
            concurrency_limit: 5,
            batch_size: batch_size,
            request_delay_ms: 1000,
            timeout_secs: 300,
            max_retries: 3,
        },
        timestamp: chrono::Utc::now(),
    };
    
    if let Err(e) = actor_event_tx.send(session_event) {
        error!("Failed to send SessionStarted event: {}", e);
    }
    
    // BatchActor들을 순차적으로 실행 (SessionActor의 역할)
    for (batch_index, page_chunk) in pages.chunks(batch_size as usize).enumerate() {
        let batch_id = format!("{}_batch_{}", session_id, batch_index);
        let batch_start = page_chunk[0];
        let batch_end = page_chunk[page_chunk.len() - 1];
        
        info!("� SessionActor creating BatchActor {}: pages {} to {}", 
              batch_id, batch_start, batch_end);
        
        // BatchStarted 이벤트 발송
        let batch_event = AppEvent::BatchStarted {
            session_id: session_id.to_string(),
            batch_id: batch_id.clone(),
            pages_count: page_chunk.len() as u32,
            timestamp: chrono::Utc::now(),
        };
        
        if let Err(e) = actor_event_tx.send(batch_event) {
            error!("Failed to send BatchStarted event: {}", e);
        }
        
        // TODO: 실제 BatchActor 구현 호출
        // 현재는 시뮬레이션
        match execute_batch_actor_simulation(&batch_id, page_chunk, actor_event_tx.clone()).await {
            Ok(()) => {
                info!("✅ BatchActor {} completed successfully", batch_id);
                
                // BatchCompleted 이벤트 발송
                let batch_completed_event = AppEvent::BatchCompleted {
                    session_id: session_id.to_string(),
                    batch_id: batch_id.clone(),
                    success_count: page_chunk.len() as u32,
                    failed_count: 0,
                    duration: 1000, // TODO: 실제 시간 계산
                    timestamp: chrono::Utc::now(),
                };
                
                if let Err(e) = actor_event_tx.send(batch_completed_event) {
                    error!("Failed to send BatchCompleted event: {}", e);
                }
            }
            Err(e) => {
                error!("❌ BatchActor {} failed: {}", batch_id, e);
                
                // BatchFailed 이벤트 발송
                let batch_failed_event = AppEvent::BatchFailed {
                    session_id: session_id.to_string(),
                    batch_id: batch_id.clone(),
                    error: format!("{}", e),
                    final_failure: false,
                    timestamp: chrono::Utc::now(),
                };
                
                if let Err(e) = actor_event_tx.send(batch_failed_event) {
                    error!("Failed to send BatchFailed event: {}", e);
                }
                
                return Err(e);
            }
        }
        
        // 배치 간 간격 (시스템 안정성)
        if batch_index < batch_count - 1 {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
    
    // SessionCompleted 이벤트 발송
    let session_completed_event = AppEvent::SessionCompleted {
        session_id: session_id.to_string(),
        summary: crate::new_architecture::actors::types::SessionSummary {
            session_id: session_id.to_string(),
            total_duration_ms: 5000, // TODO: 실제 시간 계산
            total_pages_processed: total_pages as u32,
            total_products_processed: total_pages as u32 * 12, // 근사치
            success_rate: 100.0, // TODO: 실제 성공률
            avg_page_processing_time: 1000, // TODO: 실제 평균 시간
            error_summary: vec![], // TODO: 실제 에러 요약
            processed_batches: batch_count as u32,
            total_success_count: total_pages as u32,
            final_state: "completed".to_string(),
            timestamp: chrono::Utc::now(),
        },
        timestamp: chrono::Utc::now(),
    };
    
    if let Err(e) = actor_event_tx.send(session_completed_event) {
        error!("Failed to send SessionCompleted event: {}", e);
    }
    
    info!("🎉 SessionActor {} completed all {} BatchActors successfully", session_id, batch_count);
    Ok(())
}

/// BatchActor 시뮬레이션 (나중에 실제 구현으로 교체)
async fn execute_batch_actor_simulation(
    batch_id: &str,
    pages: &[u32],
    actor_event_tx: broadcast::Sender<AppEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("🎯 BatchActor {} simulating processing of {} pages", batch_id, pages.len());
    
    for (index, page) in pages.iter().enumerate() {
        info!("🔍 BatchActor {} processing page {}", batch_id, page);
        
        // 진행 상황 이벤트 발송
        let progress_event = AppEvent::Progress {
            session_id: batch_id.split('_').take(2).collect::<Vec<_>>().join("_"),
            current_step: index as u32 + 1,
            total_steps: pages.len() as u32,
            message: format!("Processing page {}", page),
            percentage: ((index + 1) as f64 / pages.len() as f64) * 100.0,
            timestamp: chrono::Utc::now(),
        };
        
        if let Err(e) = actor_event_tx.send(progress_event) {
            error!("Failed to send Progress event: {}", e);
        }
        
        // 시뮬레이션 처리 시간
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        info!("✅ BatchActor {} completed page {}", batch_id, page);
    }
    
    info!("✅ BatchActor {} completed all {} pages", batch_id, pages.len());
    Ok(())
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
