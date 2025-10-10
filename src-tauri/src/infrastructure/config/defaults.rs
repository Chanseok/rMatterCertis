//! Default constants for the application
//!
//! This module contains all hard-coded default values used throughout the application.
//! Centralizing these values here ensures consistency and makes them easy to maintain.

/// Default number of products displayed per page on the CSA-IoT website
pub const DEFAULT_PRODUCTS_PER_PAGE: u32 = 12;

/// Maximum number of results to return for analytics queries
/// This prevents excessive memory usage and response times
pub const MAX_ANALYTICS_QUERY_LIMIT: u32 = 200;

/// Maximum number of consecutive empty page checks during site discovery
/// After this many failures, the crawling is considered fatal
pub const MAX_CONSECUTIVE_EMPTY_PAGE_CHECKS: u32 = 12;

/// Step size (in pages) for downward search when discovering the last page
/// Larger values are faster but might skip over valid pages
pub const PAGE_SEARCH_STEP: u32 = 5;

/// Maximum number of search attempts when discovering the total page count
pub const MAX_PAGE_SEARCH_ATTEMPTS: u32 = 10;

/// Default retry count for HTTP requests
pub const DEFAULT_HTTP_RETRY_COUNT: u32 = 3;

/// Default timeout for HTTP requests (in seconds)
pub const DEFAULT_HTTP_TIMEOUT_SECS: u64 = 30;

/// Cache TTL for site analysis results (in minutes)
pub const DEFAULT_SITE_ANALYSIS_CACHE_TTL_MINUTES: u64 = 5;

// User configuration defaults
/// Default delay between requests in milliseconds
pub const REQUEST_DELAY_MS: u64 = 1000;

/// Default maximum concurrent requests
pub const MAX_CONCURRENT_REQUESTS: u32 = 3;

/// Default retry attempts for failed requests
pub const RETRY_ATTEMPTS: u32 = 3;

/// Default retry delay in milliseconds (for advanced config)
pub const RETRY_DELAY_MS: u64 = 2000;

/// Default starting page number for last page search
/// This is an initial guess - the app will learn and update this value
pub const LAST_PAGE_SEARCH_START: u32 = 100;

/// Default maximum search attempts
pub const MAX_SEARCH_ATTEMPTS: u32 = 10;

/// Default request timeout in seconds
pub const REQUEST_TIMEOUT_SECONDS: u64 = 30;

/// Default maximum requests per second for HTTP client
pub const MAX_REQUESTS_PER_SECOND: u32 = 50;

/// Default user agent string for HTTP requests
pub const USER_AGENT: &str = "matter-certis-v2/1.0 (Research Tool; +https://github.com/your-repo)";

/// Default follow redirects setting
pub const FOLLOW_REDIRECTS: bool = true;

// Orchestrator configuration defaults
/// Default scheduler interval in milliseconds
pub const SCHEDULER_INTERVAL_MS: u64 = 100;

/// Default shutdown timeout in seconds
pub const SHUTDOWN_TIMEOUT_SECONDS: u64 = 30;

/// Default stats interval in seconds
pub const STATS_INTERVAL_SECONDS: u64 = 10;

/// Default maximum retries for orchestrator
pub const MAX_RETRIES: u32 = 3;

/// Default worker retry delay in milliseconds
pub const WORKER_RETRY_DELAY_MS: u64 = 1000;

/// Default backpressure threshold
pub const BACKPRESSURE_THRESHOLD: usize = 1000;

// Worker configuration defaults
/// Default maximum concurrent requests for list page fetcher - IMPROVED from 5 to 12
pub const LIST_PAGE_MAX_CONCURRENT: usize = 12;

/// Default maximum concurrent requests for product detail fetcher  
pub const PRODUCT_DETAIL_MAX_CONCURRENT: usize = 10;

/// Default batch size for database write operations (records per transaction)
pub const DB_BATCH_SIZE: usize = 100;

/// Default maximum concurrency for database operations
pub const DB_MAX_CONCURRENCY: usize = 10;

// Crawling configuration defaults
/// Default page range limit - Restored to 100 for intelligent crawling
pub const PAGE_RANGE_LIMIT: u32 = 100;

/// Default product list retry count
pub const PRODUCT_LIST_RETRY_COUNT: u32 = 3;

/// Default product detail retry count
pub const PRODUCT_DETAIL_RETRY_COUNT: u32 = 3;

/// Default auto add to local database
pub const AUTO_ADD_TO_LOCAL_DB: bool = true;

// Batch configuration defaults
/// Default batch size for crawling operations (products per batch)
pub const BATCH_SIZE: u32 = 50;

/// Default batch delay in milliseconds (delay between batches)
pub const BATCH_DELAY_MS: u64 = 100;

/// Default enable batch processing
pub const ENABLE_BATCH_PROCESSING: bool = true;

/// Default batch retry limit
pub const BATCH_RETRY_LIMIT: u32 = 3;

// Log configuration defaults
/// Default log level
pub const LOG_LEVEL: &str = "info";

/// Default JSON format setting
pub const LOG_JSON_FORMAT: bool = false;

/// Default console output setting
pub const LOG_CONSOLE_OUTPUT: bool = true;

/// Default file output setting
pub const LOG_FILE_OUTPUT: bool = true;

/// Default separate frontend/backend logs setting
pub const LOG_SEPARATE_FRONTEND_BACKEND: bool = false;

/// Default log file naming strategy
pub const LOG_FILE_NAMING_STRATEGY: &str = "unified";

/// Default maximum log file size in MB
pub const LOG_MAX_FILE_SIZE_MB: u64 = 10;

/// Default maximum log files to keep
pub const LOG_MAX_FILES: u32 = 5;

/// Default auto cleanup logs setting
pub const LOG_AUTO_CLEANUP: bool = true;

/// Default keep only latest setting
pub const LOG_KEEP_ONLY_LATEST: bool = false;

/// Default CSS selectors for finding products
pub const PRODUCT_SELECTORS: &[&str] = &[
    "div.post-feed article.type-product", // 정확한 제품 selector
    "article.type-product",               // 백업 selector
    ".product",                           // 일반적인 제품 selector
];
