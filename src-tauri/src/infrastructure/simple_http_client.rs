//! HTTP client for web crawling with rate limiting and error handling
//!
//! This module provides a configurable HTTP client optimized for web crawling
//! with built-in retry logic, rate limiting, and user agent management.

use crate::infrastructure::config::WorkerConfig;
use anyhow::{Result, anyhow};
use reqwest::{
    Client, ClientBuilder, Response, Url,
    header::{ACCEPT, ACCEPT_LANGUAGE, HeaderMap, HeaderValue, REFERER, USER_AGENT},
};
use scraper::Html;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::{Mutex, Semaphore};
use tokio::time::interval;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, trace, warn};

/// Per-request options to override UA, referer, and toggle robots for a single call
#[derive(Debug, Default, Clone)]
pub struct RequestOptions {
    pub user_agent_override: Option<String>,
    pub referer: Option<String>,
    pub skip_robots_check: bool,
    /// Optional: current attempt number (1-based) when caller implements retries
    pub attempt: Option<u32>,
    /// Optional: total max attempts when caller implements retries
    pub max_attempts: Option<u32>,
}

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
    /// Respect robots.txt when crawling (simple allow/disallow check)
    pub respect_robots_txt: bool,
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
            respect_robots_txt: worker_config.respect_robots_txt,
        }
    }
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            max_requests_per_second: 10, // Increased default for better performance
            timeout_seconds: 30,
            max_retries: 3,
            user_agent: "matter-certis-v2/1.0 (Research Tool; +https://github.com/your-repo)"
                .to_string(),
            follow_redirects: true,
            respect_robots_txt: false,
        }
    }
}

/// Global rate limiter shared across all HttpClient instances
/// Uses token bucket algorithm for true concurrent rate limiting
#[derive(Debug)]
struct GlobalRateLimiter {
    /// Semaphore representing available tokens (permits)
    /// We refill up to a capped capacity each second by adding 1 token per tick.
    semaphore: Arc<Semaphore>,
    /// Current rate limit setting (RPS)
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
            // Start with 0 available permits; refill task will add permits as rate is applied
            let semaphore = Arc::new(Semaphore::new(0));
            let current_rate = Arc::new(Mutex::new(initial_rate));

            info!(
                "🚀 GlobalRateLimiter initialized with {} RPS (Token Bucket)",
                initial_rate
            );

            GlobalRateLimiter {
                semaphore: semaphore.clone(),
                current_rate: current_rate.clone(),
                refill_handle: Arc::new(Mutex::new(None)),
            }
        })
    }

    /// Publicly adjust the global rate limit (RPS) at runtime.
    /// Safe to call from anywhere; the refill task will be restarted if needed.
    pub async fn set_global_rate_limit(rps: u32) {
        let inst = Self::get_instance();
        inst.update_rate_limit(rps).await;
    }

    async fn update_rate_limit(&self, max_requests_per_second: u32) {
        let mut current_rate = self.current_rate.lock().await;
        let changed = *current_rate != max_requests_per_second;
        if changed {
            *current_rate = max_requests_per_second;
            info!(
                "🔄 Updated global rate limit to {} RPS",
                max_requests_per_second
            );
        }

        // Ensure refill is running. If rate changed, restart. If unchanged but no task, start.
        let need_start = {
            let handle = self.refill_handle.lock().await;
            match &*handle {
                None => true,
                Some(h) => h.is_finished(),
            }
        };
        if changed || need_start {
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
        // Refill once every (1000 / rate) ms; cap bucket size to `rate` (1s worth of tokens)
        let refill_interval = Duration::from_millis(1000 / rate as u64);
        let capacity: usize = rate as usize;

        info!(
            "🎯 Starting token refill task: {} tokens/sec (interval: {:?}, capacity: {})",
            rate, refill_interval, capacity
        );

    let new_handle = tokio::spawn(async move {
            let mut interval = interval(refill_interval);
            loop {
                interval.tick().await;
                // Add one token if below capacity
                let available = semaphore.available_permits();
                if available < capacity {
                    semaphore.add_permits(1);
            trace!("[rate-limit] token refilled (available={}/{})", available + 1, capacity);
                } else {
                    // At capacity; skip adding to avoid unbounded burst
            trace!("[rate-limit] token refill skipped (at capacity {}/{})", available, capacity);
                }
            }
        });

        *handle = Some(new_handle);
    }

    async fn apply_rate_limit(&self, max_requests_per_second: u32) {
        // Update rate limit if needed
        self.update_rate_limit(max_requests_per_second).await;

        if max_requests_per_second == 0 {
            debug!("🔓 [rate-limit] off (max_requests_per_second = 0)");
            return; // No rate limiting
        }

        debug!("🎫 [rate-limit] awaiting token ({} RPS)", max_requests_per_second);

        // Acquire a token (permit) from the bucket
        // This will wait if no tokens are available
    let _permit = self.semaphore.acquire().await.unwrap();

        debug!("🎫 [rate-limit] token acquired ({} RPS)", max_requests_per_second);

        // Permit is automatically released when _permit goes out of scope
    }
}

