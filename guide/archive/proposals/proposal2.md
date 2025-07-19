# 제안: 크롤링 범위 및 동시성 실행 문제 해결 방안

**문서 목적:** `back_front.log` 분석 결과 확인된 두 가지 주요 문제(1. 지능형 범위 계산 로직 미적용, 2. 초기 동시성 비효율)의 원인을 진단하고, 이를 해결하여 시스템 성능과 안정성을 설계 수준으로 끌어올리기 위한 구체적인 기술 제안을 합니다.

--- 

## 1. 현상 및 문제 정의

로그 분석 결과, 현재 크롤링 시스템은 다음과 같은 심각한 성능 및 로직 문제를 겪고 있습니다.

1.  **지능형 범위 계산 로직 미적용:**
    -   **현상:** 로그에 따르면, 시스템은 사이트 분석(총 482페이지 확인)과 DB 분석(116개 제품 존재)을 통해 최적의 크롤링 범위(`482` ~ `383`)를 성공적으로 계산했습니다. 하지만, 실제 크롤링은 이 계산 결과를 무시하고 UI로부터 전달된 하드코딩된 범위(`1` ~ `100`)로 작업을 시작합니다.
    -   **로그 증거:**
        ```log
        INFO 💡 Intelligent range recommendation: pages 482 to 383 (current: 1 to 100)
        INFO 🔍 Collecting from 100 pages in range 1 to 100 with cancellation support
        ```
    -   **문제:** 이는 시스템의 핵심 지능화 로직이 전혀 활용되지 않고 있음을 의미하며, 불필요한 페이지를 재수집하여 리소스를 낭비하고 최신 데이터 확보를 지연시킵니다.

2.  **초기 동시성 비효율:**
    -   **현상:** 동시 작업자(concurrent requests) 수가 3으로 설정되어 있음에도 불구하고, 크롤링 시작 직후에는 **순차적인 배치 처리**가 이루어집니다. 3개의 작업을 동시에 시작하는 것이 아니라, 3개짜리 배치를 실행하고, 끝나면 딜레이를 가진 후, 다음 3개짜리 배치를 실행하는 방식으로 동작합니다.
    -   **로그 증거:**
        ```log
        INFO 🚀 Starting cancellable parallel batch of 3 pages
        ... (3개 페이지 fetch 로그)
        INFO 📊 Completed cancellable parallel batch: 3/100 pages processed...
        INFO ⏱️  Rate limiting delay applied between batches
        INFO 🚀 Starting cancellable parallel batch of 3 pages
        ```
    -   **문제:** 이는 동시성의 이점을 전혀 살리지 못하는 구조입니다. 100개의 페이지를 수집해야 한다면, 100개의 작업을 동시에 생성하고 세마포어(Semaphore)가 3개씩 처리하도록 제어해야 하는데, 현재는 3개씩 묶인 순차적인 그룹으로 처리하여 전체 작업 시간이 불필요하게 길어집니다.

## 2. 근본 원인 심층 분석

### 2.1. '범위 계산 로직'이 무시되는 이유

-   **원인:** `start_crawling` 커맨드 함수 또는 이를 호출하는 상위 서비스(`ServiceBasedBatchCrawlingEngine`)의 로직 흐름 문제입니다. 지능형 범위 계산을 수행하는 부분과, 실제 작업을 큐에 넣는 부분이 서로 다른 파라미터 소스를 사용하고 있습니다.
    1.  **계산:** `intelligent_range_recalculation` 단계에서 올바른 범위(`482..383`)를 계산합니다.
    2.  **무시:** 하지만, `ProductListCollector` 서비스를 호출할 때는 이 계산 결과를 인자로 넘겨주는 대신, UI로부터 받은 초기 요청값(`1..100`)을 그대로 사용합니다.
    -   **결론:** 계산된 결과가 최종 실행 단계에 전달되지 않고 중간에 유실되고 있습니다.

### 2.2. '초기 동시성'이 비효율적인 이유

-   **원인:** `ProductListCollectorImpl`의 `collect_page_range` 함수 구현 방식에 있습니다. 이 함수는 전체 페이지 목록을 `max_concurrent` 크기의 **청크(chunk)로 나누고, `for` 루프를 통해 이 청크들을 순차적으로 처리**합니다.
    ```rust
    // 개념적 코드 (현재 구현)
    for chunk in pages.chunks(max_concurrent) {
        // 1. 현재 청크(3개)에 대한 작업만 생성
        let mut tasks = Vec::new();
        for &page in chunk {
            tasks.push(tokio::spawn(...));
        }

        // 2. 현재 청크(3개)가 모두 끝날 때까지 기다림
        futures::future::join_all(tasks).await;

        // 3. 다음 청크로 넘어가기 전 딜레이
        tokio::time::sleep(delay).await;
    }
    ```
-   **문제점:** 이 방식은 진정한 의미의 동시성 제어가 아닙니다. 작업 하나가 빨리 끝나도 다른 작업들이 끝날 때까지 기다려야 하며, 전체 작업 파이프라인이 계속 막히게 됩니다. 이는 리소스 활용률을 심각하게 저하시킵니다.

