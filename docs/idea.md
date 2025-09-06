
╭────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
│  > SessionActor가 BatchActor에게 일을 넘기고 BatchActor가 StageActor에게 다시 일을 넘기고 StageActor는 각 Stage별로 일을 진행하는 것으로 설계      │
│    의도를 가지고 있었습니다.                                                                                                                       │
│    진행 과정에서 Crawling Plan을 잘 조정해야 하는 상황이 있었고 StageActor가 Strategy Pattern, Method Pattern 등을 활용하게 하려면 Policy가        │
│    필요해져서 Execution Plan 등으로 구현이 되기도 한 것 같습니다. 그러다보니 BatchActor의 존재 의미가 모호해졌어요. 오히려 개념적으로는 Crawling   │
│    Plan 또는 Execution Plan에 적절히 역할을 반영하는 것이 더 나은 설계인 것 같아요.                                                                │
│    어떻게 생각하시나요?                                                                                                                            │
╰────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯

✦ 매우 정확하고 통찰력 있는 분석이십니다. 개발을 진행하며 아키텍처가 자연스럽게 진화하는 과정에서 나타나는 전형적인, 그리고 아주 긍정적인 고민이라고
  생각합니다.

  결론부터 말씀드리면, 사용자님의 의견에 전적으로 동의합니다. 현재 아키텍처에서 BatchActor의 역할이 모호해졌으며, 그 개념을 ExecutionPlan에 통합하고
  StageActor가 직접 작업을 처리하도록 변경하는 것이 훨씬 더 명확하고 효율적인 설계입니다.

  왜 그렇게 생각하는지, 그리고 어떤 방향으로 리팩토링하면 좋을지에 대해 구체적인 의견을 드리겠습니다.

  왜 BatchActor의 역할이 모호해졌는가?

   1. `StageActor`의 지능화: 초기 설계와 달리, 현재의 StageActor는 단순한 작업 실행기가 아닙니다. 내부에 strategy_factory를 가지고 있어 StageType에
      맞는 로직을 동적으로 선택할 수 있으며, process_stage_items에서 Semaphore와 JoinSet을 이용해 스스로 동시성을 제어하며 아이템 묶음을 처리합니다.
      즉, `StageActor`가 이미 실질적인 "배치 처리" 능력을 갖추게 되었습니다.

   2. `ExecutionPlan`의 등장: ExecutionPlan은 크롤링의 모든 전략과 범위를 중앙에서 결정하는 "두뇌" 역할을 하게 되었습니다. 어떤 페이지를, 어떤
      순서로, 어떤 크기로 묶을지에 대한 결정은 이제 CrawlingPlanner가 ExecutionPlan을 만들 때 하는 것이 가장 자연스럽습니다. BatchActor가 중간에서
      다시 무언가를 계획하거나 분배할 여지가 사라졌습니다.

   3. 개념적 혼란: 말씀하신 대로, Session → Batch → Stage의 흐름은 직관적이지 않습니다. 일반적인 개념은 하나의 `Session`은 여러 `Stage`로 구성되고,
      각 `Stage`는 너무 클 경우 여러 `Batch`로 나뉘어 처리된다는 것입니다. 현재 코드는 StageActor가 배치 처리 능력을 갖추면서 자연스럽게 이 직관적인
      모델로 진화하고 있으며, BatchActor는 그 과정에서 남겨진 과거의 유산(legacy)이 된 것입니다.

  추천하는 리팩토링 방향

  BatchActor를 제거하고 그 역할을 ExecutionPlan과 SessionActor, StageActor에 자연스럽게 녹여내는 구체적인 계획은 다음과 같습니다.

  1단계: `ExecutionPlan` 강화

  ExecutionPlan이 "배치"의 개념까지 포함하도록 구조를 변경합니다.

    1 // types.rs (예시)
    2
    3 pub struct ExecutionPlan {
    4     // ... 기존 필드 ...
    5     // crawling_ranges 대신 더 구체적인 Stage 실행 계획 목록을 가집니다.
    6     pub stage_executions: Vec<StageExecution>,
    7 }
    8
    9 // 한 스테이지를 어떻게 실행할지에 대한 명세
   10 pub struct StageExecution {
   11     pub stage_type: StageType,
   12     // 이 스테이지에서 처리할 모든 아이템 목록
   13     pub items: Vec<StageItem>,
   14     // 이 스테이지에 적용될 동시성 및 타임아웃 설정
   15     pub concurrency_limit: u32,
   16     pub timeout_secs: u64,
   17     // 필요하다면, 이 스테이지 레벨의 배치 크기 (StageActor 내부에서 참고용)
   18     // pub internal_batch_size: Option<u32>,
   19 }

  CrawlingPlanner는 이제 페이지 범위를 계산하여 StageExecution 목록을 생성합니다. 예를 들어, 1~100 페이지를 크롤링한다면, ListPageCrawling 타입의
  StageExecution 하나를 만들고 items에 100개의 페이지 정보를 담아줍니다.

  2단계: `SessionActor`의 역할 변경

  SessionActor는 더 이상 BatchActor를 생성하지 않습니다. 대신, ExecutionPlan의 "프로젝트 관리자"가 됩니다.

   1. ExecutionPlan을 받습니다.
   2. plan.stage_executions 목록을 순회합니다.
   3. 각 StageExecution 명세에 따라 `StageActor`를 직접 생성하고, execute_stage 명령을 내립니다.
   4. 모든 StageActor의 작업이 완료될 때까지 기다리고 전체 세션 결과를 종합합니다.

  3단계: `StageActor`의 역할 단순화

  StageActor는 이제 SessionActor로부터 직접 작업을 할당받습니다. BatchActor와의 통신 코드가 모두 사라지므로 구조가 훨씬 단순해집니다. execute_stage
  메서드는 지금처럼 items 목록을 받아 내부적으로 동시 처리하는 역할을 그대로 수행하면 됩니다.

  4. `BatchActor` 제거

  위의 1~3단계가 완료되면, BatchActor와 관련된 모든 코드(batch_actor.rs, BatchConfig, BatchStarted/Completed 이벤트 등)를 프로젝트에서 완전히
  제거합니다.

  이 리팩토링의 기대 효과

   1. 명확한 아키텍처: Session → Stage → (내부 동시 처리) 라는 직관적이고 명확한 계층 구조가 확립됩니다.
   2. 코드 단순화: 불필요한 액터와 메시지 전달 과정이 사라져 코드베이스가 더 작고 이해하기 쉬워집니다.
   3. 중앙화된 제어: 모든 실행 계획과 전략이 ExecutionPlan에 집중되므로, 크롤링 동작을 수정하거나 분석하기가 매우 용이해집니다.
   4. 성능 향상: 액터 간의 메시지 직렬화/역직렬화 및 통신 오버헤드가 한 단계 줄어들어 미미하지만 성능상 이점도 얻을 수 있습니다.

  사용자님께서 느끼신 "모호함"은 아키텍처가 한 단계 더 발전할 수 있다는 매우 중요한 신호입니다. 제안 드린 방향으로 리팩토링을 진행하시면, 현재의
  훌륭한 코드베이스를 더욱 견고하고 우아하게 만드실 수 있을 것입니다.