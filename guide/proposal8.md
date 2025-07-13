# 제안 8: 상태 저장 아키텍처 완성과 실시간 UI 구현 로드맵

**문서 목적:** `proposal6` 시리즈에서 논의된 "상태 저장 백엔드" 개념을 완성하고, `interesting_visual_proposal.md`에서 제시된 "실시간 공정 대시보드" UI를 구현하기 위한 구체적이고 통합된 실행 계획을 제시합니다. 이 문서는 현재의 문제점(UI 미반영, 중복 분석)을 해결하고 최종 목표를 달성하기 위한 명확한 로드맵 역할을 합니다.

---

## 1. 최종 목표 및 현재 문제점

-   **최종 목표:** 크롤링의 전 과정을 "살아있는 생산 라인"처럼 보여주는 동적이고 인터랙티브한 UI 구현.
-   **현재 문제:**
    1.  **상태 비저장(Stateless) 백엔드:** `사이트 분석` 결과를 기억하지 못해 `크롤링 시작` 시 중복 분석 또는 하드코딩된 값 사용.
    2.  **이벤트 시스템 부재:** 백엔드의 상태 변화가 프론트엔드 UI에 실시간으로 반영되지 않음.
    3.  **역할 분담의 모호함:** 프론트엔드와 백엔드 간의 책임과 데이터 흐름이 명확하지 않음.

---

## 2. 3단계 실행 계획: 문제 해결에서 목표 달성까지

### **Phase 1: 백엔드 핵심 로직 강화 (상태 저장 및 범위 계산)**

> **목표:** 백엔드가 자신의 기억(캐시)을 올바르게 사용하여, 중복 작업을 제거하고, 가장 중요한 **크롤링 시작 범위를 정확하게 계산**하도록 만듭니다.

1.  **지능형 범위 계산 로직 버그 수정 (최우선 과제):**
    -   **문제:** 현재 `start_crawling` 시, 캐시된 DB 상태(`max_page_id`, `max_index_in_page`)를 읽고도 이를 무시하여 `No existing data`로 잘못 판단하는 버그가 있습니다.
    -   **해결:** `start_crawling` 내부의 범위 계산 로직을 수정하여, DB 커서 위치와 전체 페이지 수를 기반으로 다음 크롤링 시작 페이지(`next_page`)를 정확히 계산하도록 즉시 수정해야 합니다.
        -   **예시 로직:** `next_page = (last_saved_absolute_index / products_per_page) + 1`
    -   **기대 효과:** 증분 크롤링(Incremental Crawling)이 정상적으로 동작하여, 불필요한 전체 크롤링을 방지하고 효율성을 극대화합니다.

2.  **`SharedStateCache` 도입 및 안정화 (`src-tauri/src/state.rs`):**
    -   `proposal6_review.md`에서 제안된 `SharedStateCache` 구조체를 안정적으로 구현합니다.
    -   캐시된 데이터에는 `analyzed_at` 타임스탬프와 `TTL(Time-To-Live)`을 포함하여 데이터의 신선도를 보장합니다. (`proposal6.comment.md` 제안)

2.  **커맨드 역할 재정의 및 책임 분리:**
    -   **`analyze_system_status` 커맨드:**
        -   **역할:** 시스템(사이트, DB)을 분석하고, 그 결과를 `SharedStateCache`에 **쓰는(Write)** 유일한 통로.
        -   **구현:** 분석 완료 후, `system-state-update` 이벤트를 통해 분석 결과를 프론트엔드에 **즉시 전송**하여 UI에 반영시킵니다. (UI 미반영 문제 해결)
    -   **`start_crawling` 커맨드:**
        -   **역할:** `SharedStateCache`를 **읽어(Read)** 최적의 크롤링 범위를 계산하고, 크롤링 엔진을 시작.
        -   **시그니처 변경:** `start_crawling(profile: CrawlingProfile)` 형태로 변경하여, UI는 "어떻게"가 아닌 "무엇을" 할지만 전달합니다.
        -   **안전 장치:** 만약 캐시가 비어있거나 오래되었다면, 내부적으로 분석 로직을 먼저 호출하여 캐시를 채운 후 크롤링을 시작합니다. (`proposal6.comment.md`의 "State-Ensuring Gateway" 개념)

### **Phase 2: 이벤트 시스템 및 데이터 계약 수립**

> **목표:** 백엔드와 프론트엔드가 동일한 언어(데이터 구조)로 소통하는, 신뢰할 수 있는 실시간 이벤트 파이프라인을 구축합니다.

1.  **공유 타입 정의 (Data Contract 수립):**
    -   **위치:** `src/types/events.ts` (프론트엔드) 및 `src-tauri/src/events.rs` (백엔드)에 각각, 하지만 **내용은 동일하게** 타입을 정의합니다.
    -   **핵심:** `serde` (Rust)와 `TypeScript`가 완벽히 호환되는 필드와 타입(예: `string` <-> `String`, `number` <-> `u32/f64`, `boolean` <-> `bool`, `array` <-> `Vec`, `object` <-> `struct`)을 사용합니다.
    -   **예시 (`SystemStatePayload`):**
        ```typescript
        // src/types/events.ts
        export interface SystemStatePayload {
          isRunning: boolean;
          total_pages: number;
          db_total_products: number;
          last_db_cursor: { page: number; index: number } | null;
          // ... 기타 거시 정보
        }
        ```
        ```rust
        // src-tauri/src/events.rs
        #[derive(Clone, serde::Serialize)]
        pub struct SystemStatePayload {
            pub is_running: bool,
            pub total_pages: u32,
            pub db_total_products: u64,
            pub last_db_cursor: Option<DbCursor>,
            // ...
        }
        ```

