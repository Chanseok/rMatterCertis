//! Test-only utilities for crawl_engine module
//! Minimal stub to satisfy cfg(test) module inclusion. Extend as needed.

#[allow(dead_code)]
pub fn init_tracing_for_tests() {
    let _ = tracing_subscriber::fmt::try_init();
}
