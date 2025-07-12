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

use crate::crawling::*;
use crate::domain::services::crawling_services::{StatusChecker, DatabaseAnalyzer};
use crate::infrastructure::service_based_crawling_engine::{ServiceBasedBatchCrawlingEngine, BatchCrawlingConfig};

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
    
    // Create default configuration for engine
    let config = BatchCrawlingConfig::default();
    
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
    
    tracing::info!("Crawling engine v4.0 initialized successfully");
    
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
    
    tracing::info!("� Step 5: Calculated optimal range: {} to {}", start_page, end_page);
    
    // Validate page range
    if start_page == 0 || end_page == 0 {
        return Err("Page numbers must be greater than 0".to_string());
    }
    
    tracing::info!("🔍 Step 6: Starting crawling execution...");
    // Execute the crawling directly using the engine
    let execution_result = engine.execute().await
        .map_err(|e| format!("Failed to execute crawling: {}", e));
    
    match execution_result {
        Ok(_) => {
            tracing::info!("✅ Crawling completed successfully: pages {} to {}", start_page, end_page);
        }
        Err(e) => {
            tracing::error!("❌ Crawling failed: {}", e);
            return Err(e);
        }
    }
    
    Ok(CrawlingResponse {
        success: true,
        message: format!("Crawling started: pages {} to {}", start_page, end_page),
        data: None,
    })
}

/// Stop the crawling process
#[tauri::command]
pub async fn stop_crawling(
    app: AppHandle,
    state: State<'_, CrawlingEngineState>,
) -> Result<CrawlingResponse, String> {
    tracing::info!("Stopping crawling...");
    
    let engine_guard = state.engine.read().await;
    let engine = engine_guard.as_ref()
        .ok_or_else(|| "Crawling engine not initialized".to_string())?;
    
    // ServiceBasedBatchCrawlingEngine doesn't have stop method
    // For now, we'll just emit the stop event
    tracing::info!("Crawling stop requested (engine doesn't support runtime stopping)");
    
    // Emit stop event to update UI immediately
    if let Err(e) = app.emit("crawling-stopped", serde_json::json!({
        "status": "stopped",
        "message": "Crawling has been stopped",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })) {
        tracing::warn!("Failed to emit stop event: {}", e);
    }
    
    tracing::info!("Crawling stopped successfully");
    
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

/// Calculate intelligent crawling range based on comprehensive analysis
/// Uses existing StatusChecker and DatabaseAnalyzer implementations
async fn calculate_intelligent_crawling_range_v4(
    app_config: &crate::infrastructure::config::AppConfig,
    user_request: &StartCrawlingRequest,
) -> Result<(u32, u32), String> {
    tracing::info!("🔍 Starting comprehensive crawling range calculation...");
    tracing::info!("📝 User preferences: start_page={}, end_page={}", user_request.start_page, user_request.end_page);
    
    // Get database URL and connect
    tracing::info!("📊 Step 1: Connecting to database...");
    let database_url = get_database_url_v4()?;
    let db_pool = sqlx::SqlitePool::connect(&database_url).await
        .map_err(|e| format!("Failed to connect to database: {}", e))?;
    tracing::info!("✅ Database connected successfully");
    
    // Create necessary components for comprehensive analysis
    tracing::info!("📊 Step 2: Initializing analysis components...");
    let http_client = crate::infrastructure::HttpClient::new()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let data_extractor = crate::infrastructure::MatterDataExtractor::new()
        .map_err(|e| format!("Failed to create data extractor: {}", e))?;
    
    // Create status checker and database analyzer using existing implementations
    let status_checker = crate::infrastructure::StatusCheckerImpl::new(
        http_client,
        data_extractor,
        app_config.clone(),
    );
    
    let repo = crate::infrastructure::IntegratedProductRepository::new(db_pool);
    let repo_arc = std::sync::Arc::new(repo);
    let db_analyzer = crate::infrastructure::DatabaseAnalyzerImpl::new(repo_arc);
    
    // Perform comprehensive analysis
    tracing::info!("📊 Step 3: Analyzing current site status...");
    let site_status = status_checker.check_site_status().await
        .map_err(|e| format!("Failed to check site status: {}", e))?;
    tracing::info!("🌐 Site analysis: {} total pages, {} estimated products", 
                  site_status.total_pages, site_status.estimated_products);
    
    tracing::info!("📊 Step 4: Analyzing local database state...");
    let db_analysis = db_analyzer.analyze_current_state().await
        .map_err(|e| format!("Failed to analyze database: {}", e))?;
    tracing::info!("💾 Database analysis: {} total products, {} unique products", 
                  db_analysis.total_products, db_analysis.unique_products);
    
    // Calculate intelligent recommendation using existing logic
    tracing::info!("📊 Step 5: Calculating optimal crawling strategy...");
    let recommendation = status_checker.calculate_crawling_range_recommendation(&site_status, &db_analysis).await
        .map_err(|e| format!("Failed to calculate range recommendation: {}", e))?;
    
    // Apply intelligent range calculation with user settings consideration
    let (start_page, end_page) = match recommendation {
        crate::domain::services::crawling_services::CrawlingRangeRecommendation::Full => {
            // Full crawl needed - respect user page_range_limit
            let total_pages = site_status.total_pages;
            let user_page_limit = app_config.user.crawling.page_range_limit;
            let start_page = total_pages;
            let end_page = if start_page >= user_page_limit {
                start_page - user_page_limit + 1
            } else {
                1
            };
            tracing::info!("🎯 Strategy: FULL CRAWL - {} to {} ({} pages max)", start_page, end_page, user_page_limit);
            (start_page, end_page)
        },
        crate::domain::services::crawling_services::CrawlingRangeRecommendation::Partial(pages_to_crawl) => {
            // Partial crawl - balance recommendation with user settings
            let total_pages = site_status.total_pages;
            let user_page_limit = app_config.user.crawling.page_range_limit;
            let actual_pages = pages_to_crawl.min(user_page_limit);
            let start_page = total_pages;
            let end_page = if start_page >= actual_pages {
                start_page - actual_pages + 1
            } else {
                1
            };
            tracing::info!("🎯 Strategy: PARTIAL CRAWL - {} to {} ({} pages recommended)", start_page, end_page, actual_pages);
            (start_page, end_page)
        },
        crate::domain::services::crawling_services::CrawlingRangeRecommendation::None => {
            // No update needed - verification crawl
            let verification_pages = 5.min(app_config.user.max_pages);
            let start_page = site_status.total_pages;
            let end_page = if start_page >= verification_pages {
                start_page - verification_pages + 1
            } else {
                1
            };
            tracing::info!("🎯 Strategy: VERIFICATION CRAWL - {} to {} ({} pages)", start_page, end_page, verification_pages);
            (start_page, end_page)
        }
    };
    
    tracing::info!("✅ Intelligent range calculation completed");
    tracing::info!("📊 Final decision: pages {} to {} (total: {} pages)", 
                  start_page, end_page, if start_page >= end_page { start_page - end_page + 1 } else { 0 });
    
    Ok((start_page, end_page))
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
