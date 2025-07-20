# 최종 아키텍처(v7) vs. 현재 구현: Gap 분석 및 실행 계획

> **문서 목적:** 최종 설계 문서(`re-arch-plan-final2.md`)의 각 핵심 요소를 기준으로 현재 구현 코드를 면밀히 비교 분석합니다. 이를 통해 설계와 구현 간의 차이(Gap)를 명확히 식별하고, 완전한 설계 구현을 위한 구체적인 실행 계획을 수립합니다.

## Executive Summary (TL;DR)

현재 `new_architecture`는 설계의 **구조적 뼈대(Structural Skeleton)**, 즉 계층적 Actor(Session, Batch, Stage)와 One-shot 채널을 통한 결과 보고 메커니즘을 성공적으로 구현했습니다. 이는 아키텍처의 가장 기본적인 토대를 마련한 중요한 성과입니다.

하지만, 설계의 핵심적인 **지능과 동적 행위(Intelligence & Dynamics)**를 담당하는 컴포넌트들, 즉 **`CrawlingPlanner`**와 **`MetricsAggregator`**가 아직 구현되지 않았습니다. 또한, **프론트엔드-백엔드 연동 인터페이스**는 설계 목표와 가장 큰 차이를 보이며, 타입 시스템의 자동화된 이점을 전혀 활용하지 못하고 있습니다.

비유하자면, **"자동차의 프레임과 엔진 블록은 조립되었지만, ECU(전자제어장치), 연료분사 시스템, 그리고 계기판은 아직 연결되지 않은 상태"** 와 같습니다.

---

## 1. 아키텍처 핵심 요소별 Gap 분석

### 1.1. 계층적 Actor 모델

-   **[✅ 설계 목표]** `Session` → `Batch` → `Stage` → `Task`로 이어지는 명확한 책임과 제어의 계층.
-   **[✅ 현재 구현 상태]** `actor_system.rs`에 `SessionActor`, `BatchActor`, `StageActor`의 기본 구조가 구현되어 있습니다. `SessionActor`가 `BatchActor`를 생성하고, `BatchActor`가 `StageActor`의 로직을 실행하는 계층적 흐름이 형성되어 있습니다.
-   **[⚠️ GAP 분석]**
    -   `AsyncTask`에 해당하는 구체적인 Actor나 구조체가 아직 명확히 분리되지 않았습니다. 현재는 `StageActor`가 `process_single_item`과 같은 메서드 내에서 최소 작업 단위를 직접 처리하는 형태입니다.
    -   **결론:** 구조적 뼈대는 일치하나, 가장 말단 실행 단위의 추상화가 부족합니다.

### 1.2. 삼중 채널 시스템

-   **[✅ 설계 목표]** 제어(Control), 데이터(Data), 이벤트(Event) 채널의 완전한 분리.
-   **[✅ 현재 구현 상태]**
    -   **제어 채널:** `ActorCommand` enum과 `mpsc::channel`을 통해 상위에서 하위로 명령을 전달하는 흐름이 구현되어 있습니다.
    -   **데이터 채널:** `oneshot::channel`을 사용하여 하위 Actor(`BatchActor`)가 작업을 마친 후 상위 Actor(`SessionActor`)에게 `StageResult`를 보고하는 핵심 패턴이 **성공적으로 구현**되었습니다. 이는 설계의 가장 중요한 부분 중 하나입니다.
    -   **이벤트 채널:** `AppEvent` enum과 `mpsc::Sender<AppEvent>`를 통해 각 Actor가 이벤트를 발행하는 기본 메커니즘이 존재합니다.
-   **[⚠️ GAP 분석]**
    -   **독립적 이벤트 집계 로직 부재:** `MetricsAggregator`가 없어, 현재 발행되는 이벤트는 원시(raw) 데이터에 가깝습니다. UI가 직접 구독하기에는 정보가 파편화되어 있고, 전체 진행률이나 ETA 같은 집계된 정보가 생성되지 않습니다.
    -   **결론:** 채널의 기본 메커니즘은 구현되었으나, 이벤트 채널의 데이터를 가공하여 의미있는 정보로 만들어주는 핵심 소비자(`MetricsAggregator`)가 부재합니다.

### 1.3. 지능형 계획 수립 (`CrawlingPlanner`)

-   **[✅ 설계 목표]** `CrawlingPlanner`가 사전에 사이트와 DB 상태를 분석하여, 최적의 크롤링 전략과 범위를 동적으로 수립하고 `BatchPlan`을 생성합니다.
-   **[✅ 현재 구현 상태]** `actor_system.rs`의 `SessionActor::process_batch` 메서드를 보면, `BatchPlan` 객체가 존재합니다. `CrawlingIntegrationService`에는 `execute_site_analysis` 등 분석 기능이 존재합니다.
-   **[⚠️ GAP 분석]**
    -   **`CrawlingPlanner` 컴포넌트 완전 부재:** 현재 계획 수립은 `SessionActor`의 외부 호출자가 `pages`, `batch_size` 등 모든 것을 정적으로 결정하여 명령에 담아주는 방식입니다. 시스템이 스스로 상태를 분석하여 계획을 세우는 동적 로직이 전혀 없습니다.
    -   **결론:** 설계의 "지능"에 해당하는 부분이 완전히 누락되어 있습니다. 현재는 "수동 운전" 모드로만 동작합니다.

