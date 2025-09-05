//! Legacy compatibility wrappers for frontend invokes.
//! These map old invoke names to the new actor_system commands without exposing new APIs.

use tauri::AppHandle;

// Reuse types from actor_system where relevant
use crate::commands::crawling::actor_system::{
    ActorSystemResponse,
    get_session_status as actor_get_session_status,
    pause_session as actor_pause_session,
    request_graceful_shutdown as actor_stop_session,
    resume_session as actor_resume_session,
    resume_from_token as actor_resume_from_token,
};

#[tauri::command]
pub async fn get_crawling_status(app: AppHandle, session_id: String) -> Result<ActorSystemResponse, String> {
    actor_get_session_status(app, session_id).await
}

#[tauri::command]
pub async fn pause_crawling(app: AppHandle, session_id: String) -> Result<ActorSystemResponse, String> {
    actor_pause_session(app, session_id).await
}

#[tauri::command]
pub async fn resume_crawling(app: AppHandle, session_id: String) -> Result<ActorSystemResponse, String> {
    actor_resume_session(app, session_id).await
}

#[tauri::command]
pub async fn stop_crawling(app: AppHandle, session_id: String) -> Result<ActorSystemResponse, String> {
    // Current stop is a graceful shutdown request; session_id is accepted for compatibility.
    let _ = session_id; // not used in new API
    actor_stop_session(app).await
}

#[tauri::command]
pub async fn resume_from_token_compat(app: AppHandle, resume_token: String) -> Result<ActorSystemResponse, String> {
    actor_resume_from_token(app, resume_token).await
}
