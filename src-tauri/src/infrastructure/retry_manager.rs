//! 재시도 관리자 - INTEGRATED_PHASE2_PLAN Week 1 Day 3-4 구현
//! 
//! 이 모듈은 크롤링 작업의 재시도 메커니즘을 제공하며,
//! 다양한 에러 타입에 따른 적응적 재시도 전략을 구현합니다.

use std::time::Duration;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use anyhow::Result;
use tracing::{info, warn, debug};
use chrono::{DateTime, Utc};

use crate::domain::events::CrawlingStage;

/// 재시도 관리자
#[derive(Clone)]
pub struct RetryManager {
    max_retries: u32,
    retry_queue: Arc<Mutex<VecDeque<RetryItem>>>,
    failure_classifier: Arc<dyn FailureClassifier>,
    retry_history: Arc<RwLock<HashMap<String, Vec<RetryAttempt>>>>,
}

/// 재시도 아이템
#[derive(Debug, Clone)]
pub struct RetryItem {
    pub item_id: String,
    pub stage: CrawlingStage,
    pub attempt_count: u32,
    pub last_error: String,
    pub next_retry_time: DateTime<Utc>,
    pub exponential_backoff: Duration,
    pub original_url: String,
    pub metadata: HashMap<String, String>,
}

/// 재시도 시도 기록
#[derive(Debug, Clone)]
pub struct RetryAttempt {
    pub attempt_number: u32,
    pub attempted_at: DateTime<Utc>,
    pub error_type: ErrorClassification,
    pub backoff_duration: Duration,
    pub success: bool,
}

/// 실패 분류기 트레이트
#[async_trait::async_trait]
pub trait FailureClassifier: Send + Sync {
    async fn classify_error(&self, error: &str, stage: CrawlingStage) -> ErrorClassification;
    async fn calculate_backoff(&self, attempt_count: u32) -> Duration;
    async fn should_retry(&self, classification: &ErrorClassification, attempt_count: u32) -> bool;
}

/// 에러 분류
#[derive(Debug, Clone)]
pub enum ErrorClassification {
    Recoverable { 
        retry_after: Duration,
        category: RecoverableErrorCategory,
    },
    NonRecoverable { 
        reason: String,
        category: NonRecoverableErrorCategory,
    },
    RateLimited { 
        retry_after: Duration,
        severity: RateLimitSeverity,
    },
    NetworkError { 
        retry_after: Duration,
        error_type: NetworkErrorType,
    },
}

#[derive(Debug, Clone)]
pub enum RecoverableErrorCategory {
    TemporaryServerError,    // 5xx 응답
    ParsingError,            // HTML 구조 변경 등
    DatabaseConnectionLost,  // DB 연결 끊김
    ResourceBusy,           // 리소스 일시적 사용 불가
}

#[derive(Debug, Clone)]
pub enum NonRecoverableErrorCategory {
    AuthenticationError,     // 401, 403
    NotFound,               // 404
    InvalidConfiguration,    // 설정 오류
    CriticalSystemError,    // 시스템 레벨 에러
}

#[derive(Debug, Clone)]
pub enum RateLimitSeverity {
    Light,      // 짧은 대기 후 재시도
    Moderate,   // 중간 정도 대기
    Severe,     // 긴 대기 필요
}

#[derive(Debug, Clone)]
pub enum NetworkErrorType {
    Timeout,
    ConnectionRefused,
    DnsResolution,
    SslError,
}

