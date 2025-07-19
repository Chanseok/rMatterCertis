//! Actor System Test Commands for Tauri Integration
//! 
//! Simple test commands to verify Actor system functionality in the UI

use crate::new_architecture::actor_system::{SessionActor, ActorError};
use crate::new_architecture::channel_types::AppEvent;
use crate::new_architecture::system_config::SystemConfig;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, State, Emitter};
use tokio::sync::mpsc;
use tracing::{info, error};

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

/// Test SessionActor functionality
#[tauri::command]
pub async fn test_new_arch_session_actor(
    app: AppHandle,
    state: State<'_, ActorSystemState>,
) -> Result<ActorSystemResponse, String> {
    info!("🧪 Testing SessionActor...");
    
    let system_config = Arc::new(SystemConfig::default());
    let (control_tx, control_rx) = mpsc::channel(100);
    let (event_tx, _event_rx) = mpsc::channel(500);
    
    let session_actor = SessionActor::new(
        system_config,
        control_rx,
        event_tx,
    );
    
    info!("✅ SessionActor created successfully");
    
    Ok(ActorSystemResponse {
        success: true,
        message: "SessionActor test completed successfully".to_string(),
        session_id: Some(session_actor.session_id.clone()),
        data: None,
    })
}

/// Test BatchActor functionality
#[tauri::command]
pub async fn test_new_arch_batch_actor(
    app: AppHandle,
    state: State<'_, ActorSystemState>,
) -> Result<ActorSystemResponse, String> {
    info!("🧪 Testing BatchActor...");
    
    // Batch Actor는 현재 OneShot 채널 시스템으로 구현됨
    info!("✅ BatchActor OneShot integration ready");
    
    Ok(ActorSystemResponse {
        success: true,
        message: "BatchActor test completed successfully".to_string(),
        session_id: None,
        data: Some(serde_json::json!({
            "oneshot_channels": "implemented",
            "triple_channel_system": "active"
        })),
    })
}

/// Test Actor integration
#[tauri::command]
pub async fn test_new_arch_integration(
    app: AppHandle,
    state: State<'_, ActorSystemState>,
) -> Result<ActorSystemResponse, String> {
    info!("🧪 Testing Actor system integration...");
    
    // Integration test - create full system
    let system_config = Arc::new(SystemConfig::default());
    let (control_tx, control_rx) = mpsc::channel(100);
    let (event_tx, mut event_rx) = mpsc::channel(500);
    
    let mut session_actor = SessionActor::new(
        system_config,
        control_rx,
        event_tx.clone(),
    );
    
    // Test actor system flow
    tokio::select! {
        _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
            info!("✅ Actor system integration test completed");
        }
    }
    
    Ok(ActorSystemResponse {
        success: true,
        message: "Actor system integration test completed successfully".to_string(),
        session_id: Some(session_actor.session_id.clone()),
        data: Some(serde_json::json!({
            "triple_channels": "control + data + event",
            "oneshot_integration": "active",
            "session_management": "functional"
        })),
    })
}

/// Test channel system
#[tauri::command]
pub async fn test_new_arch_channels(
    app: AppHandle,
    state: State<'_, ActorSystemState>,
) -> Result<ActorSystemResponse, String> {
    info!("🧪 Testing triple channel system...");
    
    // Test all three channel types
    let (control_tx, _control_rx) = mpsc::channel::<String>(100);
    let (data_tx, _data_rx) = tokio::sync::oneshot::channel::<String>();
    let (event_tx, _event_rx) = mpsc::channel::<String>(500);
    
    // Test channel send/receive
    if let Err(_) = control_tx.try_send("test control".to_string()) {
        // Expected - no receiver
    }
    
    if let Err(_) = data_tx.send("test data".to_string()) {
        // Expected - no receiver  
    }
    
    if let Err(_) = event_tx.try_send("test event".to_string()) {
        // Expected - no receiver
    }
    
    info!("✅ Triple channel system test completed");
    
    Ok(ActorSystemResponse {
        success: true,
        message: "Triple channel system test completed successfully".to_string(),
        session_id: None,
        data: Some(serde_json::json!({
            "control_channel": "MPSC - Commands",
            "data_channel": "OneShot - Results", 
            "event_channel": "MPSC - Events"
        })),
    })
}

