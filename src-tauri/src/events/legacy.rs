//! Legacy event/state structs kept for backward compatibility.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCursor {
    pub page: u32,
    pub index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatePayload {
    pub is_running: bool,
    pub total_pages: u32,
    pub db_total_products: u64,
    pub last_db_cursor: Option<DbCursor>,
    pub session_target_items: u32,
    pub session_collected_items: u32,
    pub session_eta_seconds: u32,
    pub items_per_minute: f64,
    pub current_stage: String,
    pub analyzed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Active,
    Retrying,
    Success,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomicTaskEvent {
    pub task_id: String,
    pub batch_id: u32,
    pub stage_name: String,
    pub status: TaskStatus,
    pub progress: f64,
    pub message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchInfo {
    pub id: u32,
    pub status: String,
    pub progress: f64,
    pub items_total: u32,
    pub items_completed: u32,
    pub current_page: u32,
    pub pages_range: (u32, u32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageInfo {
    pub name: String,
    pub status: String,
    pub items_total: u32,
    pub items_completed: u32,
    pub items_active: u32,
    pub items_failed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveSystemState {
    pub basic_state: SystemStatePayload,
    pub current_batch: Option<BatchInfo>,
    pub stages: Vec<StageInfo>,
    pub recent_completions: Vec<AtomicTaskEvent>,
}
