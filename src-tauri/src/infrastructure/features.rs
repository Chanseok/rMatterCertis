//! Feature flags for incremental refactoring rollout.
//!
//! Source of truth: environment variables (no config coupling for now).
//! - MC_FEATURE_HTTP_CLIENT_UNIFIED (default: false)
//! - MC_FEATURE_STAGE_EXECUTOR_TEMPLATE (default: false)
//! - MC_FEATURE_EVENTS_GENERALIZED_ONLY (default: true)
//!
//! Values: "1"/"true" enable, "0"/"false" disable (case-insensitive)

fn read_flag(name: &str, default: bool) -> bool {
    match std::env::var(name) {
        Ok(val) => match val.trim() {
            v if v.eq_ignore_ascii_case("1") || v.eq_ignore_ascii_case("true") => true,
            v if v.eq_ignore_ascii_case("0") || v.eq_ignore_ascii_case("false") => false,
            _ => default,
        },
        Err(_) => default,
    }
}

/// Use unified HTTP client implementation path
pub fn feature_http_client_unified() -> bool {
    read_flag("MC_FEATURE_HTTP_CLIENT_UNIFIED", false)
}

/// Use Stage executor template + strategy pattern path
pub fn feature_stage_executor_template() -> bool {
    read_flag("MC_FEATURE_STAGE_EXECUTOR_TEMPLATE", false)
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
        // Ensure defaults when env not set
        std::env::remove_var("MC_FEATURE_HTTP_CLIENT_UNIFIED");
        std::env::remove_var("MC_FEATURE_STAGE_EXECUTOR_TEMPLATE");
        std::env::remove_var("MC_FEATURE_EVENTS_GENERALIZED_ONLY");

        assert!(!feature_http_client_unified());
        assert!(!feature_stage_executor_template());
        assert!(feature_events_generalized_only());
    }
}
