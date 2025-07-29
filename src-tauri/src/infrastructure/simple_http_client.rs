//! HTTP client for web crawling with rate limiting and error handling
//! 
//! This module provides a configurable HTTP client optimized for web crawling
//! with built-in retry logic, rate limiting, and user agent management.

use anyhow::{anyhow, Result};
use reqwest::{Client, ClientBuilder, Response};
use scraper::Html;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::time::{sleep, Instant, interval};
use tokio::sync::{Mutex, Semaphore};
use tracing::{debug, info, warn, error};
use crate::infrastructure::config::WorkerConfig;

/// Configuration for HTTP client behavior
#[derive(Debug, Clone)]
pub struct HttpClientConfig {
    /// Maximum requests per second to avoid overwhelming servers
    pub max_requests_per_second: u32,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Maximum number of retries for failed requests
    pub max_retries: u32,
    /// User agent string
    pub user_agent: String,
    /// Whether to follow redirects
    pub follow_redirects: bool,
}

impl HttpClientConfig {
    /// Create HttpClientConfig from WorkerConfig
    pub fn from_worker_config(worker_config: &WorkerConfig) -> Self {
        Self {
            max_requests_per_second: worker_config.max_requests_per_second,
            timeout_seconds: worker_config.request_timeout_seconds,
            max_retries: worker_config.max_retries,
            user_agent: worker_config.user_agent.clone(),
            follow_redirects: worker_config.follow_redirects,
        }
    }
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            max_requests_per_second: 2,
            timeout_seconds: 30,
            max_retries: 3,
            user_agent: "matter-certis-v2/1.0 (Research Tool; +https://github.com/your-repo)".to_string(),
            follow_redirects: true,
        }
    }
}

/// Global rate limiter shared across all HttpClient instances
/// Uses token bucket algorithm for true concurrent rate limiting
#[derive(Debug)]
struct GlobalRateLimiter {
    /// Semaphore representing available tokens (permits per second)
    semaphore: Arc<Semaphore>,
    /// Current rate limit setting
    current_rate: Arc<Mutex<u32>>,
    /// Token refill task handle
    refill_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

/// Truly global rate limiter instance (singleton)
static GLOBAL_RATE_LIMITER: OnceLock<GlobalRateLimiter> = OnceLock::new();

impl GlobalRateLimiter {
    fn get_instance() -> &'static GlobalRateLimiter {
        GLOBAL_RATE_LIMITER.get_or_init(|| {
            let initial_rate = 50; // Default 50 RPS
            let semaphore = Arc::new(Semaphore::new(initial_rate as usize));
            let current_rate = Arc::new(Mutex::new(initial_rate));
            
            GlobalRateLimiter {
                semaphore: semaphore.clone(),
                current_rate: current_rate.clone(),
                refill_handle: Arc::new(Mutex::new(None)),
            }
        })
    }
    
    async fn update_rate_limit(&self, max_requests_per_second: u32) {
        let mut current_rate = self.current_rate.lock().await;
        
        if *current_rate != max_requests_per_second {
            *current_rate = max_requests_per_second;
            debug!("Updated global rate limit to {} RPS", max_requests_per_second);
            
            // Restart token refill task with new rate
            self.start_refill_task(max_requests_per_second).await;
        }
    }
    
    async fn start_refill_task(&self, rate: u32) {
        let mut handle = self.refill_handle.lock().await;
        
        // Stop existing task
        if let Some(old_handle) = handle.take() {
            old_handle.abort();
        }
        
        if rate == 0 {
            return; // No rate limiting
        }
        
        let semaphore = self.semaphore.clone();
        let refill_interval = Duration::from_millis(1000 / rate as u64);
        
        let new_handle = tokio::spawn(async move {
            let mut interval = interval(refill_interval);
            loop {
                interval.tick().await;
                // Add one permit (token) to the bucket
                semaphore.add_permits(1);
            }
        });
        
        *handle = Some(new_handle);
    }
    
    async fn apply_rate_limit(&self, max_requests_per_second: u32) {
        // Update rate limit if needed
        self.update_rate_limit(max_requests_per_second).await;
        
        if max_requests_per_second == 0 {
            return; // No rate limiting
        }
        
        // Acquire a token (permit) from the bucket
        // This will wait if no tokens are available
        let _permit = self.semaphore.acquire().await.unwrap();
        debug!("Token acquired for HTTP request (rate: {} RPS)", max_requests_per_second);
        
        // Permit is automatically released when _permit goes out of scope
    }
}

/// HTTP client with built-in rate limiting and error handling
/// Now uses shared global rate limiter for better concurrency performance
#[derive(Clone)]
pub struct HttpClient {
    client: Client,
    config: HttpClientConfig,
}

impl HttpClient {
    /// 글로벌 설정에서 HttpClient 생성
    pub fn create_from_global_config() -> Result<Self> {
        use crate::infrastructure::config::ConfigManager;
        let config_manager = ConfigManager::new()?;
        // 비동기 함수를 동기적으로 호출하는 것은 권장되지 않지만, 
        // 테스트와 간단한 경우를 위해 임시로 기본 설정 사용
        let app_config = crate::infrastructure::config::AppConfig::default();
        Self::from_worker_config(&app_config.user.crawling.workers)
    }

    /// Create a new HTTP client from WorkerConfig
    pub fn from_worker_config(worker_config: &WorkerConfig) -> Result<Self> {
        let config = HttpClientConfig::from_worker_config(worker_config);
        Self::with_config(config)
    }

