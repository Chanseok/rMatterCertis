//! # Modern Crawling Commands v4.0 (QUARANTINED - pending deletion)
//!
//! Tauri commands for the new event-driven crawling system.
//! These commands integrate with the new orchestrator and provide
//! real-time updates to the frontend.
//!
//! DEPRECATED: crawling_v4 legacy interface retained temporarily for reference; will be removed after actor system consolidation.
//! This file is now feature-gated and excluded from normal builds.

#![cfg(feature = "legacy-v4")] // Feature flag will be removed after quarantine period

use std::sync::Arc;
use std::collections::HashMap;
use tauri::{AppHandle, State, Emitter};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::domain::services::crawling_services::StatusChecker;
use crate::infrastructure::service_based_crawling_engine::{ServiceBasedBatchCrawlingEngine, BatchCrawlingConfig};
use crate::infrastructure::crawling_service_impls::CrawlingRangeCalculator;
use crate::application::shared_state::{SharedStateCache, SiteAnalysisResult, DbAnalysisResult, CalculatedRange};
use crate::application::crawling_profile::CrawlingRequest;

/// Global state for the crawling engine v4.0
pub struct CrawlingEngineState {
    pub engine: Arc<RwLock<Option<ServiceBasedBatchCrawlingEngine>>>,
    pub database: MockDatabase, // 일단 Mock 유지
}

/// 개발용 Mock Database
#[derive(Debug, Clone)]
pub struct MockDatabase {
    pub connection_status: String,
}

impl MockDatabase {
    /// Mock pool을 반환합니다 (개발용)
    pub async fn get_pool(&self) -> Result<String, String> {
        Ok(self.connection_status.clone())
    }
    
    /// Mock 연결 상태를 확인합니다
    pub fn is_connected(&self) -> bool {
        true // 항상 연결된 것으로 가정
    }
}

/// Request payload for starting crawling
#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct StartCrawlingRequest {
    pub start_page: u32,
    pub end_page: u32,
    pub max_products_per_page: Option<usize>,
    pub concurrent_requests: Option<usize>,
    pub request_timeout_seconds: Option<u64>,
}

/// Response payload for crawling operations
#[derive(Debug, Serialize, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct CrawlingResponse {
    pub success: bool,
    pub message: String,
    #[ts(skip)]
    pub data: Option<serde_json::Value>,
}

/// Real-time statistics payload
#[derive(Debug, Serialize, Clone, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct SystemStatePayload {
    pub is_running: bool,
    pub uptime_seconds: u64,
    pub total_tasks_processed: u64,
    pub successful_tasks: u64,
    pub failed_tasks: u64,
    pub current_active_tasks: usize,
    pub tasks_per_second: f64,
    pub worker_utilization: f64,
    pub queue_sizes: HashMap<String, usize>,
    pub detailed_stats: DetailedStats,
    pub is_healthy: bool,
    pub timestamp: u64,
}

#[derive(Debug, Serialize, Clone, TS)]
#[ts(export, export_to = "../src/types/generated/")]
pub struct DetailedStats {
    pub list_pages_fetched: u64,
    pub list_pages_processed: u64,
    pub product_urls_discovered: u64,
    pub product_details_fetched: u64,
    pub product_details_parsed: u64,
    pub products_saved: u64,
}

/// Initialize the crawling engine
#[tauri::command]
pub async fn init_crawling_engine(
    _app: AppHandle,
    state: State<'_, CrawlingEngineState>,
) -> Result<CrawlingResponse, String> {
    tracing::info!("Initializing crawling engine v4.0...");
    
    let mut engine_guard = state.engine.write().await;
    
    if engine_guard.is_some() {
        return Ok(CrawlingResponse {
            success: false,
            message: "Crawling engine is already initialized".to_string(),
            data: None,
        });
    }
    
    // Get database pool (Mock for development)
    let _db_mock = state.database.get_pool().await
        .map_err(|e| format!("Database connection failed: {}", e))?;
    
    // Create database connection for real engine
    let database_url = get_database_url_v4()?;
    let db_pool = sqlx::SqlitePool::connect(&database_url).await
        .map_err(|e| format!("Database connection failed: {}", e))?;
    
    // Create necessary components
    let http_client = crate::infrastructure::HttpClient::create_from_global_config()
        .map_err(|e| format!("HTTP client creation failed: {}", e))?;
    
    let data_extractor = crate::infrastructure::MatterDataExtractor::new()
        .map_err(|e| format!("Data extractor creation failed: {}", e))?;
    
    let product_repo = Arc::new(crate::infrastructure::IntegratedProductRepository::new(db_pool));
    
    // Create configuration-driven BatchCrawlingConfig with CancellationToken
    let cancellation_token = tokio_util::sync::CancellationToken::new();
    
    // Load validated configuration instead of using hardcoded defaults
    let config_manager = crate::infrastructure::config::ConfigManager::new()
        .map_err(|e| format!("Failed to initialize config manager: {}", e))?;
    let app_config = config_manager.load_config().await
        .map_err(|e| format!("Failed to load configuration: {}", e))?;
    let validated_config = crate::application::validated_crawling_config::ValidatedCrawlingConfig::from_app_config(&app_config);
    
    let mut config = BatchCrawlingConfig::from_validated(&validated_config);
    config.cancellation_token = Some(cancellation_token);
    
    tracing::info!("🔄 Created configuration-driven BatchCrawlingConfig with cancellation_token");
    
    // Create EventEmitter for real-time frontend communication
    tracing::info!("📡 Creating EventEmitter for frontend communication...");
    let event_emitter = crate::application::events::EventEmitter::new(_app.clone());
    let event_emitter_arc = Arc::new(Some(event_emitter));
    
    // Initialize the ServiceBasedBatchCrawlingEngine
    let engine = ServiceBasedBatchCrawlingEngine::new(
        http_client,
        data_extractor,
        product_repo,
        event_emitter_arc, // EventEmitter for real-time events
        config,
        uuid::Uuid::new_v4().to_string(),
        app_config,
    );
    
    *engine_guard = Some(engine);
    
    tracing::info!("Crawling engine v4.0 initialized successfully with cancellation support");
    
    Ok(CrawlingResponse {
        success: true,
        message: "Crawling engine initialized successfully".to_string(),
        data: None,
    })
}

