// Deprecated placeholder: flattened into `src/events.rs`. File retained temporarily; will be removed.

impl CrawlEvent {
    pub fn new_sequence(sequence: u64, payload: Self) -> Self {
        // We already pass sequence externally; builder not strictly needed now.
        payload
    }
}

/// Helper factory functions (ergonomics)
pub struct CrawlEventFactory {
    sequence: std::sync::atomic::AtomicU64,
}

impl CrawlEventFactory {
    pub fn new(start: u64) -> Self { Self { sequence: std::sync::atomic::AtomicU64::new(start) } }
    fn next_seq(&self) -> u64 { self.sequence.fetch_add(1, std::sync::atomic::Ordering::SeqCst) }

    pub fn stage_item_started(&self, session_id: &str, batch_id: Option<&str>, stage_name: &str, item_id: &str) -> CrawlEvent {
        CrawlEvent::StageItemStarted {
            event_id: Uuid::new_v4().to_string(),
            schema_version: CRAWL_EVENT_SCHEMA_VERSION,
            sequence: self.next_seq(),
            timestamp: Utc::now(),
            session_id: session_id.to_string(),
            batch_id: batch_id.map(|s| s.to_string()),
            stage_name: stage_name.to_string(),
            item_id: item_id.to_string(),
            message: None,
        }
    }
}

/// Emit convenience (serializes & sends via Tauri handle)
pub fn emit_crawl_event(app: &tauri::AppHandle, evt: &CrawlEvent) -> Result<(), String> {
    app.emit("crawl_updates", evt).map_err(|e| e.to_string())
}

// Re-export legacy structs for existing code paths.
pub mod legacy;
pub use legacy::*;
