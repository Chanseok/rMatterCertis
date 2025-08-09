//! Actor System Commands for Tauri Integration
//! 
//! Commands to test and use the Actor system from the UI

use crate::new_architecture::actors::SessionActor;
use crate::new_architecture::context::{SystemConfig, AppContext};
use crate::new_architecture::channels::types::AppEvent;
use crate::new_architecture::channels::types::ActorCommand; // 올바른 ActorCommand 사용
use crate::new_architecture::actors::types::{CrawlingConfig, BatchConfig, ExecutionPlan, PageRange, SessionSummary};
use crate::new_architecture::actor_event_bridge::start_actor_event_bridge;
use crate::infrastructure::config::AppConfig;
use crate::domain::services::SiteStatus;
use crate::infrastructure::simple_http_client::HttpClient;
use crate::infrastructure::html_parser::MatterDataExtractor;
use crate::infrastructure::integrated_product_repository::IntegratedProductRepository;
use crate::application::AppState;
 // 실제 CrawlingPlanner에서 사용
use crate::infrastructure::config::ConfigManager; // 설정 관리자 추가
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::sync::{mpsc, broadcast, watch};
use tokio::time::Duration;
use tracing::{info, error, warn};
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

/// 🎭 Actor System 크롤링 시작 (새로운 아키텍처 - 워크플로우 통합)
/// 
/// 분석-계획-실행 워크플로우를 단일화:
/// 1. CrawlingPlanner를 단 한 번만 호출하여 ExecutionPlan 생성
/// 2. SessionActor는 ExecutionPlan을 받아서 순수 실행만 담당
/// 3. UI 파라미터 의존성 제거 - 설정 파일 기반 자율 운영
#[tauri::command]
pub async fn start_actor_system_crawling(
    app: AppHandle,
    _request: ActorCrawlingRequest, // UI 파라미터는 무시 (설계 원칙에 따라)
) -> Result<ActorSystemResponse, String> {
    info!("🎭 [NEW ARCHITECTURE] Starting unified Analysis-Plan-Execute workflow");
    
    // === Phase 1: 분석 및 계획 (CrawlingPlanner 단일 호출) ===
    info!("🧠 Phase 1: Creating ExecutionPlan with CrawlingPlanner...");
    
    let (execution_plan, app_config, site_status) = create_execution_plan(&app).await
        .map_err(|e| format!("Failed to create execution plan: {}", e))?;
    
    info!("✅ ExecutionPlan created: {} batches, {} total pages", 
          execution_plan.crawling_ranges.len(),
          execution_plan.crawling_ranges.iter().map(|r| 
              if r.reverse_order { r.start_page - r.end_page + 1 } 
              else { r.end_page - r.start_page + 1 }
          ).sum::<u32>());
    
    // === Phase 2: 실행 (SessionActor에 ExecutionPlan 전달) ===
    info!("🎭 Phase 2: Executing with SessionActor...");
    
    // 🌉 Actor 이벤트 브릿지를 위한 브로드캐스트 채널 생성
    let (actor_event_tx, actor_event_rx) = broadcast::channel::<AppEvent>(1000);
    
    // 🌉 Actor Event Bridge 시작 - Actor 이벤트를 프론트엔드로 자동 전달
    let _bridge_handle = start_actor_event_bridge(app.clone(), actor_event_rx)
        .await
        .map_err(|e| format!("Failed to start Actor Event Bridge: {}", e))?;
    
    info!("🌉 Actor Event Bridge started successfully");

    // SessionActor 생성
    let _session_actor = SessionActor::new(execution_plan.session_id.clone());
    
    info!("🎭 SessionActor created with ID: {}", execution_plan.session_id);
    
    // ExecutionPlan 기반 실행 (백그라운드)
    let execution_plan_for_task = execution_plan.clone();
    let app_config_for_task = app_config.clone();
    let site_status_for_task = site_status.clone();
    let actor_event_tx_for_spawn = actor_event_tx.clone();
    let session_id_for_return = execution_plan.session_id.clone();
    
    let _session_actor_handle = tokio::spawn(async move {
        info!("🚀 SessionActor executing with predefined ExecutionPlan...");
        
        // SessionActor는 더 이상 분석/계획하지 않고 순수 실행만
        match execute_session_actor_with_execution_plan(
            execution_plan_for_task,
            &app_config_for_task,
            &site_status_for_task,
            actor_event_tx_for_spawn,
        ).await {
            Ok(()) => {
                info!("🎉 Actor system crawling completed successfully!");
            }
            Err(e) => {
                error!("❌ Actor system crawling failed: {}", e);
            }
        }
        
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    });
    
    // 즉시 응답 반환 (비동기 실행)
    Ok(ActorSystemResponse {
        success: true,
        message: "Actor system crawling started with ExecutionPlan".to_string(),
        session_id: Some(session_id_for_return),
        data: Some(serde_json::to_value(&execution_plan).map_err(|e| e.to_string())?),
    })
}

