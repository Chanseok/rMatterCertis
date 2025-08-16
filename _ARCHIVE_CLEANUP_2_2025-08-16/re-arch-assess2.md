# 최종 아키텍처(v7) vs. 현재 구현: Gap 분석 및 실행 계획 v2

> **문서 목적:** 추가 구현이 반영된 최신 코드를 최종 설계 문서(`re-arch-plan-final2.md`)와 비교하여 심층 진단합니다. 이를 통해 설계와 구현 간의 차이(Gap)를 명확히 하고, 아키텍처 완성을 위한 구체적인 실행 계획을 제시합니다.

## 1. Executive Summary (TL;DR)

최신 구현을 분석한 결과, 프로젝트는 중요한 진전을 이루었습니다. **Actor 모델의 핵심 상호작용 패턴, 특히 `SessionActor`가 `BatchActor`를 생성하고 `oneshot` 채널을 통해 결과를 보고받는 메커니즘이 성공적으로 구현되었습니다.** 이는 설계의 구조적 뼈대를 검증한 큰 성과입니다.

하지만, 여전히 설계의 핵심적인 **지능과 동적 행위(Intelligence & Dynamics)**를 담당하는 컴포넌트들, 즉 **`CrawlingPlanner`**와 **`MetricsAggregator`**의 구현이 필요합니다. 

무엇보다 가장 시급하고 중요한 과제는 **프론트엔드-백엔드 연동 인터페이스**에 남아있는 간극을 해소하는 것입니다. 백엔드는 `ts-rs`를 통해 타입 공유 준비를 마쳤으나, 프론트엔드는 여전히 레거시 타입과 분산된 이벤트 처리 방식을 사용하고 있어 타입 안정성의 이점을 전혀 활용하지 못하고 있습니다.

**비유:** "자동차의 프레임과 엔진, 변속기(Actor 구조와 채널)는 성공적으로 결합되었지만, ECU(Planner, Aggregator)가 아직 프로그래밍되지 않았고, 운전석의 계기판(프론트엔드)은 여전히 구형 모델의 배선을 사용하고 있는 상태"입니다.

---

## 2. 아키텍처 핵심 요소별 Gap 심층 분석

### 2.1. 계층적 Actor 모델 및 제어 흐름

-   **[✅ 설계 목표]** `Session` → `Batch` → `Stage`로 이어지는 명확한 책임과 제어의 계층.
-   **[✅ 현재 구현 상태]** `actor_system.rs`에 `SessionActor`, `BatchActor`, `StageActor`의 기본 구조가 구현되어 있습니다. 특히 `SessionActor`가 `ProcessBatch` 명령을 받아 `BatchActor`를 생성하고, `spawn_and_wait_for_batch`를 통해 `oneshot` 채널로 결과를 받는 흐름은 **설계의 핵심 제어/데이터 흐름을 정확히 구현**하고 있습니다.
-   **[⚠️ GAP 분석]**
    -   **`AsyncTask` 추상화 부재:** `StageActor`가 `process_single_item_with_result`와 같은 메서드 내에서 최소 작업 단위를 직접 처리하고 있습니다. 설계에 명시된 `AsyncTask`라는 별도의 실행 단위로 분리되지 않아 `StageActor`의 책임이 비대해질 수 있습니다.
    -   **결론:** Actor 간의 상호작용이라는 가장 복잡한 뼈대는 성공적으로 구축했으나, 가장 말단 실행 단위의 추상화는 다음 과제로 남아있습니다.

### 2.2. 지능형 계획 수립 (`CrawlingPlanner`)

-   **[✅ 설계 목표]** `CrawlingPlanner`가 사전에 사이트와 DB 상태를 분석하여, 최적의 크롤링 전략과 범위를 동적으로 수립하고 `BatchPlan`을 생성합니다.
-   **[✅ 현재 구현 상태]** `crawling_integration.rs`에 `execute_site_analysis`, `calculate_crawling_recommendation` 등 계획 수립에 필요한 기반 서비스들이 구현되어 있습니다. `BatchPlan` 구조체 또한 정의되어 있습니다.
-   **[⚠️ GAP 분석]**
    -   **`CrawlingPlanner` 컴포넌트의 부재:** 현재 계획 수립은 `SessionActor`의 외부 호출자가 `pages`, `batch_size` 등 모든 것을 정적으로 결정하여 명령에 담아주는 방식입니다. 시스템이 스스로 상태를 분석하여 계획을 세우는 **동적 계획 수립 로직이 아직 구현되지 않았습니다.**
    -   **결론:** 설계의 "두뇌"에 해당하는 부분이 누락되어, 현재는 "수동 운전" 모드로만 동작합니다. 하지만 필요한 분석 기능들은 이미 구현되어 있어, 이를 조합하는 `CrawlingPlanner` 구현이 다음 단계가 되어야 합니다.

### 2.3. FE/BE 인터페이스 및 타입 안정성 (`ts-rs`)

