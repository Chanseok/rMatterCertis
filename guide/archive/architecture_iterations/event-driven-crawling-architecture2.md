# 이벤트 기반 크롤링 아키텍처 v2.0 (진화된 설계)

이 문서는 기존 이벤트 기반 크롤링 아키텍처의 장점을 극대화하면서, 식별된 단점들을 체계적으로 보완한 **차세대 견고한 크롤링 시스템**을 제안합니다. 

기존 v1.0의 핵심 원칙(명확한 분리, 전문화, 비동기 통신, 중앙 집중 제어)을 유지하되, **백프레셔 제어, 견고한 오류 처리, 적응적 리소스 관리, 운영 가시성**을 추가하여 실제 프로덕션 환경에서 안정적으로 운영 가능한 시스템으로 발전시켰습니다.

---

## 1. 핵심 개선사항 요약

### 1.1. 백프레셔 및 리소스 관리
- **Bounded Channels**: 메모리 압박 방지를 위한 큐 크기 제한
- **다층 세마포어 시스템**: HTTP, CPU, 메모리, DB I/O 각각에 대한 독립적 제한
- **적응적 스케일링**: 실시간 성능 지표 기반 작업자 수 동적 조절

### 1.2. 견고한 오류 처리
- **계층적 재시도 정책**: 지수 백오프, 회로 차단기, Dead Letter Queue
- **부분 실패 복구**: 체크포인트 기반 중단점 재시작
- **중복 방지**: Bloom Filter 기반 URL 중복 검사

### 1.3. 운영 가시성 및 제어
- **상세한 메트릭**: 큐 깊이, 작업자 활용도, 처리량, 오류율
- **동적 설정**: 런타임 정책 변경 가능
- **Health Check**: 각 구성요소의 상태 모니터링

---

## 2. 진화된 아키텍처 구성 요소

### 2.1. `CrawlingTask` v2.0 - 메타데이터 강화

```rust
/// 크롤링 작업 단위 (v2.0) - 메타데이터와 제어 정보 포함
#[derive(Debug, Clone)]
pub enum CrawlingTask {
    FetchListPage { 
        page: u32, 
        metadata: TaskMetadata 
    },
    ParseListPageHtml { 
        page: u32, 
        html_content: String, 
        metadata: TaskMetadata 
    },
    FetchProductDetailPage { 
        product_url: String, 
        metadata: TaskMetadata 
    },
    ParseProductDetailHtml { 
        product_url: String, 
        html_content: String, 
        metadata: TaskMetadata 
    },
    SaveProduct { 
        product_detail: ProductDetail, 
        metadata: TaskMetadata 
    },
}

/// 각 작업에 첨부되는 메타데이터
#[derive(Debug, Clone)]
pub struct TaskMetadata {
    pub task_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub retry_count: u32,
    pub priority: TaskPriority,
    pub source_session: String,
    pub parent_task_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    Critical = 0,    // 시스템 중요 작업
    High = 1,        // 재시도 작업
    Normal = 2,      // 일반 작업
    Low = 3,         // 백그라운드 작업
}
```

### 2.2. `BoundedWorkQueue` - 백프레셔 제어 큐

```rust
/// 백프레셔 제어가 가능한 작업 큐
pub struct BoundedWorkQueue<T> {
    sender: mpsc::Sender<T>,
    receiver: mpsc::Receiver<T>,
    capacity: usize,
    metrics: Arc<Mutex<QueueMetrics>>,
}

#[derive(Debug, Default)]
pub struct QueueMetrics {
    pub current_depth: usize,
    pub max_depth_reached: usize,
    pub total_enqueued: u64,
    pub total_dequeued: u64,
    pub blocked_enqueue_count: u64,
}

impl<T> BoundedWorkQueue<T> {
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = mpsc::channel(capacity);
        // ... 구현
    }
    
    /// 큐가 가득 찰 때 블로킹되는 송신
    pub async fn enqueue(&self, item: T) -> Result<(), QueueError> {
        match self.sender.try_send(item) {
            Ok(_) => {
                self.update_metrics_enqueue();
                Ok(())
            }
            Err(mpsc::error::TrySendError::Full(item)) => {
                // 백프레셔 발생 - 블로킹 송신으로 전환
                self.metrics.lock().await.blocked_enqueue_count += 1;
                self.sender.send(item).await.map_err(|_| QueueError::Closed)?;
                Ok(())
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Err(QueueError::Closed),
        }
    }
}
```