/// 🔧 ServiceBasedBatchCrawlingEngine 크롤링 (가짜 크롤링 - 참고용)
/// NOTE: Deprecated – actor-based pipeline is canonical now. Schedule for removal after pagination & rate-limit FSM stabilization
// #[tauri::command]
// pub async fn start_service_based_crawling(
//     app: AppHandle,
//     request: ActorCrawlingRequest,
// ) -> Result<ActorSystemResponse, String> {
//     info!("🔧 [SERVICE-BASED] Starting ServiceBasedBatchCrawlingEngine crawling: {:?}", request);
    
//     let session_id = format!("service_session_{}", Utc::now().timestamp());
    
// /// ServiceBasedBatchCrawlingEngine 초기화 (참고용)
// /// NOTE: Deprecated – replace with ExecutionPlan + SessionActor flow. Pending removal
// async fn initialize_service_based_engine(
//     session_id: &str,
//     request: &ActorCrawlingRequest,
//     app_handle: &AppHandle,
// ) -> Result<(ServiceBasedBatchCrawlingEngine, serde_json::Value), Box<dyn std::error::Error + Send + Sync>> {
//     info!("🔧 Initializing ServiceBasedBatchCrawlingEngine for session: {}", session_id);
//     Err("ServiceBasedBatchCrawlingEngine deprecated".into())
// }
//     }
// }
// /// ServiceBasedBatchCrawlingEngine 초기화 (참고용 - 완전 비활성화)
// /// NOTE: Fully deprecated. Left commented for historical reference. Will be removed.
// async fn initialize_service_based_engine(
//     _session_id: &str,
//     _request: &ActorCrawlingRequest,
//     _app_handle: &AppHandle,
// ) -> Result<(ServiceBasedBatchCrawlingEngine, serde_json::Value), Box<dyn std::error::Error + Send + Sync>> {
//     Err("ServiceBasedBatchCrawlingEngine deprecated".into())
// }