### 1.4. End-to-End 타입 안정성 (`ts-rs`)

-   **[✅ 설계 목표]** `ts-rs`를 통해 Rust 타입을 TypeScript 타입으로 자동 생성하고, 프론트엔드는 이 생성된 타입만을 사용하여 완전한 타입 안정성을 확보합니다.
-   **[✅ 현재 구현 상태]** 백엔드의 `Cargo.toml`에 `ts-rs`가 추가되었고, `domain/events/mod.rs`의 타입들에 `#[ts(export)]`가 적용되었습니다.
-   **[⚠️ GAP 분석]**
    -   **프론트엔드의 미적용:** `crawlerStore.ts`는 자동 생성된 타입을 전혀 `import`하고 있지 않으며, `types/crawling.ts` 등 수동으로 관리되는 레거시 타입에 의존하고 있습니다.
    -   **불필요한 변환 로직 잔존:** 타입 불일치로 인해, `crawlerStore.ts` 내부에 API 응답을 수동으로 변환하는 코드가 그대로 남아있습니다.
    -   **결론:** 타입 안정성 확보를 위한 가장 중요한 **프론트엔드 연동 작업이 전혀 이루어지지 않았습니다.**

---

## 2. 최종 개선 실행 계획 (Action Plan)

> **우선순위:** 1순위(인터페이스), 2순위(지능), 3순위(구조 개선) 순으로 진행하여 병목을 최소화하고 안정적으로 아키텍처를 완성합니다.

### **[1순위] Phase 1: FE/BE 인터페이스 완전 자동화**

> **목표:** End-to-End 타입 안정성을 확보하여 모든 후속 개발의 안정적인 기반을 마련합니다.

1.  **`ts-rs` 빌드 파이프라인 구축:** `build.rs`를 설정하여 `cargo build` 시 `src/types/generated/index.ts` 파일이 자동으로 생성되도록 합니다. (`guide/re-arch-assess.md`의 Phase 1 계획과 동일)
2.  **레거시 타입 완전 제거:** 프론트엔드 프로젝트의 `src/types` 내 수동 타입 파일을 모두 삭제합니다.
3.  **`crawlerStore.ts` 리팩토링:** 자동 생성된 타입을 사용하도록 `CrawlerState`와 이벤트 핸들러를 전면 수정하고, 모든 데이터 변환 로직을 제거합니다.

### **[2순위] Phase 2: 아키텍처의 두뇌 구현**

> **목표:** 설계의 핵심 지능 컴포넌트인 `CrawlingPlanner`와 `MetricsAggregator`를 구현합니다.

1.  **`CrawlingPlanner` 구현:**
    -   `CrawlingIntegrationService`의 분석 기능을 사용하는 `CrawlingPlanner` 구조체를 만듭니다.
    -   `plan_for_full_crawl()`, `plan_for_incremental_crawl()` 등의 메서드를 구현하여, 분석 결과에 따라 `Vec<BatchPlan>`을 동적으로 생성하는 로직을 작성합니다.
    -   `SessionActor`가 `StartCrawling` 명령을 받으면, 이 `CrawlingPlanner`를 호출하여 계획을 받아오도록 수정합니다.

2.  **`MetricsAggregator` 구현:**
    -   별도의 Actor 또는 비동기 Task로 `MetricsAggregator`를 구현합니다.
    -   `EventChannel`을 구독하여 모든 원시 이벤트를 수신하고, 전체 진행률/ETA 등을 계산하여 `AggregatedStateUpdate` 이벤트를 다시 `EventChannel`로 발행하는 루프를 작성합니다.
    -   `crawlerStore.ts`가 기존의 파편화된 이벤트 대신, 이 `AggregatedStateUpdate` 이벤트를 구독하여 UI를 갱신하도록 수정합니다.

### **[3순위] Phase 3: Actor 모델 구조 개선**

> **목표:** 코드의 응집도와 가독성을 높이고, 설계에 더 부합하도록 구조를 개선합니다.

1.  **Actor 파일 분리:** `actor_system.rs`에 혼재된 `SessionActor`, `BatchActor`, `StageActor`를 각각의 파일(`actors/session.rs` 등)로 분리하여 Modern Rust 2024의 모듈 구조 가이드를 준수합니다.
2.  **`AsyncTask` 추상화:** `StageActor` 내부의 `process_single_item` 로직을 별도의 `AsyncTask` 구조체나 함수로 분리하여, 최소 실행 단위의 책임을 명확히 합니다.

---

이 계획을 순서대로 실행하면, 현재의 견고한 뼈대 위에 지능과 동적인 상호작용이 더해져 `re-arch-plan-final2.md`에서 설계한 아키텍처의 모든 잠재력을 실현할 수 있을 것입니다.
