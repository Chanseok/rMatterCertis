use std::sync::{OnceLock, Arc};
use crate::application::AppState;

// Global accessor (best-effort) for places that cannot receive State<'_, AppState> easily.
// Only used for lock error counter increment on retry exhaustion.
static APP_STATE: OnceLock<Arc<AppState>> = OnceLock::new();

pub fn set_app_state(state: Arc<AppState>) {
    let _ = APP_STATE.set(state); // ignore error if already set
}

pub fn get_app_state() -> Option<Arc<AppState>> {
    APP_STATE.get().cloned()
}