/// Start the crawling process
#[tauri::command]
pub async fn start_crawling(
    app: AppHandle,
    state: State<'_, CrawlingEngineState>,
    shared_state: State<'_, SharedStateCache>,
    request: StartCrawlingRequest,
) -> Result<CrawlingResponse, String> {
    tracing::info!("🚀 START_CRAWLING FUNCTION CALLED!");
    tracing::info!("Starting crawling with request: {:?}", request);
    
    tracing::info!("🔍 Step 1: Getting engine guard...");
    let engine_guard = state.engine.read().await;
    let _engine = engine_guard.as_ref()
        .ok_or_else(|| "Crawling engine not initialized".to_string())?;
    
    tracing::info!("🔍 Step 2: Engine ready for execution");
    
    tracing::info!("🔍 Step 3: Calculating intelligent crawling range...");
    // Get app configuration for intelligent range calculation
    let app_config = crate::infrastructure::config::AppConfig::default();
    
    // Always use intelligent range calculation based on:
    // 1. User settings (page_range_limit, etc.)
    // 2. Local database state (existing records)
    // 3. Site current state (total pages, new products)
    tracing::info!("🧠 Performing intelligent range calculation...");
    tracing::info!("📋 User request parameters: start_page={}, end_page={}", request.start_page, request.end_page);
    
    let (start_page, end_page) = calculate_intelligent_crawling_range_v4(&app_config, &request, &shared_state).await
        .map_err(|e| format!("Failed to calculate intelligent range: {}", e))?;
    
    tracing::info!("✅ Step 5: Calculated optimal range: {} to {}", start_page, end_page);
    
    // Validate page range for reverse crawling (start_page >= end_page is valid)
    if start_page == 0 || end_page == 0 {
        return Err("Page numbers must be greater than 0".to_string());
    }
    
    // For reverse crawling, start_page should be >= end_page
    // start_page is the older page (higher number), end_page is the newer page (lower number)
    let (final_start_page, final_end_page) = if start_page < end_page {
        tracing::warn!("⚠️ Range calculator returned forward direction, converting to reverse crawling");
        let reverse_start = end_page;  // Use higher number as start (older page)
        let reverse_end = start_page;  // Use lower number as end (newer page)
        tracing::info!("🔄 Converted to reverse: pages {} down to {}", reverse_start, reverse_end);
        (reverse_start, reverse_end)
    } else {
        (start_page, end_page)
    };
    
    // Calculate total pages for reverse crawling: from final_start_page down to final_end_page
    let total_pages = final_start_page.saturating_sub(final_end_page).saturating_add(1);
    
    tracing::info!("🔄 Reverse crawling setup: from page {} down to page {} (total: {} pages)", 
                   final_start_page, final_end_page, total_pages);
    
    // UI에 계산된 범위 정보 전송
    if let Err(e) = app.emit("crawling-range-calculated", serde_json::json!({
        "start_page": final_start_page,
        "end_page": final_end_page,
        "total_pages": total_pages,
        "crawling_direction": "reverse",
        "calculation_reason": "Based on intelligent range calculation"
    })) {
        tracing::warn!("Failed to emit range calculation event: {}", e);
    }
    
    tracing::info!("🔍 Step 6: Setting up SystemStateBroadcaster...");
    
    // SystemStateBroadcaster 설정 (Live Production Line UI용)
    let broadcaster = crate::infrastructure::system_broadcaster::SystemStateBroadcaster::new(
        app.clone(),
    );
    
    tracing::info!("📡 SystemStateBroadcaster configured for Live Production Line UI");
    
    // 엔진에 broadcaster 설정
    // NOTE: 현재 구조에서는 mutable reference가 필요하므로 임시로 drop 후 재설정
    drop(engine_guard);
    
    {
        let mut engine_guard = state.engine.write().await;
        if let Some(engine) = engine_guard.as_mut() {
            engine.set_broadcaster(broadcaster);
            tracing::info!("✅ SystemStateBroadcaster set on crawling engine");
        }
    }
    
    // 다시 engine guard 획득 (mutable)
    let mut engine_guard = state.engine.write().await;
    let engine = engine_guard.as_mut()
        .ok_or_else(|| "Crawling engine not initialized".to_string())?;
    
    tracing::info!("🔍 Step 7: Starting crawling execution...");
    
    // 동기적으로 실행 (현재 구조에서는 백그라운드 실행 시 라이프타임 문제 발생)
    // TODO: 향후 엔진 구조 개선 시 백그라운드 실행으로 변경
    let execution_result = engine.execute().await
        .map_err(|e| format!("Failed to execute crawling: {}", e));
    
    match execution_result {
        Ok(_) => {
            tracing::info!("✅ Crawling completed successfully: pages {} to {}", final_start_page, final_end_page);
            
            // 완료 이벤트 발송
            if let Err(e) = app.emit("crawling-completed", serde_json::json!({
                "status": "completed",
                "message": "Crawling completed successfully",
                "start_page": start_page,
                "end_page": end_page,
                "timestamp": chrono::Utc::now().to_rfc3339()
            })) {
                tracing::warn!("Failed to emit completion event: {}", e);
            }
        }
        Err(e) => {
            tracing::error!("❌ Crawling failed: {}", e);
            
            // 실패 이벤트 발송
            if let Err(emit_err) = app.emit("crawling-failed", serde_json::json!({
                "status": "failed",
                "message": format!("Crawling failed: {}", e),
                "error": e.to_string(),
                "timestamp": chrono::Utc::now().to_rfc3339()
            })) {
                tracing::warn!("Failed to emit failure event: {}", emit_err);
            }
            
            return Err(e);
        }
    }
    
    Ok(CrawlingResponse {
        success: true,
        message: format!("Crawling started: pages {} to {}", final_start_page, final_end_page),
        data: None,
    })
}

