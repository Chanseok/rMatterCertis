# 통합 아키텍처 구현 진단 및 개선 가이드 (v3)

> **문서 목적:** 현재 구현 상태를 최종 설계(`re-arch-plan-final2.md`)와 비교하여 진단하고, **프론트엔드-백엔드 연동 인터페이스**를 완성하며 **Modern Rust 2024 개발 가이드**를 준수하기 위한 구체적인 개선 및 구현 계획을 제시합니다.

## 1. 현황 진단: 견고한 백엔드 설계와 프론트엔드 구현의 간극

현재 프로젝트는 `ts-rs` 도입 등 백엔드 측의 선진적인 준비에도 불구하고, 프론트엔드가 이를 효과적으로 활용하지 못해 설계의 잠재력을 온전히 발휘하지 못하고 있습니다.

- **[진단 1] 타입 시스템 불일치:** 백엔드는 타입을 자동으로 공유할 준비가 되었으나, 프론트엔드는 여전히 수동으로 관리되는 레거시 타입을 사용 중입니다. 이로 인해 불필요한 데이터 변환 로직이 발생하고 타입 안정성이 저해됩니다.
- **[진단 2] API 경계 모호성:** "삼중 채널" 설계와 달리, 여러 이벤트 채널이 분산되어 사용되어 상태 처리 로직이 복잡하고 가독성이 떨어집니다.
- **[진단 3] 상태 표현력 부족:** 프론트엔드의 상태 모델이 단순하여, 백엔드의 정교한 상태(e.g., `RecoverableError`)를 사용자에게 제대로 전달하지 못합니다.

## 2. 핵심 개선 원칙

모든 개선 작업은 아래 두 가지 핵심 원칙을 기반으로 진행합니다.

1.  **End-to-End 타입 안정성:** Rust 타입을 유일한 신뢰 출처(SSoT)로 삼아, 프론트엔드까지 이어지는 완전 자동화된 타입 시스템을 구축합니다.
2.  **Modern Rust 2024 준수:** 모든 백엔드 코드는 `re-arch-plan-final2.md`에서 제시된 엄격한 Rust 개발 가이드를 따라 작성하여 코드 품질과 유지보수성을 극대화합니다.

---

## 3. Action Plan: 단계별 개선 실행 계획

### Phase 1: 타입 시스템 완전 자동화 (Foundation)

> **목표:** 수동 타입 관리를 완전히 제거하고, 빌드 시점에 타입 정의가 자동으로 동기화되는 파이프라인을 구축합니다.

1.  **`build.rs` 설정:** `src-tauri/build.rs`를 설정하여 `cargo build` 시점에 모든 `#[ts(export)]` 타입이 `src/types/generated/` 디렉토리에 단일 파일(e.g., `index.ts`)로 생성되도록 구성합니다.
2.  **레거시 타입 완전 제거:** `src/types/` 아래의 `crawling.ts`, `events.ts` 등 수동으로 관리되던 모든 타입 파일을 **삭제**합니다.
3.  **`tsconfig.json` 경로 설정:** `tsconfig.json`에 `paths`를 설정하여 `@/types`와 같은 별칭으로 `src/types/generated`를 참조할 수 있도록 구성합니다.
4.  **.gitignore 설정:** `src/types/generated/`를 `.gitignore`에 추가하여 Git 추적에서 제외합니다.

### Phase 2: 프론트엔드 현대화 (Refactoring)

> **목표:** `crawlerStore.ts`가 자동 생성된 타입을 직접 사용하도록 리팩토링하여, 데이터 변환 로직을 제거하고 백엔드의 상태 모델과 1:1로 동기화합니다.

1.  **상태 모델 재정의:** `crawlerStore.ts`의 `CrawlerState` 인터페이스를 자동 생성된 Rust `enum` 타입을 사용하여 백엔드의 상태 기계와 일치하도록 재구성합니다.

    ```typescript
    // src/stores/crawlerStore.ts (리팩토링 후 예시)
    import type { LiveSystemState, SessionResultPayload, ErrorPayload } from '@/types'; // 자동 생성 타입 Import

    interface CrawlerState {
      liveState: LiveSystemState | null;
      lastResult: SessionResultPayload | null;
      lastError: ErrorPayload | null;
      // ...
    }
    ```

2.  **데이터 변환 로직 완전 제거:** `performSiteAnalysis`, `loadDefaultConfig` 등에 남아있는 불필요한 데이터 변환 코드를 모두 삭제합니다. API 응답은 이미 프론트엔드 타입과 완벽히 호환됩니다.

### Phase 3: API 경계 명확화 (Interface Implementation)

> **목표:** 이벤트의 목적을 이름으로 명확히 하여 "삼중 채널" 설계를 코드 레벨에서 구현하고, `crawlerStore`의 이벤트 처리 로직을 단순화합니다.

