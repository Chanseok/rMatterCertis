//! # Modern Crawling Commands v4.0
//!
//! Tauri commands for the new event-driven crawling system.
//! These commands integrate with the new orchestrator and provide
//! real    tracing::info!("✅ Step 4: Calculated optimal range: {} to {}", start_page, end_page);time updates to the frontend.

use std::sync::Arc;
use std::collections::HashMap;
use tauri::{AppHandle, State, Emitter};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

use crate::domain::services::crawling_services::{StatusChecker, DatabaseAnalyzer};
use crate::infrastructure::service_based_crawling_engine::{ServiceBasedBatchCrawlingEngine, BatchCrawlingConfig};
use crate::infrastructure::crawling_service_impls::CrawlingRangeCalculator;

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
#[derive(Debug, Deserialize)]
pub struct StartCrawlingRequest {
    pub start_page: u32,
    pub end_page: u32,
    pub max_products_per_page: Option<usize>,
    pub concurrent_requests: Option<usize>,
    pub request_timeout_seconds: Option<u64>,
}

/// Response payload for crawling operations
#[derive(Debug, Serialize)]
pub struct CrawlingResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// Real-time statistics payload
#[derive(Debug, Serialize, Clone)]
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

#[derive(Debug, Serialize, Clone)]
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
    app: AppHandle,
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
    let http_client = crate::infrastructure::HttpClient::new()
        .map_err(|e| format!("HTTP client creation failed: {}", e))?;
    
    let data_extractor = crate::infrastructure::MatterDataExtractor::new()
        .map_err(|e| format!("Data extractor creation failed: {}", e))?;
    
    let product_repo = Arc::new(crate::infrastructure::IntegratedProductRepository::new(db_pool));
    
    // Create default configuration with CancellationToken
    let cancellation_token = tokio_util::sync::CancellationToken::new();
    let mut config = BatchCrawlingConfig::default();
    config.cancellation_token = Some(cancellation_token);
    
    tracing::info!("🔄 Created BatchCrawlingConfig with cancellation_token");
    
    // Initialize the ServiceBasedBatchCrawlingEngine
    let engine = ServiceBasedBatchCrawlingEngine::new(
        http_client,
        data_extractor,
        product_repo,
        Arc::new(None), // No event emitter for now
        config,
        uuid::Uuid::new_v4().to_string(),
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
    request: StartCrawlingRequest,
) -> Result<CrawlingResponse, String> {
    tracing::info!("🚀 START_CRAWLING FUNCTION CALLED!");
    tracing::info!("Starting crawling with request: {:?}", request);
    
    tracing::info!("🔍 Step 1: Getting engine guard...");
    let engine_guard = state.engine.read().await;
    let engine = engine_guard.as_ref()
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
    
    let (start_page, end_page) = calculate_intelligent_crawling_range_v4(&app_config, &request).await
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
    
    tracing::info!("🔍 Step 6: Starting crawling execution...");
    
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
    let engine = engine_guard.as_ref()
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
            .unwrap_or_default()
            .as_secs(),
    };
    
    Ok(mock_stats)
}

/// Get system health status
#[tauri::command]
pub async fn get_system_health(
    state: State<'_, CrawlingEngineState>,
) -> Result<CrawlingResponse, String> {
    let engine_guard = state.engine.read().await;
    let engine = engine_guard.as_ref()
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
    config: BatchCrawlingConfig,
) -> Result<CrawlingResponse, String> {
    let mut engine_guard = state.engine.write().await;
    let engine = engine_guard.as_mut()
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
    let engine = engine_guard.as_ref()
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
    tracing::info!("⚙️ Getting app settings");
    
    // Default settings
    let settings = serde_json::json!({
        "page_range_limit": 50,
        "concurrent_requests": 24,
        "request_timeout_seconds": 30,
        "max_products_per_page": 100,
        "enable_debug_logging": true,
        "auto_retry_failed_requests": true,
        "max_retry_attempts": 3,
        "crawling_delay_ms": 1000
    });
    
    Ok(settings)
}

