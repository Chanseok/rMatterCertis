//! API types (DTO) for frontend-backend communication (Rust 2024 file-based gate)

#[path = "api/frontend_api.rs"]
pub mod frontend_api;
#[cfg(any(feature = "dev-tools", debug_assertions))]
#[path = "api/dashboard_types.rs"]
pub mod dashboard_types; // 🎨 Phase C: Dashboard types (dev-only)
