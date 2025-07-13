//! HTTP client for web crawling with rate limiting and error handling
//! 
//! Provides a robust HTTP client specifically designed for web scraping
//! with respect for server resources and proper error handling.

use std::time::Duration;
use reqwest::{Client, Response, header::{HeaderMap, HeaderValue, USER_AGENT}};
use anyhow::{Result, Context};
use governor::{Quota, RateLimiter, state::{direct::NotKeyed, InMemoryState}, clock::DefaultClock};
use std::num::NonZeroU32;
use tokio_util::sync::CancellationToken;

/// HTTP client configuration for crawling
#[derive(Debug, Clone, serde::Serialize)]
pub struct HttpClientConfig {
    pub user_agent: String,
    pub timeout_seconds: u64,
    pub max_requests_per_second: u32,
    pub respect_robots_txt: bool,
    pub follow_redirects: bool,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            user_agent: "matter-certis-v2/1.0 (Educational Purpose)".to_string(),
            timeout_seconds: 30,
            max_requests_per_second: 7, // 1000ms / 150ms = ~6.66 -> 7
            respect_robots_txt: true,
            follow_redirects: true,
        }
    }
}

/// Enhanced HTTP client with rate limiting for respectful crawling
pub struct HttpClient {
    client: Client,
    rate_limiter: RateLimiter<NotKeyed, InMemoryState, DefaultClock>,
    config: HttpClientConfig,
}

impl HttpClient {
    /// Create a new HTTP client with the given configuration
    pub fn new(config: HttpClientConfig) -> Result<Self> {
        // Setup headers
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&config.user_agent)
                .context("Invalid user agent")?
        );

        // Build reqwest client
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .default_headers(headers)
            .redirect(if config.follow_redirects {
                reqwest::redirect::Policy::limited(10)
            } else {
                reqwest::redirect::Policy::none()
            })
            .build()
            .context("Failed to create HTTP client")?;

        // Setup rate limiter
        let quota = Quota::per_second(
            NonZeroU32::new(config.max_requests_per_second)
                .context("Rate limit must be greater than 0")?
        );
        let rate_limiter = RateLimiter::direct(quota);

        Ok(Self {
            client,
            rate_limiter,
            config,
        })
    }

    /// Fetch a URL with rate limiting and error handling
    pub async fn get(&self, url: &str) -> Result<Response> {
        // Wait for rate limiter
        self.rate_limiter.until_ready().await;

        tracing::info!("Fetching URL: {}", url);

        let response = self.client
            .get(url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch URL: {url}"))?;

        if !response.status().is_success() {
            anyhow::bail!(
                "HTTP request failed with status {}: {}",
                response.status(),
                url
            );
        }

        tracing::debug!("Successfully fetched: {} ({})", url, response.status());
        Ok(response)
    }

    /// Fetch URL and return text content
    pub async fn get_text(&self, url: &str) -> Result<String> {
        let response = self.get(url).await?;
        let text = response.text().await
            .with_context(|| format!("Failed to read response body from: {url}"))?;
        
        Ok(text)
    }

    /// Fetch URL and return text content with cancellation support
    pub async fn get_text_with_cancellation(&self, url: &str, cancellation_token: CancellationToken) -> Result<String> {
        // Check cancellation before starting
        if cancellation_token.is_cancelled() {
            anyhow::bail!("Request cancelled before starting");
        }

        // Wait for rate limiter with cancellation support
        tokio::select! {
            _ = self.rate_limiter.until_ready() => {},
            _ = cancellation_token.cancelled() => {
                anyhow::bail!("Request cancelled during rate limiting");
            }
        }

        tracing::info!("Fetching HTML as string from: {}", url);

        // Make HTTP request with cancellation support
        let response = tokio::select! {
            result = self.client.get(url).send() => {
                result.with_context(|| format!("Failed to fetch URL: {url}"))?
            },
            _ = cancellation_token.cancelled() => {
                tracing::warn!("🛑 HTTP request cancelled for URL: {}", url);
                anyhow::bail!("HTTP request cancelled");
            }
        };

        if !response.status().is_success() {
            anyhow::bail!(
                "HTTP request failed with status {}: {}",
                response.status(),
                url
            );
        }

        // Read response body with cancellation support
        let text = tokio::select! {
            result = response.text() => {
                result.with_context(|| format!("Failed to read response body from: {url}"))?
            },
            _ = cancellation_token.cancelled() => {
                tracing::warn!("🛑 Response reading cancelled for URL: {}", url);
                anyhow::bail!("Response reading cancelled");
            }
        };

        tracing::debug!("Successfully fetched: {} ({} chars)", url, text.len());
        Ok(text)
    }

    /// Check if robots.txt allows crawling this URL
    pub async fn is_allowed_by_robots(&self, url: &str) -> Result<bool> {
        if !self.config.respect_robots_txt {
            return Ok(true);
        }

        // Parse base URL for robots.txt
        let parsed_url = reqwest::Url::parse(url)
            .context("Invalid URL format")?;
        
        let robots_url = format!("{}://{}/robots.txt", 
            parsed_url.scheme(), 
            parsed_url.host_str().unwrap_or("")
        );

        // Simple robots.txt check (for production, use a proper robots.txt parser)
        match self.get_text(&robots_url).await {
            Ok(robots_content) => {
                // Very basic check - in production, use a proper robots.txt parser
                let disallowed = robots_content.lines()
                    .filter(|line| line.trim().starts_with("Disallow:"))
                    .any(|line| {
                        let path = line.split(':').nth(1).unwrap_or("").trim();
                        !path.is_empty() && url.contains(path)
                    });
                Ok(!disallowed)
            }
            Err(_) => {
                // If robots.txt is not accessible, assume it's allowed
                tracing::warn!("Could not fetch robots.txt for {}, assuming allowed", robots_url);
                Ok(true)
            }
        }
    }

    /// Get the configuration
    pub fn config(&self) -> &HttpClientConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_http_client_creation() {
        let config = HttpClientConfig::default();
        let client = HttpClient::new(config);
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let config = HttpClientConfig {
            max_requests_per_second: 1,
            ..Default::default()
        };
        
        let client = HttpClient::new(config).unwrap();
        
        // This test would need a mock server to test properly
        // For now, just verify the client was created
        assert_eq!(client.config().max_requests_per_second, 1);
    }
}
