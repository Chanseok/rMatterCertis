use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// 데이터베이스 커서 위치
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCursor {
    pub page: u32,
    pub index: u32,
}

/// 시스템 상태 페이로드 (거시적 정보)
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

/// 개별 작업 아이템 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Active,
    Retrying,
    Success,
    Error,
}

/// 원자적 작업 이벤트 (미시적 정보)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomicTaskEvent {
    pub task_id: String,
    pub batch_id: u32,
    pub stage_name: String, // "ListPageCollection", "DetailPageCollection", "DatabaseSave"
    pub status: TaskStatus,
    pub progress: f64, // 0.0 - 1.0
    pub message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// 배치 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchInfo {
    pub id: u32,
    pub status: String,
    pub progress: f64,
    pub items_total: u32,
    pub items_completed: u32,
    pub current_page: u32,
    pub pages_range: (u32, u32), // (start, end)
}

/// 스테이지 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageInfo {
    pub name: String,
    pub status: String,
    pub items_total: u32,
    pub items_completed: u32,
    pub items_active: u32,
    pub items_failed: u32,
}

/// 확장된 시스템 상태 (Live Production Line용)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveSystemState {
    pub basic_state: SystemStatePayload,
    pub current_batch: Option<BatchInfo>,
    pub stages: Vec<StageInfo>,
    pub recent_completions: Vec<AtomicTaskEvent>,
}