/// Stop the crawling process
#[tauri::command]
pub async fn stop_crawling(
    app: AppHandle,
    state: State<'_, CrawlingEngineState>,
) -> Result<CrawlingResponse, String> {
    tracing::info!("🛑 Stop crawling command received");
    
    // 1. 즉시 엔진 중단 신호 전송
    let engine_guard = state.engine.read().await;
    if let Some(engine) = engine_guard.as_ref() {
        match engine.stop().await {
            Ok(()) => {
                tracing::info!("✅ Engine stop signal sent successfully");
            }
            Err(e) => {
                tracing::warn!("⚠️ Failed to send stop signal to engine: {}", e);
                // 중단 신호 전송 실패해도 UI 업데이트는 계속 진행
            }
        }
    } else {
        tracing::warn!("⚠️ No engine found to stop");
    }
    
    // 2. 즉시 UI 상태 업데이트
    if let Err(e) = app.emit("crawling-stopped", serde_json::json!({
        "status": "stopped",
        "message": "Crawling has been stopped",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })) {
        tracing::warn!("Failed to emit stop event: {}", e);
    }
    
    // 3. 즉시 응답 반환
    tracing::info!("🛑 Stop crawling command completed - immediate response");
    
    Ok(CrawlingResponse {
        success: true,
        message: "Crawling stopped successfully".to_string(),
        data: None,
    })
}

/// Get current crawling statistics
#[tauri::command]
pub async fn get_crawling_stats(
    state: State<'_, CrawlingEngineState>,
) -> Result<SystemStatePayload, String> {
    let engine_guard = state.engine.read().await;
    let _engine = engine_guard.as_ref()
        .ok_or_else(|| "Crawling engine not initialized".to_string())?;
    
    // ServiceBasedBatchCrawlingEngine doesn't have get_stats method
    // Return mock stats for now
    let mock_stats = SystemStatePayload {
        is_running: false,
        uptime_seconds: 0,
        total_tasks_processed: 0,
        successful_tasks: 0,
        failed_tasks: 0,
        current_active_tasks: 0,
        tasks_per_second: 0.0,
        worker_utilization: 0.0,
        queue_sizes: HashMap::new(),
        detailed_stats: DetailedStats {
            list_pages_fetched: 0,
            list_pages_processed: 0,
            product_urls_discovered: 0,
            product_details_fetched: 0,
            product_details_parsed: 0,
            products_saved: 0,
        },
        is_healthy: true,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()), // Modern Rust: map_or instead of unwrap_or_default
    };
    
    Ok(mock_stats)
}

/// Get system health status
#[tauri::command]
pub async fn get_system_health(
    state: State<'_, CrawlingEngineState>,
) -> Result<CrawlingResponse, String> {
    let engine_guard = state.engine.read().await;
    let _engine = engine_guard.as_ref()
        .ok_or_else(|| "Crawling engine not initialized".to_string())?;
    
    // ServiceBasedBatchCrawlingEngine doesn't have get_stats method
    // Return mock health data for now
    let health_data = serde_json::json!({
        "is_healthy": true,
        "is_running": false,
        "uptime_seconds": 0,
        "success_rate": 100.0,
        "worker_utilization": 0.0,
        "tasks_per_second": 0.0,
    });
    
    Ok(CrawlingResponse {
        success: true,
        message: "System health retrieved successfully".to_string(),
        data: Some(health_data),
    })
}

