# 아키텍처 문서 (v3, 2025-09-05)

이 문서는 rMatterCertis 프로젝트의 아키텍처를 설명합니다. `BatchActor`가 제거되고 `ExecutionPlan` 기반의 오케스트레이션이 강화된 최신 아키텍처를 기준으로 작성되었습니다.

## 1. 전체 아키텍처

시스템의 주요 컴포넌트와 데이터 흐름을 한눈에 볼 수 있는 최상위 다이어그램입니다.

- **핵심 변경사항**:
  - **BatchActor 제거**: `SessionActor`와 `StageActor` 사이의 중간 액터 계층이 사라져 아키텍처가 단순화되었습니다.
  - **ExecutionPlan 역할 강화**: `CrawlingPlanner`가 생성하는 `ExecutionPlan`이 전체 크롤링 세션의 실행 흐름(스테이지 순서, 작업 항목 등)을 모두 정의합니다.
  - **SessionActor의 오케스트레이터 역할**: `SessionActor`는 `ExecutionPlan`을 해석하여 필요한 `StageActor`를 직접 생성하고 관리하는 핵심 오케스트레이터(Orchestrator) 역할을 수행합니다.

```mermaid
graph TD
    subgraph Frontend
        UI(User Interface)
    end

    subgraph Backend [Tauri Core Process]
        subgraph "API Layer"
            Commands[Tauri Commands]
        end

        subgraph "Application & Domain Layer"
            SessionActor(SessionActor)
            Planner(CrawlingPlanner)
            Plan([ExecutionPlan])
            StageActor(StageActor)
            LogicFactory(StageLogicFactory)
            Strategy(StageLogic / Strategy)
        end

        subgraph "Infrastructure Layer"
            DB[(Database)]
            HTTP(HTTP Client)
            EventBus([AppEvent Bus])
        end

        subgraph "Execution"
            AsyncTask(Async Task)
        end
    end

    %% --- Data Flows ---
    UI -- "(1) Crawl Command (e.g., Start)" --> Commands
    Commands -- "(2) Create & Start" --> SessionActor

    SessionActor -- "(3) Request Plan" --> Planner
    Planner -- "(4) Read DB State" --> DB
    Planner -- "(5) Create Blueprint" --> Plan
    SessionActor -- "(6) Receives Plan" --> Plan

    SessionActor -- "(7) Execute Plan (Loop per Stage)" --> StageActor

    StageActor -- "(8) Get Logic for StageType" --> LogicFactory
    LogicFactory -- "(9) Provide Concrete Strategy" --> Strategy
    StageActor -- "(10) Spawn for Concurrency" --> AsyncTask
    AsyncTask -- "(11) Execute Logic" --> Strategy

    Strategy -- "(12) Read/Write Data" --> HTTP
    Strategy -- "(13) Read/Write Data" --> DB

    SessionActor -- "(14) Emit Lifecycle Events" --> EventBus
    StageActor -- "(14) Emit Stage Events" --> EventBus
    AsyncTask -- "(14) Emit Item Events" --> EventBus
    EventBus -- "(15) Update UI" --> UI

    %% --- Styling ---
    style SessionActor fill:#bbf,stroke:#333,stroke-width:2px
    style StageActor fill:#bbf,stroke:#333,stroke-width:2px
    style Plan fill:#f9f,stroke:#333,stroke-width:2px
```

---

## 2. 핵심 로직 다이어그램

### 2.1. Actor 크롤링 시스템 (컴포넌트 관계도)

`SessionActor`와 `StageActor` 간의 단순화된 관계와 `ExecutionPlan`을 통해 작업이 전달되는 과정을 보여줍니다.

#### Mermaid 버전
```mermaid
graph TD
    subgraph "Orchestration"
        SessionActor
        Plan([ExecutionPlan])
    end

    subgraph "Execution"
        StageActor
    end
    
    subgraph "Dependencies"
        StageDeps[(Stage Dependencies)]
    end

    SessionActor -- "(1) Consumes" --> Plan
    SessionActor -- "(2) Creates & Runs" --> StageActor
    StageActor -- "(3) Uses" --> StageDeps

```
> note for SessionActor "Manages the entire crawl session based on an ExecutionPlan. Spawns StageActors for each stage in the plan."
> note for StageActor "The workhorse. Executes a single stage (e.g., fetching URLs, saving to DB) for a set of items. Uses real services via StageDeps."

#### PlantUML 버전
```plantuml
@startuml ActorSystemComponent
title Actor-based Crawling System - Component Diagram

!theme vibrant

package "Orchestration" {
  [SessionActor]
  [ExecutionPlan]
}

package "Execution" {
  [StageActor]
}

package "Dependencies" {
  [StageDeps]
}

note top of SessionActor
  Manages the entire crawl session based
  on an ExecutionPlan. Spawns StageActors
  for each stage in the plan.
end note

note top of StageActor
  The workhorse. Executes a single stage
  (e.g., fetching URLs, saving to DB)
  for a set of items. Uses real services
  via StageDeps.
end note

[SessionActor] ..> [ExecutionPlan] : Consumes
[SessionActor] --> [StageActor] : Creates & Runs
[StageActor] --> [StageDeps] : Uses

@enduml
```

