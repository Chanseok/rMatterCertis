//! HTTP client for web crawling with rate limiting and error handling
//! 
//! This module provides a configurable HTTP client optimized for web crawling
//! with built-in retry logic, rate limiting, and user agent management.

use anyhow::{anyhow, Result};
use reqwest::{Client, ClientBuilder, Response};
use scraper::Html;
use std::time::Duration;
use tokio::time::{sleep, Instant};
use tracing::{debug, info, warn};

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

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            max_requests_per_second: 2,
            timeout_seconds: 30,
            max_retries: 3,
            user_agent: "rMatterCertis/1.0 (Research Tool; +https://github.com/your-repo)".to_string(),
            follow_redirects: true,
        }
    }
}

/// HTTP client with built-in rate limiting and error handling
#[derive(Clone)]
pub struct HttpClient {
    client: Client,
    config: HttpClientConfig,
    last_request_time: Option<Instant>,
}

impl HttpClient {
    /// Create a new HTTP client with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(HttpClientConfig::default())
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
            last_request_time: None,
        })
    }

    /// Fetch HTML content from a URL with automatic retry and rate limiting
    pub async fn fetch_html(&mut self, url: &str) -> Result<Html> {
        info!("Fetching HTML from: {}", url);
        
        let mut last_error = None;
        
        for attempt in 1..=self.config.max_retries {
            // Apply rate limiting
            self.apply_rate_limit().await;
            
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
    pub async fn fetch_response(&mut self, url: &str) -> Result<Response> {
        self.apply_rate_limit().await;
        
        let response = self.client
            .get(url)
            .send()
            .await
            .map_err(|e| anyhow!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!("HTTP error {}: {}", response.status(), url));
        }

        self.last_request_time = Some(Instant::now());
        Ok(response)
    }

    /// Check if the HTTP client is working properly
    pub async fn health_check(&mut self) -> Result<()> {
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
    async fn fetch_html_once(&mut self, url: &str) -> Result<Html> {
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
    pub async fn fetch_html_string(&mut self, url: &str) -> Result<String> {
        info!("Fetching HTML as string from: {}", url);
        
        let mut last_error = None;
        
        for attempt in 1..=self.config.max_retries {
            // Apply rate limiting
            self.apply_rate_limit().await;
            
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
    async fn fetch_html_string_once(&mut self, url: &str) -> Result<String> {
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

    /// Apply rate limiting based on configuration
    async fn apply_rate_limit(&mut self) {
        if let Some(last_request) = self.last_request_time {
            let min_interval = Duration::from_millis(1000 / self.config.max_requests_per_second as u64);
            let elapsed = last_request.elapsed();
            
            if elapsed < min_interval {
                let delay = min_interval - elapsed;
                debug!("Rate limiting: sleeping for {:?}", delay);
                sleep(delay).await;
            }
        }
        
        self.last_request_time = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = HttpClient::new();
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
        let mut client = HttpClient::new().unwrap();
        // This might fail in CI without internet, so we just test it doesn't panic
        let result = client.health_check().await;
        println!("Health check result: {:?}", result);
    }
}
