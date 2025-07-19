# 최종 실행 계획: 명령 큐 기반의 상호작용 중심 아키텍처

*본 문서는 이전의 모든 논의를 종합하고, **명령 큐(Command Queue)** 패러다임을 핵심으로 도입하여, **도메인 지식, UI 상호작용, 체계적 역할 분담**을 완벽하게 통합한 최종 실행 계획입니다.*

## 1. 최종 아키텍처 철학: 단순성, 명확성, 반응성

이 설계의 최종 목표는 **"모든 동작과 제어를 예측 가능한 '명령'으로 추상화하여, 단순하고, 명확하며, 즉각적으로 반응하는 시스템을 구축하는 것"** 입니다. 복잡한 공유 상태(Shared State)를 통한 제어 대신, 중앙 집중화된 명령 큐를 통해 시스템의 모든 흐름을 관리합니다.

### 최종 아키텍처 비전: 명령 큐가 중심이 되는 설계

```mermaid
graph TD
    subgraph "UI / External Triggers"
        A[CrawlingDashboard UI]
        B[User Commands<br/>(Start, Pause, Cancel)]
    end

    subgraph "Application Facade (API Layer)"
        C["<b>CrawlingFacade</b><br/>UI 명령을 Command로 변환"]
    end

    subgraph "Domain Logic (The Brain)"
        D["<b>CrawlingPlanner</b><br/>분석 결과를 바탕으로<br/>일련의 Command를 생성"]
    end

    subgraph "Central Control Flow"
        E["<b>Command Queue</b><br/>(MPSC Channel)<br/>모든 작업과 제어 명령의 통로"]
    end

    subgraph "Command Processor (The Engine)"
        F["<b>SessionOrchestrator</b><br/>Command를 순차적으로 처리하는<br/>단일 워커 루프"]
    end

    subgraph "Execution & Eventing"
        G[AsyncTask / BatchManager]
        H[EventHub]
    end

    A --> C
    B --> C
    C -- "제어 명령 (Cancel, Pause)" --> E
    D -- "작업 명령 (Fetch, Parse)" --> E
    
    E -- "Next Command" --> F
    F -- "executes" --> G
    F -- "emits" --> H
    G -- "emits" --> H
    H -- "updates" --> A

    style E fill:#e3f2fd,stroke:#333,stroke-width:2px
    style F fill:#fff3e0,stroke:#333,stroke-width:2px
```

---

## 2. 핵심 설계: 명령(Command) 기반 아키텍처

### 2.1. `CrawlingCommand`: 모든 것의 시작

시스템에서 일어나는 모든 일은 `CrawlingCommand`로 정의됩니다. 이는 작업과 제어를 동일한 방식으로 다룰 수 있게 해주는 강력한 추상화입니다.

```rust
// in new_architecture/command.rs

#[derive(Debug)]
pub enum CrawlingCommand {
    // 작업 명령
    FetchListPage { page: u32 },
    ParseListPage { page: u32, content: String },
    SaveProducts { products: Vec<Product> },

    // 제어 명령
    Pause,          // 현재 진행 중인 작업을 완료하고 대기
    Resume,         // 일시정지 상태에서 다시 시작
    Cancel,         // 모든 작업을 즉시 중단하고 큐를 비움
    Shutdown,       // 큐의 모든 작업을 정상적으로 완료하고 종료
}
```

### 2.2. `SessionOrchestrator`: 단순화된 명령 처리기

`Orchestrator`의 역할은 극도로 단순해집니다. **명령 큐에서 명령을 하나씩 꺼내서, `match` 문으로 처리하는 무한 루프**를 실행하는 것이 전부입니다. 더 이상 복잡한 상태 플래그를 모든 하위 작업에 전파하고 확인할 필요가 없습니다.

```rust
// in new_architecture/orchestrator.rs

pub struct SessionOrchestrator {
    // ... dependencies
    cmd_rx: mpsc::Receiver<CrawlingCommand>,
}

impl SessionOrchestrator {
    pub async fn start_processing_loop(&mut self) -> Result<WorkflowResult> {
        while let Some(command) = self.cmd_rx.recv().await {
            match command {
                CrawlingCommand::FetchListPage { page } => {
                    // ... 페이지 가져오기 작업 실행 ...
                }
                CrawlingCommand::Cancel => {
                    self.cleanup_all_tasks().await;
                    self.event_hub.emit(SessionEvent::Cancelled).await;
                    break; // 루프 종료
                }
                CrawlingCommand::Shutdown => {
                    self.event_hub.emit(SessionEvent::Completed).await;
                    break; // 루프 종료
                }
                // ... 다른 명령 처리 ...
            }
        }
        Ok(WorkflowResult::success())
    }
}
```

