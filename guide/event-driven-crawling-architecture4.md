# 이벤트 기반 크롤링 아키텍처 v4.0: 통합 시각화 및 제어 시스템

이 문서는 v3.0 지능형 적응 제어 시스템에 **직관적인 시각화 대시보드(`CrawlingCityDashboard`)와의 유기적인 통합**을 위한 인터페이스 설계를 추가한 최종 아키텍처를 제안합니다.

**v4.0의 비전: 백엔드의 모든 동적 활동을 사용자가 실시간으로 보고, 느끼고, 제어할 수 있는 완전한 통합 시스템**

--- 

## 1. v4.0의 핵심 원칙: 완전한 투명성과 상호작용

v3.0의 원칙(예측성, 적응성, 제어성) 위에 다음 원칙을 추가합니다.

- **시각적 투명성 (Visual Transparency):** 백엔드에서 일어나는 모든 복잡한 활동(작업자 스케일링, 큐 상태 변화, 리소스 사용량 변동)을 사용자가 직관적으로 이해할 수 있는 시각적 메타포(도시의 성장)로 실시간 매핑합니다.

- **즉각적 상호작용 (Instantaneous Interaction):** 사용자의 제어(운영 프로파일 변경, 일시정지 등)가 시각적 요소에 즉시 반영되어, 자신의 결정이 시스템에 미치는 영향을 실시간 피드백으로 체감하게 합니다.

---

## 2. 아키텍처 구성 요소: UI 통합 계층 추가

v3.0의 모든 구성 요소를 계승하며, 백엔드와 프론트엔드를 연결하는 명확한 인터페이스 계층을 정의합니다.

### 2.1. `SystemStateBroadcaster` - 상태 브로드캐스터 (신규)

백엔드의 다양한 내부 상태를 취합하여, 프론트엔드가 소비하기 좋은 단일 구조체(`SystemStatePayload`)로 가공한 후, 주기적으로 Tauri 이벤트를 통해 브로드캐스팅하는 전담 컴포넌트입니다.

- **역할:**
    1.  `SharedState`의 각 컴포넌트(리소스 매니저, 분석 엔진, 작업자 풀 메트릭 등)에 접근하여 데이터를 수집합니다.
    2.  수집된 데이터를 `SystemStatePayload` DTO(Data Transfer Object)로 변환합니다.
    3.  Tauri의 `WebviewWindow::emit`을 사용하여 `event://crawling-system-update` 채널로 페이로드를 1초마다 전송합니다.

### 2.2. Back-End -> Front-End 인터페이스: `SystemStatePayload`

백엔드가 프론트엔드로 전달하는 모든 실시간 정보를 담은 단일 구조체입니다. 이 구조체는 `CrawlingCityDashboard`의 상태를 완전히 정의합니다.

```rust
// Rust (Backend) - `#[derive(Serialize, Clone)]` 필요
// 이 구조체는 TypeScript 인터페이스와 1:1로 매핑됩니다.

pub struct SystemStatePayload {
  // 1. 전체 시스템 상태
  pub overall_status: OverallStatus, // 'Running', 'Paused', 'Completed' 등
  pub active_profile: OperatingProfile, // 'MaxPerformance', 'Balanced' 등
  
  // 2. 예측 정보 (PredictiveAnalyticsEngine)
  pub prediction: PredictionData,

  // 3. 진행률 정보
  pub progress: ProgressData,

  // 4. 작업자 풀(건물) 상태 (배열)
  pub worker_pools: Vec<WorkerPoolState>,

  // 5. 리소스 사용량 (ResourceManager)
  pub resource_usage: ResourceUsageData,

  // 6. 오류 및 통계
  pub error_count: u64,
  pub total_products_saved: u64,
}

// 각 하위 구조체도 Serialize, Clone 필요
pub struct PredictionData { ... }
pub struct ProgressData { ... }
pub struct WorkerPoolState { ... }
pub struct ResourceUsageData { ... }
```

```typescript
// TypeScript (Frontend) - @src/types/crawling.ts
// Rust 구조체와 완벽하게 동기화되어야 합니다.

export interface SystemStatePayload {
  overallStatus: 'Idle' | 'Running' | 'Paused' | 'Stopping' | 'Completed' | 'Error';
  activeProfile: 'MaxPerformance' | 'Balanced' | 'EcoMode' | 'Custom';
  
  prediction: {
    estimatedCompletionISO: string; // "2025-07-11T15:30:00Z"
    confidenceIntervalMinutes: [number, number]; // [25, 35]
    isAvailable: boolean;
  };

  progress: {
    totalTasks: number;
    completedTasks: number;
    percentage: number;
  };

  workerPools: WorkerPoolState[];

  resourceUsage: {
    cpuPercentage: number;
    memoryMb: number;
    memoryMaxMb: number;
  };

  errorCount: number;
  totalProductsSaved: number;
}

export interface WorkerPoolState {
  id: 'list_fetcher' | 'list_parser' | 'detail_fetcher' | 'detail_parser' | 'db_saver';
  name: string; // "페이지 수집 공장"
  activeWorkers: number;
  maxWorkers: number;
  queueDepth: number;
  queueCapacity: number;
  tasksPerMinute: number;
  avgTaskDurationMs: number;
  status: 'Idle' | 'Working' | 'Busy' | 'Error';
}
```

### 2.3. Front-End -> Back-End 인터페이스: Tauri Commands

프론트엔드에서의 사용자 제어는 Tauri의 `invoke`를 통해 백엔드의 특정 커맨드 함수를 호출하여 이루어집니다. 모든 커맨드는 `#[tauri::command]` 어트리뷰트를 가집니다.

