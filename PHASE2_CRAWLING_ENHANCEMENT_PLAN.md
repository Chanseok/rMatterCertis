# Phase 2: 크롤링 엔진 성능과 안정성 향상 계획

## 🎯 목표
크롤링 알고리즘 문서(.local/crawling_explanation.md)에 정의된 **배치 처리**와 **다단계 재시도** 메커니즘을 현재 구현에 완전히 적용하여 엔터프라이즈급 안정성 달성

## 📊 현재 상태 vs 목표 상태

### ✅ 이미 구현된 요소들
- [x] 4단계 크롤링 사이클 (Stage 1-4)
- [x] BatchProcessor → 3개 엔진으로 분화됨
- [x] TaskExecutor → 각 엔진의 stage 메서드들
- [x] 기본적인 배치 처리 (batch_size 기반)
- [x] 실시간 이벤트 시스템
- [x] 서비스 분리 아키텍처

### ⚠️ 개선 필요 영역들
- [ ] **통합된 재시도 메커니즘** (현재 각 엔진마다 다름)
- [ ] **CrawlerManager 역할 통합** (현재 분산됨)
- [ ] **실제 실시간 이벤트 연동** (프론트엔드까지)
- [ ] **성능 모니터링 및 적응형 최적화**
- [ ] **메모리 기반 상태 관리 강화**

## 🗓️ Phase 2 실행 계획

### **Week 1: 크롤링 엔진 통합 및 재시도 메커니즘**

#### 2.1.1 통합 CrawlerManager 구현 (2일)
```rust
// 목표: .local/crawling_explanation.md의 CrawlerManager 역할 구현
pub struct CrawlerManager {
    batch_processor: Arc<dyn BatchProcessor>,
    session_manager: Arc<SessionManager>,
    retry_manager: Arc<RetryManager>,
    performance_monitor: Arc<PerformanceMonitor>,
}

impl CrawlerManager {
    pub async fn start_batch_crawling(&self, config: CrawlingConfig) -> Result<String>;
    pub async fn pause_crawling(&self, session_id: &str) -> Result<()>;
    pub async fn resume_crawling(&self, session_id: &str) -> Result<()>;
    pub async fn stop_crawling(&self, session_id: &str) -> Result<()>;
}
```

#### 2.1.2 다단계 재시도 메커니즘 강화 (2일)
```rust
// 목표: crawling_explanation.md의 재시도 메커니즘 완전 구현
pub struct RetryManager {
    max_retries: u32,
    retry_queue: Arc<Mutex<VecDeque<RetryItem>>>,
    failure_classifier: Arc<dyn FailureClassifier>,
}

#[derive(Debug, Clone)]
pub struct RetryItem {
    pub item_id: String,
    pub stage: CrawlingStage,
    pub attempt_count: u32,
    pub last_error: String,
    pub next_retry_time: DateTime<Utc>,
    pub exponential_backoff: Duration,
}
```

#### 2.1.3 BatchProcessor 트레이트 통합 (1일)
```rust
// 목표: 3개 엔진을 하나의 인터페이스로 통합
#[async_trait]
pub trait BatchProcessor: Send + Sync {
    async fn execute_task(&self, item: CrawlingItem) -> Result<TaskResult>;
    async fn handle_task_success(&self, item: CrawlingItem, result: TaskResult);
    async fn handle_task_failure(&self, item: CrawlingItem, error: CrawlerError);
    async fn get_progress(&self) -> CrawlingProgress;
}

// 기존 3개 엔진을 이 트레이트로 래핑
impl BatchProcessor for AdvancedBatchCrawlingEngine { ... }
```

### **Week 2: 실시간 이벤트 시스템 완성**

#### 2.2.1 이벤트 기반 실시간 통신 (3일)
- [x] 백엔드 이벤트 발송 (이미 구현됨)
- [ ] 프론트엔드 이벤트 수신 최적화
- [ ] 이벤트 버퍼링 및 배치 처리
- [ ] 연결 상태 모니터링 및 복구

```typescript
// 목표: crawlerStore.ts에서 실시간 이벤트 완전 활용
export class CrawlerStore {
  async startRealTimeUpdates(): Promise<void> {
    // 세분화된 이벤트 구독
    await tauriApi.subscribeToProgress(this.handleProgressUpdate);
    await tauriApi.subscribeToStageChange(this.handleStageChange);
    await tauriApi.subscribeToTaskStatus(this.handleTaskStatus);
    await tauriApi.subscribeToError(this.handleError);
  }
}
```

#### 2.2.2 에러 처리 체계화 (2일)
```rust
// 목표: 계층별 에러 전파 및 복구 전략
pub enum CrawlerError {
    NetworkError { url: String, retry_after: Duration },
    ParsingError { stage: CrawlingStage, recoverable: bool },
    DatabaseError { operation: String, rollback_required: bool },
    ConfigurationError { field: String, suggestion: String },
}

pub struct ErrorRecoveryStrategy {
    pub retry_count: u32,
    pub backoff_strategy: BackoffStrategy,
    pub recovery_action: RecoveryAction,
}
```

### **Week 3: 성능 최적화 및 모니터링**

#### 2.3.1 적응형 성능 최적화 (3일)
```rust
// 목표: 런타임 성능 모니터링 및 자동 조정
pub struct AdaptivePerformanceManager {
    current_concurrency: Arc<AtomicU32>,
    response_time_tracker: Arc<Mutex<VecDeque<Duration>>>,
    error_rate_tracker: Arc<Mutex<VecDeque<f64>>>,
}

impl AdaptivePerformanceManager {
    pub async fn adjust_concurrency(&self, metrics: &PerformanceMetrics);
    pub async fn adjust_delays(&self, response_times: &[Duration]);
    pub async fn handle_rate_limiting(&self, retry_after: Duration);
}
```

#### 2.3.2 메모리 사용량 최적화 (2일)
- [ ] 메모리 프로파일링 도구 통합
- [ ] 대용량 데이터 스트리밍 처리
- [ ] 가비지 컬렉션 최적화
- [ ] 메모리 누수 방지 메커니즘

## 🎯 성공 기준

### Phase 2.1 완료 기준
- [ ] CrawlerManager가 3개 엔진을 통합 관리
- [ ] 재시도 메커니즘이 exponential backoff로 동작
- [ ] BatchProcessor 트레이트로 엔진 추상화 완료

### Phase 2.2 완료 기준  
- [ ] 실시간 이벤트가 프론트엔드까지 완전 연동
- [ ] 에러 복구 자동화 90%+ 달성
- [ ] 연결 끊김 시 자동 재연결 동작

### Phase 2.3 완료 기준
- [ ] 성능이 기준치 대비 50% 향상
- [ ] 메모리 사용량 30% 절약
- [ ] 평균 응답시간 1초 이하 유지

## 📊 예상 효과

1. **안정성**: 네트워크 오류 자동 복구 95%+
2. **성능**: 처리 속도 2배 향상 
3. **사용자 경험**: 실시간 피드백으로 대기시간 체감 50% 감소
4. **유지보수성**: 통합된 아키텍처로 코드 복잡도 30% 감소

---

**이 계획을 통해 .local/crawling_explanation.md에 정의된 이상적인 크롤링 알고리즘을 현실에서 완전히 구현하게 됩니다.**