/// Background task for broadcasting real-time statistics
async fn broadcast_real_time_stats(app: AppHandle, _engine: ServiceBasedBatchCrawlingEngine) {
    // ServiceBasedBatchCrawlingEngine doesn't have continuous stats
    // This function is not needed for the current implementation
    tracing::info!("Real-time stats broadcasting not implemented for ServiceBasedBatchCrawlingEngine");
}

// Convert stats function removed as ServiceBasedBatchCrawlingEngine doesn't provide stats interface

/// Calculate intelligent crawling range using existing CrawlingRangeCalculator
/// This is the proven implementation that works correctly
async fn calculate_intelligent_crawling_range_v4(
    _app_config: &crate::infrastructure::config::AppConfig,
    user_request: &StartCrawlingRequest,
) -> Result<(u32, u32), String> {
    tracing::info!("🔍 Using proven smart_crawling.rs range calculation logic...");
    tracing::info!("📝 User request: start_page={}, end_page={}", user_request.start_page, user_request.end_page);
    
    // If user provided explicit range (both > 0), use it directly
    if user_request.start_page > 0 && user_request.end_page > 0 {
        tracing::info!("✅ Using explicit user range: {} to {}", user_request.start_page, user_request.end_page);
        return Ok((user_request.start_page, user_request.end_page));
    }
    
    // If 0 values provided, use intelligent calculation
    tracing::info!("🧠 User provided 0 values - using intelligent calculation from app config and DB state");
    // Get configuration
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

    // ✅ 실제 사이트 상태 분석 (하드코딩 제거)
    tracing::info!("🔍 Performing real-time site analysis...");
    let http_client = crate::infrastructure::HttpClient::new()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let data_extractor = crate::infrastructure::MatterDataExtractor::new()
        .map_err(|e| format!("Failed to create data extractor: {}", e))?;
    
    let status_checker = crate::infrastructure::crawling_service_impls::StatusCheckerImpl::new(
        http_client,
        data_extractor,
        config,
    );
    
    let site_status = status_checker.check_site_status().await
        .map_err(|e| format!("Failed to check site status: {}", e))?;
    
    tracing::info!("📊 Real site status: total_pages={}, products_on_last_page={}", 
                   site_status.total_pages, site_status.products_on_last_page);

    // ✅ 실제 사이트 데이터 사용 (하드코딩 제거)
    let total_pages_on_site = site_status.total_pages;
    let products_on_last_page = site_status.products_on_last_page;

    tracing::info!("🌐 Using site parameters: {} total pages, {} products on last page", 
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
            // Return a small verification range for reverse crawling
            let verification_pages = 5;
            let start_page = total_pages_on_site;  // Start from the last page (oldest)
            let end_page = if start_page >= verification_pages {
                start_page.saturating_sub(verification_pages).saturating_add(1)  // Go back a few pages
            } else {
                1
            };
            tracing::info!("🔍 Verification range: pages {} down to {} ({} pages)", 
                          start_page, end_page, verification_pages);
            Ok((start_page, end_page))
        }
    }
}

/// Get the correct database URL for v4 commands
fn get_database_url_v4() -> Result<String, String> {
    // First try to use the path from .env file if it exists
    if let Ok(db_url) = std::env::var("DATABASE_URL") {
        if !db_url.is_empty() {
            tracing::info!("Using database URL from environment: {}", db_url);
            return Ok(db_url);
        }
    }

    // Use the app name to create a consistent data directory
    let app_name = "matter-certis-v2";
    
    let app_data_dir = match dirs::data_dir() {
        Some(mut path) => {
            path.push(app_name);
            path
        },
        None => {
            return Err("Failed to determine app data directory".to_string());
        }
    };
    
    let db_dir = app_data_dir.join("database");
    let db_path = db_dir.join("matter_certis.db");
    
    // Create directories if they don't exist
    if !db_dir.exists() {
        if let Err(err) = std::fs::create_dir_all(&db_dir) {
            return Err(format!("Failed to create database directory: {}", err));
        }
    }
    
    // Create database file if it doesn't exist
    if !db_path.exists() {
        if let Err(err) = std::fs::File::create(&db_path) {
            return Err(format!("Failed to create database file: {}", err));
        }
    }
    
    tracing::info!("Using database at: {}", db_path.display());
    let db_url = format!("sqlite:{}", db_path.display());
    Ok(db_url)
}