    /// Create a new HTTP client with custom configuration
    pub fn with_config(config: HttpClientConfig) -> Result<Self> {
        let client = ClientBuilder::new()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .user_agent(&config.user_agent)
            .cookie_store(true)
            .gzip(true)
            .redirect(if config.follow_redirects {
                reqwest::redirect::Policy::limited(10)
            } else {
                reqwest::redirect::Policy::none()
            })
            .build()
            .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?;

        Ok(Self {
            client,
            config,
        })
    }

    /// Fetch HTML content from a URL with automatic retry and rate limiting
    pub async fn fetch_html(&self, url: &str) -> Result<Html> {
        info!("Fetching HTML from: {}", url);
        
        let mut last_error = None;
        
        for attempt in 1..=self.config.max_retries {
            // Apply global rate limiting
            let rate_limiter = GlobalRateLimiter::get_instance();
            rate_limiter.apply_rate_limit(self.config.max_requests_per_second).await;
            
            match self.fetch_html_once(url).await {
                Ok(html) => {
                    debug!("Successfully fetched HTML from {} on attempt {}", url, attempt);
                    return Ok(html);
                }
                Err(e) => {
                    warn!("Attempt {} failed for {}: {}", attempt, url, e);
                    last_error = Some(e);
                    
                    if attempt < self.config.max_retries {
                        // Exponential backoff
                        let delay_seconds = 2_u64.pow(attempt - 1);
                        sleep(Duration::from_secs(delay_seconds)).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| anyhow!("Unknown error while fetching {}", url)))
    }

    /// Fetch raw response from a URL
    pub async fn fetch_response(&self, url: &str) -> Result<Response> {
        let rate_limiter = GlobalRateLimiter::get_instance();
        rate_limiter.apply_rate_limit(self.config.max_requests_per_second).await;
        
        info!("🌐 HTTP GET (HttpClient): {}", url);
        let response = self.client
            .get(url)
            .send()
            .await
            .map_err(|e| anyhow!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            error!("❌ HTTP error {}: {}", response.status(), url);
            return Err(anyhow!("HTTP error {}: {}", response.status(), url));
        }
        
        // info!("✅ HTTP Response received (HttpClient): {} - Status: {}", url, response.status());

        Ok(response)
    }

    /// Check if the HTTP client is working properly
    pub async fn health_check(&self) -> Result<()> {
        info!("Performing HTTP client health check...");
        
        let test_url = "https://httpbin.org/get";
        match self.fetch_response(test_url).await {
            Ok(response) => {
                if response.status().is_success() {
                    info!("HTTP client health check passed");
                    Ok(())
                } else {
                    Err(anyhow!("Health check failed with status: {}", response.status()))
                }
            }
            Err(e) => Err(anyhow!("Health check failed: {}", e))
        }
    }

    /// Single attempt to fetch HTML content
    async fn fetch_html_once(&self, url: &str) -> Result<Html> {
        let response = self.fetch_response(url).await?;
        
        let html_content = response
            .text()
            .await
            .map_err(|e| anyhow!("Failed to read response body: {}", e))?;

        if html_content.is_empty() {
            return Err(anyhow!("Empty response from {}", url));
        }

        Ok(Html::parse_document(&html_content))
    }

    /// Fetch HTML content and return it as a string (Send-compatible)
    pub async fn fetch_html_string(&self, url: &str) -> Result<String> {
        info!("🔄 Starting HTML fetch: {}", url);
        
        let mut last_error = None;
        
        for attempt in 1..=self.config.max_retries {
            // Apply global rate limiting
            let rate_limiter = GlobalRateLimiter::get_instance();
            rate_limiter.apply_rate_limit(self.config.max_requests_per_second).await;
            
            match self.fetch_html_string_once(url).await {
                Ok(html) => {
                    debug!("Successfully fetched HTML from {} on attempt {}", url, attempt);
                    return Ok(html);
                }
                Err(e) => {
                    warn!("Attempt {} failed for {}: {}", attempt, url, e);
                    last_error = Some(e);
                    
                    if attempt < self.config.max_retries {
                        // Exponential backoff
                        let delay_seconds = 2_u64.pow(attempt - 1);
                        sleep(Duration::from_secs(delay_seconds)).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| anyhow!("Unknown error while fetching {}", url)))
    }
    
    /// Single attempt to fetch HTML content as string (Send-compatible)
    async fn fetch_html_string_once(&self, url: &str) -> Result<String> {
        let response = self.fetch_response(url).await?;
        
        let html_content = response
            .text()
            .await
            .map_err(|e| anyhow!("Failed to read response body: {}", e))?;

        if html_content.is_empty() {
            return Err(anyhow!("Empty response from {}", url));
        }

        Ok(html_content)
    }
    
    /// Parse HTML from string (non-async, can be called after fetch)
    pub fn parse_html(&self, html_content: &str) -> Html {
        Html::parse_document(html_content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = HttpClient::create_from_global_config();
        assert!(client.is_ok());
    }

    #[test]
    fn test_custom_config() {
        let config = HttpClientConfig {
            max_requests_per_second: 1,
            timeout_seconds: 10,
            max_retries: 2,
            user_agent: "Test Agent".to_string(),
            follow_redirects: false,
        };
        
        let client = HttpClient::with_config(config);
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_health_check() {
        let client = HttpClient::create_from_global_config().unwrap();
        // This might fail in CI without internet, so we just test it doesn't panic
        let result = client.health_check().await;
        println!("Health check result: {:?}", result);
    }
}
