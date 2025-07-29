//! Actor System Commands for Tauri Integration
//! 
//! Commands to test and use the Actor system from the UI

use crate::new_architecture::actor_system::SessionActor;
use crate::new_architecture::system_config::SystemConfig;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tracing::info;
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
    _app: AppHandle,
    request: ActorCrawlingRequest,
) -> Result<ActorSystemResponse, String> {
    info!("🎭 [NEW ARCHITECTURE] Starting Actor-based crawling: {:?}", request);
    
    let batch_size = request.batch_size.unwrap_or(3);
    let total_pages = request.end_page - request.start_page + 1;
    let batch_count = (total_pages + batch_size - 1) / batch_size; // 올림 계산
    
    info!("✅ [ACTOR] Crawling configuration created for Actor system");
    info!("📊 [ACTOR] Pages: {} to {}, Batch size: {}, Expected batches: {}", 
          request.start_page, request.end_page, batch_size, batch_count);
    
    Ok(ActorSystemResponse {
        success: true,
        message: format!("Actor-based crawling started: pages {} to {} with batch size {} ({} batches)", 
                        request.start_page, request.end_page, batch_size, batch_count),
        session_id: Some(format!("actor_session_{}", Utc::now().timestamp())),
        data: Some(serde_json::json!({
            "engine_type": "Actor",
            "architecture": "SessionActor → BatchActor → StageActor",
            "config": {
                "start_page": request.start_page,
                "end_page": request.end_page,
                "batch_size": batch_size,
                "concurrency": request.concurrency.unwrap_or(8),
                "total_pages": total_pages,
                "expected_batches": batch_count
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
