//! Tests for status JSON downshift metadata exposure
use chrono::Utc;
use matter_certis_v2_lib::new_architecture::runtime::session_registry::{session_registry, SessionEntry, SessionStatus};

#[tokio::test]
async fn status_includes_downshift_metadata_when_set() {
    let sid = format!("test_{}", Utc::now().timestamp_nanos());
    {
        let registry = session_registry();
        let mut g = registry.write().await;
        g.insert(sid.clone(), SessionEntry {
            status: SessionStatus::Running,
            pause_tx: tokio::sync::watch::channel(false).0,
            started_at: Utc::now(),
            completed_at: None,
            total_pages_planned: 10,
            processed_pages: 5,
            total_batches_planned: 1,
            completed_batches: 0,
            batch_size: 5,
            concurrency_limit: 4,
            last_error: None,
            error_count: 0,
            resume_token: None,
            remaining_page_slots: Some(vec![1,2,3]),
            plan_hash: Some("hash".into()),
            removal_deadline: None,
            failed_emitted: false,
            retries_per_page: std::collections::HashMap::new(),
            failed_pages: vec![],
            retrying_pages: vec![],
            product_list_max_retries: 1,
            error_type_stats: std::collections::HashMap::new(),
            detail_tasks_total: 10,
            detail_tasks_completed: 2,
            detail_tasks_failed: 3,
            detail_retry_counts: std::collections::HashMap::new(),
            detail_retries_total: 0,
            detail_retry_histogram: std::collections::HashMap::new(),
            remaining_detail_ids: Some(vec!["a".into(),"b".into()]),
            detail_failed_ids: vec![],
            page_failure_threshold: 50,
            detail_failure_threshold: 25,
            detail_downshifted: true,
            detail_downshift_timestamp: Some(Utc::now()),
            detail_downshift_old_limit: Some(8),
            detail_downshift_new_limit: Some(4),
            detail_downshift_trigger: Some("fail_rate>0.30".into()),
        });
    }
    let payload = matter_certis_v2_lib::commands::actor_system_commands::test_build_session_status_payload(&sid).await.expect("payload");
    let details = &payload["details"];
    assert!(details["downshifted"].as_bool().unwrap());
    assert!(details["downshift_meta"]["timestamp"].is_string());
    assert_eq!(details["downshift_meta"]["old_limit"].as_u64().unwrap(), 8);
    assert_eq!(details["downshift_meta"]["new_limit"].as_u64().unwrap(), 4);
    assert_eq!(details["downshift_meta"]["trigger"].as_str().unwrap(), "fail_rate>0.30");
}
