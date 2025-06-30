# Chapter 5.4: 트랜잭션 관리 및 에러 복구

## 개요

Matter Certis v2의 Rust/Tauri 구현에서 대용량 데이터 배치 처리 시 중요한 것은 데이터 일관성과 안정성입니다. 이 섹션에서는 SeaORM과 SQLx를 활용한 트랜잭션 관리, 에러 처리, 그리고 복구 메커니즘을 다룹니다.

## 5.4.1 트랜잭션 관리 시스템

### 기본 트랜잭션 매니저

```rust
// src/services/transaction_manager.rs
use sea_orm::{DatabaseConnection, DatabaseTransaction, TransactionTrait};
use sqlx::{Postgres, Transaction};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TransactionManager {
    db: Arc<DatabaseConnection>,
    active_transactions: Arc<Mutex<HashMap<Uuid, TransactionHandle>>>,
}

#[derive(Debug)]
pub struct TransactionHandle {
    id: Uuid,
    started_at: chrono::DateTime<chrono::Utc>,
    savepoints: Vec<String>,
    context: TransactionContext,
}

#[derive(Debug, Clone)]
pub struct TransactionContext {
    pub batch_id: String,
    pub operation_type: String,
    pub expected_items: usize,
    pub processed_items: usize,
    pub retry_count: usize,
}

impl TransactionManager {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            db,
            active_transactions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn begin_transaction(
        &self,
        context: TransactionContext,
    ) -> Result<Uuid, TransactionError> {
        let txn_id = Uuid::new_v4();
        let handle = TransactionHandle {
            id: txn_id,
            started_at: chrono::Utc::now(),
            savepoints: Vec::new(),
            context,
        };

        let mut active = self.active_transactions.lock().await;
        active.insert(txn_id, handle);

        Ok(txn_id)
    }

    pub async fn execute_with_transaction<F, T>(
        &self,
        context: TransactionContext,
        operation: F,
    ) -> Result<T, TransactionError>
    where
        F: FnOnce(&DatabaseTransaction) -> BoxFuture<'_, Result<T, TransactionError>>,
    {
        let txn = self.db.begin().await?;
        
        match operation(&txn).await {
            Ok(result) => {
                txn.commit().await?;
                Ok(result)
            }
            Err(e) => {
                let _ = txn.rollback().await;
                Err(e)
            }
        }
    }
}
```

### 중첩 트랜잭션 및 세이브포인트

```rust
// src/services/nested_transaction.rs
use sea_orm::{DatabaseTransaction, TransactionTrait};

pub struct NestedTransactionManager<'a> {
    txn: &'a DatabaseTransaction,
    savepoint_counter: Arc<AtomicUsize>,
}

impl<'a> NestedTransactionManager<'a> {
    pub fn new(txn: &'a DatabaseTransaction) -> Self {
        Self {
            txn,
            savepoint_counter: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub async fn create_savepoint(&self) -> Result<String, TransactionError> {
        let counter = self.savepoint_counter.fetch_add(1, Ordering::SeqCst);
        let savepoint_name = format!("sp_{}", counter);
        
        self.txn
            .execute_unprepared(&format!("SAVEPOINT {}", savepoint_name))
            .await?;
            
        Ok(savepoint_name)
    }

    pub async fn rollback_to_savepoint(&self, savepoint: &str) -> Result<(), TransactionError> {
        self.txn
            .execute_unprepared(&format!("ROLLBACK TO SAVEPOINT {}", savepoint))
            .await?;
        Ok(())
    }

    pub async fn release_savepoint(&self, savepoint: &str) -> Result<(), TransactionError> {
        self.txn
            .execute_unprepared(&format!("RELEASE SAVEPOINT {}", savepoint))
            .await?;
        Ok(())
    }
}
```

### 배치 트랜잭션 처리

