//! Bin to generate TypeScript bindings without running tests.
//! Intended to be used by `scripts/generate_types.sh`.

fn main() {
    if let Err(e) = matter_certis_v2_lib::crawl_engine::ts_gen::generate_ts_bindings() {
        eprintln!("Failed to generate TS bindings: {e}");
        std::process::exit(1);
    }
}
