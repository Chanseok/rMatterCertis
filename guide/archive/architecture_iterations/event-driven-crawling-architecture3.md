# 이벤트 기반 크롤링 아키텍처 v3.0: 지능형 적응 제어 시스템

이 문서는 이벤트 기반 아키텍처(v2.0)의 견고함을 기반으로, **예측 분석(Predictive Analytics)과 동적 자원 제어** 기능을 통합하여, 시스템이 스스로를 진단하고 미래를 예측하며 사용자에게 실질적인 제어권을 제공하는 차세대 크롤링 시스템을 제안합니다.

**v3.0의 비전: 단순 실행을 넘어, 예측하고, 적응하고, 사용자에게 선택권을 제공하는 시스템**

---

## 1. v3.0의 핵심 진화 원칙

기존 v2.0의 원칙(분리, 전문화, 비동기 통신, 중앙 제어, 견고성, 확장성, 운영성)에 다음 세 가지 핵심 원칙을 추가합니다.

1.  **예측성 (Predictability):** 모든 작업 단위의 성능(소요 시간, 메모리 사용량)을 측정하고, 이 데이터를 기반으로 전체 작업의 완료 시간과 리소스 소요량을 지속적으로 예측하고 보정합니다.

2.  **적응성 (Adaptability):** 실시간 메트릭과 예측된 부하를 기반으로 시스템이 스스로 작업자 수, 동시성 제한 등 내부 파라미터를 동적으로 조절하여 최적의 성능을 유지합니다.

3.  **제어성 (Controllability):** 사용자에게 단순한 시작/중지 이상의 제어권을 제공합니다. 예측된 정보를 바탕으로 "속도 우선", "안정성 우선" 등 운영 프로파일을 선택하고, 시스템이 그에 맞게 동작 방식을 변경할 수 있게 합니다.

---

## 2. v3.0 아키텍처 구성 요소: 예측과 제어의 추가

v2.0의 모든 구성 요소를 계승하며, 다음 구성 요소들이 추가되거나 강화됩니다.

### 2.1. `CrawlingTask` v3.0 - 성능 측정 메타데이터 추가

모든 작업 단위에 성능 측정을 위한 메타데이터를 추가하여, 작업의 시작과 끝, 소요 시간을 정확히 추적합니다.

```rust
// v2.0 메타데이터에 성능 측정 필드 추가
#[derive(Debug, Clone)]
pub struct TaskMetadata {
    pub task_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub retry_count: u32,
    pub priority: TaskPriority,
    // ... 기존 필드 ...

    // v3.0 추가 필드
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub execution_duration_ms: Option<u64>,
}
```

### 2.2. `PredictiveAnalyticsEngine` - 예측 분석 엔진 (신규 핵심 구성 요소)

시스템의 두뇌 역할을 하는 새로운 구성 요소입니다. 모든 작업의 성능 데이터를 수집하고 분석하여 미래를 예측합니다.

```rust
/// 시스템의 성능을 분석하고 미래를 예측하는 엔진
#[derive(Debug)]
pub struct PredictiveAnalyticsEngine {
    // 작업 유형별 평균 처리 시간, 편차 등 통계 저장소
    historical_performance: Arc<Mutex<HashMap<String, TaskPerformanceProfile>>>,
    // 현재 진행 중인 작업들의 상태
    inflight_tasks: Arc<Mutex<HashMap<Uuid, InflightTaskInfo>>>,
    // 예측 결과 캐시
    prediction_cache: Arc<Mutex<PredictionResult>>,
}

#[derive(Debug, Clone)]
pub struct TaskPerformanceProfile {
    pub avg_duration_ms: f64,
    pub avg_memory_usage_kb: f64,
    pub success_rate: f64,
    pub sample_count: u64,
}

#[derive(Debug, Clone)]
pub struct PredictionResult {
    pub estimated_time_to_completion: Duration,
    pub confidence_interval: (Duration, Duration), // 예측 신뢰 구간
    pub predicted_final_resource_usage: ResourceUsage,
    pub last_updated: DateTime<Utc>,
}

impl PredictiveAnalyticsEngine {
    // 작업 완료 시 호출되어 성능 데이터를 업데이트
    pub async fn record_task_completion(&self, task: &CrawlingTask) {
        // ...
    }

    // 주기적으로 호출되어 전체 완료 시간을 재계산
    pub async fn update_predictions(&self, total_tasks: u64, completed_tasks: u64) {
        // ...
    }
}
```

