use matter_certis_v2_lib::metrics::{inc_emitted, inc_emit_fail, inc_throttled, EVENT_EMITTED_TOTAL, EVENT_EMIT_FAIL_TOTAL, EVENT_THROTTLED_TOTAL};

// Basic smoke test ensuring counters increment and label cardinality works.
#[test]
fn test_metrics_counters_increment() {
    // Increment different types
    inc_emitted("StageProgress");
    inc_emitted("StageProgress");
    inc_emitted("OverallProgressUpdate");

    inc_emit_fail("StageProgress", "emit_error");
    inc_emit_fail("StageProgress", "emit_error");
    inc_emit_fail("OverallProgressUpdate", "throttled");

    inc_throttled("StageProgress");
    inc_throttled("StageProgress");
    inc_throttled("OverallProgressUpdate");

    // Gather values
    let emitted_stage = EVENT_EMITTED_TOTAL.with_label_values(&["StageProgress"]).get();
    let emitted_overall = EVENT_EMITTED_TOTAL.with_label_values(&["OverallProgressUpdate"]).get();
    let fail_stage_emit = EVENT_EMIT_FAIL_TOTAL.with_label_values(&["StageProgress", "emit_error"]).get();
    let fail_overall_throttle = EVENT_EMIT_FAIL_TOTAL.with_label_values(&["OverallProgressUpdate", "throttled"]).get();
    let throttled_stage = EVENT_THROTTLED_TOTAL.with_label_values(&["StageProgress"]).get();
    let throttled_overall = EVENT_THROTTLED_TOTAL.with_label_values(&["OverallProgressUpdate"]).get();

    assert!(emitted_stage >= 2, "expected at least 2 StageProgress emitted, got {}", emitted_stage);
    assert!(emitted_overall >= 1, "expected at least 1 OverallProgressUpdate emitted, got {}", emitted_overall);
    assert!(fail_stage_emit >= 2, "expected at least 2 fail StageProgress emit_error, got {}", fail_stage_emit);
    assert!(fail_overall_throttle >= 1, "expected at least 1 fail OverallProgressUpdate throttled, got {}", fail_overall_throttle);
    assert!(throttled_stage >= 2, "expected at least 2 throttled StageProgress, got {}", throttled_stage);
    assert!(throttled_overall >= 1, "expected at least 1 throttled OverallProgressUpdate, got {}", throttled_overall);
}