```rust
// Rust (Backend) - 커맨드 핸들러

#[tauri::command]
async fn start_crawling(state: tauri::State<'_, AppState>, initial_profile: OperatingProfile) -> Result<(), String> {
    // ... 크롤링 세션 시작 로직 ...
}

#[tauri::command]
async fn stop_crawling(state: tauri::State<'_, AppState>) -> Result<(), String> {
    // ... CancellationToken 활성화 로직 ...
}

#[tauri::command]
async fn pause_crawling(state: tauri::State<'_, AppState>) -> Result<(), String> {
    // ... PauseToken 활성화 로직 ...
}

#[tauri::command]
async fn resume_crawling(state: tauri::State<'_, AppState>) -> Result<(), String> {
    // ... PauseToken 해제 로직 ...
}

#[tauri::command]
async fn set_operating_profile(state: tauri::State<'_, AppState>, profile: OperatingProfile) -> Result<(), String> {
    // ... UserDrivenProfileManager를 통해 프로파일 적용 ...
}
```

---

## 3. 통합 워크플로우: 도시가 살아 움직이는 과정

1.  **앱 시작:** `CrawlingCityDashboard` 컴포넌트가 마운트됩니다.
2.  **이벤트 구독:** `onMount` 훅에서 `listen<SystemStatePayload>('event://crawling-system-update', ...)`를 통해 백엔드의 상태 업데이트 이벤트를 구독합니다.
3.  **사용자 제어 (시작):** 사용자가 UI에서 "성능 최대화" 프로파일을 선택하고 "시작" 버튼을 클릭합니다.
4.  **커맨드 호출:** 프론트엔드는 `invoke('start_crawling', { profile: 'MaxPerformance' })`를 호출합니다.
5.  **백엔드 실행:** 백엔드의 `start_crawling` 커맨드가 실행되어 v3.0 아키텍처의 모든 컴포넌트(작업자, 큐, 분석 엔진 등)를 초기화하고 크롤링을 시작합니다. `SystemStateBroadcaster`도 함께 활성화됩니다.
6.  **실시간 상태 전파:**
    - `SystemStateBroadcaster`는 1초마다 백엔드 시스템의 상태를 종합하여 `SystemStatePayload`를 채웁니다.
    - 예를 들어, `AdaptiveWorkerPool`의 활성 작업자 수가 5명에서 10명으로 늘어나면, `worker_pools` 배열 내 해당 작업자 풀의 `activeWorkers` 값이 10으로 업데이트됩니다.
    - `PredictiveAnalyticsEngine`이 예상 완료 시간을 30분으로 계산하면, `prediction.estimatedCompletionISO` 값이 업데이트됩니다.
    - 채워진 `SystemStatePayload`가 `event://crawling-system-update` 이벤트로 프론트엔드에 전송됩니다.
7.  **UI 실시간 렌더링:**
    - 프론트엔드의 이벤트 리스너가 `SystemStatePayload`를 수신합니다.
    - SolidJS의 반응성 시스템(`createStore` 사용)에 따라, 이 페이로드와 바인딩된 모든 UI 요소가 자동으로 다시 렌더링됩니다.
    - 사용자는 "페이지 수집 공장"의 작업자 수가 5에서 10으로 늘어나는 애니메이션을 보게 됩니다.
    - 대시보드의 "예상 완료 시간" 텍스트가 "약 30분"으로 변경됩니다.
8.  **사용자 제어 (동적 변경):**
    - 사용자가 크롤링 도중 "절약 모드"를 선택합니다.
    - 프론트엔드는 `invoke('set_operating_profile', { profile: 'EcoMode' })`를 호출합니다.
    - 백엔드의 `UserDrivenProfileManager`가 동작하여 시스템의 동시성 제한을 낮춥니다.
    - 다음 `SystemStatePayload` 업데이트 시, 줄어든 `maxWorkers`와 `activeWorkers` 값이 프론트엔드로 전달됩니다.
    - 사용자는 도시의 공장들이 축소되고 처리량이 줄어드는 것을 시각적으로 확인하며, 자신의 결정이 시스템에 미친 영향을 즉시 피드백 받습니다.

---

## 4. `CrawlingCityDashboard`와의 정합을 위한 최종 설계

- **단일 상태 객체 원칙:** 프론트엔드는 오직 `SystemStatePayload`라는 단일 진실 공급원(Single Source of Truth)에만 의존하여 모든 것을 렌더링합니다. 프론트엔드 자체에 복잡한 상태 로직을 두지 않습니다.

- **메타포의 일관성:** 백엔드의 `WorkerPool`은 도시의 `건물`과, `Queue`는 건물의 `작업 대기열`과, `OperatingProfile`은 도시의 `운영 정책`과 같이, 백엔드 개념과 UI 메타포를 1:1로 일치시켜 사용자의 직관적인 이해를 돕습니다.

- **제어의 폐쇄 루프:** `사용자 제어 (UI) -> invoke 커맨드 (FE->BE) -> 시스템 상태 변경 (BE) -> emit 이벤트 (BE->FE) -> 상태 변화 렌더링 (UI)`의 폐쇄 루프(Closed Loop)를 형성하여, 모든 상호작용이 예측 가능하고 일관성 있게 동작하도록 보장합니다.

이 v4.0 아키텍처를 통해, 우리는 기술적으로 견고할 뿐만 아니라 사용자에게 풍부하고 직관적인 경험을 제공하는, 진정으로 완성도 높은 크롤링 애플리케이션을 구축할 수 있습니다.
