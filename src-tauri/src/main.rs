// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(missing_docs)]
#![allow(clippy::unnecessary_operation)]
#![allow(unused_must_use)]

fn main() {
    // On macOS, suppress noisy OS-level unified logging (CoreAnimation warnings like
    // "CATransformLayer ... changing property shadowOffset/shadowRadius") from cluttering the terminal.
    // This doesn't affect our application logs.
    // Note: macOS CoreAnimation console warnings are OS-level.
    // We avoid setting env vars here to comply with unsafe-code lint; see package.json scripts
    // (OS_ACTIVITY_MODE=disable CA_DEBUG_TRANSACTIONS=0) to suppress terminal noise when launching.
    matter_certis_v2_lib::run()
}
