#![allow(clippy::missing_errors_doc, clippy::unused_async)]
// Debug/logging helpers for UI-side diagnostics (dev-tools only)

use tracing::{debug, error, info, warn};

/// UI에서 보낸 로그를 백엔드 콘솔/파일로 전달합니다.
/// level: "error" | "warn" | "info" | "debug" (기본: info)
#[cfg_attr(feature = "dev-tools", tauri::command)]
pub async fn ui_debug_log(level: Option<String>, message: String) -> Result<(), String> {
    match level.as_deref() {
        Some("error") => error!("[UI] {}", message),
        Some("warn") => warn!("[UI] {}", message),
        Some("debug") => debug!("[UI] {}", message),
        _ => info!("[UI] {}", message),
    }
    Ok(())
}
