# New Architecture 구현 현황 분석: 크롤링 시작 및 계획 수립

> **문서 목적:** 사용자가 "크롤링 시작" 버튼을 눌렀을 때, `new_architecture`가 설계(`re-arch-plan-final2.md`)에 따라 어떻게 동작하는지, 특히 **크롤링 계획 수립 과정**을 중심으로 상세히 설명합니다. 이를 통해 설계와 구현의 정합성을 검증하고 개선 방향을 도출합니다.

## 1. 크롤링 시작 시의 전체 동작 흐름 (Top-Down)

현재 구현된 `actor_system.rs`와 `crawling_integration.rs`를 분석한 결과, 크롤링 시작 시의 흐름은 다음과 같이 진행됩니다.

1.  **`SessionActor` 생성 및 실행:**
    -   UI에서 크롤링 시작 명령이 들어오면, 최상위 관리자인 `SessionActor`가 생성되고 실행(`run`)됩니다.
    -   `SessionActor`는 자신의 `command_rx` (명령 수신 채널)을 통해 `ProcessBatch`와 같은 구체적인 작업 명령을 기다립니다.

2.  **`ProcessBatch` 명령 수신 및 처리:**
    -   `SessionActor`가 `ProcessBatch` 명령을 받으면, `process_batch` 메서드를 호출하여 본격적인 작업을 시작합니다.
    -   이때, `BatchPlan`이라는 실행 계획 객체를 생성합니다. 이 객체에는 크롤링할 페이지 목록, 배치 크기, 동시성 제한 등 작업에 필요한 모든 정보가 담겨 있습니다.

3.  **`BatchActor` 스폰 및 결과 대기 (One-shot Channel 활용):**
    -   `SessionActor`는 `spawn_and_wait_for_batch` 메서드를 통해 **일회성 결과 보고 채널(One-shot Channel)**을 가진 `BatchActor`를 생성하고 비동기적으로 실행(spawn)합니다.
    -   `BatchActor`는 자신에게 할당된 페이지 목록(`BatchPlan.pages`)을 받아 작업을 수행합니다.
    -   `SessionActor`는 `BatchActor`가 작업을 마치고 One-shot 채널을 통해 최종 결과(`StageResult`)를 보낼 때까지 비동기적으로 대기합니다.

4.  **`BatchActor`의 스테이지 실행:**
    -   `BatchActor`는 `process_batch_with_oneshot` 메서드 내에서, 할당된 페이지들을 `StageActor`가 처리할 수 있는 작은 단위(chunk)로 나눕니다.
    -   각 페이지(또는 아이템)에 대해 `process_single_page_with_retry`를 호출하여 실제 크롤링을 수행합니다. 이 과정에서 재시도 정책(`RetryCalculator`)이 적용됩니다.

5.  **실제 크롤링 서비스 연동:**
    -   `BatchActor`는 `execute_real_crawling_stage` 메서드를 통해 `CrawlingIntegrationService`를 호출합니다. 이 서비스는 기존에 구현된 `StatusCheckerImpl`, `ProductListCollectorImpl` 등 실제 크롤링 로직을 감싸고 있습니다.
    -   이를 통해 **새로운 Actor 모델 아키텍처가 기존의 검증된 크롤링 로직을 재사용**하게 됩니다.

6.  **결과 보고 및 세션 종료:**
    -   `BatchActor`는 모든 작업이 끝나면 그 결과를 `StageResult` 형태로 종합하여 One-shot 채널로 전송합니다.
    -   `SessionActor`는 이 결과를 수신하고, `handle_batch_result`를 통해 성공/실패 여부를 판단한 후 관련 이벤트를 발행하고 세션을 종료합니다.

---

## 2. 크롤링 계획 수립 과정 (상세 분석)

설계 문서(`re-arch-plan-final2.md`)에서는 `CrawlingPlanner`라는 별도의 컴포넌트가 "사이트/DB 분석"을 통해 지능적으로 실행 계획을 수립하는 것을 이상적인 목표로 제시했습니다. 현재 구현은 이 목표를 향한 **첫 번째 단계를 구현**한 상태입니다.

### 현재 구현된 계획 수립 방식

현재 크롤링 계획 수립은 `SessionActor`가 `ProcessBatch` 명령을 받는 시점에 **명령에 포함된 파라미터를 기반으로 `BatchPlan`을 생성**하는 방식으로 이루어집니다. `CrawlingPlanner`라는 별도의 Actor나 컴포넌트는 아직 구현되지 않았습니다.

```rust
// actor_system.rs - SessionActor::process_batch

async fn process_batch(
    &mut self,
    pages: Vec<u32>,
    config: BatchConfig,
    batch_size: u32,
    concurrency_limit: u32,
) -> Result<(), ActorError> {
    // 💡 핵심: 명령으로 받은 인자를 그대로 사용하여 실행 계획(BatchPlan)을 생성
    let batch_plan = BatchPlan {
        batch_id: Uuid::new_v4().to_string(),
        pages, // UI 또는 상위 로직에서 결정한 페이지 목록
        config: config.clone(),
        batch_size, // UI 또는 상위 로직에서 결정한 배치 크기
        concurrency_limit, // UI 또는 상위 로직에서 결정한 동시성
    };

    // ... 이 계획을 사용하여 BatchActor를 스폰
    match self.spawn_and_wait_for_batch(batch_plan).await {
        // ...
    }
}
```

