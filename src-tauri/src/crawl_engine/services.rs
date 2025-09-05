// Rust 2024 gate file for `crawl_engine::services`
// Replaces directory-level mod.rs and pins submodules explicitly.

#[path = "services/crawling_integration.rs"]
pub mod crawling_integration;
#[path = "services/crawling_planner.rs"]
pub mod crawling_planner;
#[path = "services/planning_service.rs"]
pub mod planning_service; // Phase 1: centralized planning facade
#[path = "services/data_quality_analyzer.rs"]
pub mod data_quality_analyzer;
#[path = "services/performance_optimizer.rs"]
pub mod performance_optimizer; // 🔧 Phase C: 성능 최적화 서비스
#[path = "services/real_crawling_commands.rs"]
pub mod real_crawling_commands;
#[path = "services/data_consistency_checker.rs"]
pub mod data_consistency_checker;

// Maintain prior public surface
pub use crawling_planner::CrawlingPlanner;
pub use performance_optimizer::PerformanceOptimizer;