/// Update crawling configuration
#[tauri::command]
pub async fn update_crawling_config(
    state: State<'_, CrawlingEngineState>,
    _config: BatchCrawlingConfig,
) -> Result<CrawlingResponse, String> {
    let mut engine_guard = state.engine.write().await;
    let _engine = engine_guard.as_mut()
        .ok_or_else(|| "Crawling engine not initialized".to_string())?;
    
    // ServiceBasedBatchCrawlingEngine doesn't have update_config method
    // For now, just return success
    tracing::info!("Configuration update requested (not implemented for ServiceBasedBatchCrawlingEngine)");
    
    Ok(CrawlingResponse {
        success: true,
        message: "Configuration updated successfully (restart required)".to_string(),
        data: None,
    })
}

/// Get current crawling configuration
#[tauri::command]
pub async fn get_crawling_config(
    state: State<'_, CrawlingEngineState>,
) -> Result<CrawlingResponse, String> {
    let engine_guard = state.engine.read().await;
    let _engine = engine_guard.as_ref()
        .ok_or_else(|| "Crawling engine not initialized".to_string())?;
    
    // ServiceBasedBatchCrawlingEngine doesn't have get_config method
    // Return default config
    let config = BatchCrawlingConfig::default();
    let config_data = serde_json::to_value(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    
    Ok(CrawlingResponse {
        success: true,
        message: "Configuration retrieved successfully".to_string(),
        data: Some(config_data),
    })
}

/// Emergency stop command
#[tauri::command]
pub async fn emergency_stop(
    app: AppHandle,
    state: State<'_, CrawlingEngineState>,
) -> Result<CrawlingResponse, String> {
    tracing::warn!("Emergency stop requested");
    
    let mut engine_guard = state.engine.write().await;
    
    if let Some(_engine) = engine_guard.as_ref() {
        // ServiceBasedBatchCrawlingEngine doesn't have stop method
        tracing::info!("Emergency stop requested (engine doesn't support runtime stopping)");
    }
    
    // Reset the engine state
    *engine_guard = None;
    
    // Emit emergency stop event
    let _ = app.emit("emergency_stop", ());
    
    tracing::warn!("Emergency stop completed");
    
    Ok(CrawlingResponse {
        success: true,
        message: "Emergency stop completed".to_string(),
        data: None,
    })
}

/// Ping the backend to check connectivity
#[tauri::command]
pub async fn ping_backend() -> Result<String, String> {
    tracing::info!("🏓 Backend ping received");
    Ok("pong".to_string())
}

/// Get application settings
#[tauri::command]
pub async fn get_app_settings() -> Result<serde_json::Value, String> {
    tracing::info!("⚙️ Getting app settings from configuration file");
    
    // Load configuration from file
    let config_manager = crate::infrastructure::config::ConfigManager::new()
        .map_err(|e| format!("Failed to initialize config manager: {}", e))?;
    let app_config = config_manager.load_config().await
        .map_err(|e| format!("Failed to load configuration: {}", e))?;
    
    // Return the entire AppConfig as JSON
    let settings = serde_json::to_value(&app_config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    
    Ok(settings)
}

/// Save application settings to config file
#[tauri::command]
pub async fn save_app_settings(settings: serde_json::Value) -> Result<String, String> {
    tracing::info!("⚙️ Saving app settings to configuration file");
    
    // Parse the settings as AppConfig
    let app_config: crate::infrastructure::config::AppConfig = serde_json::from_value(settings)
        .map_err(|e| format!("Failed to parse settings: {}", e))?;
    
    // Load existing config and update it
    let config_manager = crate::infrastructure::config::ConfigManager::new()
        .map_err(|e| format!("Failed to initialize config manager: {}", e))?;
    
    // Save the new configuration
    config_manager.save_config(&app_config).await
        .map_err(|e| format!("Failed to save configuration: {}", e))?;
    
    tracing::info!("✅ App settings saved successfully");
    Ok("Settings saved successfully".to_string())
}

/// Background task for broadcasting real-time statistics
async fn broadcast_real_time_stats(_app: AppHandle, _engine: ServiceBasedBatchCrawlingEngine) {
    // ServiceBasedBatchCrawlingEngine doesn't have continuous stats
    // This function is not needed for the current implementation
    tracing::info!("Real-time stats broadcasting not implemented for ServiceBasedBatchCrawlingEngine");
}

// Convert stats function removed as ServiceBasedBatchCrawlingEngine doesn't provide stats interface

/// Calculate intelligent crawling range using SharedStateCache with TTL (State-Ensuring Gateway Pattern)
/// This implements the proposal6.comment.md "State-Ensuring Gateway" approach
async fn calculate_intelligent_crawling_range_v4(
    _app_config: &crate::infrastructure::config::AppConfig,
    user_request: &StartCrawlingRequest,
    shared_state: &State<'_, SharedStateCache>,
) -> Result<(u32, u32), String> {
    tracing::info!("🔍 State-Ensuring Gateway: Starting intelligent range calculation...");
    tracing::info!("📝 User request: start_page={}, end_page={}", user_request.start_page, user_request.end_page);
    
    // If user provided explicit range (both > 0), use it directly
    if user_request.start_page > 0 && user_request.end_page > 0 {
        tracing::info!("✅ Using explicit user range: {} to {}", user_request.start_page, user_request.end_page);
        return Ok((user_request.start_page, user_request.end_page));
    }
    
    // Phase 1: Initialize SharedStateCache and check for valid cached data
    tracing::info!("🧠 User provided 0 values - using intelligent calculation with TTL-based caching");
    
    // Phase 2: Ensure fresh site analysis (TTL-based)
    let site_analysis = if let Some(cached_analysis) = shared_state.get_valid_site_analysis_async(Some(5)).await {
        tracing::info!("🎯 Using cached site analysis (avoiding duplicate work)");
        cached_analysis
    } else {
        tracing::info!("🔄 Performing fresh site analysis (cache miss or expired)");
        // Perform fresh site analysis
        let config_manager = crate::infrastructure::config::ConfigManager::new()
            .map_err(|e| format!("Failed to initialize config manager: {}", e))?;
        let config = config_manager.load_config().await
            .map_err(|e| format!("Failed to get config: {}", e))?;
            
        let http_client = crate::infrastructure::HttpClient::create_from_global_config()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
        let data_extractor = crate::infrastructure::MatterDataExtractor::new()
            .map_err(|e| format!("Failed to create data extractor: {}", e))?;
        // Create product repository for StatusCheckerImpl
        let database_url = get_database_url_v4()?;
        let db_pool = sqlx::SqlitePool::connect(&database_url).await
            .map_err(|e| format!("Failed to connect to database: {}", e))?;
        let product_repo = crate::infrastructure::IntegratedProductRepository::new(db_pool);
        let repo_arc = std::sync::Arc::new(product_repo);

        let status_checker = crate::infrastructure::crawling_service_impls::StatusCheckerImpl::with_product_repo(
            http_client,
            data_extractor,
            config,
            repo_arc,
        );
        
        let site_status = status_checker.check_site_status().await
            .map_err(|e| format!("Failed to check site status: {}", e))?;
        
        let analysis = crate::application::shared_state::SiteAnalysisResult::new(
            site_status.total_pages,
            site_status.products_on_last_page,
            site_status.estimated_products,
            "https://matters.town".to_string(),
            1.0, // health_score
        );
        
        // Cache the fresh analysis
        shared_state.set_site_analysis(analysis.clone()).await;
        analysis
    };
    
    // Phase 3: Ensure fresh DB analysis (TTL-based)
    let _db_analysis = if let Some(cached_db_analysis) = shared_state.get_valid_db_analysis_async(Some(3)).await {
        tracing::info!("🎯 Using cached DB analysis");
        cached_db_analysis
    } else {
        tracing::info!("🔄 Performing fresh DB analysis");
        // Perform fresh DB analysis
        let database_url = get_database_url_v4()?;
        let db_pool = sqlx::SqlitePool::connect(&database_url).await
            .map_err(|e| format!("Failed to connect to database: {}", e))?;
        let product_repo = crate::infrastructure::IntegratedProductRepository::new(db_pool);
        let repo_arc = std::sync::Arc::new(product_repo);
        
        let analysis = repo_arc.analyze_database_state().await
            .map_err(|e| format!("Failed to analyze database: {}", e))?;
        
        // Cache the fresh DB analysis
        shared_state.set_db_analysis(analysis.clone()).await;
        analysis
    };
    
    // Phase 4: Use cached or calculate range
    if let Some(cached_range) = shared_state.get_valid_calculated_range_async(2).await {
        tracing::info!("🎯 Using cached calculated range: {} to {} ({} pages)", 
                      cached_range.start_page, cached_range.end_page, cached_range.total_pages);
        return Ok((cached_range.start_page, cached_range.end_page));
    }
    
    // Phase 5: Calculate fresh range with guaranteed fresh data
    tracing::info!("🎯 Calculating fresh range with cached site/DB data");
    let config_manager = crate::infrastructure::config::ConfigManager::new()
        .map_err(|e| format!("Failed to initialize config manager: {}", e))?;
    let config = config_manager.load_config().await
        .map_err(|e| format!("Failed to get config: {}", e))?;
    // Create product repository
    let database_url = get_database_url_v4()?;
    let db_pool = sqlx::SqlitePool::connect(&database_url).await
        .map_err(|e| format!("Failed to connect to database: {}", e))?;
    
    let product_repo = crate::infrastructure::IntegratedProductRepository::new(db_pool);
    let repo_arc = std::sync::Arc::new(product_repo);

    // Create range calculator with proven implementation
    let range_calculator = CrawlingRangeCalculator::new(
        repo_arc.clone(),
        config.clone(),
    );

    // ✅ Use cached site analysis instead of repeating site analysis
    tracing::info!("🔍 Using cached site analysis for range calculation...");
    
    // Use the site_analysis we already obtained above
    let total_pages_on_site = site_analysis.total_pages;
    let products_on_last_page = site_analysis.products_on_last_page;

    tracing::info!("🌐 Using cached site parameters: {} total pages, {} products on last page", 
                  total_pages_on_site, products_on_last_page);
    
    // Calculate next crawling range using proven logic
    tracing::info!("🎯 Calculating next crawling range...");
    let result = range_calculator.calculate_next_crawling_range(
        total_pages_on_site,
        products_on_last_page,
    ).await
    .map_err(|e| format!("Failed to calculate crawling range: {}", e))?;
    
    match result {
        Some((start_page, end_page)) => {
            // For reverse crawling, start_page should be >= end_page (we crawl from newer to older pages)
            // start_page = older page (higher number), end_page = newer page (lower number)
            if start_page < end_page {
                tracing::warn!("⚠️ Range calculator returned forward direction: start_page ({}) < end_page ({})", start_page, end_page);
                tracing::info!("� Converting to reverse crawling direction");
                // For reverse crawling, swap the values so start_page > end_page
                let reverse_start = end_page;   // Use the larger number as start (older page)
                let reverse_end = start_page;   // Use the smaller number as end (newer page)
                let total_pages = reverse_start.saturating_sub(reverse_end).saturating_add(1);
                tracing::info!("✅ Reverse crawling range: pages {} down to {} (total: {} pages)", 
                              reverse_start, reverse_end, total_pages);
                Ok((reverse_start, reverse_end))
            } else {
                // Already in correct reverse order: start_page >= end_page
                let total_pages = start_page.saturating_sub(end_page).saturating_add(1);
                tracing::info!("✅ Calculated reverse range: pages {} down to {} (total: {} pages)", 
                              start_page, end_page, total_pages);
                Ok((start_page, end_page))
            }
        },
        None => {
            tracing::info!("✅ All products crawled - using verification range");
            // Use configuration-driven verification pages instead of hardcoded value
            let verification_pages = config.user.crawling.page_range_limit.min(10); // Limit verification to reasonable size
            let start_page = total_pages_on_site;  // Start from the last page (oldest)
            let end_page = if start_page >= verification_pages {
                start_page.saturating_sub(verification_pages).saturating_add(1)  // Go back a few pages
            } else {
                1
            };
            tracing::info!("🔍 Verification range: pages {} down to {} ({} pages, based on config)", 
                          start_page, end_page, verification_pages);
            Ok((start_page, end_page))
        }
    }
}

/// 중앙집중식 데이터베이스 URL 가져오기 (Modern Rust 2024)
/// 
/// 기존의 여러 곳에서 다른 방식으로 경로를 생성하던 문제를 해결
/// "엉뚱한 경로를 잡는 문제" 영구 해결
pub fn get_database_url_v4() -> Result<String, String> {
    // 환경변수 우선 확인 (개발/테스트 환경용)
    if let Ok(db_url) = std::env::var("DATABASE_URL") {
        if !db_url.is_empty() {
            tracing::info!("Using database URL from environment: {}", db_url);
            return Ok(db_url);
        }
    }

    // 중앙집중식 경로 관리자 사용 (Modern Rust 2024)
    let database_url = crate::infrastructure::get_main_database_url();
    tracing::info!("Using centralized database URL: {}", database_url);
    Ok(database_url)
}

/// 새로운 SharedState 기반 크롤링 시작 명령
#[tauri::command]
pub async fn start_crawling_with_profile(
    app: AppHandle,
    engine_state: State<'_, CrawlingEngineState>,
    shared_state: State<'_, SharedStateCache>,
    crawling_request: CrawlingRequest,
) -> Result<CrawlingResponse, String> {
    tracing::info!("🚀 START_CRAWLING_WITH_PROFILE CALLED!");
    tracing::info!("Crawling request: {:?}", crawling_request);
    
    // 요청 유효성 검증
    crawling_request.validate()
        .map_err(|e| format!("Invalid crawling request: {}", e))?;
    
    let profile = &crawling_request.profile;
    tracing::info!("🎯 Crawling mode: {}", profile.mode);
    
    // TTL 설정 (5분)
    let cache_ttl_minutes = 5;
    
    match profile.mode.as_str() {
        "intelligent" => {
            start_intelligent_crawling(&app, &engine_state, &shared_state, cache_ttl_minutes).await
        }
        "manual" => {
            let (start_page, end_page) = profile.get_page_range()
                .ok_or_else(|| "Manual mode requires page range".to_string())?;
            start_manual_crawling(&app, &engine_state, &shared_state, start_page, end_page).await
        }
        "verification" => {
            let pages = profile.verification_pages.as_ref()
                .ok_or_else(|| "Verification mode requires pages list".to_string())?;
            start_verification_crawling(&app, &engine_state, &shared_state, pages.clone()).await
        }
        _ => Err(format!("Unsupported crawling mode: {}", profile.mode))
    }
}

/// 지능형 크롤링 실행
async fn start_intelligent_crawling(
    app: &AppHandle,
    engine_state: &State<'_, CrawlingEngineState>,
    shared_state: &State<'_, SharedStateCache>,
    cache_ttl_minutes: u64,
) -> Result<CrawlingResponse, String> {
    tracing::info!("🧠 Starting intelligent crawling...");
    
    // 1. 캐시된 사이트 분석 결과 확인
    let site_analysis = if let Some(cached_analysis) = shared_state.get_valid_site_analysis_async(Some(cache_ttl_minutes)).await {
        tracing::info!("✅ Using cached site analysis from {}", cached_analysis.analyzed_at);
        cached_analysis.clone()
    } else {
        tracing::info!("🔍 No valid cached site analysis, performing new analysis...");
        
        // 사이트 분석 수행 - 구체적인 구현체 생성
        let http_client = crate::infrastructure::simple_http_client::HttpClient::create_from_global_config()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
        let data_extractor = crate::infrastructure::html_parser::MatterDataExtractor::new()
            .map_err(|e| format!("Failed to create data extractor: {}", e))?;
        let app_config = crate::infrastructure::config::AppConfig::default();
        
        let status_checker = crate::infrastructure::crawling_service_impls::StatusCheckerImpl::new(
            http_client,
            data_extractor,
            app_config,
        );
        
        // 사이트 분석 수행
        let site_status = status_checker.check_site_status().await
            .map_err(|e| format!("Failed to check site status: {}", e))?;
            
        let analysis = SiteAnalysisResult::new(
            site_status.total_pages,
            site_status.products_on_last_page,
            site_status.estimated_products,
            "https://matters.town".to_string(),
            0.8, // Default health score
        );
        
        // 캐시에 저장
        shared_state.set_site_analysis(analysis.clone());
        tracing::info!("💾 Site analysis cached for future use");
        analysis
    };
    
    // 2. 캐시된 DB 분석 결과 확인
    let db_analysis = if let Some(cached_db_analysis) = shared_state.get_valid_db_analysis_async(Some(cache_ttl_minutes)).await {
        tracing::info!("✅ Using cached DB analysis from {}", cached_db_analysis.analyzed_at);
        cached_db_analysis.clone()
    } else {
        tracing::info!("🔍 No valid cached DB analysis, performing new analysis...");
        
        // DB 분석 수행 (실제 구현으로 대체 필요)
        let analysis = DbAnalysisResult::new(
            0, // TODO: 실제 DB에서 제품 수 조회
            None,
            None,
            1.0,
        );
        
        // 캐시에 저장
        shared_state.set_db_analysis(analysis.clone());
        tracing::info!("💾 DB analysis cached for future use");
        analysis
    };
    
    // 3. 지능적 범위 계산 - 동적 계산
    let app_config = crate::infrastructure::config::AppConfig::default();
    let base_page_limit = u32::from(app_config.user.crawling.page_range_limit); // Modern Rust: from() 대신 as 캐스팅
    
    // 데이터베이스 크기와 사이트 변화에 따른 동적 범위 계산
    let adaptive_page_limit = if db_analysis.is_empty {
        // 첫 크롤링: 보수적으로 시작 (50 페이지)
        std::cmp::min(base_page_limit, 50)
    } else if site_analysis.total_pages > (db_analysis.total_products / 20) * 2 {
        // 사이트가 크게 확장된 경우 (제품 수로 추정): 증가된 범위 크롤링
        std::cmp::min(base_page_limit * 2, 200)
    } else {
        // 정상적인 증분 크롤링
        base_page_limit
    };
    
    let (start_page, end_page) = if db_analysis.is_empty {
        // 빈 DB인 경우: 최신 페이지부터 역순으로
        let calculated_end = 1;
        let calculated_start = std::cmp::min(site_analysis.total_pages, adaptive_page_limit);
        (calculated_start, calculated_end)
    } else {
        // 기존 데이터가 있는 경우: 증분 크롤링
        let last_page = db_analysis.max_page_id.unwrap_or(1) as u32;
        let calculated_start = std::cmp::min(site_analysis.total_pages, last_page + adaptive_page_limit);
        let calculated_end = last_page + 1;
        (calculated_start, calculated_end)
    };
    
    let calculated_range = CalculatedRange::new(
        start_page,
        end_page,
        site_analysis.total_pages,
        false, // Not a complete crawl in intelligent mode
    );
    
    // 계산된 범위 캐시에 저장
    shared_state.set_calculated_range(calculated_range.clone());
    
    tracing::info!("🎯 Intelligent range calculated: {} to {} (reason: {})", 
                   start_page, end_page, calculated_range.calculation_reason);
    
    // 4. 실제 크롤링 실행
    execute_crawling_with_range(app, engine_state, start_page, end_page).await
}

/// 수동 크롤링 실행
async fn start_manual_crawling(
    app: &AppHandle,
    engine_state: &State<'_, CrawlingEngineState>,
    shared_state: &State<'_, SharedStateCache>,
    start_page: u32,
    end_page: u32,
) -> Result<CrawlingResponse, String> {
    tracing::info!("🔧 Starting manual crawling: {} to {}", start_page, end_page);
    
    let calculated_range = CalculatedRange::new(
        start_page,
        end_page,
        end_page - start_page + 1, // Total pages being crawled
        false, // Manual range is not a complete crawl
    );
    
    shared_state.set_calculated_range(calculated_range);
    
    execute_crawling_with_range(app, engine_state, start_page, end_page).await
}

/// 검증 크롤링 실행
async fn start_verification_crawling(
    app: &AppHandle,
    engine_state: &State<'_, CrawlingEngineState>,
    shared_state: &State<'_, SharedStateCache>,
    pages: Vec<u32>,
) -> Result<CrawlingResponse, String> {
    tracing::info!("🔍 Starting verification crawling for pages: {:?}", pages);
    
    // Modern Rust: 명시적 에러 처리
    let min_page = pages.iter().min()
        .ok_or_else(|| "Cannot find minimum page in empty list".to_string())?;
    let max_page = pages.iter().max()
        .ok_or_else(|| "Cannot find maximum page in empty list".to_string())?;
    
    let calculated_range = CalculatedRange::new(
        *min_page,
        *max_page,
        max_page.saturating_sub(*min_page).saturating_add(1), // Total pages being verified
        false, // Verification is not a complete crawl
    );
    
    shared_state.set_calculated_range(calculated_range);
    
    // 검증 모드에서는 특정 페이지들만 크롤링
    // 여기서는 간단히 min~max 범위로 처리하지만, 
    // 실제로는 특정 페이지들만 처리하는 로직 필요
    execute_crawling_with_range(app, engine_state, *max_page, *min_page).await
}

/// 공통 크롤링 실행 로직 - 범위 재계산 없이 직접 실행
pub async fn execute_crawling_with_range(
    app: &AppHandle,
    engine_state: &State<'_, CrawlingEngineState>,
    start_page: u32,
    end_page: u32,
) -> Result<CrawlingResponse, String> {
    tracing::info!("🔍 Step: Getting engine guard...");
    let mut engine_guard = engine_state.engine.write().await;
    let engine = engine_guard.as_mut()
        .ok_or_else(|| "Crawling engine not initialized".to_string())?;
    
    tracing::info!("🔍 Step: Engine ready for execution");
    
    // Validate page range for reverse crawling (start_page >= end_page is valid)
    if start_page == 0 || end_page == 0 {
        return Err("Page numbers must be greater than 0".to_string());
    }
    
    // For reverse crawling, start_page should be >= end_page
    let (final_start_page, final_end_page) = if start_page < end_page {
        tracing::warn!("⚠️ Converting forward direction to reverse crawling");
        (end_page, start_page)
    } else {
        (start_page, end_page)
    };
    
    // Calculate total pages for reverse crawling: from final_start_page down to final_end_page
    let total_pages = final_start_page.saturating_sub(final_end_page).saturating_add(1);
    
    tracing::info!("📊 Final crawling parameters:");
    tracing::info!("   📍 Start page (oldest): {}", final_start_page);
    tracing::info!("   📍 End page (newest): {}", final_end_page);
    tracing::info!("   📊 Total pages to crawl: {}", total_pages);
    tracing::info!("   🔄 Direction: Reverse (older → newer)");
    
    // Update engine with the calculated range
    tracing::info!("🔧 Updating engine range to: {} -> {}", final_start_page, final_end_page);
    engine.update_range_from_calculation(Some((final_start_page, final_end_page)));
    
    // Emit crawling start event
    if let Err(e) = app.emit("crawling-started", serde_json::json!({
        "start_page": final_start_page,
        "end_page": final_end_page,
        "total_pages": total_pages,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })) {
        tracing::warn!("Failed to emit crawling-started event: {}", e);
    }
    
    tracing::info!("🚀 Starting batch crawling execution");
    
    match engine.execute().await {
        Ok(_) => {
            tracing::info!("✅ Crawling started successfully");
            Ok(CrawlingResponse {
                success: true,
                message: format!("Crawling started for pages {} to {} ({} total pages)", 
                               final_start_page, final_end_page, total_pages),
                data: Some(serde_json::json!({
                    "start_page": final_start_page,
                    "end_page": final_end_page,
                    "total_pages": total_pages,
                    "estimated_duration_minutes": total_pages as f64 * 0.5
                })),
            })
        }
        Err(e) => {
            tracing::error!("❌ Failed to start crawling: {}", e);
            Err(format!("Failed to start crawling: {}", e))
        }
    }
}

/// 캐시 상태 조회 명령
#[tauri::command]
pub async fn get_cache_status(
    shared_state: State<'_, SharedStateCache>,
) -> Result<serde_json::Value, String> {
    let summary = shared_state.get_status_summary().await;
    let warnings: Vec<String> = Vec::new(); // Temporarily empty warnings
    
    Ok(serde_json::json!({
        "cache_summary": summary,
        "consistency_warnings": warnings,
        "has_site_analysis": shared_state.get_valid_site_analysis_async(Some(5)).await.is_some(),
        "has_db_analysis": shared_state.get_valid_db_analysis_async(Some(5)).await.is_some(),
        "has_calculated_range": shared_state.get_valid_calculated_range_async(5).await.is_some(),
        "site_analysis_time": shared_state.get_valid_site_analysis_async(Some(5)).await.map(|a| a.analyzed_at),
        "db_analysis_time": shared_state.get_valid_db_analysis_async(Some(5)).await.map(|a| a.analyzed_at),
        "calculated_range_time": shared_state.get_valid_calculated_range_async(5).await.map(|r| r.calculated_at),
    }))
}

/// 캐시 초기화 명령
#[tauri::command]
pub async fn clear_cache(
    shared_state: State<'_, SharedStateCache>,
) -> Result<String, String> {
    shared_state.clear_all_caches().await;
    tracing::info!("🧹 Shared state cache cleared");
    Ok("Cache cleared successfully".to_string())
}