-   **[✅ 설계 목표]** `ts-rs`를 통해 Rust 타입을 TypeScript 타입으로 자동 생성하고, 프론트엔드는 이 생성된 타입만을 사용하여 완전한 타입 안정성을 확보합니다.
-   **[✅ 현재 구현 상태]** 백엔드의 `domain/events/mod.rs`에 `#[ts(export)]`가 적용되어 타입 공유 준비가 완료되었습니다.
-   **[⚠️ GAP 분석]**
    -   **프론트엔드의 완전한 미적용:** `crawlerStore.ts`와 `tauri-api.ts`는 자동 생성된 타입을 전혀 사용하지 않고, `src/types/crawling.ts` 등 **수동으로 관리되는 레거시 타입에 100% 의존**하고 있습니다. 이는 `ts-rs` 도입의 이점을 완전히 무효화합니다.
    -   **불필요한 데이터 변환 로직:** 타입 불일치로 인해, `crawlerStore.ts` 내부에 API 응답을 수동으로 변환하는 코드가 그대로 남아있습니다.
    -   **분산된 이벤트 처리:** `tauri-api.ts`는 `subscribeToProgress`, `subscribeToTaskStatus` 등 여러 개별 이벤트를 구독하고 있습니다. 이는 설계의 "통합 이벤트 채널"(`EventHub`) 개념과 상반됩니다.
    -   **결론:** **가장 시급하게 해결해야 할 가장 큰 Gap입니다.** 이 문제를 해결하지 않으면, 백엔드의 견고함이 프론트엔드까지 이어지지 못하고 계속해서 유지보수 비용을 발생시킬 것입니다.

---

## 3. 최종 개선 실행 계획 (Action Plan)

> **우선순위:** 1순위(인터페이스), 2순위(지능), 3순위(구조 개선) 순으로 진행하여 병목을 최소화하고 안정적으로 아키텍처를 완성합니다.

### **[1순위] Phase 1: FE/BE 인터페이스 완전 자동화 및 현대화**

> **목표:** End-to-End 타입 안정성을 확보하여 모든 후속 개발의 안정적인 기반을 마련합니다.

1.  **`ts-rs` 빌드 파이프라인 구축:** `src-tauri/build.rs`를 설정하여 `cargo build` 시 `src/types/generated/index.ts` 파일이 자동으로 생성되도록 합니다.
2.  **레거시 타입 완전 제거:** 프론트엔드 프로젝트의 `src/types` 내 수동 타입 파일을 모두 **삭제**하고, `tsconfig.json`의 `paths`를 설정하여 `@/types` 별칭으로 자동 생성된 타입만 참조하도록 강제합니다.
3.  **`tauri-api.ts` 리팩토링:** 여러 개별 `listen` 함수들을 **단일 `listenToBackendEvents` 함수**로 통합합니다. 이 함수는 백엔드에서 오는 모든 이벤트를 수신하고, `event.payload.type`에 따라 콜백을 호출하는 라우터 역할을 합니다.
4.  **`crawlerStore.ts` 리팩토링:**
    -   자동 생성된 타입을 사용하도록 `CrawlerState`를 재정의합니다.
    -   `listenToBackendEvents`를 사용하여 이벤트를 처리하고, 모든 데이터 변환 로직을 제거합니다.
    -   백엔드의 `StageResult`와 같은 상세한 상태를 반영하도록 상태 기계를 고도화합니다.

### **[2순위] Phase 2: 아키텍처의 두뇌 및 감각 구현**

> **목표:** 설계의 핵심 지능 컴포넌트인 `CrawlingPlanner`와 `MetricsAggregator`를 구현합니다.

1.  **`CrawlingPlanner` 구현:**
    -   `crawling_integration.rs`의 분석 기능들을 사용하는 `CrawlingPlanner` 구조체를 구현합니다.
    -   `plan()` 메서드에서 분석 결과를 바탕으로 최적의 `Vec<BatchPlan>`을 동적으로 생성하는 로직을 작성합니다.
    -   `SessionActor`가 `StartCrawling` 명령을 받으면, 이 `CrawlingPlanner`를 호출하여 계획을 받아오도록 수정합니다.

2.  **`MetricsAggregator` 구현 (선택적):**
    -   *초기 단계에서는 이 구현을 생략하고, 프론트엔드에서 직접 간단한 집계를 수행할 수 있습니다. 하지만 장기적으로는 구현이 권장됩니다.*
    -   별도의 Actor 또는 비동기 Task로 `MetricsAggregator`를 구현합니다.
    -   `EventChannel`을 구독하여 원시 이벤트를 수신하고, 전체 진행률/ETA 등을 계산하여 `AggregatedStateUpdate` 이벤트를 다시 발행하는 루프를 작성합니다.

### **[3순위] Phase 3: Actor 모델 구조 개선**

> **목표:** 코드의 응집도와 가독성을 높이고, 설계에 더 부합하도록 구조를 개선합니다.

1.  **Actor 파일 분리:** `actor_system.rs`에 혼재된 `SessionActor`, `BatchActor`, `StageActor`를 각각의 파일(`actors/session.rs` 등)로 분리하여 Modern Rust 2024의 모듈 구조 가이드를 준수합니다.
2.  **`AsyncTask` 추상화:** `StageActor` 내부의 `process_single_item_with_result` 로직을 별도의 `AsyncTask` 구조체나 함수로 분리하여, 최소 실행 단위의 책임을 명확히 합니다.

---

이 계획을 순서대로 실행하면, 현재의 견고한 뼈대 위에 지능과 동적인 상호작용이 더해져 `re-arch-plan-final2.md`에서 설계한 아키텍처의 모든 잠재력을 실현할 수 있을 것입니다.