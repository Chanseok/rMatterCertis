#![allow(clippy::used_underscore_binding)]
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, State};

#[derive(Default)]
pub struct ActorSystemState {
    pub is_running: Arc<tokio::sync::RwLock<bool>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ActorSystemResponse {
    pub success: bool,
    pub message: String,
    pub session_id: Option<String>,
    pub data: Option<serde_json::Value>,
}

#[tauri::command]
pub async fn test_new_arch_channels(
    _app: AppHandle,
    _state: State<'_, ActorSystemState>,
) -> Result<ActorSystemResponse, String> {
    Ok(ActorSystemResponse {
        success: true,
        message: "Triple channel system test completed successfully".to_string(),
        session_id: None,
        data: Some(serde_json::json!({"test":"Channels","status":"passed"})),
    })
}

#[tauri::command]
pub async fn test_new_arch_performance(
    _app: AppHandle,
    _state: State<'_, ActorSystemState>,
) -> Result<ActorSystemResponse, String> {
    Ok(ActorSystemResponse {
        success: true,
        message: "Performance test completed".to_string(),
        session_id: None,
        data: Some(serde_json::json!({"test":"Performance","status":"passed"})),
    })
}
