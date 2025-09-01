use tauri::AppHandle;
use tracing::info;

/// Lightweight UI debug logger to verify FE->BE invoke path and button presses.
/// # Errors
/// Returns an error string if logging fails (currently infallible).
#[tauri::command(async)]
pub async fn ui_debug_log(app: AppHandle, message: String) -> Result<(), String> {
    let _ = app; // reserved for future enrichment (e.g., event emission)
    info!(target = "ui_debug", "UI: {}", message);
    Ok(())
}