--- 

## 3. 해결 방안 및 구체적인 기술 제안

### 3.1. 제안 1: 지능형 범위 계산 결과가 최종 실행에 반영되도록 수정

**핵심 목표:** 계산된 최적의 범위를 최종 작업 실행자에게 명확하게 전달하고 사용하도록 강제합니다.

1.  **상태 및 컨텍스트 구조체 활용:**
    -   **실행 방안:** 크롤링 세션 전체의 상태를 관리하는 컨텍스트(Context) 구조체를 도입하거나 기존 구조체를 확장합니다. `start_crawling` 함수의 시작점에서 이 컨텍스트를 생성합니다.
    -   `intelligent_range_recalculation` 단계가 끝나면, 계산된 `(start_page, end_page)`를 이 컨텍스트 객체에 명시적으로 저장합니다.
    -   이후 `ProductListCollector`를 호출할 때는, UI 입력값이 아닌 **컨텍스트 객체에 저장된 범위 값**을 인자로 전달합니다.

2.  **`start_crawling` 로직 리팩토링:**
    -   **실행 방안:** `start_crawling` 함수의 책임을 명확히 분리합니다. UI 입력은 단지 '크롤링 시작'이라는 의도를 전달하는 용도로만 사용하고, 실제 파라미터(범위 등)는 함수 내부의 분석 및 계산 단계를 통해 결정되도록 로직을 수정합니다. UI에서 받은 범위 값은 사용자가 '수동 범위 지정'을 명시적으로 선택했을 때만 사용하도록 예외 처리합니다.
    -   **기대 효과:** 분석-계산-실행 단계가 강력하게 결합되어, 계산 결과가 누락되는 실수를 원천적으로 방지하고 항상 시스템이 최적의 판단을 내리도록 보장합니다.

### 3.2. 제안 2: 진정한 동시성 실행을 위한 작업자 로직 개선

**핵심 목표:** `chunks` 기반의 순차적 배치 처리를 버리고, 세마포어를 이용한 진정한 동시성 제어 파이프라인을 구축합니다.

1.  **`collect_page_range` 함수 전면 재구현:**
    -   **실행 방안:** `for chunk in ...` 루프를 제거하고, 다음과 같은 로직으로 재구현합니다.
        1.  **세마포어 생성:** `let semaphore = Arc::new(Semaphore::new(max_concurrent));`
        2.  **모든 작업 즉시 생성:** 크롤링할 모든 페이지(예: 100개)에 대해 `for` 루프를 돌며, **100개의 `tokio::spawn` 태스크를 미리 전부 생성**하여 `Vec<JoinHandle>`에 저장합니다.
        3.  **세마포어로 제어:** 각 `spawn`된 태스크 내부의 시작 지점에서 `semaphore.acquire().await`를 호출합니다. 이렇게 하면, 100개의 태스크가 생성은 되었지만, 오직 3개만이 세마포어의 허가(permit)를 받아 실제 네트워크 요청을 시작할 수 있습니다.
        4.  **파이프라인 실행:** 작업 하나가 끝나면 `permit`이 자동으로 반납되고, 대기 중이던 다른 태스크가 즉시 그 `permit`을 획득하여 실행을 시작합니다. 이로써 작업 파이프라인이 멈춤 없이 계속 흘러가게 됩니다.
        5.  **최종 대기:** 모든 태스크를 생성한 후, `futures::future::join_all(tasks).await`를 호출하여 100개의 모든 작업이 완료되기를 기다립니다.

    -   **개념적 코드 (수정 후):**
        ```rust
        async fn collect_page_range(&self, pages: Vec<u32>) -> Result<...> {
            let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent));
            let mut tasks = Vec::new();

            for page in pages {
                let sem_clone = semaphore.clone();
                tasks.push(tokio::spawn(async move {
                    // 실행 허가를 받을 때까지 대기
                    let _permit = sem_clone.acquire().await.unwrap();
                    // ... 실제 페이지 fetch 로직 ...
                }));
            }

            // 모든 작업이 완료될 때까지 기다림
            let results = futures::future::join_all(tasks).await;
            // ... 결과 취합 ...
        }
        ```
    -   **기대 효과:** 크롤링 시작과 동시에 설정된 `max_concurrent` 만큼의 작업이 병렬로 실행되어 유휴 시간 없이 최대 효율로 리소스를 활용합니다. 전체 작업 완료 시간이 극적으로 단축됩니다.

## 4. 결론

현재 시스템은 지능형 아키텍처의 기반은 갖추었으나, 핵심 로직의 연동 부재와 비효율적인 동시성 구현으로 인해 제 성능을 발휘하지 못하고 있습니다. 위에 제안된 **1) 범위 계산 로직 연동**과 **2) 동시성 실행 방식 개선**을 적용하면, 시스템은 설계 본연의 지능적이고 효율적인 모습으로 동작하게 될 것입니다. 이 두 가지 개선 작업을 최우선 순위로 진행할 것을 강력히 권장합니다.