### 2.3. `CrawlingPlanner`: 명령 생성기

`CrawlingPlanner`의 역할은 이제 `CrawlingPlan`이라는 데이터 구조를 만드는 것을 넘어, **실행 가능한 `CrawlingCommand`의 전체 시퀀스를 생성하여 큐에 채우는 것**으로 확장됩니다.

```rust
// in CrawlingPlanner

pub async fn plan_and_queue_tasks(
    &self,
    // ... inputs
    cmd_tx: &mpsc::Sender<CrawlingCommand>
) -> Result<()> {
    let plan = self.create_plan(...).await?;

    for page in plan.target_pages {
        cmd_tx.send(CrawlingCommand::FetchListPage { page }).await?;
    }

    // 모든 작업이 끝나면 스스로 종료하도록 Shutdown 명령 추가
    cmd_tx.send(CrawlingCommand::Shutdown).await?;

    Ok(())
}
```

---

## 3. UI 상호작용의 혁신

### 3.1. 즉각적인 중단(Cancel) 처리

사용자가 '중단'을 요청하면, `Facade`는 `CrawlingCommand::Cancel`을 큐에 보냅니다. `Orchestrator`는 진행 중이던 단 하나의 작업만 마치고 바로 다음으로 `Cancel` 명령을 수신하여 모든 것을 중단합니다. 이는 **시스템의 반응성을 극대화**합니다.

### 3.2. 이벤트 발행의 명확화

이벤트는 이제 각 **명령이 처리되기 직전과 직후**에 발행되어, UI가 현재 어떤 명령이 처리되고 있는지 명확하게 알 수 있습니다.

```rust
// in SessionOrchestrator loop

let command = self.cmd_rx.recv().await;

// 이벤트 발행: "이제 이 명령을 시작합니다"
self.event_hub.emit(Event::CommandStarted(command.clone())).await;

// ... 명령 실행 ...

// 이벤트 발행: "이 명령이 완료/실패했습니다"
self.event_hub.emit(Event::CommandFinished(result)).await;
```

---

## 4. 최종 실행 계획: Clean Slate & Command-Driven

1.  **Phase 0: Core Infrastructure (1주)**
    *   `CrawlingCommand` Enum 정의
    *   `EventHub` 및 기본 이벤트 타입 정의
    *   `SessionOrchestrator`의 핵심 `while let Some(cmd) = ...` 루프 및 `Command Queue` (MPSC 채널) 구현

2.  **Phase 1: Analysis & Planning (2주)**
    *   `SiteStatusChecker`, `DatabaseAnalyzer` 구현
    *   **`CrawlingPlanner` 구현:** 분석 결과를 바탕으로 `CrawlingCommand` 시퀀스를 생성하는 로직 완성
    *   `CrawlingFacade`에서 분석/계획을 거쳐 `Command Queue`에 명령을 채우는 전체 흐름 연결

3.  **Phase 2: Execution & UI (2주)**
    *   `BatchManager` 및 `AsyncTask` 구현: `Orchestrator`로부터 받은 명령을 실제 수행하는 로직
    *   UI 컴포넌트 개발: `EventHub`를 구독하여 실시간으로 상태, 진행률, 로그 표시
    *   UI의 제어 버튼(중단, 일시정지)과 `Facade`의 명령 전송 기능 연동

4.  **Phase 3: Integration & Testing (1주)**
    *   전체 시스템 통합 테스트 및 E2E 시나리오 검증
    *   기존 시스템과의 성능 벤치마크
    *   최종 전환 준비

## 5. 결론: 우리가 만들고자 하는 시스템

이 문서는 `re-arch-plan.md`의 초기 아이디어에서 출발하여, 수많은 검토와 제안을 거쳐 완성된 최종 청사진입니다. 이 설계는 다음과 같은 시스템을 약속합니다.

*   **단순하고 예측 가능한 시스템:** 복잡한 공유 상태 대신, 중앙 집중화된 명령 큐를 통해 제어 흐름을 단순화했습니다.
*   **과거의 지혜를 담은 시스템:** `CrawlingPlanner`를 통해 기존의 성공적인 도메인 지식을 체계적으로 계승했습니다.
*   **사용자와 소통하는 시스템:** UI와의 상호작용을 최우선으로 고려하여, 즉각적인 피드백과 제어가 가능한 구조를 완성했습니다.

이것이 우리가 구축할, 더 안정적이고, 더 효율적이며, 더 사용하기 좋은 새로운 크롤링 시스템의 최종 모습입니다.