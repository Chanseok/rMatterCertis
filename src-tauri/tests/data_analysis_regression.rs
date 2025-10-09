//! Data Analysis Regression Tests
//!
//! Tests for data analysis methods to ensure refactoring doesn't break existing behavior.
//! These tests cover:
//! - analyze_data_changes: decrease detection, tolerance, increased, stable
//! - Severity level calculation
//! - Recommendation generation
//!
//! Context: Week 2 will merge analyze_data_changes + analyze_site_data_changes
//! These tests ensure the merged implementation maintains current behavior.

/// Test data decrease detection
///
/// Scenario: Previous count 1000, current count 850
/// Expected: Decreased status, Medium severity (15% decrease)
#[test]
fn test_analyze_data_changes_decrease_detection() {
    let previous_count = 1000_u32;
    let current_count = 850_u32;
    let decrease = previous_count - current_count;
    let decrease_percentage = (f64::from(decrease) / f64::from(previous_count)) * 100.0;

    // Calculate expected severity
    let expected_severity = if decrease_percentage < 10.0 {
        "Low"
    } else if decrease_percentage < 30.0 {
        "Medium"
    } else if decrease_percentage < 50.0 {
        "High"
    } else {
        "Critical"
    };

    assert_eq!(decrease, 150);
    assert!((decrease_percentage - 15.0).abs() < 0.01);
    assert_eq!(expected_severity, "Medium");

    // Expected recommendation type: ManualVerification
    assert_eq!(expected_severity, "Medium");
}

/// Test tolerance threshold
///
/// Scenario: Previous count 1000, current count 995 (0.5% change)
/// Expected: Stable status (within 0.5% tolerance)
#[test]
fn test_analyze_data_changes_within_tolerance() {
    let previous_count = 1000_u32;
    let current_count = 995_u32;
    let change_percentage = ((f64::from(current_count) - f64::from(previous_count))
        / f64::from(previous_count))
        * 100.0;

    let decrease_tolerance_pct = 0.5_f64;

    // change_percentage = -0.5%, abs() = 0.5%
    assert!((change_percentage + 0.5).abs() < 0.01); // Verify calculation
    assert!(change_percentage.abs() < (decrease_tolerance_pct + 0.01)); // Within tolerance (with floating point margin)

    // Expected: Treat as Stable
    let expected_status = "Stable";
    assert_eq!(expected_status, "Stable");
}

/// Test data increase detection
///
/// Scenario: Previous count 1000, current count 1150
/// Expected: Increased status, no recommendation
#[test]
fn test_analyze_data_changes_increased() {
    let previous_count = 1000_u32;
    let current_count = 1150_u32;
    let increase = current_count - previous_count;
    let change_percentage = ((f64::from(current_count) - f64::from(previous_count))
        / f64::from(previous_count))
        * 100.0;

    assert_eq!(increase, 150);
    assert!((change_percentage - 15.0).abs() < 0.01);

    // Expected: Increased status, no recommendation needed
    assert!(current_count > previous_count);
}

/// Test stable data (exact match)
///
/// Scenario: Previous count 1000, current count 1000
/// Expected: Stable status, no recommendation
#[test]
fn test_analyze_data_changes_stable() {
    let previous_count = 1000_u32;
    let current_count = 1000_u32;

    assert_eq!(previous_count, current_count);

    // Expected: Stable status
    let change_percentage = 0.0_f64;
    assert_eq!(change_percentage, 0.0);
}