### 2.3. `ResourceManager` - 다층 리소스 제어

```rust
/// 시스템 리소스를 계층적으로 관리하는 매니저
pub struct ResourceManager {
    pub http_semaphore: Arc<Semaphore>,
    pub cpu_semaphore: Arc<Semaphore>,
    pub memory_semaphore: Arc<Semaphore>,
    pub db_semaphore: Arc<Semaphore>,
    pub resource_monitor: Arc<ResourceMonitor>,
}

impl ResourceManager {
    pub fn new(config: &ResourceConfig) -> Self {
        Self {
            http_semaphore: Arc::new(Semaphore::new(config.max_concurrent_http)),
            cpu_semaphore: Arc::new(Semaphore::new(config.max_concurrent_cpu_tasks)),
            memory_semaphore: Arc::new(Semaphore::new(config.max_memory_intensive_tasks)),
            db_semaphore: Arc::new(Semaphore::new(config.max_concurrent_db_ops)),
            resource_monitor: Arc::new(ResourceMonitor::new()),
        }
    }
    
    /// 리소스 사용량 기반 적응적 조절
    pub async fn adjust_limits(&self) {
        let usage = self.resource_monitor.get_current_usage().await;
        
        if usage.memory_pressure > 0.8 {
            // 메모리 압박 시 동시 작업 수 감소
            self.reduce_concurrent_limits().await;
        } else if usage.cpu_utilization < 0.5 {
            // CPU 여유 시 동시 작업 수 증가
            self.increase_concurrent_limits().await;
        }
    }
}
```

### 2.4. `RetryPolicy` - 계층적 재시도 시스템

```rust
/// 계층적 재시도 정책
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
    pub jitter: bool,
}

impl RetryPolicy {
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        let delay = self.base_delay.as_millis() as f64 
            * self.backoff_multiplier.powi(attempt as i32);
        
        let delay = delay.min(self.max_delay.as_millis() as f64);
        
        let delay = if self.jitter {
            delay * (0.5 + rand::random::<f64>() * 0.5)
        } else {
            delay
        };
        
        Duration::from_millis(delay as u64)
    }
}

/// 재시도 관리자
pub struct RetryManager {
    retry_queue: BoundedWorkQueue<RetryTask>,
    dead_letter_queue: BoundedWorkQueue<DeadLetterTask>,
    circuit_breakers: HashMap<String, CircuitBreaker>,
    policies: HashMap<String, RetryPolicy>,
}

#[derive(Debug)]
pub struct RetryTask {
    pub original_task: CrawlingTask,
    pub error: CrawlingError,
    pub next_retry_at: DateTime<Utc>,
}
```

### 2.5. `CircuitBreaker` - 회로 차단기

```rust
/// 회로 차단기 - 연속 실패 시 일시적 차단
#[derive(Debug)]
pub struct CircuitBreaker {
    state: Arc<Mutex<CircuitBreakerState>>,
    failure_threshold: u32,
    recovery_timeout: Duration,
    half_open_max_calls: u32,
}

#[derive(Debug, Clone)]
enum CircuitBreakerState {
    Closed { failure_count: u32 },
    Open { opened_at: DateTime<Utc> },
    HalfOpen { success_count: u32, failure_count: u32 },
}

impl CircuitBreaker {
    pub async fn call<F, T, E>(&self, f: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: FnOnce() -> Result<T, E>,
    {
        match self.can_execute().await? {
            true => {
                match f() {
                    Ok(result) => {
                        self.record_success().await;
                        Ok(result)
                    }
                    Err(error) => {
                        self.record_failure().await;
                        Err(CircuitBreakerError::ExecutionFailed(error))
                    }
                }
            }
            false => Err(CircuitBreakerError::CircuitOpen),
        }
    }
}
```

