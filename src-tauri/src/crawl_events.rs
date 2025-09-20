//! Unified structured CrawlEvent schema (v1) + legacy state structs (flat module)
//! Modern Rust 2024: avoid `mod.rs` when a flat file suffices.
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use tauri::Emitter; // for AppHandle.emit
use crate::metrics::{inc_emitted, inc_emit_fail};

pub const CRAWL_EVENT_SCHEMA_VERSION: u8 = 1;

#[derive(Serialize, Debug, Clone)]
#[serde(tag = "event_type", rename_all = "PascalCase")]
pub enum CrawlEvent {
    CrawlSessionStarted { event_id: String, schema_version: u8, sequence: u64, timestamp: DateTime<Utc>, session_id: String, total_stages: Option<u16>, message: Option<String> },
    CrawlSessionCompleted { event_id: String, schema_version: u8, sequence: u64, timestamp: DateTime<Utc>, session_id: String, total_duration_ms: Option<u64>, successful_items_count: Option<u64>, failed_items_count: Option<u64>, message: Option<String> },
    CrawlSessionFailed { event_id: String, schema_version: u8, sequence: u64, timestamp: DateTime<Utc>, session_id: String, error_code: Option<String>, error_category: Option<String>, error_message: String, retriable: Option<bool>, message: Option<String> },
    StageStarted { event_id: String, schema_version: u8, sequence: u64, timestamp: DateTime<Utc>, session_id: String, batch_id: Option<String>, stage_name: String, stage_index: u32, total_items_in_stage: Option<u32>, message: Option<String> },
    StageProgress { event_id: String, schema_version: u8, sequence: u64, timestamp: DateTime<Utc>, session_id: String, batch_id: Option<String>, stage_name: String, stage_index: u32, current_item_index: Option<u32>, total_items_in_stage: Option<u32>, progress_percentage: Option<f32>, message: Option<String> },
    StageItemStarted { event_id: String, schema_version: u8, sequence: u64, timestamp: DateTime<Utc>, session_id: String, batch_id: Option<String>, stage_name: String, item_id: String, message: Option<String> },
    StageItemCompleted { event_id: String, schema_version: u8, sequence: u64, timestamp: DateTime<Utc>, session_id: String, batch_id: Option<String>, stage_name: String, item_id: String, duration_ms: Option<u64>, message: Option<String> },
    StageItemFailed { event_id: String, schema_version: u8, sequence: u64, timestamp: DateTime<Utc>, session_id: String, batch_id: Option<String>, stage_name: String, item_id: String, error_code: Option<String>, error_category: Option<String>, error_message: String, retry_possible: Option<bool>, message: Option<String> },
    StageItemRetrying { event_id: String, schema_version: u8, sequence: u64, timestamp: DateTime<Utc>, session_id: String, batch_id: Option<String>, stage_name: String, item_id: String, retry_attempt: u32, max_retries: Option<u32>, reason: Option<String>, message: Option<String> },
    OverallProgressUpdate { event_id: String, schema_version: u8, sequence: u64, timestamp: DateTime<Utc>, session_id: String, current_stage_index: Option<u32>, total_stages: Option<u32>, overall_progress_percentage: Option<f32>, completed_items_count: Option<u64>, total_items_count: Option<u64>, message: Option<String> },
}

impl CrawlEvent { pub fn new_sequence(sequence: u64, payload: Self) -> Self { let _ = sequence; payload } }

pub struct CrawlEventFactory { sequence: std::sync::atomic::AtomicU64 }
impl CrawlEventFactory {
    pub fn new(start: u64) -> Self { Self { sequence: std::sync::atomic::AtomicU64::new(start) } }
    fn next_seq(&self) -> u64 { self.sequence.fetch_add(1, std::sync::atomic::Ordering::SeqCst) }
    pub fn stage_item_started(&self, session_id: &str, batch_id: Option<&str>, stage_name: &str, item_id: &str) -> CrawlEvent { CrawlEvent::StageItemStarted { event_id: Uuid::new_v4().to_string(), schema_version: CRAWL_EVENT_SCHEMA_VERSION, sequence: self.next_seq(), timestamp: Utc::now(), session_id: session_id.to_string(), batch_id: batch_id.map(|s| s.to_string()), stage_name: stage_name.to_string(), item_id: item_id.to_string(), message: None } }
}

pub fn emit_crawl_event(app: &tauri::AppHandle, evt: &CrawlEvent) -> Result<(), String> {
    let etype = match evt { CrawlEvent::CrawlSessionStarted {..} => "CrawlSessionStarted", CrawlEvent::CrawlSessionCompleted {..} => "CrawlSessionCompleted", CrawlEvent::CrawlSessionFailed {..} => "CrawlSessionFailed", CrawlEvent::StageStarted {..} => "StageStarted", CrawlEvent::StageProgress {..} => "StageProgress", CrawlEvent::StageItemStarted {..} => "StageItemStarted", CrawlEvent::StageItemCompleted {..} => "StageItemCompleted", CrawlEvent::StageItemFailed {..} => "StageItemFailed", CrawlEvent::StageItemRetrying {..} => "StageItemRetrying", CrawlEvent::OverallProgressUpdate {..} => "OverallProgressUpdate" };
    match app.emit("crawl_updates", evt) {
        Ok(_) => { inc_emitted(etype); Ok(()) }
        Err(e) => { inc_emit_fail(etype, "emit"); Err(e.to_string()) }
    }
}

// ---- Legacy state structs (kept for compatibility) ----
#[derive(Debug, Clone, Serialize, Deserialize)] pub struct DbCursor { pub page: u32, pub index: u32 }
#[derive(Debug, Clone, Serialize, Deserialize)] pub struct SystemStatePayload { pub is_running: bool, pub total_pages: u32, pub db_total_products: u64, pub last_db_cursor: Option<DbCursor>, pub session_target_items: u32, pub session_collected_items: u32, pub session_eta_seconds: u32, pub items_per_minute: f64, pub current_stage: String, pub analyzed_at: Option<DateTime<Utc>> }
#[derive(Debug, Clone, Serialize, Deserialize)] pub enum TaskStatus { Pending, Active, Retrying, Success, Error }
#[derive(Debug, Clone, Serialize, Deserialize)] pub struct AtomicTaskEvent { pub task_id: String, pub batch_id: u32, pub stage_name: String, pub status: TaskStatus, pub progress: f64, pub message: Option<String>, pub timestamp: DateTime<Utc> }
#[derive(Debug, Clone, Serialize, Deserialize)] pub struct BatchInfo { pub id: u32, pub status: String, pub progress: f64, pub items_total: u32, pub items_completed: u32, pub current_page: u32, pub pages_range: (u32, u32) }
#[derive(Debug, Clone, Serialize, Deserialize)] pub struct StageInfo { pub name: String, pub status: String, pub items_total: u32, pub items_completed: u32, pub items_active: u32, pub items_failed: u32 }
#[derive(Debug, Clone, Serialize, Deserialize)] pub struct LiveSystemState { pub basic_state: SystemStatePayload, pub current_batch: Option<BatchInfo>, pub stages: Vec<StageInfo>, pub recent_completions: Vec<AtomicTaskEvent> }
