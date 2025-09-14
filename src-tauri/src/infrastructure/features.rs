//! Feature flags for incremental refactoring rollout.
//!
//! Source of truth: environment variables (no config coupling for now).
//! - `MC_FEATURE_HTTP_CLIENT_UNIFIED` (default: false)
//! - `MC_FEATURE_STAGE_EXECUTOR_TEMPLATE` (deprecated, permanently enabled)
//! - Legacy event emission has been removed. Backend always emits unified `actor-event` only.
//! - `MC_FEATURE_EMIT_PAGETASK_LEGACY` (removed) — `PageTask`* variants were deleted; always use `PageLifecycle`
//!
//!
//! Values: "1"/"true" enable, "0"/"false" disable (case-insensitive)

// Environment variable accessor (production vs. tests)
#[cfg(not(test))]
fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

#[cfg(test)]
mod test_env {
    use once_cell::sync::Lazy;
    use std::collections::HashMap;
    use std::sync::Mutex;

    // Shared map to simulate environment in tests without touching real process env
    pub static TEST_ENV: Lazy<Mutex<HashMap<String, String>>> =
        Lazy::new(|| Mutex::new(HashMap::new()));

    pub fn get(name: &str) -> Option<String> {
        TEST_ENV.lock().unwrap().get(name).cloned()
    }
}

#[cfg(test)]
fn env_var(name: &str) -> Option<String> {
    test_env::get(name)
}

fn read_flag(name: &str, default: bool) -> bool {
    env_var(name).map_or(default, |val| match val.trim() {
        v if v.eq_ignore_ascii_case("1") || v.eq_ignore_ascii_case("true") => true,
        v if v.eq_ignore_ascii_case("0") || v.eq_ignore_ascii_case("false") => false,
        _ => default,
    })
}

/// Use unified HTTP client implementation path
#[must_use]
pub fn feature_http_client_unified() -> bool {
    read_flag("MC_FEATURE_HTTP_CLIENT_UNIFIED", false)
}

/// Use Stage executor template + strategy pattern path.
/// NOTE: Legacy (non-template) path has been removed; keep this permanently true.
#[must_use]
pub const fn feature_stage_executor_template() -> bool { true }

// Unified event emission is now permanent. No feature flag remains for legacy/generalized events.

// Phase* events removed; obsolete flag deleted.

// PageTask* legacy emission removed. Always emit PageLifecycle only.

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use std::sync::Mutex;

    // Serialize tests in this module to avoid races on TEST_ENV
    static TEST_GUARD: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    #[test]
    fn defaults_are_sane() {
        let _g = TEST_GUARD.lock().unwrap();
        // Clear test env map
        #[allow(clippy::unwrap_used)]
        super::test_env::TEST_ENV.lock().unwrap().clear();

        assert!(!feature_http_client_unified());
    assert!(feature_stage_executor_template());
        // unified actor-event emission is always on (no flag)
        // pagetask legacy removed
    }

    #[test]
    fn explicit_values_parse() {
        let _g = TEST_GUARD.lock().unwrap();
        // Set values in test env map
        let mut map = super::test_env::TEST_ENV.lock().unwrap();
        map.insert("MC_FEATURE_HTTP_CLIENT_UNIFIED".into(), "1".into());
    map.insert("MC_FEATURE_STAGE_EXECUTOR_TEMPLATE".into(), "false".into()); // ignored
        // unified actor-event emission has no flag anymore
        // pagetask legacy removed
        drop(map);

        assert!(feature_http_client_unified());
    assert!(feature_stage_executor_template());
        // unified actor-event emission is always on (no flag)
        // pagetask legacy removed
    }
}
