//! AppEvent -> CrawlEvent mapping layer (phase 1)
//! Additive-only; not all AppEvent variants are mapped initially.

use chrono::Utc;
use crate::crawl_events::{CrawlEvent, CRAWL_EVENT_SCHEMA_VERSION};
use crate::crawl_engine::actors::types::AppEvent;
use uuid::Uuid;

/// Attempt to map an `AppEvent` into a structured `CrawlEvent`.
/// Returns None when the event is currently not represented in the structured schema.
pub fn map_app_event(ev: &AppEvent, sequence: u64) -> Option<CrawlEvent> {
    match ev {
        AppEvent::SessionStarted { session_id, .. } => Some(CrawlEvent::CrawlSessionStarted {
            event_id: Uuid::new_v4().to_string(),
            schema_version: CRAWL_EVENT_SCHEMA_VERSION,
            sequence,
            timestamp: Utc::now(),
            session_id: session_id.clone(),
            total_stages: None,
            message: None,
        }),
        AppEvent::SessionCompleted { session_id, summary: _summary, .. } => Some(CrawlEvent::CrawlSessionCompleted {
            event_id: Uuid::new_v4().to_string(),
            schema_version: CRAWL_EVENT_SCHEMA_VERSION,
            sequence,
            timestamp: Utc::now(),
            session_id: session_id.clone(),
            total_duration_ms: None, // TODO: derive overall duration from external tracker
            successful_items_count: None,
            failed_items_count: None,
            message: None,
        }),
        AppEvent::SessionFailed { session_id, error, final_failure, .. } => Some(CrawlEvent::CrawlSessionFailed {
            event_id: Uuid::new_v4().to_string(),
            schema_version: CRAWL_EVENT_SCHEMA_VERSION,
            sequence,
            timestamp: Utc::now(),
            session_id: session_id.clone(),
            error_code: None,
            error_category: Some("SessionFailure".into()),
            error_message: error.clone(),
            retriable: Some(!final_failure),
            message: None,
        }),
        AppEvent::StageStarted { session_id, batch_id, stage_type, items_count, .. } => Some(CrawlEvent::StageStarted {
            event_id: Uuid::new_v4().to_string(),
            schema_version: CRAWL_EVENT_SCHEMA_VERSION,
            sequence,
            timestamp: Utc::now(),
            session_id: session_id.clone(),
            batch_id: batch_id.clone(),
            stage_name: format!("{:?}", stage_type),
            stage_index: 0, // TODO: derive real index from orchestration state
            total_items_in_stage: Some(*items_count),
            message: None,
        }),
        AppEvent::StageItemStarted { session_id, batch_id, stage_type, item_id, .. } => Some(CrawlEvent::StageItemStarted {
            event_id: Uuid::new_v4().to_string(),
            schema_version: CRAWL_EVENT_SCHEMA_VERSION,
            sequence,
            timestamp: Utc::now(),
            session_id: session_id.clone(),
            batch_id: batch_id.clone(),
            stage_name: format!("{:?}", stage_type),
            item_id: item_id.clone(),
            message: None,
        }),
        AppEvent::StageItemCompleted { session_id, batch_id, stage_type, item_id, duration_ms, success, retry_count, .. } => Some(CrawlEvent::StageItemCompleted {
            event_id: Uuid::new_v4().to_string(),
            schema_version: CRAWL_EVENT_SCHEMA_VERSION,
            sequence,
            timestamp: Utc::now(),
            session_id: session_id.clone(),
            batch_id: batch_id.clone(),
            stage_name: format!("{:?}", stage_type),
            item_id: item_id.clone(),
            duration_ms: Some(*duration_ms),
            message: Some(format!("success={} retries={}", success, retry_count)),
        }),
        AppEvent::StageFailed { session_id, batch_id, stage_type, error, .. } => Some(CrawlEvent::StageItemFailed { // reuse StageItemFailed for stage-level failure (could add StageFailed variant later)
            event_id: Uuid::new_v4().to_string(),
            schema_version: CRAWL_EVENT_SCHEMA_VERSION,
            sequence,
            timestamp: Utc::now(),
            session_id: session_id.clone(),
            batch_id: batch_id.clone(),
            stage_name: format!("{:?}", stage_type),
            item_id: "__stage__".into(),
            error_code: None,
            error_category: Some("StageFailure".into()),
            error_message: error.clone(),
            retry_possible: None,
            message: None,
        }),
        AppEvent::StageRetrying { session_id, batch_id, stage_type, attempt, max_attempts, reason, .. } => Some(CrawlEvent::StageItemRetrying {
            event_id: Uuid::new_v4().to_string(),
            schema_version: CRAWL_EVENT_SCHEMA_VERSION,
            sequence,
            timestamp: Utc::now(),
            session_id: session_id.clone(),
            batch_id: batch_id.clone(),
            stage_name: format!("{:?}", stage_type),
            item_id: "__stage__".into(),
            retry_attempt: *attempt,
            max_retries: Some(*max_attempts),
            reason: reason.clone(),
            message: None,
        }),
        AppEvent::Progress { session_id, percentage, current_step, total_steps, .. } => Some(CrawlEvent::OverallProgressUpdate {
            event_id: Uuid::new_v4().to_string(),
            schema_version: CRAWL_EVENT_SCHEMA_VERSION,
            sequence,
            timestamp: Utc::now(),
            session_id: session_id.clone(),
            current_stage_index: Some(*current_step),
            total_stages: Some(*total_steps),
            overall_progress_percentage: Some(*percentage as f32),
            completed_items_count: None,
            total_items_count: None,
            message: None,
        }),
        _ => None,
    }
}