```rust
// src/services/batch_transaction.rs
use crate::models::{DeviceInfo, BatchOperation};
use sea_orm::{DatabaseTransaction, Set, ActiveModelTrait};

pub struct BatchTransactionProcessor {
    transaction_manager: Arc<TransactionManager>,
    batch_size: usize,
}

impl BatchTransactionProcessor {
    pub async fn process_device_batch(
        &self,
        devices: Vec<DeviceInfo>,
        operation: BatchOperation,
    ) -> Result<BatchResult, TransactionError> {
        let context = TransactionContext {
            batch_id: Uuid::new_v4().to_string(),
            operation_type: operation.to_string(),
            expected_items: devices.len(),
            processed_items: 0,
            retry_count: 0,
        };

        self.transaction_manager
            .execute_with_transaction(context, |txn| {
                Box::pin(async move {
                    let mut results = Vec::new();
                    let nested_mgr = NestedTransactionManager::new(txn);

                    for chunk in devices.chunks(self.batch_size) {
                        let savepoint = nested_mgr.create_savepoint().await?;
                        
                        match self.process_chunk(txn, chunk, &operation).await {
                            Ok(chunk_result) => {
                                nested_mgr.release_savepoint(&savepoint).await?;
                                results.push(chunk_result);
                            }
                            Err(e) => {
                                warn!("Chunk processing failed, rolling back to savepoint: {}", e);
                                nested_mgr.rollback_to_savepoint(&savepoint).await?;
                                
                                // 개별 아이템 재시도
                                let individual_results = self
                                    .retry_individual_items(txn, chunk, &operation)
                                    .await?;
                                results.extend(individual_results);
                            }
                        }
                    }

                    Ok(BatchResult {
                        total_processed: results.iter().map(|r| r.success_count).sum(),
                        total_failed: results.iter().map(|r| r.failure_count).sum(),
                        results,
                    })
                })
            })
            .await
    }

    async fn process_chunk(
        &self,
        txn: &DatabaseTransaction,
        chunk: &[DeviceInfo],
        operation: &BatchOperation,
    ) -> Result<ChunkResult, TransactionError> {
        match operation {
            BatchOperation::Insert => self.batch_insert(txn, chunk).await,
            BatchOperation::Update => self.batch_update(txn, chunk).await,
            BatchOperation::Upsert => self.batch_upsert(txn, chunk).await,
        }
    }

    async fn retry_individual_items(
        &self,
        txn: &DatabaseTransaction,
        items: &[DeviceInfo],
        operation: &BatchOperation,
    ) -> Result<Vec<ChunkResult>, TransactionError> {
        let mut results = Vec::new();
        
        for item in items {
            let nested_mgr = NestedTransactionManager::new(txn);
            let savepoint = nested_mgr.create_savepoint().await?;
            
            match self.process_single_item(txn, item, operation).await {
                Ok(_) => {
                    nested_mgr.release_savepoint(&savepoint).await?;
                    results.push(ChunkResult {
                        success_count: 1,
                        failure_count: 0,
                        errors: Vec::new(),
                    });
                }
                Err(e) => {
                    nested_mgr.rollback_to_savepoint(&savepoint).await?;
                    results.push(ChunkResult {
                        success_count: 0,
                        failure_count: 1,
                        errors: vec![e.to_string()],
                    });
                }
            }
        }
        
        Ok(results)
    }
}
```

## 5.4.2 에러 처리 및 분류

### 에러 타입 정의

```rust
// src/errors/transaction_error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TransactionError {
    #[error("Database connection error: {0}")]
    ConnectionError(#[from] sea_orm::DbErr),
    
    #[error("Transaction timeout after {timeout}ms")]
    TimeoutError { timeout: u64 },
    
    #[error("Constraint violation: {constraint}")]
    ConstraintViolation { constraint: String },
    
    #[error("Deadlock detected in transaction {txn_id}")]
    DeadlockError { txn_id: String },
    
    #[error("Serialization failure: {details}")]
    SerializationFailure { details: String },
    
    #[error("Insufficient resources: {resource}")]
    ResourceExhausted { resource: String },
    
    #[error("Data validation error: {field} - {message}")]
    ValidationError { field: String, message: String },
    
    #[error("Retry limit exceeded: {attempts} attempts")]
    RetryLimitExceeded { attempts: usize },
    
    #[error("Custom error: {0}")]
    Custom(String),
}

#[derive(Debug, Clone)]
pub enum ErrorSeverity {
    Critical,    // 즉시 중단
    Recoverable, // 재시도 가능
    Warning,     // 계속 진행 가능
}

#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub severity: ErrorSeverity,
    pub retry_count: usize,
    pub max_retries: usize,
    pub backoff_delay: Duration,
    pub correlation_id: String,
}
```

### 에러 분류기

