pub mod crawling_planner;
pub mod crawling_integration;
pub mod real_crawling_integration;
pub mod real_crawling_commands;
pub mod performance_optimizer;  // 🔧 Phase C: 성능 최적화 서비스
pub mod data_quality_analyzer;  // 🔍 데이터 품질 분석 서비스

pub use crawling_planner::CrawlingPlanner;
pub mod data_consistency_checker;
pub use performance_optimizer::PerformanceOptimizer;
pub use real_crawling_integration::RealCrawlingIntegration;