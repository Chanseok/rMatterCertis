// Gate file for crawl_engine::stages::strategies
// Exposes the default strategy family (Rust 2024 style)

#[path = "strategies/default.rs"]
pub mod default;

pub use default::{
    DataSavingLogic, DataValidationLogic, ListPageLogic, ProductDetailLogic, StatusCheckLogic,
};
