# 설계-구현 정합성 확보를 위한 개선 실행 계획 v2

> **문서 목적:** `ts-rs` 도입 이후, 프론트엔드와 백엔드 간의 타입 안정성을 완전히 확보하고, `re-arch-plan-final.md`의 아키텍처 설계를 구현에 100% 반영하기 위한 구체적인 실행 계획을 정의합니다.

## 1. 현황 분석: "엔진은 교체했으나, 계기판은 아직 그대로"

- **긍정적인 변화 (✅):**
    - **`ts-rs` 도입 완료:** `Cargo.toml`에 `ts-rs` 의존성이 추가되었고, Rust의 핵심 데이터 구조에 `#[ts(export)]` 어트리뷰트가 적용되었습니다. 타입 동기화를 위한 기반이 성공적으로 마련되었습니다.

- **개선이 시급한 부분 (⚠️):**
    - **수동 타입 정의 잔존:** `ts-rs`로 자동 생성된 타입을 사용하는 대신, `src/types/events.ts`와 같이 수동으로 관리되는 타입 파일에 여전히 의존하고 있습니다.
    - **레거시 타입 의존성:** `crawlerStore.ts`가 자동 생성된 신규 타입이 아닌, `src/types/crawling.ts` 등 기존의 레거시 타입을 사용하고 있습니다.
    - **불필요한 데이터 변환 로직:** 타입 불일치로 인해 `crawlerStore.ts` 내부에 백엔드 응답을 프론트엔드 모델에 맞게 변환하는 코드가 남아있어, 코드 복잡성과 버그 발생 가능성을 높이고 있습니다.

## 2. 목표: 타입 안정성 완전 자동화 및 아키텍처 정합성 확보

Rust의 타입을 "신뢰할 수 있는 단일 출처(Single Source of Truth)"로 삼아, 프론트엔드-백엔드 간의 모든 데이터 모델을 자동으로 동기화하고, `crawlerStore`를 백엔드의 정교한 상태 모델과 일치시켜 설계의 모든 이점을 구현단에서 실현합니다.

---

## 3. 단계별 실행 계획 (Action Plan)

### Phase 1: `ts-rs` 워크플로우 확립 (가장 시급)

> **목표:** 수동 타입 관리를 완전히 제거하고, 빌드 시점에 타입 정의가 자동으로 생성 및 동기화되는 파이프라인을 구축합니다.

1.  **빌드 스크립트 설정:**
    - `src-tauri/build.rs` 파일을 생성하거나 기존 빌드 스크립트를 수정합니다.
    - `ts-rs`가 생성하는 TypeScript 파일을 `src/types/generated/` 디렉토리처럼 명확히 구분된 경로에 출력하도록 설정합니다. 이 경로는 Git 추적에서 제외되어야 합니다.

2.  **레거시 타입 파일 제거:**
    - `src/types/events.ts`, `src/types/crawling.ts`, `src/types/domain.ts` 등 수동으로 관리되던 모든 타입 정의 파일을 **과감히 삭제**합니다.

3.  **.gitignore 설정:**
    - 자동 생성된 타입 파일이 저장될 `src/types/generated/` 디렉토리를 `.gitignore`에 추가합니다. 타입 정의는 항상 Rust 소스코드로부터 새로 생성되어야 합니다.

### Phase 2: `crawlerStore.ts` 현대화 리팩토링

> **목표:** `crawlerStore`가 자동 생성된 타입을 직접 사용하도록 리팩토링하여, 데이터 변환 로직을 제거하고 백엔드의 상태 모델과 1:1로 동기화합니다.

1.  **레거시 Import 제거:**
    - `crawlerStore.ts` 상단에서 삭제된 레거시 타입(`CrawlingProgress` 등)에 대한 `import` 구문을 모두 제거합니다.