### 2.6. `DuplicationFilter` - 중복 방지 시스템

```rust
/// Bloom Filter 기반 중복 URL 검사
pub struct DuplicationFilter {
    bloom_filter: Arc<Mutex<BloomFilter>>,
    processed_urls: Arc<Mutex<HashSet<String>>>,
    persistent_store: Arc<dyn PersistentUrlStore>,
}

impl DuplicationFilter {
    pub async fn is_duplicate(&self, url: &str) -> bool {
        // 1단계: 빠른 Bloom Filter 검사
        if !self.bloom_filter.lock().await.contains(url) {
            return false;
        }
        
        // 2단계: 정확한 HashSet 검사
        if self.processed_urls.lock().await.contains(url) {
            return true;
        }
        
        // 3단계: 영구 저장소 검사 (캐시 미스 시)
        self.persistent_store.contains(url).await
    }
    
    pub async fn mark_as_processed(&self, url: &str) {
        self.bloom_filter.lock().await.insert(url);
        self.processed_urls.lock().await.insert(url.to_string());
        self.persistent_store.insert(url).await;
    }
}
```

### 2.7. `CheckpointManager` - 체크포인트 시스템

```rust
/// 크롤링 진행 상태 체크포인트 관리
pub struct CheckpointManager {
    storage: Arc<dyn CheckpointStorage>,
    checkpoint_interval: Duration,
    last_checkpoint: Arc<Mutex<DateTime<Utc>>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CrawlingCheckpoint {
    pub session_id: String,
    pub timestamp: DateTime<Utc>,
    pub completed_pages: Vec<u32>,
    pub failed_tasks: Vec<FailedTask>,
    pub queue_states: HashMap<String, QueueState>,
    pub statistics: CrawlingStats,
}

impl CheckpointManager {
    pub async fn save_checkpoint(&self, state: &CrawlingState) -> Result<(), CheckpointError> {
        let checkpoint = CrawlingCheckpoint {
            session_id: state.session_id.clone(),
            timestamp: Utc::now(),
            completed_pages: state.get_completed_pages(),
            failed_tasks: state.get_failed_tasks(),
            queue_states: state.get_queue_states(),
            statistics: state.stats.clone(),
        };
        
        self.storage.save(&checkpoint).await
    }
    
    pub async fn restore_from_checkpoint(&self, session_id: &str) -> Option<CrawlingCheckpoint> {
        self.storage.load(session_id).await
    }
}
```

### 2.8. `AdvancedSharedState` - 강화된 공유 상태

```rust
/// 강화된 공유 상태 (v2.0)
pub struct AdvancedSharedState {
    // 기본 제어
    pub cancellation_token: CancellationToken,
    pub pause_token: PauseToken,
    
    // 리소스 관리
    pub resource_manager: Arc<ResourceManager>,
    
    // 오류 처리
    pub retry_manager: Arc<RetryManager>,
    
    // 중복 방지
    pub duplication_filter: Arc<DuplicationFilter>,
    
    // 체크포인트
    pub checkpoint_manager: Arc<CheckpointManager>,
    
    // 메트릭 및 모니터링
    pub metrics_collector: Arc<MetricsCollector>,
    
    // 동적 설정
    pub config_manager: Arc<ConfigManager>,
}

#[derive(Debug)]
pub struct MetricsCollector {
    pub queue_metrics: HashMap<String, Arc<Mutex<QueueMetrics>>>,
    pub worker_metrics: HashMap<String, Arc<Mutex<WorkerMetrics>>>,
    pub system_metrics: Arc<Mutex<SystemMetrics>>,
    pub error_metrics: Arc<Mutex<ErrorMetrics>>,
}

#[derive(Debug, Default)]
pub struct WorkerMetrics {
    pub active_workers: usize,
    pub total_tasks_processed: u64,
    pub average_processing_time: Duration,
    pub current_utilization: f64,
    pub last_activity: DateTime<Utc>,
}
```