```rust
// src/services/error_classifier.rs
pub struct ErrorClassifier;

impl ErrorClassifier {
    pub fn classify_error(error: &TransactionError) -> ErrorContext {
        match error {
            TransactionError::DeadlockError { .. } => ErrorContext {
                severity: ErrorSeverity::Recoverable,
                retry_count: 0,
                max_retries: 5,
                backoff_delay: Duration::from_millis(100),
                correlation_id: Uuid::new_v4().to_string(),
            },
            
            TransactionError::SerializationFailure { .. } => ErrorContext {
                severity: ErrorSeverity::Recoverable,
                retry_count: 0,
                max_retries: 3,
                backoff_delay: Duration::from_millis(50),
                correlation_id: Uuid::new_v4().to_string(),
            },
            
            TransactionError::TimeoutError { .. } => ErrorContext {
                severity: ErrorSeverity::Recoverable,
                retry_count: 0,
                max_retries: 2,
                backoff_delay: Duration::from_secs(1),
                correlation_id: Uuid::new_v4().to_string(),
            },
            
            TransactionError::ConstraintViolation { .. } => ErrorContext {
                severity: ErrorSeverity::Warning,
                retry_count: 0,
                max_retries: 0,
                backoff_delay: Duration::ZERO,
                correlation_id: Uuid::new_v4().to_string(),
            },
            
            TransactionError::ConnectionError(_) => ErrorContext {
                severity: ErrorSeverity::Critical,
                retry_count: 0,
                max_retries: 10,
                backoff_delay: Duration::from_secs(5),
                correlation_id: Uuid::new_v4().to_string(),
            },
            
            _ => ErrorContext {
                severity: ErrorSeverity::Critical,
                retry_count: 0,
                max_retries: 0,
                backoff_delay: Duration::ZERO,
                correlation_id: Uuid::new_v4().to_string(),
            },
        }
    }

    pub fn should_retry(error_context: &ErrorContext) -> bool {
        match error_context.severity {
            ErrorSeverity::Critical => false,
            ErrorSeverity::Recoverable => error_context.retry_count < error_context.max_retries,
            ErrorSeverity::Warning => false,
        }
    }
}
```

## 5.4.3 재시도 메커니즘

### 백오프 전략

```rust
// src/services/retry_strategy.rs
use tokio::time::{sleep, Duration};
use rand::Rng;

#[derive(Debug, Clone)]
pub enum BackoffStrategy {
    Fixed(Duration),
    Linear(Duration),
    Exponential { base: Duration, max: Duration },
    ExponentialWithJitter { base: Duration, max: Duration },
}

pub struct RetryManager {
    strategy: BackoffStrategy,
    max_attempts: usize,
}

impl RetryManager {
    pub fn new(strategy: BackoffStrategy, max_attempts: usize) -> Self {
        Self {
            strategy,
            max_attempts,
        }
    }

    pub async fn execute_with_retry<F, T, E>(
        &self,
        mut operation: F,
    ) -> Result<T, TransactionError>
    where
        F: FnMut() -> BoxFuture<'_, Result<T, E>>,
        E: Into<TransactionError> + Clone,
    {
        let mut last_error = None;
        
        for attempt in 0..self.max_attempts {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    let tx_error = error.into();
                    let error_context = ErrorClassifier::classify_error(&tx_error);
                    
                    if !ErrorClassifier::should_retry(&error_context) {
                        return Err(tx_error);
                    }
                    
                    if attempt < self.max_attempts - 1 {
                        let delay = self.calculate_delay(attempt);
                        info!(
                            "Attempt {} failed, retrying in {:?}. Error: {}",
                            attempt + 1, delay, tx_error
                        );
                        sleep(delay).await;
                    }
                    
                    last_error = Some(tx_error);
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| {
            TransactionError::RetryLimitExceeded {
                attempts: self.max_attempts,
            }
        }))
    }

    fn calculate_delay(&self, attempt: usize) -> Duration {
        match &self.strategy {
            BackoffStrategy::Fixed(duration) => *duration,
            
            BackoffStrategy::Linear(base) => *base * (attempt as u32 + 1),
            
            BackoffStrategy::Exponential { base, max } => {
                let delay = *base * (2_u32.pow(attempt as u32));
                std::cmp::min(delay, *max)
            }
            
            BackoffStrategy::ExponentialWithJitter { base, max } => {
                let exponential_delay = *base * (2_u32.pow(attempt as u32));
                let capped_delay = std::cmp::min(exponential_delay, *max);
                
                let mut rng = rand::thread_rng();
                let jitter_factor = rng.gen_range(0.5..1.5);
                
                Duration::from_millis(
                    (capped_delay.as_millis() as f64 * jitter_factor) as u64
                )
            }
        }
    }
}
```

### 트랜잭션 재시도 래퍼

