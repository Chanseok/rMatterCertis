//! API types (DTO) for frontend-backend communication (Rust 2024 file-based gate)

#[cfg(any(feature = "dev-tools", debug_assertions))]
#[path = "api/dashboard_types.rs"]
pub mod dashboard_types;
#[path = "api/frontend_api.rs"]
pub mod frontend_api; // 🎨 Phase C: Dashboard types (dev-only)