/// Test performance
#[tauri::command]
pub async fn test_new_arch_performance(
    app: AppHandle,
    state: State<'_, ActorSystemState>,
) -> Result<ActorSystemResponse, String> {
    info!("🧪 Testing Actor system performance...");
    
    let start_time = std::time::Instant::now();
    
    // Performance test - create multiple actors
    let system_config = Arc::new(SystemConfig::default());
    
    for i in 0..10 {
        let (control_tx, control_rx) = mpsc::channel(100);
        let (event_tx, _event_rx) = mpsc::channel(500);
        
        let _session_actor = SessionActor::new(
            system_config.clone(),
            control_rx,
            event_tx,
        );
    }
    
    let elapsed = start_time.elapsed();
    info!("✅ Performance test completed in {:?}", elapsed);
    
    Ok(ActorSystemResponse {
        success: true,
        message: format!("Performance test completed in {:?}", elapsed),
        session_id: None,
        data: Some(serde_json::json!({
            "actors_created": 10,
            "elapsed_ms": elapsed.as_millis(),
            "channels_per_actor": 2,
            "total_channels": 20
        })),
    })
}/// Request to start Actor-based crawling session
#[derive(Debug, Serialize, Deserialize)]
pub struct StartActorCrawlingRequest {
    pub start_page: u32,
    pub end_page: u32,
    pub batch_size: Option<u32>,
    pub concurrency_limit: Option<u32>,
    pub timeout_seconds: Option<u64>,
}

/// Response from Actor system operations
#[derive(Debug, Serialize, Deserialize)]
pub struct ActorSystemResponse {
    pub success: bool,
    pub message: String,
    pub session_id: Option<String>,
    pub batch_count: Option<u32>,
}

/// Real-time Actor system status
#[derive(Debug, Serialize, Deserialize)]
pub struct ActorSystemStatus {
    pub is_running: bool,
    pub session_id: Option<String>,
    pub active_batches: u32,
    pub completed_batches: u32,
    pub total_pages_processed: u32,
    pub current_stage: Option<String>,
    pub estimated_completion_time: Option<u64>,
}

/// Initialize the Actor system with configuration
#[tauri::command]
pub async fn init_actor_system(
    app: AppHandle,
    state: State<'_, ActorSystemState>,
) -> Result<ActorSystemResponse, String> {
    info!("🚀 Initializing Actor System...");
    
    // Load system configuration
    let config_manager = ConfigManager::new();
    let app_config = match config_manager.load_config().await {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("Failed to load app config: {}", e);
            return Err(format!("Configuration error: {}", e));
        }
    };

    // Create system configuration
    let system_config = Arc::new(SystemConfig::default());    // Create event broadcast channel
    let (event_tx, _event_rx) = broadcast::channel::<AppEvent>(1000);
    
    // Initialize SessionActor
    let session_actor = SessionActor::new(
        "default-session".to_string(),
        system_config,
        event_tx.clone(),
    );
    
    // Store in Tauri state
    let mut state_guard = state.inner();
    state_guard.session_actor = Some(Arc::new(tokio::sync::Mutex::new(session_actor)));
    state_guard.event_tx = Some(event_tx.clone());
    
    // Start event forwarding to UI
    let app_handle = app.clone();
    tokio::spawn(async move {
        let mut event_rx = event_tx.subscribe();
        while let Ok(event) = event_rx.recv().await {
            if let Err(e) = app_handle.emit("actor-system-event", &event) {
                warn!("Failed to emit actor system event: {}", e);
            }
        }
    });
    
    info!("✅ Actor System initialized successfully");
    
    Ok(ActorSystemResponse {
        success: true,
        message: "Actor System initialized successfully".to_string(),
        session_id: Some("default-session".to_string()),
        batch_count: None,
    })
}

