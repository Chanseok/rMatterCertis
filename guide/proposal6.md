# 제안: 백엔드 핵심 로직 검토 및 정상화를 위한 구현 방안

**문서 목적:** 현재 백엔드 구현에서 확인된 3가지 핵심 문제(1. 범위 설정 오류, 2. 비효율적 동시성, 3. 이벤트 누락)를 명확히 진단하고, 이를 해결하여 시스템을 설계된 아키텍처에 맞게 정상화하기 위한 구체적인 기술 제안을 합니다.

--- 

## 1. 진단: 현재 백엔드는 "반쪽짜리" 상태

현재 백엔드는 이벤트 기반 아키텍처의 뼈대는 갖추었으나, 가장 중요한 두뇌와 신경계가 빠져있는 상태입니다.

-   **두뇌 (지능)의 부재:** 스스로 최적의 작업 범위를 계산하는 지능형 로직이 실행 로직과 연결되지 않았습니다.
-   **신경계 (반응속도)의 부재:** 수많은 작업을 동시에 처리하여 성능을 극대화하는 동시성 모델이 잘못 구현되어 있으며, 작업 하나하나의 상태 변화를 UI에 전달하는 실시간 이벤트 시스템이 누락되었습니다.

이 문제를 해결하지 않으면, 우리가 설계한 v4.0 이상의 아키텍처가 제공하는 사용자 경험과 성능은 결코 달성할 수 없습니다.

## 2. 핵심 문제 해결 방안

### 2.1. 문제 1: 지능형 크롤링 범위 설정 기능 정상화

-   **진단:** 시스템이 최적의 크롤링 범위를 성공적으로 계산하고도, 이를 무시하고 UI의 정적 요청 값으로 작업을 시작합니다.
-   **원인:** 범위 계산 로직의 결과가 실제 작업 실행 함수의 인자로 전달되지 않고 유실됩니다.
-   **해결 방안:** `start_crawling` 커맨드의 워크플로우를 명확하게 수정합니다.
    1.  **`start_crawling` 함수 내 책임 분리:** 함수를 "1. 사전 분석 및 범위 계산" 단계와 "2. 계산된 범위로 작업 실행" 단계로 명확히 나눕니다.
    2.  **결과 전달 강제:** 1단계에서 계산된 `(start_page, end_page)` 결과를 변수에 저장하고, 이 변수를 2단계 `engine.start_crawling_session` 함수의 인자로 **반드시** 사용하도록 코드를 수정합니다.
    3.  **코드 예시 (개념):**
        ```rust
        // src-tauri/src/commands/crawling_v4.rs
        #[tauri::command]
        pub async fn start_crawling(...) {
            // ...
            // 1. 지능형 범위 계산 (proposal3, 4의 로직 호출)
            let maybe_range = engine.calculate_intelligent_range().await?;

            if let Some((start_page, end_page)) = maybe_range {
                // 2. 계산된 결과로 크롤링 세션 시작!
                engine.start_crawling_session(start_page, end_page).await?;
            } else {
                // 크롤링 할 내용이 없음, UI에 "완료" 상태 전송
            }
            // ...
        }
        ```

### 2.2. 문제 2: 진정한 동시성 실행 모델 구현

-   **진단:** 동시성 설정에도 불구하고, 작업들이 작은 묶음(chunk) 단위로 순차 처리되어 성능이 저하됩니다.
-   **원인:** `for chunk in ...` 루프를 사용하는 비효율적인 동시성 패턴이 구현되어 있습니다.
-   **해결 방안:** "Spawn All, Control with Semaphore" 패턴으로 전면 교체합니다.
    1.  **`Orchestrator` 또는 `WorkerPool` 수정:** `for chunk` 루프를 완전히 제거합니다.
    2.  **모든 작업 즉시 생성:** 크롤링할 전체 페이지 목록(예: 100개)을 대상으로 `for` 루프를 돌며, **100개의 모든 비동기 태스크(`tokio::spawn`)를 한 번에 생성**합니다.
    3.  **세마포어로 제어:** 각 태스크 내의 시작점에서 `semaphore.acquire().await`를 호출하여 동시에 실행되는 작업의 수를 제한합니다. 작업이 끝나면 `permit`이 자동으로 반납되어 대기 중인 다른 태스크가 즉시 실행됩니다.
    4.  **코드 예시 (개념):**
        ```rust
        // src-tauri/src/crawling/orchestrator.rs (또는 관련 작업자)
        async fn process_page_range(&self, pages: Vec<u32>) {
            let semaphore = Arc::new(Semaphore::new(self.config.max_concurrency));
            let mut tasks = Vec::new();

            for page in pages {
                let sem_clone = semaphore.clone();
                tasks.push(tokio::spawn(async move {
                    let _permit = sem_clone.acquire().await.unwrap();
                    // ... 실제 페이지 fetch 로직 ...
                }));
            }

            futures::future::join_all(tasks).await;
        }
        ```

### 2.3. 문제 3: 실시간 피드백을 위한 원자적 이벤트 구현

-   **진단:** 개별 작업의 상태 변화(시작, 완료, 실패 등)가 UI로 전송되지 않아, 상세하고 동적인 시각화가 불가능합니다.
-   **원인:** 주기적인 종합 상태 스냅샷 이벤트만 존재하고, 개별 상태 변화를 즉시 알리는 이벤트 시스템이 누락되었습니다.
-   **해결 방안:** `proposal5.md`에서 제안된 "듀얼 채널 이벤트 시스템"을 구현합니다.
    1.  **`AtomicTaskEvent` 정의:** `TaskStarted`, `TaskCompleted`, `TaskFailed` 등을 포함하는 `enum`을 Rust와 TypeScript 양쪽에 정의합니다.
    2.  **`Orchestrator`에서 이벤트 발생:** `Orchestrator`가 개별 작업을 처리하고 그 결과를 받는 즉시, `TaskResult`를 `AtomicTaskEvent`로 변환하여 `event://atomic-task-update` 채널로 `emit`하는 로직을 추가합니다.
    3.  **코드 예시 (개념):**
        ```rust
        // src-tauri/src/crawling/orchestrator.rs
        // process_single_task_static 함수가 끝나는 지점

        let result = worker_pool.process_task(task, ...).await;

        // 결과를 즉시 UI에 전송할 이벤트로 변환
        let event = AtomicTaskEvent::from(result);
        app_handle.emit("atomic-task-update", event).unwrap();
        ```

## 4. 결론 및 권장 로드맵

현재 백엔드는 잠재력 있는 아키텍처 위에 세워졌지만, 핵심적인 기능들이 활성화되지 않은 상태입니다. 위 세 가지 문제를 해결하는 것은 선택이 아닌 필수입니다.

**권장 진행 순서:**
1.  **1단계 (성능 정상화):** **문제 2(동시성 모델)**를 최우선으로 해결하여 시스템의 기본 성능을 확보합니다.
2.  **2단계 (로직 정상화):** **문제 1(범위 설정)**을 해결하여 시스템이 올바른 비즈니스 로직으로 동작하게 합니다.
3.  **3단계 (UX 고도화):** **문제 3(원자적 이벤트)**을 해결하여, 정상화된 시스템의 동작을 UI가 완벽하게 표현할 수 있도록 합니다.

이 로드맵에 따라 개선을 진행하면, 백엔드는 설계 문서에 정의된 대로 지능적이고, 효율적이며, 반응성이 뛰어난 시스템으로 거듭날 것입니다.