### 2.2. Crawl Engine 서비스 (플래닝 과정)

`CrawlingPlanner`가 `PlanningStrategy`를 사용하여 어떻게 `ExecutionPlan`을 생성하는지, 그 과정에서 어떤 의존성을 갖는지 보여줍니다.

#### Mermaid 버전
```mermaid
graph TD
    subgraph "Strategies"
        A[IntelligentPlanningStrategy]
        B[ManualPlanningStrategy]
        IPlanningStrategy(<<interface>> PlanningStrategy)
    end

    subgraph "Core Service"
        Planner(CrawlingPlanner)
    end

    subgraph "Dependencies"
        StatusChecker(<<interface>> IStatusChecker)
        DBAnalyzer(<<interface>> IDatabaseAnalyzer)
    end

    subgraph "Output"
        Plan([ExecutionPlan])
    end

    A -- "Implements" --> IPlanningStrategy
    B -- "Implements" --> IPlanningStrategy
    
    Planner -- "Uses" --> IPlanningStrategy
    Planner -- "Uses" --> StatusChecker
    Planner -- "Uses" --> DBAnalyzer
    Planner -- "Produces" --> Plan

    linkStyle 0,1 stroke-width:1px,fill:none,stroke:gray,stroke-dasharray: 5 5;
```

#### PlantUML 버전
```plantuml
@startuml CrawlEngineServiceDiagram
title Crawl Engine - Planning Services

interface PlanningStrategy {
  + plan(overrides): ExecutionPlan
}
class IntelligentPlanningStrategy implements PlanningStrategy
class ManualPlanningStrategy implements PlanningStrategy

class CrawlingPlanner {
  + create_crawling_plan(): ExecutionPlan
}

interface IStatusChecker
interface IDatabaseAnalyzer
class ExecutionPlan

CrawlingPlanner ..> PlanningStrategy : uses
CrawlingPlanner ..> IStatusChecker : uses
CrawlingPlanner ..> IDatabaseAnalyzer : uses
CrawlingPlanner ..> ExecutionPlan : produces

@enduml
```

### 2.3. 도메인 모델 (클래스 다이어그램)

시스템의 핵심 데이터 구조인 `ProductUrl`, `ProductDetail` 등의 관계를 보여줍니다.

#### Mermaid 버전
```mermaid
classDiagram
    class ProductUrl {
        +String url
        +i32 page_id
        +i32 index_in_page
    }
    class Product {
        +String manufacturer
        +String model
    }
    class ProductDetail {
        +String device_type
        +String software_version
    }
    class IntegratedProduct {
        +i64 id
        +String name
    }

    Product --|> ProductUrl
    ProductDetail --|> Product
    IntegratedProduct --|> ProductDetail
   
```
> note for IntegratedProduct "The central data model, combining all product information for database storage."

#### PlantUML 버전
```plantuml
@startuml DomainModelDiagram
title Domain Models - Product Data Structure

class ProductUrl {
  + url: String
  + page_id: i32
  + index_in_page: i32
}
class Product extends ProductUrl {
  + manufacturer: String
  + model: String
}
class ProductDetail extends Product {
  + device_type: String
  + software_version: String
  + ...
}
class IntegratedProduct extends ProductDetail {
  + id: i64
  + name: String
  + ...
}

note top of IntegratedProduct
  The central data model, combining all
  product information for database storage.
end note
@enduml
```

---

## 3. StageActor 아키텍처 심층 분석

### 3.1. StageActor와 전략 패턴 (클래스 다이어그램)

`StageActor`는 특정 `StageType`에 맞는 로직을 실행하기 위해 **전략 패턴**을 사용합니다. `StageLogicFactory`는 `StageType`에 따라 적절한 `IStageLogic` 구현체를 생성하여 `StageActor`에 제공합니다.

#### Mermaid 버전
```mermaid
classDiagram
    class StageActor {
        -IStageLogicFactory strategy_factory
        +execute_stage(type, items)
    }
    class IStageLogicFactory {
        <<interface>>
        +logic_for(type) IStageLogic
    }
    class DefaultStageLogicFactory {
        +logic_for(type) IStageLogic
    }
    class IStageLogic {
        <<interface>>
        +execute(input) StageOutput
    }
    class ListPageLogic
    class DataSavingLogic
    class StageDeps {
        +http_client
        +product_repo
    }

    DefaultStageLogicFactory --|> IStageLogicFactory
    ListPageLogic --|> IStageLogic
    DataSavingLogic --|> IStageLogic

    StageActor --> IStageLogicFactory : uses
    IStageLogicFactory --> IStageLogic : creates
    StageActor --> IStageLogic : uses
    IStageLogic --> StageDeps : uses
```