1.  **목적 기반 이벤트 이름 사용:** 백엔드에서 `emit`하는 이벤트 이름을 목적에 따라 명확하게 구분합니다.
    - **상태 이벤트:** `event-system-state` (`LiveSystemState` 페이로드)
    - **결과 데이터:** `event-session-result` (`SessionResultPayload` 페이로드)
    - **오류:** `event-crawling-error` (`ErrorPayload` 페이로드)

2.  **프론트엔드 리스너 단순화:** `crawlerStore.ts`에서 각 목적에 맞는 이벤트를 명시적으로 구독하여 코드의 가독성과 유지보수성을 높입니다.

    ```typescript
    // crawlerStore.ts (리팩토링 후 예시)
    import { listen } from '@tauri-apps/api/event';
    import type { LiveSystemState } from '@/types';

    // 실시간 상태 업데이트 구독
    await listen<LiveSystemState>('event-system-state', (event) => {
        // event.payload는 이미 LiveSystemState 타입임이 보장됨
        setCrawlerState('liveState', event.payload);
    });
    ```

---

## 4. Modern Rust 2024 개발 가이드 (엄격 준수 사항)

> **목표:** 모든 백엔드 코드가 최상의 품질과 일관성을 유지하도록 합니다.

-   [ ] **모듈 구조 (`mod.rs` 금지):**
    -   **준수:** `src/my_module/mod.rs` 대신 `src/my_module.rs` 또는 `src/my_module/lib.rs`를 사용합니다.
    -   **이유:** 모듈 구조를 명확하게 하고, 파일 탐색을 용이하게 합니다.

-   [ ] **에러 핸들링 (`thiserror` 사용):**
    -   **준수:** 라이브러리 레벨의 에러는 `thiserror`를 사용하여 구체적인 에러 타입을 정의합니다. `anyhow`는 최상위 애플리케이션 레벨에서만 제한적으로 사용합니다.
    -   **이유:** 호출자가 에러 타입에 따라 분기 처리를 할 수 있도록 하여, 더 견고한 코드를 작성하게 합니다.

-   [ ] **비동기 트레이트 (`async_trait` 제거):**
    -   **준수:** Rust 2024 에디션에서는 `async fn in traits`가 안정화되었으므로, `async_trait` 매크로 의존성을 제거하고 `trait MyTrait { async fn my_func(&self); }` 와 같이 직접 사용합니다.
    -   **이유:** 불필요한 의존성을 제거하고, 언어의 기본 기능을 활용합니다.

-   [ ] **패닉 금지 (`unwrap`, `expect` 금지):**
    -   **준수:** 모든 `Option`과 `Result`는 `if let`, `match`, `?` 연산자 등을 통해 명시적으로 처리합니다. 테스트 코드를 제외한 모든 곳에서 `.unwrap()`과 `.expect()` 사용을 금지합니다.
    -   **이유:** 패닉은 스레드를 중단시켜 복구 불가능한 상태를 만듭니다. 모든 오류는 복구 가능하도록 처리하는 것을 원칙으로 합니다.

-   [ ] **엄격한 `clippy` 린트 적용:**
    -   **준수:** `Cargo.toml`에 `[lints.clippy]` 설정을 유지하고, `pedantic`, `nursery` 등의 경고를 해결합니다.
    -   **이유:** 잠재적인 버그와 안티패턴을 컴파일 타임에 발견하여 코드 품질을 높입니다.

-   [ ] **테스트 프레임워크 (`cargo nextest`):**
    -   **준수:** `cargo test` 대신 `cargo nextest`를 사용하여 테스트를 실행합니다.
    -   **이유:** 더 빠른 테스트 실행, 더 나은 UI, 테스트별 타임아웃 설정 등 개발 생산성을 향상시킵니다.

## 5. 기대 효과

본 가이드를 따를 경우, 프로젝트는 다음과 같은 긍정적 효과를 얻게 됩니다.

-   **완전한 타입 안정성:** 백엔드부터 프론트엔드까지 이어지는 End-to-End 타입 안정성을 확보하여 런타임 에러를 원천적으로 줄입니다.
-   **유지보수 비용 극감:** 데이터 변환 로직과 수동 타입 관리가 사라져 코드베이스가 단순해지고 유지보수가 용이해집니다.
-   **개발 생산성 향상:** 백엔드 API 변경이 프론트엔드 타입에 자동으로 반영되어, 개발자는 비즈니스 로직에만 집중할 수 있습니다.
-   **설계-구현 일치:** `re-arch-plan-final2.md`에서 설계한 정교한 아키텍처가 코드 레벨에서 완벽하게 구현됩니다.