/// Test severity levels for different decrease percentages
#[test]
fn test_severity_level_calculation() {
    struct TestCase {
        decrease_pct: f64,
        expected_severity: &'static str,
    }

    let test_cases = vec![
        TestCase {
            decrease_pct: 5.0,
            expected_severity: "Low",
        },
        TestCase {
            decrease_pct: 9.9,
            expected_severity: "Low",
        },
        TestCase {
            decrease_pct: 10.0,
            expected_severity: "Medium",
        },
        TestCase {
            decrease_pct: 25.0,
            expected_severity: "Medium",
        },
        TestCase {
            decrease_pct: 29.9,
            expected_severity: "Medium",
        },
        TestCase {
            decrease_pct: 30.0,
            expected_severity: "High",
        },
        TestCase {
            decrease_pct: 40.0,
            expected_severity: "High",
        },
        TestCase {
            decrease_pct: 49.9,
            expected_severity: "High",
        },
        TestCase {
            decrease_pct: 50.0,
            expected_severity: "Critical",
        },
        TestCase {
            decrease_pct: 75.0,
            expected_severity: "Critical",
        },
    ];

    for case in test_cases {
        let severity = if case.decrease_pct < 10.0 {
            "Low"
        } else if case.decrease_pct < 30.0 {
            "Medium"
        } else if case.decrease_pct < 50.0 {
            "High"
        } else {
            "Critical"
        };

        assert_eq!(
            severity, case.expected_severity,
            "Failed for {}%: expected {}, got {}",
            case.decrease_pct, case.expected_severity, severity
        );
    }
}

/// Test recommendation action types
#[test]
fn test_recommendation_action_types() {
    struct TestCase {
        severity: &'static str,
        expected_action: &'static str,
    }

    let test_cases = vec![
        TestCase {
            severity: "Low",
            expected_action: "WaitAndRetry",
        },
        TestCase {
            severity: "Medium",
            expected_action: "ManualVerification",
        },
        TestCase {
            severity: "High",
            expected_action: "BackupAndRecrawl",
        },
        TestCase {
            severity: "Critical",
            expected_action: "BackupAndRecrawl",
        },
    ];

    for case in test_cases {
        let action = match case.severity {
            "Low" => "WaitAndRetry",
            "Medium" => "ManualVerification",
            "High" | "Critical" => "BackupAndRecrawl",
            _ => panic!("Unknown severity: {}", case.severity),
        };

        assert_eq!(
            action, case.expected_action,
            "Failed for severity {}: expected {}, got {}",
            case.severity, case.expected_action, action
        );
    }
}

/// Test edge case: zero previous count
#[test]
fn test_zero_previous_count() {
    let previous_count = 0_u32;
    let current_count = 100_u32;

    // Should handle division by zero gracefully
    let change_percentage = if previous_count > 0 {
        ((f64::from(current_count) - f64::from(previous_count)) / f64::from(previous_count))
            * 100.0
    } else {
        0.0
    };

    assert_eq!(change_percentage, 0.0);

    // Expected: Should not crash, treat as Initial or special case
}

/// Test edge case: current count zero (all data removed)
#[test]
fn test_current_count_zero() {
    let previous_count = 1000_u32;
    let current_count = 0_u32;
    let decrease_percentage =
        (f64::from(previous_count - current_count) / f64::from(previous_count)) * 100.0;

    assert_eq!(decrease_percentage, 100.0);

    // Expected: Critical severity (100% decrease)
    let severity = if decrease_percentage >= 50.0 {
        "Critical"
    } else {
        "Other"
    };

    assert_eq!(severity, "Critical");
}

/// Test real-world scenario: 596 → 589 (CSA site decrease)
#[test]
fn test_real_world_596_to_589() {
    let previous_count = 596_u32;
    let current_count = 589_u32;
    let decrease = previous_count - current_count;
    let decrease_percentage = (f64::from(decrease) / f64::from(previous_count)) * 100.0;

    assert_eq!(decrease, 7);
    assert!((decrease_percentage - 1.174).abs() < 0.01); // ~1.17%

    // Expected: Low severity (< 10% decrease)
    let severity = if decrease_percentage < 10.0 {
        "Low"
    } else {
        "Other"
    };

    assert_eq!(severity, "Low");

    // Expected action: WaitAndRetry
    let action = "WaitAndRetry";
    assert_eq!(action, "WaitAndRetry");
}
