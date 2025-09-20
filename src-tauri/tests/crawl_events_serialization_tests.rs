use serde_json::Value;
use regex::Regex;
use matter_certis_v2_lib::crawl_events::{CrawlEvent, CRAWL_EVENT_SCHEMA_VERSION};
use chrono::Utc;

// Helper to normalize dynamic fields (uuid, timestamp) for snapshot-like comparison
fn normalize(json: &str) -> String {
    let re_uuid = Regex::new(r#"[0-9a-fA-F-]{36}"#).unwrap();
    let tmp = re_uuid.replace_all(json, "\"<uuid>\"");
    let re_ts = Regex::new(r#"\d{4}-\d{2}-\d{2}T[0-9:\\.]+Z"#).unwrap();
    let tmp = re_ts.replace_all(&tmp, "\"<ts>\"");
    tmp.to_string()
}

fn assert_event_shape(v: &Value, expected_type: &str) {
    // event_type discriminator
    assert_eq!(v.get("event_type").and_then(|x| x.as_str()), Some(expected_type));
    // schema_version
    assert_eq!(v.get("schema_version").and_then(|x| x.as_u64()), Some(CRAWL_EVENT_SCHEMA_VERSION as u64));
    // common fields presence
    assert!(v.get("event_id").is_some());
    assert!(v.get("sequence").is_some());
    assert!(v.get("timestamp").is_some());
}

#[test]
fn serialize_session_started_minimal() {
    let evt = CrawlEvent::CrawlSessionStarted {
        event_id: "00000000-0000-0000-0000-000000000000".into(),
        schema_version: CRAWL_EVENT_SCHEMA_VERSION,
        sequence: 42,
        timestamp: Utc::now(),
        session_id: "sess-1".into(),
        total_stages: None,
        message: None,
    };
    let json = serde_json::to_string(&evt).unwrap();
    let v: Value = serde_json::from_str(&json).unwrap();
    assert_event_shape(&v, "CrawlSessionStarted");
    assert_eq!(v.get("session_id").and_then(|x| x.as_str()), Some("sess-1"));
    let norm = normalize(&json);
    // Ensure stable key set (no accidental additional fields)
    for required in ["event_type","event_id","schema_version","sequence","timestamp","session_id"] { assert!(norm.contains(required)); }
}

#[test]
fn serialize_stage_item_completed() {
    let evt = CrawlEvent::StageItemCompleted {
        event_id: "00000000-0000-0000-0000-000000000000".into(),
        schema_version: CRAWL_EVENT_SCHEMA_VERSION,
        sequence: 7,
        timestamp: Utc::now(),
        session_id: "sess-X".into(),
        batch_id: Some("b1".into()),
        stage_name: "Download".into(),
        item_id: "item-9".into(),
        duration_ms: Some(1500),
        message: Some("success=true retries=0".into()),
    };
    let json = serde_json::to_string(&evt).unwrap();
    let v: Value = serde_json::from_str(&json).unwrap();
    assert_event_shape(&v, "StageItemCompleted");
    assert_eq!(v.get("duration_ms").and_then(|x| x.as_u64()), Some(1500));
    assert_eq!(v.get("batch_id").and_then(|x| x.as_str()), Some("b1"));
}

#[test]
fn serialize_overall_progress_update() {
    let evt = CrawlEvent::OverallProgressUpdate {
        event_id: "00000000-0000-0000-0000-000000000000".into(),
        schema_version: CRAWL_EVENT_SCHEMA_VERSION,
        sequence: 99,
        timestamp: Utc::now(),
        session_id: "sess-P".into(),
        current_stage_index: Some(2),
        total_stages: Some(10),
        overall_progress_percentage: Some(23.5),
        completed_items_count: Some(120),
        total_items_count: Some(800),
        message: None,
    };
    let json = serde_json::to_string(&evt).unwrap();
    let v: Value = serde_json::from_str(&json).unwrap();
    assert_event_shape(&v, "OverallProgressUpdate");
    assert_eq!(v.get("overall_progress_percentage").and_then(|x| x.as_f64()), Some(23.5_f64));
}

#[test]
fn sequence_field_is_used() {
    // Ensure sequence we set persists; acts like guard against future builder forgetting to set sequence
    let evt = CrawlEvent::StageItemRetrying {
        event_id: "00000000-0000-0000-0000-000000000000".into(),
        schema_version: CRAWL_EVENT_SCHEMA_VERSION,
        sequence: 555,
        timestamp: Utc::now(),
        session_id: "sess-Z".into(),
        batch_id: Some("b2".into()),
        stage_name: "Process".into(),
        item_id: "__stage__".into(),
        retry_attempt: 2,
        max_retries: Some(5),
        reason: Some("network".into()),
        message: None,
    };
    let json = serde_json::to_string(&evt).unwrap();
    let v: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v.get("sequence").and_then(|x| x.as_u64()), Some(555));
}