---

## 3. 진화된 워크플로우

### 3.1. 적응적 데이터 흐름

```rust
/// 적응적 작업자 풀 - 성능 기반 동적 스케일링
pub struct AdaptiveWorkerPool<T> {
    workers: Vec<JoinHandle<()>>,
    metrics: Arc<Mutex<WorkerMetrics>>,
    config: WorkerPoolConfig,
    scaling_policy: ScalingPolicy,
}

impl<T> AdaptiveWorkerPool<T> {
    pub async fn adjust_worker_count(&mut self) {
        let metrics = self.metrics.lock().await;
        let current_count = self.workers.len();
        
        let target_count = self.scaling_policy.calculate_target_workers(
            current_count,
            metrics.current_utilization,
            metrics.average_processing_time,
        );
        
        if target_count > current_count {
            self.scale_up(target_count - current_count).await;
        } else if target_count < current_count {
            self.scale_down(current_count - target_count).await;
        }
    }
}

#[derive(Debug)]
pub struct ScalingPolicy {
    pub min_workers: usize,
    pub max_workers: usize,
    pub target_utilization: f64,
    pub scale_up_threshold: f64,
    pub scale_down_threshold: f64,
    pub cooldown_period: Duration,
}
```

### 3.2. 견고한 오류 처리 흐름

```rust
/// 강화된 작업자 루프 - 오류 처리 및 재시도 포함
async fn resilient_worker_loop<T>(
    queue: Arc<BoundedWorkQueue<T>>,
    shared_state: Arc<AdvancedSharedState>,
    worker_id: String,
) where
    T: CrawlingTask + Send + 'static,
{
    let mut consecutive_failures = 0;
    let max_consecutive_failures = 5;
    
    loop {
        tokio::select! {
            biased;
            
            // 취소 신호 처리
            _ = shared_state.cancellation_token.cancelled() => {
                info!("Worker {} shutting down due to cancellation", worker_id);
                break;
            }
            
            // 일시정지 신호 처리
            _ = shared_state.pause_token.paused() => {
                info!("Worker {} paused", worker_id);
                shared_state.pause_token.wait_for_resume().await;
                info!("Worker {} resumed", worker_id);
                continue;
            }
            
            // 작업 처리
            result = queue.dequeue() => {
                match result {
                    Ok(Some(task)) => {
                        match process_task_with_retry(task, &shared_state).await {
                            Ok(_) => {
                                consecutive_failures = 0;
                                shared_state.metrics_collector.record_success(&worker_id).await;
                            }
                            Err(error) => {
                                consecutive_failures += 1;
                                shared_state.metrics_collector.record_failure(&worker_id, &error).await;
                                
                                if consecutive_failures >= max_consecutive_failures {
                                    warn!("Worker {} has {} consecutive failures, entering recovery mode", 
                                          worker_id, consecutive_failures);
                                    tokio::time::sleep(Duration::from_secs(30)).await;
                                    consecutive_failures = 0;
                                }
                            }
                        }
                    }
                    Ok(None) => {
                        // 큐가 비어있음 - 잠시 대기
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    Err(error) => {
                        error!("Worker {} queue error: {:?}", worker_id, error);
                        break;
                    }
                }
            }
        }
    }
}

/// 재시도 로직이 포함된 작업 처리
async fn process_task_with_retry<T>(
    task: T,
    shared_state: &AdvancedSharedState,
) -> Result<(), CrawlingError> 
where
    T: CrawlingTask,
{
    let task_type = task.get_type();
    let circuit_breaker = shared_state.retry_manager
        .get_circuit_breaker(&task_type);
    
    circuit_breaker.call(|| async {
        // 실제 작업 처리
        process_task(task, shared_state).await
    }).await.map_err(|e| match e {
        CircuitBreakerError::CircuitOpen => CrawlingError::CircuitOpen,
        CircuitBreakerError::ExecutionFailed(err) => err,
    })
}
```