impl RetryManager {
    /// 새 재시도 관리자 생성
    pub fn new(max_retries: u32) -> Self {
        let failure_classifier = Arc::new(StandardFailureClassifier::new());
        
        Self {
            max_retries,
            retry_queue: Arc::new(Mutex::new(VecDeque::new())),
            failure_classifier,
            retry_history: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 실패한 작업을 재시도 큐에 추가
    pub async fn add_failed_item(&self, 
        item_id: String,
        stage: CrawlingStage,
        error: String,
        url: String,
        metadata: HashMap<String, String>
    ) -> Result<bool> {
        info!("🔄 Adding failed item to retry queue: {} (stage: {:?})", item_id, stage);
        
        // 에러 분류
        let classification = self.failure_classifier.classify_error(&error, stage.clone()).await;
        
        // 현재 시도 횟수 확인
        let current_attempts = self.get_retry_count(&item_id).await;
        
        // 재시도 가능 여부 확인
        if !self.failure_classifier.should_retry(&classification, current_attempts).await {
            warn!("❌ Item {} cannot be retried: {:?}", item_id, classification);
            return Ok(false);
        }
        
        if current_attempts >= self.max_retries {
            warn!("❌ Item {} exceeded max retries ({})", item_id, self.max_retries);
            return Ok(false);
        }
        
        // 백오프 시간 계산
        let backoff = self.failure_classifier.calculate_backoff(current_attempts + 1).await;
        let next_retry_time = chrono::Utc::now() + chrono::Duration::from_std(backoff)?;
        
        // 재시도 아이템 생성
        let retry_item = RetryItem {
            item_id: item_id.clone(),
            stage,
            attempt_count: current_attempts + 1,
            last_error: error.clone(),
            next_retry_time,
            exponential_backoff: backoff,
            original_url: url,
            metadata,
        };
        
        // 큐에 추가
        {
            let mut queue = self.retry_queue.lock().await;
            queue.push_back(retry_item);
        }
        
        // 기록 업데이트
        self.record_retry_attempt(&item_id, classification.clone(), backoff, false).await;
        
        info!("✅ Item {} scheduled for retry in {:?}", item_id, backoff);
        Ok(true)
    }
    
    /// 재시도 가능한 아이템 가져오기
    pub async fn get_ready_items(&self) -> Result<Vec<RetryItem>> {
        let mut queue = self.retry_queue.lock().await;
        let now = chrono::Utc::now();
        let mut ready_items = Vec::new();
        let mut remaining_items = VecDeque::new();
        
        while let Some(item) = queue.pop_front() {
            if item.next_retry_time <= now {
                debug!("🟢 Item {} is ready for retry", item.item_id);
                ready_items.push(item);
            } else {
                remaining_items.push_back(item);
            }
        }
        
        *queue = remaining_items;
        
        if !ready_items.is_empty() {
            info!("📤 Retrieved {} items ready for retry", ready_items.len());
        }
        
        Ok(ready_items)
    }
    
    /// 재시도 성공 기록
    pub async fn mark_retry_success(&self, item_id: &str) -> Result<()> {
        info!("✅ Retry succeeded for item: {}", item_id);
        self.record_retry_attempt(item_id, 
            ErrorClassification::Recoverable { 
                retry_after: Duration::from_secs(0), 
                category: RecoverableErrorCategory::TemporaryServerError 
            }, 
            Duration::from_secs(0), 
            true
        ).await;
        Ok(())
    }
    
    /// 재시도 기록 저장
    async fn record_retry_attempt(&self, item_id: &str, classification: ErrorClassification, backoff: Duration, success: bool) {
        let mut history = self.retry_history.write().await;
        let attempts = history.entry(item_id.to_string()).or_insert_with(Vec::new);
        
        attempts.push(RetryAttempt {
            attempt_number: attempts.len() as u32 + 1,
            attempted_at: chrono::Utc::now(),
            error_type: classification,
            backoff_duration: backoff,
            success,
        });
    }
    
    /// 아이템의 재시도 횟수 조회
    async fn get_retry_count(&self, item_id: &str) -> u32 {
        let history = self.retry_history.read().await;
        history.get(item_id).map(|attempts| attempts.len() as u32).unwrap_or(0)
    }
    
    /// 재시도 통계 조회
    pub async fn get_retry_stats(&self) -> RetryStats {
        let history = self.retry_history.read().await;
        let queue = self.retry_queue.lock().await;
        
        let total_items = history.len();
        let pending_retries = queue.len();
        let successful_retries = history.values()
            .flatten()
            .filter(|attempt| attempt.success)
            .count();
        let failed_retries = history.values()
            .flatten()
            .filter(|attempt| !attempt.success)
            .count();
            
        RetryStats {
            total_items,
            pending_retries,
            successful_retries,
            failed_retries,
            max_retries: self.max_retries,
        }
    }
}

/// 재시도 통계
#[derive(Debug, Clone, serde::Serialize)]
pub struct RetryStats {
    pub total_items: usize,
    pub pending_retries: usize,
    pub successful_retries: usize,
    pub failed_retries: usize,
    pub max_retries: u32,
}

/// 표준 실패 분류기 구현
pub struct StandardFailureClassifier {
    base_backoff: Duration,
    max_backoff: Duration,
}

impl StandardFailureClassifier {
    pub fn new() -> Self {
        Self {
            base_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(300), // 5분 최대
        }
    }
}

#[async_trait::async_trait]
impl FailureClassifier for StandardFailureClassifier {
    async fn classify_error(&self, error: &str, _stage: CrawlingStage) -> ErrorClassification {
        let error_lower = error.to_lowercase();
        
        // HTTP 상태 코드 기반 분류
        if error_lower.contains("429") || error_lower.contains("rate limit") {
            return ErrorClassification::RateLimited {
                retry_after: Duration::from_secs(60),
                severity: RateLimitSeverity::Moderate,
            };
        }
        
        if error_lower.contains("401") || error_lower.contains("403") {
            return ErrorClassification::NonRecoverable {
                reason: "Authentication or authorization error".to_string(),
                category: NonRecoverableErrorCategory::AuthenticationError,
            };
        }
        
        if error_lower.contains("404") {
            return ErrorClassification::NonRecoverable {
                reason: "Resource not found".to_string(),
                category: NonRecoverableErrorCategory::NotFound,
            };
        }
        
        if error_lower.contains("5") && (error_lower.contains("00") || error_lower.contains("02") || error_lower.contains("03")) {
            return ErrorClassification::Recoverable {
                retry_after: Duration::from_secs(5),
                category: RecoverableErrorCategory::TemporaryServerError,
            };
        }
        
        // 네트워크 에러 분류
        if error_lower.contains("timeout") {
            return ErrorClassification::NetworkError {
                retry_after: Duration::from_secs(10),
                error_type: NetworkErrorType::Timeout,
            };
        }
        
        if error_lower.contains("connection refused") {
            return ErrorClassification::NetworkError {
                retry_after: Duration::from_secs(30),
                error_type: NetworkErrorType::ConnectionRefused,
            };
        }
        
        // 파싱 에러
        if error_lower.contains("parse") || error_lower.contains("html") {
            return ErrorClassification::Recoverable {
                retry_after: Duration::from_secs(2),
                category: RecoverableErrorCategory::ParsingError,
            };
        }
        
        // 기본값: 일반적인 복구 가능 에러
        ErrorClassification::Recoverable {
            retry_after: Duration::from_secs(5),
            category: RecoverableErrorCategory::TemporaryServerError,
        }
    }
    
    async fn calculate_backoff(&self, attempt_count: u32) -> Duration {
        // 지수 백오프: base_backoff * 2^(attempt_count - 1) + jitter
        let exponential = self.base_backoff.as_secs() * 2_u64.pow(attempt_count.saturating_sub(1));
        let jitter = (attempt_count as u64) % 3; // 간단한 지터 (rand 대신)
        let total_secs = (exponential + jitter).min(self.max_backoff.as_secs());
        
        Duration::from_secs(total_secs)
    }
    
    async fn should_retry(&self, classification: &ErrorClassification, attempt_count: u32) -> bool {
        match classification {
            ErrorClassification::NonRecoverable { .. } => false,
            ErrorClassification::RateLimited { severity, .. } => {
                match severity {
                    RateLimitSeverity::Severe => attempt_count <= 1, // 심각한 rate limit은 1회만
                    _ => attempt_count <= 3,
                }
            },
            ErrorClassification::Recoverable { .. } => attempt_count <= 5,
            ErrorClassification::NetworkError { .. } => attempt_count <= 3,
        }
    }
}
