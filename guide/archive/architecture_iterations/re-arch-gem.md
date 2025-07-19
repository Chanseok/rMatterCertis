# Implementation Blueprint: A Unified Vision for the rMatterCertis Crawling Engine

*Filename: `guide/re-arch-gem.md`*

*본 문서는 `re-arch.md`의 구조적 제안과 `re-arch-red.md`의 구체적인 비전을 통합하여, 즉시 구현에 착수할 수 있는 실용적인 아키텍처 청사진을 제공합니다.*

## 1. 최종 비전: 신뢰 기반의 자율 실행 아키텍처

우리의 목표는 **"각 컴포넌트가 자신의 책임을 완벽히 수행하고, 이들의 결과가 비동기 메시지를 통해 자연스럽게 상위 계층으로 전파되어, 전체 워크플로우가 유기적으로 완성되는 시스템"**을 구축하는 것입니다.

핵심은 **신뢰**입니다. 상위 계층은 하위 계층의 성공을 믿고 작업을 위임하며, 하위 계층은 자신의 상태와 결과를 투명하게 보고합니다. 이 신뢰는 **명확하게 정의된 인터페이스(Traits)와 비동기 메시징(Channels)**를 통해 시스템적으로 보장됩니다.

---

## 2. The Blueprint: 구체적인 컴포넌트 설계

### 2.1. 생명선: 비동기 이벤트 채널 (MPSC)

모든 컴포넌트는 `tokio::sync::mpsc` 채널을 통해 상호작용합니다. 이는 시스템의 모든 상호작용을 단일 지점에서 관찰하고 디버깅할 수 있게 해주는 강력한 도구입니다.

```rust
// src-tauri/src/domain/events.rs

/// 시스템의 모든 상태 변화를 나타내는 최상위 이벤트 열거형
#[derive(Debug, Clone)]
pub enum AppEvent {
    // L3: 세션 레벨 이벤트
    Session(SessionEvent),
    // L2: 스테이지 레벨 이벤트
    Stage(StageEvent),
    // L1: 태스크 레벨 이벤트 (주로 디버깅/로깅용)
    Task(TaskEvent),
}

#[derive(Debug, Clone)]
pub enum SessionEvent {
    WorkflowStarted,
    WorkflowCompleted { results: CrawlingResult },
    WorkflowFailed { error: String },
    StateChanged(SessionState),
}

#[derive(Debug, Clone)]
pub enum StageEvent {
    StageStarted { name: String },
    StageCompleted { name: String, result: StageResult },
    StageFailed { name: String, error: String },
    Progress { name: String, current: u32, total: u32, message: String },
}

#[derive(Debug, Clone)]
pub enum TaskEvent {
    TaskStarted { name: String, id: String },
    TaskCompleted { name: String, id: String, result: TaskResult },
    TaskFailed { name: String, id: String, error: String },
    TaskRetrying { name: String, id: String, attempt: u32 },
}

// ... 각 이벤트에 필요한 상세 정보를 담는 Result 구조체들 ...
```

### 2.2. 일꾼: 독립적인 비동기 태스크 (The `AsyncTask` Trait)

모든 개별 작업(페이지 요청, DB 조회 등)은 이 `trait`을 구현합니다. 이를 통해 모든 `Task`는 교체 가능하고 독립적으로 테스트할 수 있습니다.

```rust
// src-tauri/src/application/tasks/mod.rs
use async_trait::async_trait;
use tokio::sync::mpsc;

#[async_trait]
pub trait AsyncTask: Send + Sync {
    type Output: Send;

    // 각 Task는 고유한 ID와 이름을 가집니다.
    fn id(&self) -> String;
    fn name(&self) -> String;

    // 실행 로직. 이벤트 발행을 위해 Sender를 인자로 받습니다.
    async fn execute(
        &self,
        context: AppContext, // DB 커넥션, 설정 등 공유 상태
        event_tx: mpsc::Sender<AppEvent>,
    ) -> Result<Self::Output, anyhow::Error>;
}
```

### 2.3. 작업반장: 스테이지 러너 (The `StageRunner` Trait)

각 스테이지(준비, 수집 등)의 실행을 책임집니다. `Task`를 생성하고, 동시성을 제어하며, `Task`의 결과를 집계하여 스테이지의 최종 결과를 만들어냅니다.

```rust
// src-tauri/src/application/stages/mod.rs

#[async_trait]
pub trait StageRunner: Send + Sync {
    fn name(&self) -> String;

    // 실행 로직. 전체 워크플로우 이벤트를 발행하기 위해 Sender를 받습니다.
    async fn run(
        &self,
        context: AppContext,
        event_tx: mpsc::Sender<AppEvent>,
    ) -> Result<StageResult, anyhow::Error>;
}
```

### 2.4. 지휘자: 세션 오케스트레이터 (The `SessionOrchestrator`)

전체 워크플로우를 지휘합니다. `StageRunner`들을 순서대로 실행하고, 시스템의 최상위 상태(`SessionState`)를 관리합니다.

