
# 최종 아키텍처 현행화 및 최종 개선 계획 (v4)

> **문서 목적**: `re-arch-plan-final2.md`와 `re-arch-plan-final3.md`의 지향점과, 최근 성공적으로 완료된 아키텍처 리팩토링 결과를 종합하여 현 상태를 기록하고, 백엔드 코드의 완결성을 100%로 끌어올리기 위한 최종 개선 과제를 정의하는 단일 최신 계획 문서(Single Source of Truth)를 수립합니다.

---

## 1. 현재 아키텍처 현황 및 성과 (AS-IS & Achievements)

최근 진행된 대규모 리팩토링을 통해, 크롤링 엔진은 초기 목표였던 **"담백하고, 유연하며, 확장 가능한 구조"** 를 성공적으로 달성했습니다.

- **✅ Actor 책임과 역할 완벽 분리**:
  - 세션의 전체 생명주기 관리 책임이 `commands` 모듈에서 `SessionActor` 내부로 완벽하게 이전되었습니다.
  - 이를 통해 Actor 모델의 원칙을 준수하고, 코드의 응집도와 명확성이 극대화되었습니다.

- **✅ Strategy + Template Method 패턴 도입**:
  - `StageActor`는 이제 실행의 '틀(Template)'만 제공하며, 실제 로직은 `crawl_engine/stages/strategies/` 폴더 내의 `StageStrategy` 구현체들이 담당합니다.
  - 새로운 Stage 추가 시 기존 코드를 수정할 필요가 없는, OCP 원칙을 준수하는 유연한 구조가 완성되었습니다.

- **✅ 이벤트 시스템 통합 및 명확화**:
  - 레거시 `CrawlingEvent`가 완전히 제거되고, 모든 이벤트가 `AppEvent`로 통합되었습니다.
  - `Phase`, `PageTask`, `*Lifecycle` 등 모호하고 중복된 이벤트들이 정리되어, `Session` > `Batch` > `Stage` > `Task`의 명확한 계층 구조를 따르는 이벤트 체계가 완성되었습니다.

## 2. 최종 완결성을 위한 개선 과제 (Final Polishing Tasks)

현재 아키텍처는 95% 이상 완성되었으며, 핵심 구조는 완벽합니다. 아래 제안들은 코드의 완결성을 100%로 끌어올리기 위한 최종 "다듬기(Polish)" 단계에 해당합니다.

### 과제 1: `PlanningService` 및 `PlanningStrategy` 패턴 도입

- **목표**: 자동/수동/재개 등 다양한 계획 수립 시나리오를 명확히 분리하여, `commands` 모듈의 책임을 더욱 줄이고 계획 수립 로직의 확장성을 확보합니다.
- **현황**: 현재 계획 수립 로직은 `commands/actor_system_commands.rs`의 `create_execution_plan` 함수에 집중되어 있습니다.
- **개선 방안**:
  1.  `PlanningStrategy` 트레이트를 정의합니다. (`async fn create_plan(...)`)
  2.  `IntelligentPlanningStrategy`(자동), `ManualPlanningStrategy`(수동) 등 구체적인 전략 클래스를 구현합니다.
  3.  `PlanningService`를 도입하여, `ActorCrawlingRequest`의 내용에 따라 적절한 `Strategy`를 선택하고 실행하는 역할을 담당합니다.
  4.  Tauri Command는 이 `PlanningService`를 호출하는 역할만 수행하도록 단순화합니다.

### 과제 2: `CrawlingPolicy` 객체 도입

- **목표**: 크롤링의 세부 동작 규칙(중복 처리, 재시도 등)을 코드에서 분리하여, 명시적이고 유연하게 관리합니다.
- **현황**: 현재는 이러한 정책들이 각 Actor 또는 서비스 로직 내에 암시적으로 구현되어 있습니다.
- **개선 방안**:
  1.  `CrawlingPolicy` 구조체를 정의합니다. (예: `duplicate_handling: Skip/Overwrite`, `retry_attempts: 3`)
  2.  `ExecutionPlan` 생성 시, 현재 시나리오(자동/수동)에 맞는 `CrawlingPolicy`를 함께 생성하여 `ExecutionPlan`에 포함시킵니다.
  3.  각 Actor는 작업을 수행할 때, `ExecutionPlan`에 담긴 `CrawlingPolicy`를 참고하여 자신의 세부 동작을 결정합니다.

### 과제 3: 횡단 관심사 처리 고도화 (AOP 스타일)

- **목표**: 이벤트 발행, 로깅 로직을 비즈니스 로직과 더욱 완벽하게 분리합니다.
- **현황**: 현재는 `StageActor`의 Template Method 등에서 횡단 관심사를 처리하는 구조의 기반이 마련되었습니다.
- **개선 방안**:
  1.  **Middleware 패턴 도입**: `StageActor`가 `StageStrategy`를 실행하기 전후에 로깅, 이벤트 발행 등을 처리하는 `Middleware` 계층을 도입하는 것을 고려할 수 있습니다.
  2.  예를 들어, `LoggingMiddleware`, `EventingMiddleware` 등을 구현하여 `StageStrategy`의 `execute` 메서드를 감싸는(wrapping) 구조로 만듭니다. 이는 Actor의 핵심 비즈니스 로직을 더욱 순수하게 유지하는 데 도움이 됩니다.

## 3. 최종 검증 및 마무리 (Final Verification)

위의 개선 과제들이 모두 완료된 후, 프로젝트의 안정성과 코드 품질을 최종적으로 보장하기 위해 다음 단계를 반드시 거칩니다.

1.  **`cargo check`**: 전체 프로젝트가 문제없이 컴파일되는지 확인합니다.
2.  **`cargo clippy -- -D warnings`**: 모든 Clippy 경고를 에러 수준으로 격상하여 잠재적인 문제를 모두 수정하고, 최상의 코드 품질을 확보합니다.
3.  **`cargo test --all-features`**: 모든 단위 테스트 및 통합 테스트를 실행하여, 아키텍처 변경으로 인한 기능적 회귀(regression)가 없음을 완벽하게 검증합니다.