2.  **자동 생성 타입 Import:**
    - `import type { LiveSystemState, ProgressUpdate, ... } from '../types/generated';` 와 같이 `ts-rs`가 생성한 새로운 타입을 직접 가져옵니다.

3.  **상태(State) 모델 재정의:**
    - `CrawlerState` 인터페이스를 자동 생성된 타입을 사용하여 백엔드의 상태 모델과 일치하도록 재구성합니다.

    ```typescript
    // src/stores/crawlerStore.ts (리팩토링 후 예시)
    import type { LiveSystemState, SessionResultPayload, ErrorPayload } from '../types/generated';

    interface CrawlerState {
      // ✅ 백엔드와 1:1로 매칭되는 새로운 상태 모델
      liveState: LiveSystemState | null;
      lastResult: SessionResultPayload | null;
      lastError: ErrorPayload | null;

      // ... 기타 UI 상태
      isConnected: boolean;
      isInitialized: boolean;
    }
    ```

4.  **데이터 변환 로직 제거:**
    - `performSiteAnalysis`, `loadDefaultConfig` 등의 메서드에 남아있는 불필요한 데이터 변환 로직을 모두 삭제합니다. API 응답은 이미 프론트엔드 타입과 일치하므로 변환이 필요 없습니다.

### Phase 3: API 경계 명확화 (삼중 채널 개념 구현)

> **목표:** 이벤트의 목적을 이름으로 명확히 하여 "삼중 채널" 설계를 코드 레벨에서 구현하고, `crawlerStore`의 이벤트 처리 로직을 단순화합니다.

1.  **목적 기반 이벤트 이름 정의:**
    - 백엔드에서 Tauri 이벤트를 `emit`할 때, 추상적인 이름 대신 목적이 명확한 이름을 사용합니다.

    - **이벤트 (Event Channel):** 실시간 상태 및 진행률 업데이트
        - `event-system-state`: `LiveSystemState` 페이로드 전달
        - `event-progress-update`: `ProgressUpdate` 페이로드 전달
    - **데이터 (Data Channel):** 작업의 최종 결과
        - `event-session-result`: `SessionResultPayload` 페이로드 전달
    - **오류 (Error Handling):**
        - `event-crawling-error`: `ErrorPayload` 페이로드 전달

2.  **프론트엔드 리스너 리팩토링:**
    - `crawlerStore.ts`에서 각 목적에 맞는 이벤트를 명시적으로 구독하고, 타입이 보장된 페이로드를 직접 사용합니다.

    ```typescript
    // crawlerStore.ts (리팩토링 후 예시)
    import { listen } from '@tauri-apps/api/event';
    import type { LiveSystemState, SessionResultPayload } from '../types/generated';

    // 실시간 상태 업데이트 구독
    await listen<LiveSystemState>('event-system-state', (event) => {
        // event.payload는 이미 LiveSystemState 타입임이 보장됨
        setCrawlerState('liveState', event.payload);
    });

    // 최종 결과 구독
    await listen<SessionResultPayload>('event-session-result', (event) => {
        setCrawlerState('lastResult', event.payload);
        setCrawlerState('liveState', null); // 작업 종료 후 상태 초기화
    });
    ```

## 4. 기대 효과

위 계획을 모두 실행하면 다음과 같은 효과를 얻을 수 있습니다.

- **완전한 타입 안정성:** Rust에서 프론트엔드까지 이어지는 сквозной(end-to-end) 타입 안정성을 확보합니다.
- **유지보수 비용 감소:** 수동 동기화 작업과 데이터 변환 로직이 사라져 코드베이스가 단순해지고 유지보수가 용이해집니다.
- **개발 생산성 향상:** 백엔드 API 변경 시 프론트엔드 타입이 자동으로 업데이트되어 개발자가 비즈니스 로직에만 집중할 수 있습니다.
- **아키텍처 정합성:** `re-arch-plan-final.md`에서 설계한 정교한 아키텍처가 코드 레벨에서 완벽하게 구현됩니다.