# 제안: 실시간 UI 시각화를 위한 백엔드 이벤트 아키텍처 고도화

**문서 목적:** 현재의 Task Queue 기반 아키텍처의 독립성을 유지하면서, UI가 각 단위 작업의 상태 변화를 실시간으로 추적하고, 정확한 성능 예측 정보를 표시할 수 있도록 백엔드의 이벤트 시스템과 데이터 제공 방식을 고도화하는 구체적인 방안을 제안합니다.

--- 

## 1. 문제 정의: 단일 정보 채널의 한계

현재 아키텍처는 주기적으로 시스템의 종합 상태를 UI에 보내는 '스냅샷' 방식에 가깝습니다. 이 방식은 전체 진행률이나 평균적인 성능을 보여주기에는 효율적이지만, 다음과 같은 상세한 UI 요구사항을 만족시키기 어렵습니다.

-   **실시간 애니메이션의 부재:** 초당 수십 개의 작업이 완료되더라도, 1초에 한 번 보내는 스냅샷 정보로는 각 작업이 언제 끝났는지 알 수 없어 '푱푱푱'과 같은 개별 애니메이션을 표현할 수 없습니다.
-   **부정확한 예측:** ETA나 리소스 사용량 같은 복잡한 예측 정보는 매 작업마다 계산하기에는 비용이 너무 큽니다. 하지만 너무 가끔 계산하면 현실과 동떨어진 부정확한 정보를 표시하게 됩니다.

## 2. 핵심 해결 전략: "듀얼 채널" 이벤트 시스템 도입

이 문제를 해결하기 위해, 목적에 따라 두 개의 분리된 이벤트 채널을 운영할 것을 제안합니다.

-   **채널 1: 상태 스냅샷 채널 (State Snapshot Channel):**
    -   **목적:** 거시적(Macro) 정보 제공. 전체 진행률, 예측 시간, 리소스 사용량 등 종합적인 현황판 데이터 전송.
    -   **이벤트:** `event://system-state-update` (기존과 동일)
    -   **주기:** 1~2초마다 한 번씩, 백엔드가 종합적인 `SystemStatePayload`를 전송합니다.
    -   **특징:** 정보량이 많고 무겁지만, 전송 빈도가 낮아 효율적입니다.

-   **채널 2: 원자적 태스크 이벤트 채널 (Atomic Task Event Channel):**
    -   **목적:** 미시적(Micro) 정보 제공. 개별 작업의 상태 변경(완료, 실패, 재시도)을 실시간으로 전송.
    -   **이벤트:** `event://atomic-task-update` (신규)
    -   **주기:** 각 작업이 상태가 변경되는 **즉시** 전송됩니다.
    -   **특징:** 정보량은 작고 가볍지만, 전송 빈도가 매우 높아 실시간성이 극대화됩니다.

## 3. 백엔드 구현 방안

### 3.1. `Orchestrator`의 역할 강화: 이벤트 발생의 중심

오케스트레이터는 두 종류의 이벤트를 모두 발생시키는 중심 허브 역할을 수행합니다.

1.  **원자적 이벤트 발생:** `process_single_task_static` 함수가 `TaskResult`를 반환하는 즉시, 해당 결과를 `AtomicTaskEvent`로 변환하여 `emit`합니다.
2.  **상태 스냅샷 발생:** `Orchestrator`가 시작될 때, 별도의 비동기 태스크를 생성(`tokio::spawn`)하고, 이 태스크는 `tokio::time::interval`을 사용하여 주기적으로 종합 상태를 `emit`합니다.

### 3.2. 원자적 이벤트 (`AtomicTaskEvent`) 구현

-   **목적:** UI가 특정 작업 아이템의 상태를 변경하고 애니메이션을 트리거하는 데 필요한 최소한의 정보를 담습니다.
-   **Rust 구조체 정의 (`tasks.rs` 또는 `events.rs`):**
    ```rust
    #[derive(Debug, Clone, Serialize)]
    pub enum AtomicTaskEvent {
        TaskStarted {
            task_id: TaskId,
            task_type: String,
        },
        TaskCompleted {
            task_id: TaskId,
            task_type: String,
            duration_ms: u64,
        },
        TaskFailed {
            task_id: TaskId,
            task_type: String,
            error_message: String,
        },
        TaskRetrying {
            task_id: TaskId,
            task_type: String,
            retry_count: u32,
        },
    }
    ```