#### PlantUML 버전
```plantuml
@startuml StageActorStrategyPattern
title StageActor - Strategy Pattern

class StageActor {
  - strategy_factory: IStageLogicFactory
  + execute_stage(type, items)
}
interface IStageLogicFactory {
  + logic_for(type): IStageLogic
}
class DefaultStageLogicFactory implements IStageLogicFactory
interface IStageLogic {
  + execute(input): StageOutput
}
class ListPageLogic implements IStageLogic
class DataSavingLogic implements IStageLogic
class StageDeps

StageActor -> IStageLogicFactory : uses
DefaultStageLogicFactory .> IStageLogic : creates
StageActor ..> IStageLogic : uses
IStageLogic ..> StageDeps : uses
@enduml
```

### 3.2. StageActor 아이템 처리 과정 (시퀀스 다이어그램)

`SessionActor`가 `StageActor`를 호출하여 작업을 처리하는, 단순화된 상호작용을 보여줍니다.

#### Mermaid 버전
```mermaid
sequenceDiagram
    participant SessionActor
    participant StageActor
    participant AsyncTask
    participant StageLogic
    
    SessionActor->>StageActor: execute_stage(type, items)
    activate StageActor
    
    loop for each item in items
        StageActor->>+AsyncTask: spawn(item)
        AsyncTask->>StageLogic: execute(item)
        StageLogic-->>AsyncTask: result
        AsyncTask-->>-StageActor: result
    end

    StageActor-->>SessionActor: StageResult
    deactivate StageActor
```

#### PlantUML 버전
```plantuml
@startuml StageActorSequence
title StageActor - Item Processing Sequence

participant SessionActor
participant StageActor
participant AsyncTask
participant "logic: IStageLogic" as Logic

SessionActor -> StageActor: execute_stage(type, items)
activate StageActor

loop for each item in items
    create participant AsyncTask
    StageActor -> AsyncTask: spawn(item)
    activate AsyncTask
    
    AsyncTask -> Logic: execute(item)
    activate Logic
    Logic --> AsyncTask: result
    deactivate Logic
    
    AsyncTask --> StageActor: result
    deactivate AsyncTask
end

StageActor --> SessionActor: StageResult
deactivate StageActor
@enduml
```

---

## 4. 데이터 흐름 다이어그램 (Activity Diagram)

데이터가 각 스테이지를 거치며 어떻게 변환되는지 보여줍니다. `SessionActor`가 전체 파이프라인을 관리합니다.
#### PlantUML 버전
```plantuml
@startuml DataFlow
title Data Flow through Crawling Stages

start
:SessionActor starts with ExecutionPlan;
-> [Vec<PageNumber>];
:Stage 1: **ListPageCrawling**;
note right
  Input: Page Numbers
  Task: Collect all product URLs
end note
-> [Vec<ProductUrl>];
:Stage 2: **ProductDetailCrawling**;
note right
  Input: Product URLs
  Task: Fetch detail for each URL
end note
-> [Vec<ProductDetail>];
:Stage 3: **DataValidation**;
note right
  Input: Collected Product Details
  Task: Validate data quality
end note
-> [Vec<ValidatedProductDetail>];
:Stage 4: **DataSaving**;
note right
  Input: Validated Product Info
  Task: Save to Database (Create/Update)
end note
-> [DB Write Result];
stop
@enduml
```

#### Mermaid 버전
```mermaid
%%{init: {'theme':'default'}}%%
graph TD
    %% Stage 1
    subgraph Row1[ ]
        direction LR
        B{Stage 1: ListPageCrawling}
        NoteB[/"📄 **Note**<br>Input: Page Numbers<br>Task: Collect all product URLs"/]
    end
    style NoteB fill:#fff9c4,stroke:#d4b483,stroke-width:1px,color:#5d4037,font-size:12px

    %% Stage 2
    subgraph Row2[ ]
        direction LR
        C{Stage 2: ProductDetailCrawling}
        NoteC[/"📄 **Note**<br>Input: Product URLs<br>Task: Fetch detail for each URL"/]
    end
    style NoteC fill:#fff9c4,stroke:#d4b483,stroke-width:1px,color:#5d4037,font-size:12px

    %% Stage 3
    subgraph Row3[ ]
        direction LR
        D{Stage 3: DataValidation}
        NoteD[/"📄 **Note**<br>Input: Product Details<br>Task: Validate data quality"/]
    end
    style NoteD fill:#fff9c4,stroke:#d4b483,stroke-width:1px,color:#5d4037,font-size:12px

    %% Stage 4
    subgraph Row4[ ]
        direction LR
        E{Stage 4: DataSaving}
        NoteE[/"📄 **Note**<br>Input: Validated Details<br>Task: Save to Database"/]
    end
    style NoteE fill:#fff9c4,stroke:#d4b483,stroke-width:1px,color:#5d4037,font-size:12px

    %% 연결 관계
    A[Start: SessionActor receives ExecutionPlan] --> B
    B -- Vec<ProductUrl> --> C    
    C -- Vec<ProductDetail> --> D
    D -- Vec<ValidatedProductDetail> --> E
    E -- DB Write Result --> F[End]
```

