//! Minimal progress feed adapter for UI (Phase 2 skeleton)
//! Consumes AppEvent stream (broadcast receiver) and reduces to compact + dynamic models.

use crate::new_architecture::actors::types::AppEvent;

#[derive(Debug, Clone)]
pub struct CompactProgress {
    pub session_id: String,
    pub percentage: f64,
    pub current_step: u32,
    pub total_steps: u32,
    pub last_message: String,
}

#[derive(Debug, Default)]
pub struct ProgressReducer {
    compact: Option<CompactProgress>,
}

impl ProgressReducer {
    pub fn new() -> Self { Self { compact: None } }
    pub fn apply(&mut self, ev: &AppEvent) {
        match ev {
            AppEvent::Progress { session_id, current_step, total_steps, message, percentage, .. } => {
                self.compact = Some(CompactProgress { session_id: session_id.clone(), percentage: *percentage, current_step: *current_step, total_steps: *total_steps, last_message: message.clone() });
            }
            _ => {}
        }
    }
    pub fn snapshot(&self) -> Option<CompactProgress> { self.compact.clone() }
}
