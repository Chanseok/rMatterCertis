// Gate file for `crawl_engine::services` (module index)
// Explicit submodules; replaces legacy empty mod.rs

pub mod crawling_integration;
pub mod crawling_planner;
pub mod planning_service; // Phase 1: centralized planning facade
pub mod data_quality_analyzer;
pub mod performance_optimizer; // 🔧 Phase C: 성능 최적화 서비스
pub mod real_crawling_commands;
pub mod real_crawling_integration; // 🔍 데이터 품질 분석 서비스
pub mod data_consistency_checker;

// Maintain prior public surface
pub use crawling_planner::CrawlingPlanner;
pub use performance_optimizer::PerformanceOptimizer;
pub use real_crawling_integration::RealCrawlingIntegration;