### 2.3. `UserDrivenProfileManager` - 사용자 프로파일 관리자 (신규)

사용자가 선택한 운영 프로파일에 따라 시스템의 동작 파라미터를 동적으로 변경하는 역할을 합니다.

```rust
/// 사용자가 선택한 운영 프로파일을 시스템 설정에 반영하는 관리자
#[derive(Debug)]
pub struct UserDrivenProfileManager {
    config_manager: Arc<ConfigManager>, // v2.0의 동적 설정 관리자
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperatingProfile {
    // 가장 빠른 속도, 리소스 사용량 최대화
    MaxPerformance,
    // 안정적인 처리량과 리소스 사용량의 균형
    Balanced,
    // 시스템 부하 최소화, 느리지만 안전한 실행
    EcoMode,
    // 사용자가 직접 모든 파라미터를 설정
    Custom(CustomProfileSettings),
}

impl UserDrivenProfileManager {
    pub async fn apply_profile(&self, profile: OperatingProfile) {
        let updates = match profile {
            OperatingProfile::MaxPerformance => ConfigUpdates { /* ... */ },
            OperatingProfile::Balanced => ConfigUpdates { /* ... */ },
            OperatingProfile::EcoMode => ConfigUpdates { /* ... */ },
            OperatingProfile::Custom(settings) => ConfigUpdates::from(settings),
        };
        self.config_manager.update_config(updates).await;
    }
}
```

### 2.4. `AdvancedSharedState` v3.0 - 모든 것을 연결

새로운 엔진과 관리자를 `SharedState`에 통합하여 모든 작업자가 접근할 수 있게 합니다.

```rust
/// v3.0 강화된 공유 상태
pub struct AdvancedSharedState {
    // v2.0 구성 요소
    pub cancellation_token: CancellationToken,
    pub resource_manager: Arc<ResourceManager>,
    pub retry_manager: Arc<RetryManager>,
    // ...

    // v3.0 추가 구성 요소
    pub analytics_engine: Arc<PredictiveAnalyticsEngine>,
    pub profile_manager: Arc<UserDrivenProfileManager>,
}
```

---

## 3. v3.0의 진화된 워크플로우

### 3.1. 예측 기반의 지능적 작업자 루프

작업자는 이제 단순히 작업을 처리하는 것을 넘어, 자신의 성능을 측정하고 분석 엔진에 보고하는 역할을 수행합니다.

```rust
/// v3.0 지능형 작업자 루프
async fn intelligent_worker_loop<T>(
    // ... 기존 파라미터 ...
    shared_state: Arc<AdvancedSharedState>,
) {
    loop {
        tokio::select! {
            // ... 취소, 일시정지 처리 ...

            maybe_task = input_queue.recv() => {
                if let Some(mut task) = maybe_task {
                    // 1. 작업 시작 시간 기록
                    task.get_metadata_mut().started_at = Some(Utc::now());

                    // 2. 실제 작업 처리 (v2.0의 재시도/회로차단기 로직 포함)
                    let result = process_task(task, &shared_state).await;

                    // 3. 작업 완료 후 성능 측정 및 보고
                    if let Ok(mut completed_task) = result {
                        let metadata = completed_task.get_metadata_mut();
                        metadata.completed_at = Some(Utc::now());
                        metadata.execution_duration_ms = Some(
                            (metadata.completed_at.unwrap() - metadata.started_at.unwrap()).num_milliseconds() as u64
                        );
                        
                        // 4. 분석 엔진에 성능 데이터 보고
                        shared_state.analytics_engine.record_task_completion(&completed_task).await;
                    }
                }
            }
        }
    }
}
```

### 3.2. 지속적인 예측 및 보정 워크플로우

오케스트레이터는 별도의 비동기 태스크를 실행하여 주기적으로 예측을 업데이트하고 사용자에게 제공합니다.