-   **`Orchestrator` 수정:**
    ```rust
    // orchestrator.rs의 process_single_task_static 내부
    let result = worker_pool.process_task(task, ...).await;

    // 이벤트 생성 및 즉시 전송!
    let event = AtomicTaskEvent::from(result);
    app_handle.emit("atomic-task-update", event).unwrap();
    ```

### 3.3. 상태 스냅샷 (`SystemStatePayload`) 및 예측 엔진 연동

-   **목적:** UI의 전체 현황판과 예측 패널에 필요한 모든 종합 정보를 제공합니다.
-   **`PredictiveAnalyticsEngine`의 역할:**
    1.  **데이터 수집:** `Orchestrator`로부터 완료된 모든 `AtomicTaskEvent::TaskCompleted` 정보를 받아, 작업 유형별 평균 처리 시간(`avg_duration_ms`)과 메모리 사용량 등의 통계를 내부적으로 계속 업데이트합니다.
    2.  **예측 계산:** 주기적으로(또는 요청 시), 현재 큐에 남아있는 작업들의 수와 종류, 그리고 지금까지 축적된 통계 데이터를 바탕으로 **전체 예상 완료 시간(ETA)**과 **신뢰 구간**을 계산합니다. 이 계산 결과를 내부 캐시에 저장합니다.
-   **상태 스냅샷 생성 태스크 로직:**
    ```rust
    // Orchestrator가 생성하는 별도의 태스크
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;

            // 각 모듈에서 최신 정보 수집
            let queue_stats = queue_manager.get_stats().await;
            let resource_usage = resource_manager.get_usage().await;
            let prediction = analytics_engine.get_current_prediction().await;

            // SystemStatePayload DTO 생성
            let payload = SystemStatePayload {
                // ... 모든 종합 정보를 채움 ...
                prediction: prediction,
                resourceUsage: resource_usage,
                queueStats: queue_stats,
            };

            // UI로 스냅샷 전송
            app_handle.emit("system-state-update", payload).unwrap();
        }
    });
    ```

## 4. 프론트엔드-백엔드 인터페이스 명세 (v5.0)

### 채널 1: `event://system-state-update`
-   **Payload:** `SystemStatePayload` (v4.0 제안 내용과 유사하며, 예측 및 리소스 정보 포함)

### 채널 2: `event://atomic-task-update`
-   **Payload:** `AtomicTaskEvent` (TypeScript 인터페이스)
    ```typescript
    export type AtomicTaskEvent = 
      | { type: 'TaskStarted', taskId: string, taskType: string }
      | { type: 'TaskCompleted', taskId: string, taskType: string, durationMs: number }
      | { type: 'TaskFailed', taskId: string, taskType:string, errorMessage: string }
      | { type: 'TaskRetrying', taskId: string, taskType: string, retryCount: number };
    ```

## 5. 기대 효과

-   **최상의 사용자 경험:** 사용자는 전체적인 진행 상황을 한눈에 파악하면서도, 개별 작업들이 실시간으로 처리되는 생동감 넘치는 모습을 보며 지루할 틈 없이 크롤링 과정을 즐길 수 있습니다.
-   **정확한 정보 제공:** 비용이 큰 예측 계산은 주기적으로 수행하여 시스템 부하를 줄이고, 상태 변화는 즉시 전파하여 UI의 반응성을 극대화하는 두 마리 토끼를 모두 잡을 수 있습니다.
-   **아키텍처 일관성 유지:** 작업자의 독립성은 그대로 유지됩니다. 작업자는 자신의 일만 처리하고, 이벤트 발생의 책임은 상위 레이어인 `Orchestrator`가 지므로, 설계 철학을 위배하지 않으면서도 필요한 기능을 모두 구현할 수 있습니다.

이 듀얼 채널 이벤트 시스템을 도입함으로써, 우리는 기술적으로 견고하고 효율적이면서도 사용자에게 최고의 시각적 경험을 제공하는 진정한 의미의 차세대 크롤링 애플리케이션을 완성할 수 있습니다.
