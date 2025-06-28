//! Tauri commands module
//! 
//! This module contains all Tauri commands that expose
//! backend functionality to the frontend.

// Example command - remove this when implementing real commands
#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}