2.  **듀얼 채널 이벤트 시스템 구현:**
    -   **`system-state-update` 이벤트 (거시 정보):** `analyze_system_status` 커맨드 완료 시, 그리고 크롤링 중 1~2초 간격으로 `SystemStateBroadcaster`가 위에서 정의한 `SystemStatePayload`를 `emit`합니다.
    -   **`atomic-task-update` 이벤트 (미시 정보):** 개별 작업 완료 시, `AtomicTaskPayload` (별도 정의 필요)를 `emit`합니다.

### **Phase 3: 프론트엔드 UI 구현 및 동적 연동**

> **목표:** 백엔드에서 오는 실시간 이벤트를 받아, `interesting_visual_proposal.md`에서 설계한 동적인 UI를 완성합니다.

1.  **SolidJS 스토어 설계 (`src/stores/crawlingProcessStore.ts`):**
    -   `interesting_visual_proposal.md`에서 제안된 `CrawlingSessionState` 인터페이스를 기반으로 중앙 스토어를 구현합니다. 이 스토어는 거시 정보(`macroState`)와 미시 정보(`batches`, `stages`, `items`)를 모두 관리합니다.

2.  **이벤트 리스너 연동 (`src/components/CrawlingProcessDashboard.tsx`):**
    -   `onMount` 훅에서 백엔드의 `system-state-update`와 `atomic-task-update` 이벤트를 `listen`합니다.
    -   수신된 이벤트 페이로드에 맞춰 `setSessionStore`를 호출하여 스토어의 상태를 업데이트합니다.
        -   `system-state-update` -> `setSessionStore('macroState', ...)`
        -   `atomic-task-update` -> `setSessionStore('batches', batchId, 'stages', stageName, 'items', itemId, 'status', newStatus)` 와 같이 중첩된 상태를 정밀하게 업데이트합니다.

3.  **컴포넌트 구현 및 애니메이션:**
    -   스토어의 데이터를 받아 화면을 그리는 리액티브 컴포넌트들(`<MissionBriefingPanel>`, `<StageLane>`, `<TaskItemCard>`)을 구현합니다.
    -   `TaskItemCard` 컴포넌트에서 `item.status` 변화를 감지(`createEffect`)하여, 'success' 상태가 되면 CSS 애니메이션(`pop-out`)을 트리거하여 "푱!" 하고 사라지는 효과를 구현합니다.

---

## 4. 기대 효과

-   **문제 해결:** 중복 분석, 설정 무시, UI 미반영 등 현재 겪고 있는 모든 문제가 근본적으로 해결됩니다.
-   **명확한 아키텍처:** 프론트엔드(View)와 백엔드(State & Logic)의 역할이 명확히 분리되어 유지보수성이 향상됩니다.
-   **최종 목표 달성:** 상태를 기억하고, 스스로 판단하며, 모든 과정을 실시간으로 보고하는 지능적이고 반응적인 애플리케이션 아키텍처가 완성됩니다.

이 로드맵을 따르면, 현재의 교착 상태를 해결하고 우리가 목표로 하는 혁신적인 UI 경험을 체계적으로 구현해 나갈 수 있을 것입니다.

### **Phase 4: 설정 시스템 및 UI 리팩토링 (후속 작업)**

> **목표:** 복잡하고 방만한 설정 구조를 명확하게 재정의하고, UI와 실제 설정을 완벽하게 동기화하여 사용자 경험을 개선하고 잠재적 버그를 제거합니다.

1.  **설정 스키마 재정의 (`matter_certis_config.json`):**
    -   **분석:** 현재 설정 파일의 모든 필드를 분석하여, 사용되지 않거나, 중복되거나, 의미가 불명확한 필드를 식별합니다.
    -   **재설계:** `crawling`, `database`, `logging`, `ui` 등 명확한 섹션으로 그룹화하여 스키마를 재설계합니다. 각 필드의 이름과 타입을 명확히 정의하고, 기본값을 설정합니다.
    -   **문서화:** 재설계된 스키마에 대한 설명을 `README.md` 또는 별도의 문서에 기록하여, 어떤 설정이 어떤 역할을 하는지 누구나 쉽게 파악할 수 있도록 합니다.

2.  **설정 UI 리팩토링 (`SettingsTab.tsx`):**
    -   **UI 구조 개선:** 재설계된 스키마에 맞춰, 설정 UI를 섹션별로 명확하게 그룹화합니다. 각 섹션은 Expandable/Collapsible하게 만들어 복잡도를 낮춥니다.
    -   **불필요한 요소 제거:** '개발용', '프로덕션용' 등 현재 사용하지 않는 버튼이나 설정 항목을 과감히 제거합니다.
    -   **누락된 기능 복구:** 사라진 '로그 레벨 설정'과 같은 중요한 설정 기능을 UI에 다시 추가하고, 백엔드의 로깅 시스템과 올바르게 연동합니다.
    -   **양방향 동기화:** UI에서 설정을 변경하면 즉시 백엔드 설정이 업데이트되고, 백엔드에서 변경이 발생하면(예: 프로그램 시작 시 기본값 생성) UI에도 반영되는 완벽한 양방향 동기화를 구현합니다.