```rust
// src/services/transaction_retry.rs
pub struct TransactionRetryWrapper {
    transaction_manager: Arc<TransactionManager>,
    retry_manager: RetryManager,
}

impl TransactionRetryWrapper {
    pub fn new(
        transaction_manager: Arc<TransactionManager>,
        retry_strategy: BackoffStrategy,
        max_attempts: usize,
    ) -> Self {
        Self {
            transaction_manager,
            retry_manager: RetryManager::new(retry_strategy, max_attempts),
        }
    }

    pub async fn execute_with_retry<F, T>(
        &self,
        context: TransactionContext,
        operation: F,
    ) -> Result<T, TransactionError>
    where
        F: Fn(&DatabaseTransaction) -> BoxFuture<'_, Result<T, TransactionError>> + Clone,
    {
        let mut retry_context = context.clone();
        
        self.retry_manager
            .execute_with_retry(|| {
                let tx_mgr = self.transaction_manager.clone();
                let op = operation.clone();
                let mut ctx = retry_context.clone();
                
                Box::pin(async move {
                    ctx.retry_count += 1;
                    tx_mgr.execute_with_transaction(ctx, op).await
                })
            })
            .await
    }
}
```

## 5.4.4 복구 메커니즘

### 트랜잭션 로그 및 복구

```rust
// src/services/transaction_recovery.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransactionLogEntry {
    pub transaction_id: String,
    pub batch_id: String,
    pub operation_type: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub status: TransactionStatus,
    pub data_snapshot: Option<String>,
    pub error_details: Option<String>,
    pub recovery_attempts: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TransactionStatus {
    Started,
    InProgress,
    Committed,
    RolledBack,
    Failed,
    Recovering,
}

pub struct TransactionRecoveryManager {
    db: Arc<DatabaseConnection>,
    log_storage: Arc<dyn TransactionLogStorage>,
}

#[async_trait]
pub trait TransactionLogStorage: Send + Sync {
    async fn log_transaction(&self, entry: TransactionLogEntry) -> Result<(), TransactionError>;
    async fn get_failed_transactions(&self) -> Result<Vec<TransactionLogEntry>, TransactionError>;
    async fn update_transaction_status(
        &self,
        transaction_id: &str,
        status: TransactionStatus,
    ) -> Result<(), TransactionError>;
}

impl TransactionRecoveryManager {
    pub async fn recover_failed_transactions(&self) -> Result<RecoveryReport, TransactionError> {
        let failed_transactions = self.log_storage.get_failed_transactions().await?;
        let mut recovery_report = RecoveryReport::new();

        for transaction in failed_transactions {
            match self.attempt_recovery(&transaction).await {
                Ok(result) => {
                    recovery_report.add_success(transaction.transaction_id, result);
                }
                Err(e) => {
                    recovery_report.add_failure(transaction.transaction_id, e);
                }
            }
        }

        Ok(recovery_report)
    }

    async fn attempt_recovery(
        &self,
        transaction: &TransactionLogEntry,
    ) -> Result<RecoveryResult, TransactionError> {
        // 복구 시도 횟수 제한
        if transaction.recovery_attempts >= 3 {
            return Err(TransactionError::RetryLimitExceeded {
                attempts: transaction.recovery_attempts,
            });
        }

        // 상태에 따른 복구 전략
        match transaction.status {
            TransactionStatus::Failed => {
                self.recover_failed_transaction(transaction).await
            }
            TransactionStatus::InProgress => {
                self.recover_interrupted_transaction(transaction).await
            }
            _ => Err(TransactionError::Custom(
                "Transaction does not require recovery".to_string(),
            )),
        }
    }

    async fn recover_failed_transaction(
        &self,
        transaction: &TransactionLogEntry,
    ) -> Result<RecoveryResult, TransactionError> {
        // 데이터 일관성 검증
        let consistency_check = self.verify_data_consistency(&transaction.batch_id).await?;
        
        if consistency_check.is_consistent {
            return Ok(RecoveryResult::AlreadyConsistent);
        }

        // 스냅샷 데이터가 있는 경우 복원
        if let Some(snapshot_data) = &transaction.data_snapshot {
            self.restore_from_snapshot(snapshot_data).await?;
            return Ok(RecoveryResult::RestoredFromSnapshot);
        }

        // 재실행 시도
        self.retry_transaction(transaction).await
    }

    async fn verify_data_consistency(
        &self,
        batch_id: &str,
    ) -> Result<ConsistencyReport, TransactionError> {
        // 데이터 무결성 검증 로직
        // 외래 키 제약조건, 중복 데이터, 누락 데이터 등 확인
        
        let integrity_checks = vec![
            self.check_foreign_key_constraints(batch_id).await?,
            self.check_duplicate_records(batch_id).await?,
            self.check_missing_records(batch_id).await?,
        ];

        Ok(ConsistencyReport {
            is_consistent: integrity_checks.iter().all(|check| check.passed),
            issues: integrity_checks.into_iter().filter(|check| !check.passed).collect(),
        })
    }
}
```

