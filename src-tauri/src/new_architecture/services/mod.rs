// Core service modules for new architecture
pub mod crawling_integration;
pub mod crawling_planner;
pub mod real_crawling_integration;

// Re-export key types
pub use crawling_planner::CrawlingPlanner;
pub use crawling_integration::CrawlingIntegrationService;
pub use real_crawling_integration::RealCrawlingStageExecutor;