### 3.3. 지능적 큐 관리

```rust
/// 우선순위 기반 큐 관리자
pub struct PriorityQueueManager {
    queues: HashMap<TaskPriority, BoundedWorkQueue<CrawlingTask>>,
    scheduler: TaskScheduler,
}

impl PriorityQueueManager {
    pub async fn enqueue_with_priority(&self, task: CrawlingTask) -> Result<(), QueueError> {
        let priority = task.get_metadata().priority;
        
        // 중복 검사
        if self.is_duplicate(&task).await {
            return Ok(()); // 중복 작업 무시
        }
        
        // 우선순위 큐에 추가
        self.queues.get(&priority)
            .ok_or(QueueError::InvalidPriority)?
            .enqueue(task)
            .await
    }
    
    pub async fn dequeue_by_priority(&self) -> Option<CrawlingTask> {
        // 높은 우선순위부터 확인
        for priority in [TaskPriority::Critical, TaskPriority::High, 
                        TaskPriority::Normal, TaskPriority::Low] {
            if let Some(queue) = self.queues.get(&priority) {
                if let Ok(Some(task)) = queue.try_dequeue() {
                    return Some(task);
                }
            }
        }
        None
    }
}
```

---

## 4. 운영 모니터링 및 제어

### 4.1. 실시간 대시보드 메트릭

```rust
/// 실시간 크롤링 대시보드용 메트릭
#[derive(Debug, Serialize)]
pub struct DashboardMetrics {
    pub overall_status: CrawlingStatus,
    pub throughput: ThroughputMetrics,
    pub queue_health: HashMap<String, QueueHealthMetrics>,
    pub worker_health: HashMap<String, WorkerHealthMetrics>,
    pub error_summary: ErrorSummaryMetrics,
    pub resource_usage: ResourceUsageMetrics,
    pub estimated_completion: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct QueueHealthMetrics {
    pub current_depth: usize,
    pub capacity: usize,
    pub utilization_percentage: f64,
    pub enqueue_rate: f64,
    pub dequeue_rate: f64,
    pub backpressure_incidents: u64,
}

#[derive(Debug, Serialize)]
pub struct WorkerHealthMetrics {
    pub active_count: usize,
    pub total_count: usize,
    pub average_task_duration: Duration,
    pub tasks_per_minute: f64,
    pub error_rate: f64,
    pub last_activity: DateTime<Utc>,
}
```

### 4.2. 동적 설정 관리

```rust
/// 런타임 설정 변경 가능한 설정 관리자
pub struct ConfigManager {
    config: Arc<RwLock<CrawlingConfig>>,
    config_watchers: Vec<ConfigWatcher>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingConfig {
    pub resource_limits: ResourceLimits,
    pub retry_policies: HashMap<String, RetryPolicy>,
    pub queue_capacities: HashMap<String, usize>,
    pub worker_counts: HashMap<String, WorkerCountConfig>,
    pub rate_limits: HashMap<String, RateLimit>,
}

impl ConfigManager {
    pub async fn update_config(&self, updates: ConfigUpdates) -> Result<(), ConfigError> {
        let mut config = self.config.write().await;
        
        // 설정 검증
        self.validate_config_updates(&updates)?;
        
        // 설정 적용
        config.apply_updates(updates);
        
        // 변경사항을 모든 watchers에게 알림
        for watcher in &self.config_watchers {
            watcher.notify_config_changed(&config).await;
        }
        
        Ok(())
    }
}
```

### 4.3. Health Check 시스템