```rust
// src-tauri/src/application/session.rs

pub struct SessionOrchestrator {
    context: AppContext, // 공유 상태 (Arc<Mutex<...>>)
    stages: Vec<Box<dyn StageRunner>>,
}

impl SessionOrchestrator {
    pub async fn run_workflow(&self) {
        let (event_tx, mut event_rx) = mpsc::channel(128);

        // UI(또는 로거)가 이벤트를 수신할 수 있도록 리스너를 스폰합니다.
        let ui_event_tx = event_tx.clone();
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                // 이벤트를 UI로 보내거나 로그로 남깁니다.
                println!("[EVENT]: {:?}", event);
            }
        });

        ui_event_tx.send(AppEvent::Session(SessionEvent::WorkflowStarted)).await.ok();

        for stage in &self.stages {
            ui_event_tx.send(AppEvent::Stage(StageEvent::StageStarted { name: stage.name() })).await.ok();
            
            match stage.run(self.context.clone(), ui_event_tx.clone()).await {
                Ok(result) => {
                    ui_event_tx.send(AppEvent::Stage(StageEvent::StageCompleted { name: stage.name(), result })).await.ok();
                }
                Err(e) => {
                    ui_event_tx.send(AppEvent::Stage(StageEvent::StageFailed { name: stage.name(), error: e.to_string() })).await.ok();
                    // 워크플로우를 중단하고 실패 처리
                    break;
                }
            }
        }
        // ... 워크플로우 완료/실패 이벤트 발행 ...
    }
}
```

---

## 3. 실행 시나리오: "크롤링 준비 단계" 구현 예시

**요구사항:** `SiteStatusCheckTask`와 `DbStatusCheckTask`를 **동시에** 실행하고, 그 결과를 **비동기 채널로** 받아 종합하여 `CrawlingRange`를 계산한다.

```rust
// src-tauri/src/application/stages/preparation.rs

pub struct PreparationStageRunner;

#[async_trait]
impl StageRunner for PreparationStageRunner {
    fn name(&self) -> String { "Preparation".to_string() }

    async fn run(...) -> Result<StageResult, anyhow::Error> {
        // 1. 이 스테이지만의 내부 통신용 채널을 생성합니다.
        let (task_tx, mut task_rx) = mpsc::channel(10);

        // 2. Task들을 생성하고, Concurrency Pool에 던져 동시 실행시킵니다.
        let site_task = SiteStatusCheckTask::new();
        let db_task = DbStatusCheckTask::new();
        
        // tokio::spawn을 사용하여 각 태스크를 독립적으로 실행
        tokio::spawn(site_task.execute(context.clone(), task_tx.clone()));
        tokio::spawn(db_task.execute(context.clone(), task_tx.clone()));

        // 3. Task들의 결과를 비동기적으로 수집하고 집계합니다.
        let mut site_status = None;
        let mut db_status = None;
        let mut tasks_completed = 0;

        while let Some(event) = task_rx.recv().await {
            if let AppEvent::Task(task_event) = event {
                match task_event {
                    TaskEvent::TaskCompleted { name, result, .. } => {
                        if name == "SiteStatusCheck" { site_status = Some(result.into()); }
                        if name == "DbStatusCheck" { db_status = Some(result.into()); }
                        tasks_completed += 1;
                    }
                    TaskEvent::TaskFailed { .. } => { /* 실패 처리 */ }
                    _ => {}
                }
            }
            if tasks_completed == 2 { break; } // 두 태스크가 모두 완료되면 루프 종료
        }

        // 4. 수집된 결과로 최종 비즈니스 로직(범위 계산)을 수행합니다.
        let crawling_range = calculate_range(site_status, db_status);

        // 5. 스테이지의 최종 결과를 반환합니다.
        Ok(StageResult::Preparation(crawling_range))
    }
}
```

## 4. 검증 및 테스트 전략

*   **Unit Test:** `trait` 기반 설계 덕분에 각 `Task`와 `StageRunner`를 쉽게 테스트할 수 있습니다. 예를 들어, `PreparationStageRunner`를 테스트할 때 실제 DB나 네트워크에 의존하는 대신, 즉시 결과를 반환하는 Mock `AsyncTask`를 주입하면 됩니다.
*   **Integration Test:** `SessionOrchestrator`를 중심으로 실제 `StageRunner`들을 연결하여 전체 워크플로우가 의도대로 동작하는지 검증합니다. 이때 `mpsc` 채널의 `Receiver`를 통해 발생하는 이벤트의 순서와 내용이 정확한지 확인하는 것이 핵심입니다.

이 청사진은 우리가 직면한 파편화와 통합 실패 문제를 해결하기 위한 구체적이고 실행 가능한 로드맵입니다. 이 설계를 따르면, 각자 독립적으로 컴포넌트를 개발하더라도 **정의된 인터페이스와 메시징 시스템 덕분에 최종적으로는 반드시 통합될 수밖에 없는 구조**를 만들 수 있습니다.