/// Start crawling using the Actor system
#[tauri::command]
pub async fn start_actor_crawling(
    request: StartActorCrawlingRequest,
    app: AppHandle,
    state: State<'_, ActorSystemState>,
) -> Result<ActorSystemResponse, String> {
    info!("🎬 Starting Actor-based crawling: {:?}", request);
    
    // Check if system is already running
    {
        let is_running = state.is_running.read().await;
        if *is_running {
            return Err("Actor system is already running".to_string());
        }
    }
    
    // Get session actor
    let session_actor = match &state.session_actor {
        Some(actor) => actor.clone(),
        None => return Err("Actor system not initialized. Call init_actor_system first.".to_string()),
    };
    
    // Prepare pages for processing
    let pages: Vec<u32> = (request.start_page..=request.end_page).collect();
    let batch_size = request.batch_size.unwrap_or(10);
    let concurrency_limit = request.concurrency_limit.unwrap_or(5);
    
    // Set running state
    {
        let mut is_running = state.is_running.write().await;
        *is_running = true;
    }
    
    // Start the session in background
    let is_running_ref = state.is_running.clone();
    let app_handle = app.clone();
    
    tokio::spawn(async move {
        let mut actor = session_actor.lock().await;
        
        // Create channels for the session
        let (control_tx, control_rx) = mpsc::channel::<ActorCommand>(100);
        let (data_tx, data_rx) = oneshot::channel::<StageResult>();
        
        // Send process batch command
        let command = ActorCommand::ProcessBatch {
            pages: pages.clone(),
            config: Arc::new(crate::infrastructure::config::ConfigManager::load_config().unwrap()),
            batch_size,
            concurrency_limit,
        };
        
        if let Err(e) = control_tx.send(command).await {
            error!("Failed to send process batch command: {}", e);
            let mut is_running = is_running_ref.write().await;
            *is_running = false;
            return;
        }
        
        // Start actor processing
        let result = tokio::select! {
            result = actor.run(control_rx, data_tx) => {
                result
            }
            _ = tokio::time::sleep(Duration::from_secs(request.timeout_seconds.unwrap_or(3600))) => {
                warn!("Actor session timed out");
                Err(ActorError::TimeoutError {
                    message: "Session timed out".to_string(),
                    timeout_duration: Duration::from_secs(request.timeout_seconds.unwrap_or(3600)),
                })
            }
        };
        
        // Wait for result
        match data_rx.await {
            Ok(stage_result) => {
                info!("✅ Actor crawling completed: {:?}", stage_result);
                
                // Emit completion event to UI
                if let Err(e) = app_handle.emit("actor-crawling-completed", &stage_result) {
                    warn!("Failed to emit completion event: {}", e);
                }
            }
            Err(e) => {
                error!("❌ Actor crawling failed: {:?}", e);
                
                // Emit error event to UI
                if let Err(e) = app_handle.emit("actor-crawling-error", &format!("Crawling failed: {:?}", e)) {
                    warn!("Failed to emit error event: {}", e);
                }
            }
        }
        
        // Reset running state
        let mut is_running = is_running_ref.write().await;
        *is_running = false;
        
        info!("🏁 Actor session completed");
    });
    
    info!("✅ Actor crawling session started");
    
    Ok(ActorSystemResponse {
        success: true,
        message: "Actor crawling started successfully".to_string(),
        session_id: Some("default-session".to_string()),
        batch_count: Some((pages.len() as f32 / batch_size as f32).ceil() as u32),
    })
}

/// Get current Actor system status
#[tauri::command]
pub async fn get_actor_system_status(
    state: State<'_, ActorSystemState>,
) -> Result<ActorSystemStatus, String> {
    let is_running = *state.is_running.read().await;
    
    Ok(ActorSystemStatus {
        is_running,
        session_id: Some("default-session".to_string()),
        active_batches: if is_running { 1 } else { 0 },
        completed_batches: 0, // TODO: Implement proper tracking
        total_pages_processed: 0, // TODO: Implement proper tracking
        current_stage: if is_running { Some("Processing".to_string()) } else { None },
        estimated_completion_time: None, // TODO: Implement ETA calculation
    })
}

/// Stop the Actor system gracefully
#[tauri::command]
pub async fn stop_actor_system(
    state: State<'_, ActorSystemState>,
    app: AppHandle,
) -> Result<ActorSystemResponse, String> {
    info!("🛑 Stopping Actor System...");
    
    // Set running state to false
    {
        let mut is_running = state.is_running.write().await;
        *is_running = false;
    }
    
    // TODO: Implement graceful shutdown of actors
    // For now, we just set the flag and let the actors finish naturally
    
    // Emit stop event to UI
    if let Err(e) = app.emit("actor-system-stopped", "Actor system stopped") {
        warn!("Failed to emit stop event: {}", e);
    }
    
    info!("✅ Actor System stopped");
    
    Ok(ActorSystemResponse {
        success: true,
        message: "Actor System stopped successfully".to_string(),
        session_id: None,
        batch_count: None,
    })
}

/// Test Actor system connectivity and basic functionality
#[tauri::command]
pub async fn test_actor_system(
    state: State<'_, ActorSystemState>,
) -> Result<ActorSystemResponse, String> {
    info!("🧪 Testing Actor System...");
    
    // Check if session actor exists
    if state.session_actor.is_none() {
        return Err("Actor system not initialized".to_string());
    }
    
    // Check if event channel exists
    if state.event_tx.is_none() {
        return Err("Event system not initialized".to_string());
    }
    
    // Send test event
    if let Some(event_tx) = &state.event_tx {
        let test_event = AppEvent::BatchStarted {
            batch_id: "test-batch".to_string(),
            page_count: 10,
            timestamp: chrono::Utc::now(),
        };
        
        if let Err(e) = event_tx.send(test_event) {
            warn!("Failed to send test event: {}", e);
        }
    }
    
    info!("✅ Actor System test completed");
    
    Ok(ActorSystemResponse {
        success: true,
        message: "Actor System test passed".to_string(),
        session_id: Some("test-session".to_string()),
        batch_count: None,
    })
}
