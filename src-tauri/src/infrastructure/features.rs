//! Feature flags for incremental refactoring rollout.
//!
//! Source of truth: environment variables (no config coupling for now).
//! - MC_FEATURE_HTTP_CLIENT_UNIFIED (default: false)
//! - MC_FEATURE_STAGE_EXECUTOR_TEMPLATE (deprecated, permanently enabled)
//! - MC_FEATURE_EVENTS_GENERALIZED_ONLY (default: true)
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
    match env_var(name) {
        Some(val) => match val.trim() {
            v if v.eq_ignore_ascii_case("1") || v.eq_ignore_ascii_case("true") => true,
            v if v.eq_ignore_ascii_case("0") || v.eq_ignore_ascii_case("false") => false,
            _ => default,
        },
        None => default,
    }
}

/// Use unified HTTP client implementation path
pub fn feature_http_client_unified() -> bool {
    read_flag("MC_FEATURE_HTTP_CLIENT_UNIFIED", false)
}

/// Use Stage executor template + strategy pattern path
pub fn feature_stage_executor_template() -> bool {
    // Permanently enabled. Legacy path removed.
    true
}

/// Emit only generalized events (and optionally deprecate stage-specific ones)
pub fn feature_events_generalized_only() -> bool {
    read_flag("MC_FEATURE_EVENTS_GENERALIZED_ONLY", true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        // Clear test env map
        #[allow(clippy::unwrap_used)]
        super::test_env::TEST_ENV.lock().unwrap().clear();

        assert!(!feature_http_client_unified());
        assert!(feature_stage_executor_template());
        assert!(feature_events_generalized_only());
    }

    #[test]
    fn explicit_values_parse() {
        // Set values in test env map
        let mut map = super::test_env::TEST_ENV.lock().unwrap();
        map.insert("MC_FEATURE_HTTP_CLIENT_UNIFIED".into(), "1".into());
        map.insert("MC_FEATURE_STAGE_EXECUTOR_TEMPLATE".into(), "false".into()); // ignored now
        map.insert("MC_FEATURE_EVENTS_GENERALIZED_ONLY".into(), "0".into());
        drop(map);

        assert!(feature_http_client_unified());
        assert!(feature_stage_executor_template());
        assert!(!feature_events_generalized_only());
    }
}