1.  **주기적 실행**: 오케스트레이터는 10초마다 `analytics_engine.update_predictions()`를 호출하는 백그라운드 태스크를 실행합니다.
2.  **데이터 수집**: `update_predictions`는 `historical_performance`와 현재 큐에 남아있는 작업의 수, 종류를 가져옵니다.
3.  **계산**: `(남은 작업 수) * (작업 유형별 평균 처리 시간)`을 기반으로 기본 완료 시간을 계산합니다. 현재 시스템 처리량(throughput)과 리소스 포화도를 고려하여 시간을 보정합니다.
4.  **신뢰도 산출**: 과거 예측의 정확도와 현재 시스템 상태의 변동성을 기반으로 예측의 신뢰 구간(예: 25분 ~ 35분 사이 완료)을 계산합니다.
5.  **결과 캐싱**: 계산된 `PredictionResult`를 캐시에 저장합니다.
6.  **UI 업데이트**: UI는 이 캐시된 예측 결과를 폴링(polling)하여 사용자에게 실시간으로 보여줍니다.

### 3.3. 사용자 프로파일 기반 동적 제어 워크플로우

1.  **사용자 선택**: 사용자가 UI에서 "속도 우선" 프로파일을 선택합니다.
2.  **명령 전달**: UI는 오케스트레이터를 통해 `profile_manager.apply_profile(OperatingProfile::MaxPerformance)`를 호출합니다.
3.  **설정 변경**: `ProfileManager`는 `MaxPerformance`에 맞는 설정값(예: `max_concurrent_http = 50`, `queue_capacity = 10000`)을 `ConfigManager`에 전달하여 업데이트합니다.
4.  **시스템 적응**: `ConfigManager`는 변경된 설정을 각 구성 요소(리소스 매니저, 작업자 풀 등)에 전파합니다.
5.  **동적 반영**: `ResourceManager`는 세마포어의 허용량을 늘리고, `AdaptiveWorkerPool`은 더 많은 작업자를 생성하여 시스템이 즉시 새로운 프로파일에 맞게 동작을 변경합니다.

---

## 4. v3.0 아키텍처의 독보적 장점

- **예측 가능한 시스템 (Predictable):** "언제 끝날지 모르는" 블랙박스 시스템에서 벗어나, 사용자에게 합리적인 완료 시간과 그 신뢰도를 제공하여 답답함을 해소합니다.

- **사용자 중심 제어 (User-Centric):** 사용자는 자신의 요구사항(빠른 완료, 시스템 안정성 등)에 맞춰 시스템의 동작 방식을 직접 선택할 수 있는 **의미 있는 제어권**을 갖게 됩니다.

- **지능형 자가 최적화 (Self-Optimizing):** 시스템이 과거의 경험(성능 데이터)을 학습하고 현재 상태를 진단하여, 스스로 최적의 운영 파라미터를 찾아 적응해 나갑니다.

- **완벽한 운영 가시성 (Total-Visibility):** 단순한 현재 상태 모니터링을 넘어, 미래 상태(예상 완료 시간, 예상 리소스 사용량)까지 제공하여 완벽한 운영 가시성을 확보합니다.

---

## 5. 구현 로드맵

### Phase 1: 데이터 수집 기반 구축
1.  `CrawlingTask`에 성능 측정용 메타데이터 필드 추가.
2.  작업자 루프를 수정하여 작업 처리 시간 측정 및 로깅 기능 구현.
3.  `PredictiveAnalyticsEngine`의 기본 구조와 `record_task_completion` 메서드 구현 (우선 인메모리 저장).

### Phase 2: 예측 엔진 및 UI 연동
1.  `update_predictions` 로직을 구현하여 기본적인 완료 시간 예측 기능 완성.
2.  `SharedState`에 분석 엔진 통합.
3.  UI에서 예측 결과를 표시하는 대시보드 컴포넌트 개발.

### Phase 3: 동적 제어 시스템 구현
1.  v2.0의 `ConfigManager`와 `AdaptiveWorkerPool` 구현.
2.  `UserDrivenProfileManager`를 구현하여 사전 정의된 프로파일(MaxPerformance, Balanced, EcoMode) 기능 제공.
3.  UI에서 사용자가 프로파일을 선택하고 시스템에 적용하는 기능 구현.

### Phase 4: 고도화 및 정교화
1.  예측 엔진에 신뢰 구간 계산, 리소스 사용량 예측 등 고급 기능 추가.
2.  과거 성능 데이터를 파일이나 경량 DB에 영속적으로 저장하여 재사용하는 기능 구현.
3.  시스템이 특정 패턴(예: 특정 도메인의 응답 지연)을 학습하여 자동으로 동작을 미세 조정하는 자가 최적화 로직 구현.
