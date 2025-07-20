// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(missing_docs)]
#![allow(clippy::unnecessary_operation)]
#![allow(unused_must_use)]

fn main() {
    matter_certis_v2_lib::run()
}