```rust
/// 시스템 상태 검사
pub struct HealthChecker {
    checks: Vec<Box<dyn HealthCheck>>,
    check_interval: Duration,
}

#[async_trait]
pub trait HealthCheck: Send + Sync {
    async fn check(&self) -> HealthStatus;
    fn name(&self) -> &str;
}

#[derive(Debug)]
pub struct HealthStatus {
    pub is_healthy: bool,
    pub details: HashMap<String, String>,
    pub last_checked: DateTime<Utc>,
}

pub struct QueueHealthCheck {
    queues: HashMap<String, Arc<BoundedWorkQueue<CrawlingTask>>>,
}

#[async_trait]
impl HealthCheck for QueueHealthCheck {
    async fn check(&self) -> HealthStatus {
        let mut is_healthy = true;
        let mut details = HashMap::new();
        
        for (name, queue) in &self.queues {
            let metrics = queue.get_metrics().await;
            let utilization = metrics.current_depth as f64 / queue.capacity() as f64;
            
            if utilization > 0.9 {
                is_healthy = false;
                details.insert(format!("{}_queue", name), 
                             format!("High utilization: {:.1}%", utilization * 100.0));
            }
        }
        
        HealthStatus {
            is_healthy,
            details,
            last_checked: Utc::now(),
        }
    }
    
    fn name(&self) -> &str {
        "queue_health"
    }
}
```

---

## 5. 아키텍처 v2.0의 주요 장점

### 5.1. 견고성 (Resilience)
- **자동 복구**: 회로 차단기와 재시도 메커니즘으로 일시적 장애 극복
- **부분 실패 격리**: 특정 작업자나 큐의 실패가 전체 시스템에 미치는 영향 최소화
- **체크포인트 기반 재시작**: 중단 시점부터 작업 재개 가능

### 5.2. 확장성 (Scalability)
- **적응적 스케일링**: 실시간 성능 지표 기반 작업자 수 자동 조절
- **백프레셔 제어**: 메모리 압박 상황에서도 안정적 동작
- **우선순위 기반 처리**: 중요한 작업 우선 처리로 효율성 극대화

### 5.3. 운영성 (Operability)
- **종합적 모니터링**: 큐, 작업자, 시스템 리소스 상태 실시간 가시성
- **동적 설정**: 운영 중 설정 변경으로 유연한 튜닝 가능
- **예측 가능한 완료 시간**: 현재 진행률 기반 완료 시간 예측

### 5.4. 성능 (Performance)
- **지능적 리소스 관리**: CPU, 메모리, 네트워크, DB 각각 독립적 최적화
- **중복 제거**: Bloom Filter 기반 효율적 중복 URL 검사
- **배치 처리**: 관련 작업들의 배치 처리로 처리량 향상

---

## 6. 구현 우선순위

### Phase 1: 핵심 견고성 구현
1. `BoundedWorkQueue` 구현
2. 기본 재시도 메커니즘 (`RetryManager`)
3. `CircuitBreaker` 구현
4. 강화된 작업자 루프

### Phase 2: 모니터링 및 가시성
1. `MetricsCollector` 구현
2. Health Check 시스템
3. 실시간 대시보드 메트릭
4. 로깅 및 추적 시스템

### Phase 3: 고급 기능
1. `CheckpointManager` 구현
2. 동적 설정 관리 (`ConfigManager`)
3. 적응적 스케일링 (`AdaptiveWorkerPool`)
4. 우선순위 기반 큐 관리

### Phase 4: 최적화 및 정교화
1. 성능 프로파일링 도구
2. 예측 분석 (완료 시간 예측)
3. 자동 튜닝 알고리즘
4. 분산 처리 지원

이러한 진화된 아키텍처를 통해 기존 v1.0의 장점을 유지하면서도 실제 프로덕션 환경에서 안정적이고 효율적으로 운영할 수 있는 견고한 크롤링 시스템을 구축할 수 있습니다.
