//! Removed: legacy vs stage parity smoke test. BatchActor path is retired and a single
//! Stage-only runtime is used. This file remains only to prevent accidental reintroduction
//! of the old test name in CI/scripts.

#[test]
fn parity_smoke_test_retired() {
    // No-op test: kept to avoid broken references; intentional pass.
    assert!(true);
}
