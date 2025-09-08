#![cfg(feature = "dev-tools")]
//! Actor System Monitoring (dev-only)
//!
//! Placeholder module retained for UI wiring and future tools.
//! No public #[tauri::command] functions are exposed here.

use tracing::info;

/// Returns a short, static status string for sanity checks.
/// This is dev-tools only and not registered as a Tauri command.
pub fn actor_monitoring_status_stub() -> &'static str {
    info!("actor_system_monitoring stub queried");
    "ok"
}