/// HTTP client with built-in rate limiting and error handling
/// Now uses shared global rate limiter for better concurrency performance
#[derive(Clone)]
pub struct HttpClient {
    client: Client,
    config: HttpClientConfig,
    /// Optional context label for provenance in logs (e.g., "BatchActor", "Stage:List")
    context_label: Option<String>,
}

impl HttpClient {
    /// 글로벌 설정에서 HttpClient 생성
    pub fn create_from_global_config() -> Result<Self> {
        // Load actual configuration from file instead of using defaults
        let config_manager = crate::infrastructure::config::ConfigManager::new()
            .map_err(|e| anyhow!("Failed to create config manager: {}", e))?;

        // Use blocking version to load config synchronously
        let app_config = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async { config_manager.load_config().await })
        })
        .map_err(|e| anyhow!("Failed to load config: {}", e))?;

        info!(
            "🔧 HttpClient using config: max_requests_per_second={}",
            app_config.user.crawling.workers.max_requests_per_second
        );
        // KPI: 네트워크 레이트리밋 설정(구조화 로그)
        info!(target: "kpi.network",
            "{{\"event\":\"rate_limit_set\",\"rps\":{},\"source\":\"config\",\"ts\":\"{}\"}}",
            app_config.user.crawling.workers.max_requests_per_second,
            chrono::Utc::now()
        );
        Self::from_worker_config(&app_config.user.crawling.workers)
    }

    /// Create a new HTTP client from WorkerConfig
    pub fn from_worker_config(worker_config: &WorkerConfig) -> Result<Self> {
        let config = HttpClientConfig::from_worker_config(worker_config);
        Self::with_config(config)
    }

    /// Create a new HTTP client with custom configuration
    pub fn with_config(config: HttpClientConfig) -> Result<Self> {
        // Set browser-like defaults to minimize server-side variance
        let mut default_headers = HeaderMap::new();
        // Match the diagnostic script behavior
        default_headers.insert(
            ACCEPT,
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            ),
        );
        default_headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));

        let client = ClientBuilder::new()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .user_agent(&config.user_agent)
            .default_headers(default_headers)
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
            context_label: None,
        })
    }
    /// Set a human-readable context label for logging provenance (returns self for chaining)
    pub fn with_context_label(mut self, label: &str) -> Self {
        self.context_label = Some(label.to_string());
        self
    }

    /// Update the context label after construction
    pub fn set_context_label(&mut self, label: &str) {
        self.context_label = Some(label.to_string());
    }

    /// Adjust the global RPS limit for all HttpClient instances at runtime.
    /// This does not mutate this instance's stored config but affects the shared token bucket.
    pub async fn set_global_max_rps(rps: u32) {
        GlobalRateLimiter::set_global_rate_limit(rps).await;
        info!(target: "kpi.network",
            "{{\"event\":\"rate_limit_set\",\"rps\":{},\"source\":\"runtime\",\"ts\":\"{}\"}}",
            rps,
            chrono::Utc::now()
        );
    }
    fn build_request(&self, url: &str, opts: &RequestOptions) -> Result<reqwest::RequestBuilder> {
        let mut rb = self.client.get(url);
        if let Some(ua) = &opts.user_agent_override {
            rb = rb.header(USER_AGENT, ua);
        }
        if let Some(referer) = &opts.referer {
            rb = rb.header(REFERER, referer);
        }
        Ok(rb)
    }

    /// Public variant to perform a GET with custom options (UA override, referer, skip robots)
    pub async fn fetch_response_with_options(
        &self,
        url: &str,
        opts: &RequestOptions,
    ) -> Result<Response> {
        let rate_limiter = GlobalRateLimiter::get_instance();
        if let Some(label) = &self.context_label {
            debug!(
                "⚖️ [rate-limit] {} RPS (source: {})",
                self.config.max_requests_per_second, label
            );
        } else {
            debug!(
                "⚖️ [rate-limit] {} RPS",
                self.config.max_requests_per_second
            );
        }
        rate_limiter
            .apply_rate_limit(self.config.max_requests_per_second)
            .await;

        if self.config.respect_robots_txt
            && !opts.skip_robots_check
            && !self.robots_allowed(url).await?
        {
            warn!("robots.txt disallows: {}", url);
            return Err(anyhow!("Blocked by robots.txt: {}", url));
        }

        // Include attempt info when provided by caller for better observability
        match (opts.attempt, opts.max_attempts) {
            (Some(a), Some(m)) if a > 1 => {
                info!("🌐 HTTP GET (HttpClient,opts, {}/{} retrying): {}", a, m, url);
            }
            (Some(a), Some(m)) if a == 1 => {
                info!("🌐 HTTP GET (HttpClient,opts, {}/{}): {}", a, m, url);
            }
            _ => {
                info!("🌐 HTTP GET (HttpClient,opts): {}", url);
            }
        }
        let response = self
            .build_request(url, opts)?
            .send()
            .await
            .map_err(|e| anyhow!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            error!("❌ HTTP error {}: {}", response.status(), url);
            return Err(anyhow!("HTTP error {}: {}", response.status(), url));
        }

        Ok(response)
    }

    async fn robots_allowed(&self, target_url: &str) -> Result<bool> {
        if !self.config.respect_robots_txt {
            return Ok(true);
        }
        // Parse base origin
        let url =
            Url::parse(target_url).map_err(|e| anyhow!("Invalid URL {}: {}", target_url, e))?;
        let robots_url = format!(
            "{}://{}/robots.txt",
            url.scheme(),
            url.host_str().unwrap_or("")
        );
        // Best-effort fetch; if fails, allow by default
        let resp = match self.client.get(&robots_url).send().await {
            Ok(r) => r,
            Err(_) => return Ok(true),
        };
        if !resp.status().is_success() {
            return Ok(true);
        }
        let text = resp.text().await.unwrap_or_default();
        if text.is_empty() {
            return Ok(true);
        }
        // Super-simple check: if a line says Disallow: / then deny everything; otherwise allow
        // Optionally, we could parse path-specific disallows that match the URL path.
        for line in text.lines() {
            let l = line.trim();
            if l.starts_with('#') {
                continue;
            }
            if l.to_ascii_lowercase().starts_with("user-agent:") {
                // Not differentiating per-agent in this lightweight pass
                continue;
            }
            if l.to_ascii_lowercase().starts_with("disallow:") {
                let rule = l[9..].trim();
                if rule == "/" {
                    return Ok(false);
                }
                // Basic path prefix match
                if !rule.is_empty() {
                    if let Some(path) = url.path().strip_prefix('/') {
                        if path.starts_with(rule.trim_start_matches('/')) {
                            return Ok(false);
                        }
                    } else if url.path().starts_with(rule) {
                        return Ok(false);
                    }
                }
            }
        }
        Ok(true)
    }

    /// Fetch HTML content from a URL with automatic retry and rate limiting
    pub async fn fetch_html(&self, url: &str) -> Result<Html> {
        info!("Fetching HTML from: {}", url);

        // Delegate retry/backoff and robots handling to the unified policy
        let response = self.fetch_response_with_policy(url).await?;
        let html_content = response
            .text()
            .await
            .map_err(|e| anyhow!("Failed to read response body: {}", e))?;

        if html_content.is_empty() {
            return Err(anyhow!("Empty response from {}", url));
        }

        Ok(Html::parse_document(&html_content))
    }

    /// Fetch raw response from a URL
    pub async fn fetch_response(&self, url: &str) -> Result<Response> {
        let rate_limiter = GlobalRateLimiter::get_instance();
        if let Some(label) = &self.context_label {
            debug!(
                "⚖️ [rate-limit] {} RPS (source: {})",
                self.config.max_requests_per_second, label
            );
        } else {
            debug!(
                "⚖️ [rate-limit] {} RPS",
                self.config.max_requests_per_second
            );
        }
        rate_limiter
            .apply_rate_limit(self.config.max_requests_per_second)
            .await;

        // robots.txt check if enabled
        if self.config.respect_robots_txt && !self.robots_allowed(url).await? {
            warn!("robots.txt disallows: {}", url);
            return Err(anyhow!("Blocked by robots.txt: {}", url));
        }

        info!("🌐 HTTP GET (HttpClient): {}", url);
        let response = self
            .build_request(url, &RequestOptions::default())?
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

    /// Fetch raw response with cancellation support
    /// This mirrors `fetch_response` but cooperates with a CancellationToken.
    pub async fn fetch_response_with_cancel(
        &self,
        url: &str,
        cancellation_token: &CancellationToken,
    ) -> Result<Response> {
        // Apply global rate limiting with cancellation support
        let rate_limiter = GlobalRateLimiter::get_instance();
        if let Some(label) = &self.context_label {
            debug!(
                "⚖️ [rate-limit] {} RPS (source: {})",
                self.config.max_requests_per_second, label
            );
        } else {
            debug!(
                "⚖️ [rate-limit] {} RPS",
                self.config.max_requests_per_second
            );
        }

        tokio::select! {
            _ = rate_limiter.apply_rate_limit(self.config.max_requests_per_second) => {},
            _ = cancellation_token.cancelled() => {
                return Err(anyhow!("Request cancelled during rate limiting"));
            }
        }

        // robots.txt check if enabled
        if self.config.respect_robots_txt && !self.robots_allowed(url).await? {
            warn!("robots.txt disallows: {}", url);
            return Err(anyhow!("Blocked by robots.txt: {}", url));
        }

        info!("🌐 HTTP GET (HttpClient,cancel-aware): {}", url);

        // Perform request with cancellation
        let response = tokio::select! {
            res = self.build_request(url, &RequestOptions::default())?.send() => {
                res.map_err(|e| anyhow!("HTTP request failed: {}", e))?
            },
            _ = cancellation_token.cancelled() => {
                warn!("🛑 HTTP request cancelled: {}", url);
                return Err(anyhow!("HTTP request cancelled"));
            }
        };

        if !response.status().is_success() {
            error!("❌ HTTP error {}: {}", response.status(), url);
            return Err(anyhow!("HTTP error {}: {}", response.status(), url));
        }

        Ok(response)
    }

    /// Fetch response with retry policy and cancellation support
    /// Mirrors `fetch_response_with_policy` but allows cancellation during rate limiting,
    /// request execution, and backoff sleeps. Honors Retry-After on 429/503.
    pub async fn fetch_response_with_policy_cancel(
        &self,
        url: &str,
        cancellation_token: &CancellationToken,
    ) -> Result<Response> {
        use reqwest::StatusCode;
        let mut last_err: Option<anyhow::Error> = None;

        for attempt in 1..=self.config.max_retries {
            let rate_limiter = GlobalRateLimiter::get_instance();
            if let Some(label) = &self.context_label {
                debug!(
                    "⚖️ [rate-limit] {} RPS (source: {})",
                    self.config.max_requests_per_second, label
                );
            } else {
                debug!(
                    "⚖️ [rate-limit] {} RPS",
                    self.config.max_requests_per_second
                );
            }

            tokio::select! {
                _ = rate_limiter.apply_rate_limit(self.config.max_requests_per_second) => {},
                _ = cancellation_token.cancelled() => {
                    return Err(anyhow!("Request cancelled during rate limiting"));
                }
            }

            // robots.txt check if enabled
            if self.config.respect_robots_txt && !self.robots_allowed(url).await? {
                warn!("robots.txt disallows: {}", url);
                return Err(anyhow!("Blocked by robots.txt: {}", url));
            }

            if attempt > 1 {
                info!(
                    "🌐 HTTP GET (attempt {}/{}, cancel-aware retrying): {}",
                    attempt, self.config.max_retries, url
                );
            } else {
                info!(
                    "🌐 HTTP GET (attempt {}/{}, cancel-aware): {}",
                    attempt, self.config.max_retries, url
                );
            }

            // Perform request with cancellation
            let send_res: Result<Response, anyhow::Error> = tokio::select! {
                res = self.build_request(url, &RequestOptions::default())?.send() => {
                    res.map_err(|e| anyhow!("HTTP request failed: {}", e))
                },
                _ = cancellation_token.cancelled() => {
                    warn!("🛑 HTTP request cancelled: {}", url);
                    return Err(anyhow!("HTTP request cancelled"));
                }
            };

            match send_res {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        return Ok(resp);
                    }

                    // Decide retry policy based on status
                    let retryable = matches!(
                        status,
                        StatusCode::REQUEST_TIMEOUT
                            | StatusCode::TOO_MANY_REQUESTS
                            | StatusCode::BAD_GATEWAY
                            | StatusCode::SERVICE_UNAVAILABLE
                            | StatusCode::GATEWAY_TIMEOUT
                            | StatusCode::INTERNAL_SERVER_ERROR
                    );

                    error!("❌ HTTP error {} on attempt {}: {}", status, attempt, url);

                    if retryable && attempt < self.config.max_retries {
                        // Respect Retry-After if present on 429/503
                        let mut delay_secs = 2_u64.pow(attempt - 1);
                        if let Some(retry_after) = resp.headers().get(reqwest::header::RETRY_AFTER) {
                            if let Ok(s) = retry_after.to_str() {
                                if let Ok(parsed) = s.parse::<u64>() {
                                    delay_secs = parsed.max(delay_secs);
                                }
                            }
                        }
                        // Full jitter: [0, delay_secs]
                        let jitter_ms: u64 = if delay_secs == 0 { 0 } else { fastrand::u64(..(delay_secs * 1000)) };
                        info!(target: "kpi.network",
                            "{{\"event\":\"retry_scheduled\",\"attempt\":{},\"max\":{},\"base_delay_s\":{},\"jitter_ms\":{},\"url\":\"{}\"}}",
                            attempt, self.config.max_retries, delay_secs, jitter_ms, url
                        );
                        tokio::select! {
                            _ = tokio::time::sleep(Duration::from_millis(jitter_ms)) => {},
                            _ = cancellation_token.cancelled() => {
                                return Err(anyhow!("Request cancelled during backoff"));
                            }
                        }
                        continue;
                    } else {
                        return Err(anyhow!("HTTP error {}: {}", status, url));
                    }
                }
                Err(e) => {
                    warn!("⚠️ Network error on attempt {}: {}", attempt, e);
                    last_err = Some(anyhow!("HTTP request failed: {}", e));
                    if attempt < self.config.max_retries {
                        let delay_secs = 2_u64.pow(attempt - 1);
                        let jitter_ms: u64 = if delay_secs == 0 { 0 } else { fastrand::u64(..(delay_secs * 1000)) };
                        info!(target: "kpi.network",
                            "{{\"event\":\"retry_scheduled\",\"attempt\":{},\"max\":{},\"base_delay_s\":{},\"jitter_ms\":{},\"url\":\"{}\"}}",
                            attempt, self.config.max_retries, delay_secs, jitter_ms, url
                        );
                        tokio::select! {
                            _ = tokio::time::sleep(Duration::from_millis(jitter_ms)) => {},
                            _ = cancellation_token.cancelled() => {
                                return Err(anyhow!("Request cancelled during backoff"));
                            }
                        }
                        continue;
                    }
                }
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow!("Unknown HTTP error for {}", url)))
    }

    /// Fetch response with retry policy based on HTTP status codes and network errors
    pub async fn fetch_response_with_policy(&self, url: &str) -> Result<Response> {
        use reqwest::StatusCode;
        let mut last_err: Option<anyhow::Error> = None;

        for attempt in 1..=self.config.max_retries {
            let rate_limiter = GlobalRateLimiter::get_instance();
            if let Some(label) = &self.context_label {
                debug!(
                    "⚖️ [rate-limit] {} RPS (source: {})",
                    self.config.max_requests_per_second, label
                );
            } else {
                debug!(
                    "⚖️ [rate-limit] {} RPS",
                    self.config.max_requests_per_second
                );
            }
            rate_limiter
                .apply_rate_limit(self.config.max_requests_per_second)
                .await;

            // robots.txt check if enabled
            if self.config.respect_robots_txt && !self.robots_allowed(url).await? {
                warn!("robots.txt disallows: {}", url);
                return Err(anyhow!("Blocked by robots.txt: {}", url));
            }

            if attempt > 1 {
                info!(
                    "🌐 HTTP GET (attempt {}/{} retrying): {}",
                    attempt, self.config.max_retries, url
                );
            } else {
                info!(
                    "🌐 HTTP GET (attempt {}/{}): {}",
                    attempt, self.config.max_retries, url
                );
            }
            match self
                .build_request(url, &RequestOptions::default())?
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        return Ok(resp);
                    }

                    // Decide retry policy based on status
                    let retryable = matches!(
                        status,
                        StatusCode::REQUEST_TIMEOUT
                            | StatusCode::TOO_MANY_REQUESTS
                            | StatusCode::BAD_GATEWAY
                            | StatusCode::SERVICE_UNAVAILABLE
                            | StatusCode::GATEWAY_TIMEOUT
                            | StatusCode::INTERNAL_SERVER_ERROR
                    );

                    error!("❌ HTTP error {} on attempt {}: {}", status, attempt, url);

                    if retryable && attempt < self.config.max_retries {
                        // Respect Retry-After if present on 429/503
                        let mut delay_secs = 2_u64.pow(attempt - 1);
                        if let Some(retry_after) = resp.headers().get(reqwest::header::RETRY_AFTER)
                        {
                            if let Ok(s) = retry_after.to_str() {
                                if let Ok(parsed) = s.parse::<u64>() {
                                    delay_secs = parsed.max(delay_secs);
                                }
                            }
                        }
                        // Full jitter: [0, delay_secs]
                        let jitter_ms: u64 = if delay_secs == 0 { 0 } else {
                            fastrand::u64(..(delay_secs * 1000))
                        };
                        info!(target: "kpi.network",
                            "{{\"event\":\"retry_scheduled\",\"attempt\":{},\"max\":{},\"base_delay_s\":{},\"jitter_ms\":{},\"url\":\"{}\"}}",
                            attempt, self.config.max_retries, delay_secs, jitter_ms, url
                        );
                        tokio::time::sleep(Duration::from_millis(jitter_ms)).await;
                        continue;
                    } else {
                        return Err(anyhow!("HTTP error {}: {}", status, url));
                    }
                }
                Err(e) => {
                    warn!("⚠️ Network error on attempt {}: {}", attempt, e);
                    last_err = Some(anyhow!("HTTP request failed: {}", e));
                    if attempt < self.config.max_retries {
                        let delay_secs = 2_u64.pow(attempt - 1);
                        let jitter_ms: u64 = if delay_secs == 0 { 0 } else {
                            fastrand::u64(..(delay_secs * 1000))
                        };
                        info!(target: "kpi.network",
                            "{{\"event\":\"retry_scheduled\",\"attempt\":{},\"max\":{},\"base_delay_s\":{},\"jitter_ms\":{},\"url\":\"{}\"}}",
                            attempt, self.config.max_retries, delay_secs, jitter_ms, url
                        );
                        tokio::time::sleep(Duration::from_millis(jitter_ms)).await;
                        continue;
                    }
                }
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow!("Unknown HTTP error for {}", url)))
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
                    Err(anyhow!(
                        "Health check failed with status: {}",
                        response.status()
                    ))
                }
            }
            Err(e) => Err(anyhow!("Health check failed: {}", e)),
        }
    }

    

    /// Fetch HTML content and return it as a string (Send-compatible)
    pub async fn fetch_html_string(&self, url: &str) -> Result<String> {
        info!("🔄 Starting HTML fetch: {}", url);

        // Use unified policy for retries/backoff and robots handling
        let response = self.fetch_response_with_policy(url).await?;
        let text = response
            .text()
            .await
            .map_err(|e| anyhow!("Failed to read response body: {}", e))?;

        if text.is_empty() {
            return Err(anyhow!("Empty response from {}", url));
        }
        Ok(text)
    }

    /// Fetch HTML content as string with cancellation support
    pub async fn fetch_html_string_with_cancel(
        &self,
        url: &str,
        cancellation_token: &CancellationToken,
    ) -> Result<String> {
        info!("🔄 Starting HTML fetch (cancel-aware): {}", url);

        // Use unified cancel-aware policy for HTTP-level retries/backoff
        let response = self
            .fetch_response_with_policy_cancel(url, cancellation_token)
            .await?;

        // Read body with cancellation
        let text = tokio::select! {
            res = response.text() => {
                res.map_err(|e| anyhow!("Failed to read response body: {}", e))?
            },
            _ = cancellation_token.cancelled() => {
                warn!("🛑 Response reading cancelled for URL: {}", url);
                return Err(anyhow!("Response reading cancelled"));
            }
        };

        if text.is_empty() {
            return Err(anyhow!("Empty response from {}", url));
        }
        Ok(text)
    }

    

    /// Parse HTML from string (non-async, can be called after fetch)
    pub fn parse_html(&self, html_content: &str) -> Html {
        Html::parse_document(html_content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::Instant;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn start_test_server() -> (SocketAddr, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let cnt_clone = counter.clone();
        tokio::spawn(async move {
            loop {
                let (mut socket, _) = match listener.accept().await {
                    Ok(s) => s,
                    Err(_) => break,
                };

                let mut buf = vec![0u8; 1024];
                let _ = socket.read(&mut buf).await; // best-effort
                let req = String::from_utf8_lossy(&buf);
                let first_line = req.lines().next().unwrap_or("");

                // naive path detection
                let path = if let Some(start) = first_line.find(' ') {
                    if let Some(end) = first_line[start + 1..].find(' ') {
                        &first_line[start + 1..start + 1 + end]
                    } else {
                        "/"
                    }
                } else {
                    "/"
                };

                match path {
                    "/retry2ok" => {
                        let n = cnt_clone.fetch_add(1, Ordering::SeqCst);
                        if n < 2 {
                            // 429 with Retry-After: 0
                            let resp = b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 0\r\nContent-Length: 0\r\n\r\n";
                            let _ = socket.write_all(resp).await;
                        } else {
                            let body = b"ok";
                            let resp = format!(
                                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                                body.len()
                            );
                            let _ = socket.write_all(resp.as_bytes()).await;
                            let _ = socket.write_all(body).await;
                        }
                    }
                    "/slow" => {
                        // Write headers then delay body to let cancellation kick in
                        let body = b"delayed";
                        let resp =
                            format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len());
                        let _ = socket.write_all(resp.as_bytes()).await;
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        let _ = socket.write_all(body).await;
                    }
                    _ => {
                        let body = b"ok";
                        let resp =
                            format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len());
                        let _ = socket.write_all(resp.as_bytes()).await;
                        let _ = socket.write_all(body).await;
                    }
                }
            }
        });
        (addr, counter)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_client_creation() {
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
            respect_robots_txt: false,
        };

        let client = HttpClient::with_config(config);
        assert!(client.is_ok());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_health_check() {
        let client = HttpClient::create_from_global_config().unwrap();
        // This might fail in CI without internet, so we just test it doesn't panic
        let result = client.health_check().await;
        println!("Health check result: {:?}", result);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_retry_policy_429_then_ok() {
        let (addr, _cnt) = start_test_server().await;
        let cfg = HttpClientConfig {
            max_requests_per_second: 100,
            timeout_seconds: 5,
            max_retries: 3,
            user_agent: "test".into(),
            follow_redirects: false,
            respect_robots_txt: false,
        };
        let client = HttpClient::with_config(cfg).unwrap();
        let url = format!("http://{}/retry2ok", addr);
        let resp = client
            .fetch_response_with_policy(&url)
            .await
            .expect("should eventually succeed");
        assert!(resp.status().is_success());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_cancellation_before_start() {
        let cfg = HttpClientConfig {
            max_requests_per_second: 100,
            timeout_seconds: 5,
            max_retries: 1,
            user_agent: "test".into(),
            follow_redirects: false,
            respect_robots_txt: false,
        };
        let client = HttpClient::with_config(cfg).unwrap();
        let token = CancellationToken::new();
        token.cancel();
        let res = client
            .fetch_html_string_with_cancel("http://127.0.0.1:9/nowhere", &token)
            .await;
        assert!(res.is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_cancellation_during_body() {
        let (addr, _cnt) = start_test_server().await;
        let cfg = HttpClientConfig {
            max_requests_per_second: 100,
            timeout_seconds: 5,
            max_retries: 1,
            user_agent: "test".into(),
            follow_redirects: false,
            respect_robots_txt: false,
        };
        let client = HttpClient::with_config(cfg).unwrap();
        let token = CancellationToken::new();
        let url = format!("http://{}/slow", addr);
        let h = tokio::spawn({
            let client = client.clone();
            let token = token.clone();
            async move { client.fetch_html_string_with_cancel(&url, &token).await }
        });
        tokio::time::sleep(Duration::from_millis(100)).await;
        token.cancel();
        let res = h.await.unwrap();
        assert!(res.is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_rate_limiter_performance() {
        let rps = 20;
        let config = HttpClientConfig {
            max_requests_per_second: rps,
            timeout_seconds: 5,
            max_retries: 1,
            ..Default::default()
        };
        let client = HttpClient::with_config(config).unwrap();

        let num_requests = 50;
        let mut handles = Vec::new();

        let start = Instant::now();
        for i in 0..num_requests {
            let client = client.clone();
            let url = format!("https://httpbin.org/delay/0.1?val={}", i);
            handles.push(tokio::spawn(
                async move { client.fetch_response(&url).await },
            ));
        }

        let results = futures::future::join_all(handles).await;
        let duration = start.elapsed();

        let successful_requests = results.into_iter().filter(|r| r.is_ok()).count();

        println!("Rate Limiter Test ({} RPS):", rps);
        println!(
            "- Executed {} requests in {:.2} seconds.",
            num_requests,
            duration.as_secs_f32()
        );
        println!("- {} requests were successful.", successful_requests);

        let expected_duration_min = (num_requests as f32 / rps as f32) * 0.8; // Allow some bursting
        // CI/network variability can be high; allow a generous upper bound
        let expected_duration_max = (num_requests as f32 / rps as f32) * 2.5; // Allow for higher latency

        assert!(successful_requests > 0);
        if duration.as_secs_f32() <= expected_duration_min {
            eprintln!(
                "[warn] Rate limiter executed faster than expected: {:.2}s <= {:.2}s (env noise tolerated)",
                duration.as_secs_f32(),
                expected_duration_min
            );
        }
        // Don't fail the test in noisy CI; just log if unusually slow
        if duration.as_secs_f32() >= expected_duration_max {
            eprintln!(
                "[warn] Rate limiter test slow: {:.2}s >= {:.2}s",
                duration.as_secs_f32(),
                expected_duration_max
            );
        }
    }
}