**상세 설명:**

1.  **계획 주체:** 현재는 `SessionActor` 외부의 호출자(예: Tauri 커맨드 핸들러)가 계획의 주체입니다. 호출자가 크롤링할 `pages` 목록과 `batch_size`, `concurrency_limit` 등을 모두 결정하여 `ProcessBatch` 명령에 담아 전달해야 합니다.
2.  **계획 내용:** `BatchPlan`은 "어떤 페이지들을(pages), 어떤 설정으로(config), 한 번에 몇 개씩(batch_size), 최대 몇 개의 동시 작업으로(concurrency_limit) 처리할 것인가"를 정의합니다.
3.  **지능성 수준:** 현재는 **정적(Static) 계획 수립** 단계입니다. DB 상태나 사이트 변경 사항을 동적으로 분석하여 크롤링 범위를 최적화하는 지능적인 로직은 아직 포함되어 있지 않습니다.

### 설계 목표와 현재 구현의 차이점 및 발전 방향

-   **설계 목표:** `CrawlingPlanner`가 `check_site_status`, `get_database_statistics` 등을 통해 **시스템의 현재 상태를 분석**하고, "신규 페이지만 크롤링", "누락된 데이터만 복구", "전체 재수집" 등 **최적의 크롤링 전략과 범위를 동적으로 결정**하여 `BatchPlan`을 생성합니다.
-   **현재 구현:** `CrawlingPlanner`의 기능이 아직 구현되지 않았으며, 계획 수립이 외부(호출자)에 의해 정적으로 이루어집니다.

**따라서, 현재 구현은 설계의 최종 목표를 향한 중요한 첫걸음이며, 향후 다음과 같이 발전시켜야 합니다.**

1.  **`CrawlingPlanner` 구현:** `re-arch-plan-final2.md`에 명시된 대로, 사이트와 DB를 분석하는 로직을 가진 `CrawlingPlanner` 컴포넌트를 구현합니다.
2.  **`SessionActor`와 연동:** `SessionActor`가 `StartCrawling` 명령을 받으면, `CrawlingPlanner`를 호출하여 분석 및 계획 수립을 요청합니다.
3.  **동적 계획 생성:** `CrawlingPlanner`는 분석 결과를 바탕으로 최적의 `BatchPlan`들을 생성하여 `SessionActor`에게 반환합니다.
4.  **실행:** `SessionActor`는 `CrawlingPlanner`로부터 받은 동적 계획에 따라 `BatchActor`들을 실행합니다.

## 3. Modern Rust 2024 개발 가이드 준수 현황

- **[✅ 잘된 점]**
    - **`thiserror` 사용:** `ActorError` enum에 `#[derive(Debug, thiserror::Error)]`가 적용되어 있어, 에러 타입을 명확히 구분하고 있습니다.
    - **패닉 방지:** 코드 전반에 걸쳐 `Result`와 `?` 연산자를 사용하며, `unwrap()`이나 `expect()`의 사용이 보이지 않습니다.
    - **엄격한 `clippy` 린트:** 파일 상단에 `#![warn(clippy::all, ...)]` 등이 선언되어 있어 코드 품질 관리에 신경 쓰고 있음을 알 수 있습니다.

- **[⚠️ 개선 필요]**
    - **`async_trait` 잠재적 사용:** 현재 코드에서는 보이지 않지만, 향후 Actor 트레이트를 정의할 경우 `async_trait` 매크로 대신 Rust 2024의 `async fn in traits`를 사용해야 합니다.
    - **모듈 구조:** 현재는 단일 파일(`actor_system.rs`)에 모든 Actor가 정의되어 있지만, 향후 기능이 복잡해지면 `session_actor.rs`, `batch_actor.rs` 등으로 파일을 분리하여 `mod.rs` 없는 모듈 구조를 적용해야 합니다.

## 4. 결론 및 다음 단계 제안

- **결론:** 현재 `new_architecture`는 설계의 핵심인 **Actor 모델과 One-shot 채널을 이용한 결과 보고 메커니즘을 성공적으로 구현**했습니다. 하지만, **지능형 계획 수립(`CrawlingPlanner`) 기능은 아직 구현되지 않은 상태**이며, 이는 아키텍처의 완전한 잠재력을 발휘하기 위한 다음 핵심 과제입니다.

- **다음 단계 제안:**
    1.  **`CrawlingPlanner` 구현:** `CrawlingIntegrationService`의 `execute_site_analysis`, `calculate_crawling_recommendation` 등의 메서드를 활용하여, 동적으로 크롤링 계획(`Vec<BatchPlan>`)을 생성하는 `CrawlingPlanner`를 구현합니다.
    2.  **`SessionActor` 로직 수정:** `SessionActor`가 `StartCrawling` 명령을 받았을 때, 정적인 파라미터를 사용하는 대신 `CrawlingPlanner`를 호출하여 동적으로 생성된 계획에 따라 `BatchActor`를 실행하도록 수정합니다.
    3.  **파일 구조 리팩토링:** `actor_system.rs` 파일이 비대해지고 있으므로, 각 Actor를 별도의 파일로 분리하여 Modern Rust 2024의 모듈 구조 가이드를 준수하도록 리팩토링을 진행합니다.