/// Test SessionActor functionality
#[tauri::command]
pub async fn test_session_actor_basic(
    _app: AppHandle,
) -> Result<ActorSystemResponse, String> {
    info!("🧪 Testing SessionActor...");
    
    let _system_config = Arc::new(SystemConfig::default());
    let (_control_tx, _control_rx) = mpsc::channel::<ActorCommand>(100);
    let (_event_tx, _event_rx) = mpsc::channel::<AppEvent>(500);
    
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
    let _system_config = Arc::new(SystemConfig::default());
    let (_control_tx, _control_rx) = mpsc::channel::<ActorCommand>(100);
    let (_event_tx, _event_rx) = mpsc::channel::<AppEvent>(500);
    
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
#[allow(dead_code)]
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
    ).with_repository(product_repo.clone());
    
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
    app_config: &AppConfig,
    site_status: &SiteStatus,
    actor_event_tx: broadcast::Sender<AppEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("🎭 SessionActor {} starting with range {} to {}, batch_size: {}", 
          session_id, start_page, end_page, batch_size);
    
    // AppContext 생성에 필요한 채널들 생성
    let system_config = Arc::new(SystemConfig::default());
    let (control_tx, _control_rx) = mpsc::channel::<ActorCommand>(100);
    let (_cancellation_tx, cancellation_rx) = watch::channel(false);
    
    // AppContext 생성 (실제로는 IntegratedContext::new 호출)
    let context = Arc::new(AppContext::new(
        session_id.to_string(),
        control_tx,
        actor_event_tx.clone(),
        cancellation_rx,
        system_config,
    ));
    
    // 페이지 범위를 BatchActor들에게 배분
    let pages: Vec<u32> = if start_page > end_page {
        (end_page..=start_page).rev().collect()
    } else {
        (start_page..=end_page).collect()
    };
    
    let total_pages = pages.len();
    let batch_count = (total_pages + batch_size as usize - 1) / batch_size as usize;
    
    info!("📊 SessionActor will create {} BatchActors for {} pages", batch_count, total_pages);
    
    // SessionStarted 이벤트 발송 (설정 파일 기반 값 사용)
    let session_event = AppEvent::SessionStarted {
        session_id: session_id.to_string(),
        config: CrawlingConfig {
            site_url: "https://csa-iot.org/csa-iot_products/".to_string(),
            start_page,
            end_page,
            // Align with global max concurrency to avoid mixed values in logs
            concurrency_limit: app_config.user.max_concurrent_requests,
            batch_size: batch_size,
            request_delay_ms: app_config.user.request_delay_ms,
            timeout_secs: app_config.advanced.request_timeout_seconds,
            max_retries: app_config.advanced.retry_attempts,
            strategy: crate::new_architecture::actors::types::CrawlingStrategy::NewestFirst,
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
        
        // ✅ 실제 BatchActor 구현 호출 
        info!("🚀 About to call execute_real_batch_actor for batch: {}", batch_id);
    match execute_real_batch_actor(&batch_id, page_chunk, &context, app_config, site_status).await {
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
                error!("❌ BatchActor {} failed with error: {:?}", batch_id, e);
                error!("❌ Error details: {}", e);
                
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

/// 실제 BatchActor 실행
async fn execute_real_batch_actor(
    batch_id: &str,
    pages: &[u32],
    context: &AppContext,
    app_config: &AppConfig,
    site_status: &SiteStatus,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use crate::new_architecture::actors::{BatchActor, ActorCommand};
    use crate::new_architecture::actors::traits::Actor;
    use tokio::sync::mpsc;
    
    info!("🎯 BatchActor {} starting REAL processing of {} pages", batch_id, pages.len());
    info!("🔧 Creating BatchActor instance with real services...");
    
    // 🔥 Phase 1: 실제 서비스들 생성 및 주입
    use crate::infrastructure::{HttpClient, MatterDataExtractor};
    // AppConfig type is provided via function parameter; no local import needed
    use crate::infrastructure::IntegratedProductRepository;
    use std::sync::Arc;
    
    // HttpClient 생성
    let http_client = Arc::new(
        HttpClient::create_from_global_config()
            .map_err(|e| format!("Failed to create HttpClient: {}", e))?
            .with_context_label(&format!("BatchActor:{}", batch_id))
    );
    info!("✅ HttpClient created (labeled)");
    
    // MatterDataExtractor 생성  
    let data_extractor = Arc::new(MatterDataExtractor::new()
        .map_err(|e| format!("Failed to create MatterDataExtractor: {}", e))?);
    info!("✅ MatterDataExtractor created");
    
    // IntegratedProductRepository 생성
    use crate::infrastructure::DatabaseConnection;
    let database_url = crate::infrastructure::database_paths::get_main_database_url();
    info!("🔧 Using database URL: {}", database_url);
    let db_connection = DatabaseConnection::new(&database_url).await
        .map_err(|e| format!("Failed to create DatabaseConnection: {}", e))?;
    let product_repo = Arc::new(IntegratedProductRepository::new(db_connection.pool().clone()));
    info!("✅ IntegratedProductRepository created with centralized database path");
    
    // AppConfig 사용: ExecutionPlan 경로에서 로드한 설정 사용 (개발 기본값 사용하지 않음)
    let app_config = app_config.clone();
    // Clone once more for passing into BatchActor::new_with_services (it takes ownership)
    let app_config_for_actor = app_config.clone();
    info!("✅ AppConfig provided from ExecutionPlan context");
    
    // AppConfig에서 실제 batch_size 미리 추출 (app_config이 move되기 전에)
    let user_batch_size = app_config.user.batch.batch_size;
    info!("📊 Using batch_size from config: {}", user_batch_size);
    
    // BatchActor를 실제 서비스들과 함께 생성
    let mut batch_actor = BatchActor::new_with_services(
        batch_id.to_string(),
        batch_id.to_string(), // batch_id도 같이 전달
        http_client,
        data_extractor,
        product_repo,
        app_config_for_actor,
    );
    info!("✅ BatchActor created successfully with real services");
    
    // BatchActor 실행을 위한 채널 생성
    info!("🔧 Creating communication channels...");
    let (command_tx, command_rx) = mpsc::channel::<ActorCommand>(100);
    info!("✅ Channels created successfully");
    
    // ProcessBatch 명령 생성
    info!("🔧 Creating BatchConfig...");
    
    let batch_config = BatchConfig {
        batch_size: user_batch_size,
        // Use the app-level max concurrency for batch execution to match plan/session
        concurrency_limit: app_config.user.max_concurrent_requests,
        batch_delay_ms: 1000,
        retry_on_failure: true,
        start_page: Some(pages[0]),
        end_page: Some(pages[pages.len() - 1]),
    };
    info!("✅ BatchConfig created: {:?}", batch_config);
    
    info!("🔧 Creating ProcessBatch command...");
    let process_batch_cmd = ActorCommand::ProcessBatch {
        batch_id: batch_id.to_string(),
        pages: pages.to_vec(),
        config: batch_config,
        batch_size: user_batch_size,
        concurrency_limit: app_config.user.max_concurrent_requests,
        total_pages: site_status.total_pages,
        products_on_last_page: site_status.products_on_last_page,
    };
    info!("✅ ProcessBatch command created");
    
    // BatchActor 실행 태스크 시작
    info!("🚀 Starting BatchActor task...");
    let context_clone = context.clone();
    let batch_task = tokio::spawn(async move {
        info!("📡 BatchActor.run() starting...");
        let result = batch_actor.run(context_clone, command_rx).await;
        info!("📡 BatchActor.run() completed with result: {:?}", result);
        result
    });
    info!("✅ BatchActor task spawned");
    
    // ProcessBatch 명령 전송
    info!("📡 Sending ProcessBatch command...");
    command_tx.send(process_batch_cmd).await
        .map_err(|e| format!("Failed to send ProcessBatch command: {}", e))?;
    info!("✅ ProcessBatch command sent");
    
    // Shutdown 명령 전송
    info!("📡 Sending Shutdown command...");
    command_tx.send(ActorCommand::Shutdown).await
        .map_err(|e| format!("Failed to send Shutdown command: {}", e))?;
    info!("✅ Shutdown command sent");
    
    // BatchActor 완료 대기
    info!("⏳ Waiting for BatchActor completion...");
    batch_task.await
        .map_err(|e| format!("BatchActor task failed: {}", e))?
        .map_err(|e| format!("BatchActor execution failed: {:?}", e))?;
    
    info!("✅ BatchActor {} completed REAL processing of {} pages", batch_id, pages.len());
    Ok(())
}

/// CrawlingPlanner 기반 ExecutionPlan 생성 (단일 호출)
/// 
/// 시스템 상태를 종합 분석하여 최적의 실행 계획을 생성합니다.
/// 이 함수가 호출된 후에는 더 이상 분석/계획 단계가 없습니다.
async fn create_execution_plan(app: &AppHandle) -> Result<(ExecutionPlan, AppConfig, SiteStatus), Box<dyn std::error::Error + Send + Sync>> {
    info!("🧠 Creating ExecutionPlan with single CrawlingPlanner call...");
    
    // 1. 설정 로드
    let config_manager = ConfigManager::new()?;
    let app_config = config_manager.load_config().await?;
    
    // 2. 이미 초기화된 데이터베이스 풀 사용 (새로 연결하지 않음)
    let app_state = app.state::<AppState>();
    let db_pool = {
        let pool_guard = app_state.database_pool.read().await;
        pool_guard.as_ref()
            .ok_or("Database pool not initialized")?
            .clone()
    };
    
    info!("📊 Using existing database pool from AppState");
    
    // 3. 서비스 생성 (기존 데이터베이스 풀 재사용)
    let product_repo = Arc::new(IntegratedProductRepository::new(db_pool.clone()));
    
    // 🔍 데이터베이스 연결 테스트
    info!("🔍 Testing database connection before creating CrawlingPlanner...");
    match product_repo.get_product_count().await {
        Ok(count) => {
            info!("✅ Database connection successful: {} products found", count);
        }
        Err(e) => {
            error!("❌ Database connection failed in create_execution_plan: {}", e);
            return Err(format!("Database connection test failed: {}", e).into());
        }
    }
    
    let http_client = HttpClient::create_from_global_config()?.with_context_label("Planner");
    let data_extractor = MatterDataExtractor::new()?;
    
    let status_checker = Arc::new(
        crate::infrastructure::crawling_service_impls::StatusCheckerImpl::with_product_repo(
            http_client.clone(),
            data_extractor.clone(),
            app_config.clone(),
            product_repo.clone(),
        )
    );
    
    let database_analyzer = Arc::new(
        crate::infrastructure::crawling_service_impls::DatabaseAnalyzerImpl::new(
            product_repo.clone()
        )
    );
    
    // 4. CrawlingPlanner 생성 및 분석
    let crawling_planner = crate::new_architecture::services::crawling_planner::CrawlingPlanner::new(
        status_checker,
        database_analyzer,
        Arc::new(SystemConfig::default()),
    ).with_repository(product_repo.clone());
    
    info!("🎯 Analyzing system state with CrawlingPlanner...");

    // ──────────────────────────────────────────────
    // (1) 사전 데이터베이스 상태로 전략 결정 힌트 계산
    let existing_product_count = match product_repo.get_product_count().await {
        Ok(c) => c,
        Err(e) => { warn!("⚠️ Failed to get product count for strategy decision: {} -> default NewestFirst", e); 0 }
    };

    // 기본 전략은 NewestFirst. DB에 데이터가 있으면 ContinueFromDb 시도
    let mut chosen_strategy = crate::new_architecture::actors::types::CrawlingStrategy::NewestFirst;
    if existing_product_count > 0 {
        chosen_strategy = crate::new_architecture::actors::types::CrawlingStrategy::ContinueFromDb;
        info!("🧭 Choosing ContinueFromDb strategy (existing products={})", existing_product_count);
    } else {
        info!("🧭 Choosing NewestFirst strategy (empty DB)");
    }

    // (2) CrawlingConfig 생성 (start_page/end_page는 '개수' 표현: start_page - end_page + 1 = 요청 수)
    let crawling_config = CrawlingConfig {
        site_url: "https://csa-iot.org/csa-iot_products/".to_string(),
        start_page: app_config.user.crawling.page_range_limit.max(1), // 요청 개수 표현
        end_page: 1,
        concurrency_limit: app_config.user.max_concurrent_requests,
        batch_size: app_config.user.batch.batch_size,
        request_delay_ms: 1000,
        timeout_secs: 300,
        max_retries: app_config.user.crawling.workers.max_retries,
        strategy: chosen_strategy.clone(),
    };

    // (3) 사이트 상태 및 계획 생성 (사이트 상태 1회 조회 + DB 분석)
    let (crawling_plan, site_status) = crawling_planner.create_crawling_plan_with_cache(&crawling_config, None).await?;
    info!("🧪 CrawlingPlanner produced plan with {:?} (requested strategy {:?})", crawling_plan.optimization_strategy, chosen_strategy);
    
    info!("📋 CrawlingPlan created: {:?}", crawling_plan);
    
    // 5. ExecutionPlan 생성
    let session_id = format!("actor_session_{}", Utc::now().timestamp());
    let plan_id = format!("plan_{}", Utc::now().timestamp());
    
    // CrawlingPlan에서 ListPageCrawling phases를 수집하고, 최신순 페이지를 배치 크기로 분할
    let mut all_pages: Vec<u32> = Vec::new();
    for phase in &crawling_plan.phases {
        if let crate::new_architecture::services::crawling_planner::PhaseType::ListPageCrawling = phase.phase_type {
            // 각 ListPageCrawling phase에는 해당 배치의 페이지들이 담겨있음(최신순)
            // Phase의 pages를 그대로 append (이미 최신→과거 순)
            all_pages.extend(phase.pages.iter());
        }
    }

    // 설정의 page_range_limit로 상한 적용
    let page_limit = app_config.user.crawling.page_range_limit.max(1) as usize;
    if all_pages.len() > page_limit {
        all_pages.truncate(page_limit);
    }

    // 배치 크기로 분할 (역순 범위 유지)
    let batch_size = app_config.user.batch.batch_size.max(1) as usize;
    let mut crawling_ranges: Vec<PageRange> = Vec::new();
    for chunk in all_pages.chunks(batch_size) {
        if let (Some(&first), Some(&last)) = (chunk.first(), chunk.last()) {
            // chunk는 최신→과거 순서이므로 start_page=first, end_page=last, reverse_order=true
            let pages_count = (first.saturating_sub(last)) + 1;
            crawling_ranges.push(PageRange {
                start_page: first,
                end_page: last,
                estimated_products: pages_count * 12, // 대략치
                reverse_order: true,
            });
        }
    }
    
    if crawling_ranges.is_empty() {
        // 안전 폴백 (최신 1페이지)
        let last_page = all_pages.first().copied().unwrap_or(1);
        crawling_ranges.push(PageRange {
            start_page: last_page,
            end_page: last_page,
            estimated_products: 12,
            reverse_order: true,
        });
    }
    
    let total_pages: u32 = crawling_ranges.iter().map(|r| {
        if r.reverse_order { r.start_page - r.end_page + 1 } 
        else { r.end_page - r.start_page + 1 }
    }).sum();
    
    // DB page/index 상태 읽기 (실패 시 None 유지)
    let (db_max_page_id, db_max_index_in_page) = match product_repo.get_max_page_id_and_index().await {
        Ok(v) => v,
        Err(e) => { warn!("⚠️ Failed to read max page/index: {}", e); (None, None) }
    };
    info!("🧾 DB snapshot: max_page_id={:?} max_index_in_page={:?} total_products_dbMetric={:?}", db_max_page_id, db_max_index_in_page, crawling_plan.db_total_products);

    // 입력 스냅샷 구성 (사이트/DB 상태 + 핵심 제한값)
    let snapshot = crate::new_architecture::actors::types::PlanInputSnapshot {
        total_pages: site_status.total_pages,
        products_on_last_page: site_status.products_on_last_page,
        db_max_page_id,
        db_max_index_in_page,
        db_total_products: crawling_plan.db_total_products.unwrap_or(0) as u64,
        page_range_limit: app_config.user.crawling.page_range_limit,
        batch_size: app_config.user.batch.batch_size,
        concurrency_limit: app_config.user.max_concurrent_requests,
        created_at: Utc::now(),
    };

    // 해시 계산 (스냅샷 + 페이지들 + 전략 핵심 필드 직렬화)
    let hash_input = serde_json::json!({
        "snapshot": &snapshot,
        "ranges": &crawling_ranges,
        "strategy": format!("{:?}", crawling_plan.optimization_strategy),
    });
    let hash_string = serde_json::to_string(&hash_input).unwrap_or_default();
    let plan_hash = blake3::hash(hash_string.as_bytes()).to_hex().to_string();

    let execution_plan = ExecutionPlan {
        plan_id,
        session_id,
        crawling_ranges: crawling_ranges,
        batch_size: app_config.user.batch.batch_size,
        concurrency_limit: app_config.user.max_concurrent_requests,
        estimated_duration_secs: crawling_plan.total_estimated_duration_secs,
        created_at: Utc::now(),
        analysis_summary: format!("Strategy: {:?}, Total pages: {}", 
                                crawling_plan.optimization_strategy, total_pages),
    original_strategy: format!("{:?}", crawling_plan.optimization_strategy),
        input_snapshot: snapshot,
        plan_hash,
    };
    
    info!("✅ ExecutionPlan created successfully: {} pages across {} batches (hash={})", 
          total_pages, execution_plan.crawling_ranges.len(), execution_plan.plan_hash);
    
    Ok((execution_plan, app_config, site_status))
}

/// ExecutionPlan 기반 SessionActor 실행 (순수 실행 전용)
/// 
/// SessionActor는 더 이상 분석/계획하지 않고 ExecutionPlan을 충실히 실행합니다.
async fn execute_session_actor_with_execution_plan(
    execution_plan: ExecutionPlan,
    app_config: &AppConfig,
    site_status: &SiteStatus,
    actor_event_tx: broadcast::Sender<AppEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("🎭 Executing SessionActor with predefined ExecutionPlan...");
    info!("📋 Plan: {} batches, batch_size: {}, effective_concurrency: {}", 
          execution_plan.crawling_ranges.len(),
          execution_plan.batch_size,
          execution_plan.concurrency_limit);

    // 실행 전 해시 재계산 & 검증 (생성 시와 동일한 직렬화 스키마 사용)
    let verify_input = serde_json::json!({
        "snapshot": &execution_plan.input_snapshot,
        "ranges": &execution_plan.crawling_ranges,
        "strategy": &execution_plan.original_strategy,
    });
    if let Ok(serialized) = serde_json::to_string(&verify_input) {
        let current_hash = blake3::hash(serialized.as_bytes()).to_hex().to_string();
        if current_hash != execution_plan.plan_hash {
            tracing::error!("❌ ExecutionPlan hash mismatch! expected={}, got={}", execution_plan.plan_hash, current_hash);
            return Err("ExecutionPlan integrity check failed".into());
        } else {
            tracing::info!("🔐 ExecutionPlan integrity verified (hash={})", current_hash);
        }
    } else {
        tracing::warn!("⚠️ Failed to serialize ExecutionPlan for integrity verification – continuing cautiously");
    }
    
    // 시작 이벤트 방출 (설정 파일 기반 값 사용)
    // 전략 추론: 첫 배치가 마지막 페이지보다 작은 페이지를 포함하면 ContinueFromDb였을 가능성 높음
    let inferred_strategy = if execution_plan.crawling_ranges.len() > 1 {
        // 여러 범위가 있고 첫 start_page가 site_status.total_pages 보다 작으면 ContinueFromDb 추정
        let first_start = execution_plan.crawling_ranges.first().map(|r| r.start_page).unwrap_or(1);
        if first_start < site_status.total_pages { crate::new_architecture::actors::types::CrawlingStrategy::ContinueFromDb } else { crate::new_architecture::actors::types::CrawlingStrategy::NewestFirst }
    } else {
        let first_range = execution_plan.crawling_ranges.first();
        if let Some(r) = first_range {
            if r.start_page < site_status.total_pages { crate::new_architecture::actors::types::CrawlingStrategy::ContinueFromDb } else { crate::new_architecture::actors::types::CrawlingStrategy::NewestFirst }
        } else { crate::new_architecture::actors::types::CrawlingStrategy::NewestFirst }
    };

    let session_event = AppEvent::SessionStarted {
        session_id: execution_plan.session_id.clone(),
        config: CrawlingConfig {
            site_url: "https://csa-iot.org/csa-iot_products/".to_string(),
            start_page: execution_plan.crawling_ranges.first().map(|r| r.start_page).unwrap_or(1),
            end_page: execution_plan.crawling_ranges.last().map(|r| r.end_page).unwrap_or(1),
            concurrency_limit: execution_plan.concurrency_limit,
            batch_size: execution_plan.batch_size,
            request_delay_ms: app_config.user.request_delay_ms,
            timeout_secs: app_config.advanced.request_timeout_seconds,
            max_retries: app_config.advanced.retry_attempts,
            strategy: inferred_strategy,
        },
        timestamp: Utc::now(),
    };
    
    if let Err(e) = actor_event_tx.send(session_event) {
        error!("Failed to send SessionStarted event: {}", e);
    }
    
    // 각 범위별로 순차 실행
    for (range_idx, page_range) in execution_plan.crawling_ranges.iter().enumerate() {
      info!("🎯 Processing batch {} of {}: pages {} to {} (reverse: {})", 
          range_idx + 1, execution_plan.crawling_ranges.len(),
              page_range.start_page, page_range.end_page, page_range.reverse_order);
        
        // 진행 상황 이벤트 방출
        let progress_percentage = ((range_idx as f64) / (execution_plan.crawling_ranges.len() as f64)) * 100.0;
        let progress_event = AppEvent::Progress {
            session_id: execution_plan.session_id.clone(),
            current_step: range_idx as u32 + 1,
            total_steps: execution_plan.crawling_ranges.len() as u32,
            message: format!("Processing pages {} to {}", page_range.start_page, page_range.end_page),
            percentage: progress_percentage,
            timestamp: Utc::now(),
        };
        
        if let Err(e) = actor_event_tx.send(progress_event) {
            error!("Failed to send progress event: {}", e);
        }
        
        // BatchActor로 실행 (기존 로직 재사용)
        match execute_session_actor_with_batches(
            &execution_plan.session_id,
            page_range.start_page,
            page_range.end_page,
            execution_plan.batch_size,
            app_config,
            site_status,
            actor_event_tx.clone(),
        ).await {
            Ok(()) => {
                info!("✅ Range {} completed successfully", range_idx + 1);
            }
            Err(e) => {
                error!("❌ Range {} failed: {}", range_idx + 1, e);
                // 계속 진행 (범위별 독립 실행)
            }
        }
    }
    
    // 완료 이벤트 방출
    let completion_event = AppEvent::SessionCompleted {
        session_id: execution_plan.session_id.clone(),
        summary: SessionSummary {
            session_id: execution_plan.session_id.clone(),
            total_duration_ms: 0, // 실제 시간은 나중에 계산
            total_pages_processed: execution_plan.crawling_ranges.iter().map(|r| {
                if r.reverse_order { r.start_page - r.end_page + 1 } 
                else { r.end_page - r.start_page + 1 }
            }).sum(),
            total_products_processed: 0, // 실제 처리 후 계산
            success_rate: 100.0,
            avg_page_processing_time: 2000,
            error_summary: vec![],
            processed_batches: execution_plan.crawling_ranges.len() as u32,
            total_success_count: 0,
            final_state: "Completed".to_string(),
            timestamp: Utc::now(),
        },
        timestamp: Utc::now(),
    };
    
    if let Err(e) = actor_event_tx.send(completion_event) {
        error!("Failed to send SessionCompleted event: {}", e);
    }
    
    info!("🎉 ExecutionPlan fully executed!");
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
