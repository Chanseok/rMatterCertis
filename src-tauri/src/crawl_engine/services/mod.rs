// Gate file for `crawl_engine::services` (module index)
// Explicit submodules; replaces legacy empty mod.rs

pub mod crawling_integration;
pub mod crawling_planner;
pub mod planning_service; // Phase 1: centralized planning facade
pub mod data_quality_analyzer;
pub mod performance_optimizer; // 🔧 Phase C: 성능 최적화 서비스
pub mod real_crawling_commands;
pub mod data_consistency_checker;

// Maintain prior public surface
pub use crawling_planner::CrawlingPlanner;
pub use performance_optimizer::PerformanceOptimizer;