### 자동 복구 스케줄러

```rust
// src/services/recovery_scheduler.rs
use tokio_cron_scheduler::{Job, JobScheduler};

pub struct RecoveryScheduler {
    scheduler: JobScheduler,
    recovery_manager: Arc<TransactionRecoveryManager>,
}

impl RecoveryScheduler {
    pub async fn new(recovery_manager: Arc<TransactionRecoveryManager>) -> Result<Self, TransactionError> {
        let scheduler = JobScheduler::new().await?;
        
        Ok(Self {
            scheduler,
            recovery_manager,
        })
    }

    pub async fn start_periodic_recovery(&self) -> Result<(), TransactionError> {
        let recovery_mgr = self.recovery_manager.clone();
        
        // 매시간 복구 작업 실행
        let job = Job::new_async("0 0 * * * *", move |_uuid, _l| {
            let mgr = recovery_mgr.clone();
            Box::pin(async move {
                info!("Starting scheduled transaction recovery");
                
                match mgr.recover_failed_transactions().await {
                    Ok(report) => {
                        info!("Recovery completed: {:?}", report);
                    }
                    Err(e) => {
                        error!("Recovery failed: {}", e);
                    }
                }
            })
        })?;

        self.scheduler.add(job).await?;
        self.scheduler.start().await?;
        
        info!("Recovery scheduler started");
        Ok(())
    }
}
```

## 5.4.5 모니터링 및 메트릭

### 트랜잭션 메트릭 수집

```rust
// src/services/transaction_metrics.rs
use prometheus::{Counter, Histogram, Gauge, Registry};
use std::sync::Arc;

pub struct TransactionMetrics {
    pub transaction_total: Counter,
    pub transaction_duration: Histogram,
    pub active_transactions: Gauge,
    pub rollback_total: Counter,
    pub retry_total: Counter,
    pub recovery_total: Counter,
}

impl TransactionMetrics {
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let transaction_total = Counter::new(
            "transaction_total",
            "Total number of transactions"
        )?;
        
        let transaction_duration = Histogram::new(
            "transaction_duration_seconds",
            "Transaction duration in seconds"
        )?;
        
        let active_transactions = Gauge::new(
            "active_transactions",
            "Number of active transactions"
        )?;
        
        let rollback_total = Counter::new(
            "transaction_rollback_total",
            "Total number of transaction rollbacks"
        )?;
        
        let retry_total = Counter::new(
            "transaction_retry_total",
            "Total number of transaction retries"
        )?;
        
        let recovery_total = Counter::new(
            "transaction_recovery_total",
            "Total number of transaction recoveries"
        )?;

        registry.register(Box::new(transaction_total.clone()))?;
        registry.register(Box::new(transaction_duration.clone()))?;
        registry.register(Box::new(active_transactions.clone()))?;
        registry.register(Box::new(rollback_total.clone()))?;
        registry.register(Box::new(retry_total.clone()))?;
        registry.register(Box::new(recovery_total.clone()))?;

        Ok(Self {
            transaction_total,
            transaction_duration,
            active_transactions,
            rollback_total,
            retry_total,
            recovery_total,
        })
    }
}

pub struct TransactionMonitor {
    metrics: Arc<TransactionMetrics>,
    health_checker: Arc<HealthChecker>,
}

impl TransactionMonitor {
    pub async fn record_transaction_start(&self) {
        self.metrics.transaction_total.inc();
        self.metrics.active_transactions.inc();
    }

    pub async fn record_transaction_end(&self, duration: Duration, success: bool) {
        self.metrics.transaction_duration.observe(duration.as_secs_f64());
        self.metrics.active_transactions.dec();
        
        if !success {
            self.metrics.rollback_total.inc();
        }
    }

    pub async fn record_retry(&self) {
        self.metrics.retry_total.inc();
    }

    pub async fn record_recovery(&self) {
        self.metrics.recovery_total.inc();
    }
}
```

이 섹션에서는 Rust/Tauri 환경에서의 강력한 트랜잭션 관리와 에러 복구 시스템을 구축하는 방법을 다루었습니다. 다음으로 5.5 성능 최적화 섹션을 작성하겠습니다.
