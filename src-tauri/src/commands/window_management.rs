//! Window Management Commands
//! 
//! Tauri commands for window state management, positioning, and sizing.
//! Provides functionality to save/load window state, set position/size, and maximize.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, Window};
use tracing::{error, info};
use std::fs;
use std::path::PathBuf;

/// Window state structure that matches frontend expectations
/// ts-rs를 통해 TypeScript 타입 자동 생성 (Modern Rust 2024 정책)
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct WindowState {
    pub position: Position,
    pub size: Size,
    pub zoom_level: f64,
    pub last_active_tab: String,
    pub is_maximized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            position: Position { x: 100, y: 100 },
            size: Size { width: 1200.0, height: 800.0 },
            zoom_level: 1.0,
            last_active_tab: "settings".to_string(),
            is_maximized: false,
        }
    }
}

/// Get the path for window state file
fn get_window_state_path() -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    let mut path = tauri::api::path::app_local_data_dir(&tauri::Config::default())
        .ok_or("Failed to get app local data directory")?;
    path.push("window_state.json");
    
    // Create parent directory if it doesn't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    Ok(path)
}

/// Save window state to file
#[tauri::command]
pub async fn save_window_state(state: WindowState) -> Result<(), String> {
    info!("💾 Saving window state: {:?}", state);
    
    let path = get_window_state_path()
        .map_err(|e| format!("Failed to get window state path: {}", e))?;
    
    let json = serde_json::to_string_pretty(&state)
        .map_err(|e| format!("Failed to serialize window state: {}", e))?;
    
    fs::write(&path, json)
        .map_err(|e| format!("Failed to write window state file: {}", e))?;
    
    info!("✅ Window state saved to: {:?}", path);
    Ok(())
}

/// Load window state from file
#[tauri::command]
pub async fn load_window_state() -> Result<Option<WindowState>, String> {
    let path = get_window_state_path()
        .map_err(|e| format!("Failed to get window state path: {}", e))?;
    
    if !path.exists() {
        info!("📁 No window state file found, using defaults");
        return Ok(None);
    }
    
    let json = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read window state file: {}", e))?;
    
    let state: WindowState = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to parse window state: {}", e))?;
    
    info!("📂 Window state loaded: {:?}", state);
    Ok(Some(state))
}

/// Set window position
#[tauri::command]
pub async fn set_window_position(app_handle: AppHandle, x: i32, y: i32) -> Result<(), String> {
    let window = app_handle.get_window("main")
        .ok_or("Main window not found")?;
    
    let position = PhysicalPosition::new(x, y);
    window.set_position(position)
        .map_err(|e| format!("Failed to set window position: {}", e))?;
    
    info!("📐 Window position set to: ({}, {})", x, y);
    Ok(())
}

/// Set window size
#[tauri::command]
pub async fn set_window_size(app_handle: AppHandle, width: f64, height: f64) -> Result<(), String> {
    let window = app_handle.get_window("main")
        .ok_or("Main window not found")?;
    
    let size = PhysicalSize::new(width, height);
    window.set_size(size)
        .map_err(|e| format!("Failed to set window size: {}", e))?;
    
    info!("📏 Window size set to: {}x{}", width, height);
    Ok(())
}

/// Maximize window
#[tauri::command]
pub async fn maximize_window(app_handle: AppHandle) -> Result<(), String> {
    let window = app_handle.get_window("main")
        .ok_or("Main window not found")?;
    
    window.maximize()
        .map_err(|e| format!("Failed to maximize window: {}", e))?;
    
    info!("🔲 Window maximized");
    Ok(())
}

/// Minimize window
#[tauri::command]
pub async fn minimize_window(app_handle: AppHandle) -> Result<(), String> {
    let window = app_handle.get_window("main")
        .ok_or("Main window not found")?;
    
    window.minimize()
        .map_err(|e| format!("Failed to minimize window: {}", e))?;
    
    info!("📉 Window minimized");
    Ok(())
}

/// Unmaximize (restore) window
#[tauri::command]
pub async fn unmaximize_window(app_handle: AppHandle) -> Result<(), String> {
    let window = app_handle.get_window("main")
        .ok_or("Main window not found")?;
    
    window.unmaximize()
        .map_err(|e| format!("Failed to unmaximize window: {}", e))?;
    
    info!("🔳 Window restored (unmaximized)");
    Ok(())
}

/// Get current window state
#[tauri::command]
pub async fn get_window_state(app_handle: AppHandle) -> Result<WindowState, String> {
    let window = app_handle.get_window("main")
        .ok_or("Main window not found")?;
    
    // Get current position
    let position = window.outer_position()
        .map_err(|e| format!("Failed to get window position: {}", e))?;
    
    // Get current size
    let size = window.outer_size()
        .map_err(|e| format!("Failed to get window size: {}", e))?;
    
    // Get maximized state
    let is_maximized = window.is_maximized()
        .map_err(|e| format!("Failed to get window maximized state: {}", e))?;
    
    let current_state = WindowState {
        position: Position {
            x: position.x,
            y: position.y,
        },
        size: Size {
            width: size.width as f64,
            height: size.height as f64,
        },
        zoom_level: 1.0, // Default zoom level
        last_active_tab: "settings".to_string(), // Default tab
        is_maximized,
    };
    
    info!("📊 Current window state: {:?}", current_state);
    Ok(current_state)
}
