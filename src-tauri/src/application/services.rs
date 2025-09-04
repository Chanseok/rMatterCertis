// Compatibility shim: delegate to the directory module to avoid ambiguity
// This file exists temporarily during refactor; do not add new code here.
#![allow(unused)]
#[path = "services/mod.rs"]
pub mod services;
pub use services::*;
