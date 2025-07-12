//! # Modern Crawling Commands v4.0
//!
//! Tauri commands for the new event-driven crawling system.
//! These commands integrate with the new orchestrator and provide
//! real-time updates to the frontend.

use std::sync::Arc;
use std::collections::HashMap;
use tauri::{AppHandle, Manager, State, Emitter};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

use crate::crawling::*;
// use crate::infrastructure::Database;  // 임시 비활성화

/// Global state for the crawling engine (개발 중 Mock)
pub struct CrawlingEngineState {
    pub engine: Arc<RwLock<Option<CrawlingEngine>>>,
    pub database: MockDatabase, // 개발용 Mock
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
    pub start_url: String,
    pub max_pages: Option<u32>,
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
    let _database_pool = state.database.get_pool().await
        .map_err(|e| format!("Database connection failed: {}", e))?;
    
    // Create default configuration
    let config = CrawlingConfig::default();
    
    // Initialize the engine with mock database
    let engine = CrawlingEngine::with_config(config).await
        .map_err(|e| format!("Engine initialization failed: {}", e))?;
    
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
    tracing::info!("Starting crawling with request: {:?}", request);
    
    let engine_guard = state.engine.read().await;
    let engine = engine_guard.as_ref()
        .ok_or_else(|| "Crawling engine not initialized".to_string())?;
    
    // Start the engine if not already running
    if !engine.is_running().await {
        engine.start().await
            .map_err(|e| format!("Failed to start engine: {}", e))?;
    }
    
    // Start crawling from the specified URL
    engine.start_crawling_session(1, 10).await
        .map_err(|e| format!("Failed to start crawling: {}", e))?;
    
    // Start real-time statistics broadcasting
    let app_handle = app.clone();
    let engine_clone = engine.clone();
    tokio::spawn(async move {
        broadcast_real_time_stats(app_handle, engine_clone).await;
    });
    
    tracing::info!("Crawling started successfully from: {}", request.start_url);
    
    Ok(CrawlingResponse {
        success: true,
        message: format!("Crawling started from: {}", request.start_url),
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
    
    engine.stop().await
        .map_err(|e| format!("Failed to stop engine: {}", e))?;
    
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
    
    // 통계 조회
    let stats = engine.get_stats().await;
    
    Ok(convert_stats_to_payload(stats, engine.is_running().await))
}

/// Get system health status
#[tauri::command]
pub async fn get_system_health(
    state: State<'_, CrawlingEngineState>,
) -> Result<CrawlingResponse, String> {
    let engine_guard = state.engine.read().await;
    let engine = engine_guard.as_ref()
        .ok_or_else(|| "Crawling engine not initialized".to_string())?;
    
    let stats = engine.get_stats().await;
    
    let health_data = serde_json::json!({
        "is_healthy": stats.is_healthy,
        "is_running": engine.is_running().await,
        "uptime_seconds": stats.total_tasks_created, // Mock value
        "success_rate": if stats.total_tasks_processed() > 0 {
            (stats.tasks_completed as f64 / stats.total_tasks_processed() as f64) * 100.0
        } else {
            0.0
        },
        "worker_utilization": 75.0, // Mock value
        "tasks_per_second": stats.processing_rate,
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
    config: CrawlingConfig,
) -> Result<CrawlingResponse, String> {
    let mut engine_guard = state.engine.write().await;
    let engine = engine_guard.as_mut()
        .ok_or_else(|| "Crawling engine not initialized".to_string())?;
    
    engine.update_config(config).await
        .map_err(|e| format!("Failed to update config: {}", e))?;
    
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
    
    let config = engine.get_config();
    let config_data = serde_json::to_value(config)
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
    
    if let Some(engine) = engine_guard.as_ref() {
        // Force stop the engine
        let _ = engine.stop().await;
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

/// Background task for broadcasting real-time statistics
async fn broadcast_real_time_stats(app: AppHandle, engine: CrawlingEngine) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
    
    loop {
        interval.tick().await;
        
        // Check if engine is still running
        if !engine.is_running().await {
            break;
        }
        
        // Get current stats
        let stats = engine.get_stats().await;
        let payload = convert_stats_to_payload(stats, engine.is_running().await);
        
        // Emit to frontend
        if let Err(e) = app.emit("system_state_update", &payload) {
            tracing::error!("Failed to emit system state update: {}", e);
        }
    }
    
    tracing::info!("Real-time stats broadcasting stopped");
}

/// Convert engine stats to frontend payload
fn convert_stats_to_payload(stats: CrawlingStats, is_running: bool) -> SystemStatePayload {
    let mut queue_sizes = HashMap::new();
    queue_sizes.insert("list_page_fetch".to_string(), stats.queue_sizes.list_page_fetch);
    queue_sizes.insert("list_page_parse".to_string(), stats.queue_sizes.list_page_parse);
    queue_sizes.insert("product_detail_fetch".to_string(), stats.queue_sizes.product_detail_fetch);
    queue_sizes.insert("product_detail_parse".to_string(), stats.queue_sizes.product_detail_parse);
    queue_sizes.insert("product_save".to_string(), stats.queue_sizes.product_save);
    
    SystemStatePayload {
        is_running,
        uptime_seconds: 0, // Mock value for now
        total_tasks_processed: stats.total_tasks_processed(),
        successful_tasks: stats.tasks_completed,
        failed_tasks: stats.tasks_failed,
        current_active_tasks: stats.active_tasks,
        tasks_per_second: stats.processing_rate,
        worker_utilization: 75.0, // Mock value
        queue_sizes,
        detailed_stats: DetailedStats {
            list_pages_fetched: stats.list_pages_fetched,
            list_pages_processed: stats.list_pages_processed as u64,
            product_urls_discovered: stats.product_urls_discovered,
            product_details_fetched: stats.product_details_fetched,
            product_details_parsed: stats.product_details_parsed,
            products_saved: stats.products_saved,
        },
        is_healthy: stats.is_healthy,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn payload_conversion() {
        // Test payload conversion logic
        let stats = CrawlingEngineStats {
            uptime: std::time::Duration::from_secs(100),
            total_tasks_processed: 50,
            successful_tasks: 45,
            failed_tasks: 5,
            current_active_tasks: 3,
            tasks_per_second: 0.5,
            worker_utilization: 75.0,
            list_pages_fetched: 10,
            list_pages_processed: 10,
            product_urls_discovered: 200,
            product_details_fetched: 180,
            product_details_parsed: 175,
            products_saved: 170,
            queue_sizes: QueueSizes {
                list_page_fetch: 5,
                list_page_parse: 3,
                product_detail_fetch: 25,
                product_detail_parse: 20,
                product_save: 15,
            },
            is_healthy: true,
        };
        
        let payload = convert_stats_to_payload(stats, true);
        
        assert_eq!(payload.is_running, true);
        assert_eq!(payload.uptime_seconds, 100);
        assert_eq!(payload.total_tasks_processed, 50);
        assert_eq!(payload.successful_tasks, 45);
        assert_eq!(payload.is_healthy, true);
    }
}
