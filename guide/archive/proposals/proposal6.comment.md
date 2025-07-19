# 제안 코멘트: `proposal6.md`의 상태 저장 아키텍처 강화 방안

**문서 목적:** `proposal6.md`에서 제안된 "상태 저장 백엔드"라는 훌륭한 방향성을 기반으로, 실제 프로덕션 환경에서 발생할 수 있는 엣지 케이스와 사용자 경험의 흐름을 고려하여, 시스템을 더욱 견고하고 예측 가능하게 만들기 위한 구체적인 보완 사항을 제안합니다.

--- 

## 1. `proposal6.md`의 핵심 아이디어 (요약)

-   **방향성:** 백엔드가 "기억"을 가질 수 있도록 `AppStateCache`를 도입하여, 분석 결과를 저장하고 재사용한다.
-   **역할 분리:** `StatusTab`은 분석 및 캐시 업데이트를, `크롤링 시작`은 캐시 활용 및 실행을 담당한다.
-   **결론:** 이 방향성은 **전적으로 올바르며, 반드시 추진해야 할 핵심 개선안**입니다.

## 2. 보완 제안: 프로덕션 레벨의 견고함 확보

`proposal6.md`의 설계를 더욱 완벽하게 만들기 위해 다음 세 가지 보완책을 제안합니다.

### 2.1. 보완 제안 1: "오래된 캐시(Stale Cache)" 문제 해결

-   **문제점:** 사용자가 사이트 분석 후 오랜 시간이 지나 크롤링을 시작하면, 백엔드는 낡은 정보(예: 한 시간 전의 `total_pages`)를 기반으로 잘못된 크롤링 범위를 계산할 수 있습니다.

-   **해결 방안:** **캐시에 유효 시간(Time-To-Live, TTL) 개념을 도입합니다.**

    1.  **`SiteAnalysisResult` 구조체 수정:** 캐시된 시간을 기록하기 위한 타임스탬프 필드를 추가합니다.
        ```rust
        // src-tauri/src/state.rs (예시)
        pub struct SiteAnalysisResult {
            pub total_pages: u32,
            // ... 기타 분석 결과 ...
            pub last_checked_at: DateTime<Utc>, // 타임스탬프 필드 추가!
        }
        ```

    2.  **`start_crawling` 커맨드 로직 강화:** 캐시를 사용하기 전에 신선도(Freshness)를 검사하는 로직을 추가합니다.
        ```rust
        // src-tauri/src/commands/crawling_v4.rs (개념 코드)
        #[tauri::command]
        pub async fn start_crawling(...) {
            let cache = app_state.cache.lock().await;
            let mut needs_re_analysis = true; // 기본적으로는 재분석이 필요하다고 가정

            if let Some(ref analysis) = cache.site_analysis {
                // 캐시가 5분 이내의 신선한 정보인지 확인
                if Utc::now().signed_duration_since(analysis.last_checked_at) < Duration::minutes(5) {
                    info!("신선한 캐시를 사용합니다.");
                    needs_re_analysis = false;
                }
            }

            if needs_re_analysis {
                info!("캐시가 없거나 오래되었습니다. 시스템 분석을 다시 실행합니다.");
                // 내부적으로 분석 로직을 호출하여 캐시를 갱신
                self.run_and_cache_analysis(&mut cache).await?;
            }

            // 이제 캐시는 항상 최신 정보임을 보장할 수 있음
            let range = self.calculate_range_from_cache(&cache).await?;
            // ... 계산된 범위로 크롤링 실행 ...
        }
        ```

-   **기대 효과:** 데이터의 최신성을 보장하여, 항상 가장 정확한 상태를 기반으로 크롤링을 수행하고 오류를 방지합니다.

### 2.2. 보완 제안 2: 사용자 경험 흐름(User Flow) 통일

-   **문제점:** 사용자가 `StatusTab`을 거치지 않고 바로 `크롤링 시작`을 누르는 경우와, 거친 후 누르는 경우의 내부 동작 흐름이 달라져 코드 복잡성이 증가하고 예측이 어려워질 수 있습니다.

-   **해결 방안:** **`start_crawling` 커맨드를 유일한 "상태 보장 관문(State-Ensuring Gateway)"으로 설계합니다.**

    -   **`StatusTab`의 역할 재정의:** `StatusTab`의 분석 버튼은 사용자에게 현재 상태를 **"미리 보여주는(Preview)"** 편의 기능으로 역할을 명확히 합니다. 이 버튼을 누르든 안 누르든, 실제 크롤링 실행 시의 데이터 정합성에는 영향을 미치지 않습니다.
    -   **`start_crawling`의 책임 통일:** `start_crawling` 커맨드는 **언제나** 위 `2.1`에서 제안된 "캐시 신선도 검사 및 갱신" 로직을 **반드시** 수행하도록 책임을 통일합니다.

-   **기대 효과:** 사용자가 어떤 순서로 버튼을 누르든, 시스템은 항상 일관되고 예측 가능한 방식으로 동작합니다. 코드의 흐름이 단일화되어 유지보수성이 향상됩니다.

### 2.3. 보완 제안 3: 상태 관리 역할 명확화 및 통합

-   **문제점:** `AppStateCache`(준-정적 정보)와 `Orchestrator`의 `SharedState`(실시간 운영 정보)가 분리되어 있어, 두 상태 간의 불일치 또는 책임의 모호함이 발생할 수 있습니다.

-   **해결 방안:** **데이터 흐름을 단방향으로 만들고, 각 상태 객체의 책임을 명확히 분리합니다.**

    1.  **`AppStateCache`의 역할 한정:** **"설정 및 분석 결과 캐시"** 로 역할을 명확히 합니다. 크롤링 세션이 시작되기 **전**에 필요한 데이터를 담습니다. (예: `total_pages`, `last_db_cursor`, 사용자 설정 등)

    2.  **`Orchestrator`의 `SharedState` 역할 한정:** **"실시간 운영(Runtime) 상태"** 만을 전담합니다. (예: `active_workers`, `queue_depth`, `eta`, 현재 처리량 등)

    3.  **명확한 데이터 흐름 정의:**
        -   `Orchestrator`는 **시작 시점**에 `AppStateCache`의 정보를 **단 한 번 읽어서(read)** 자신의 초기 설정을 구성합니다.
        -   `Orchestrator`는 자신의 실시간 운영 상태를 `AppStateCache`에 다시 **쓰지(write) 않습니다.**

    4.  **`SystemStateBroadcaster`의 역할:** UI에 상태를 전송하는 `SystemStateBroadcaster`는 이 두 곳(`AppStateCache`와 `Orchestrator`의 `SharedState`)의 정보를 **모두 취합(aggregate)**하여 최종적인 `SystemStatePayload`를 만들어 UI에 전달하는 유일한 통로가 됩니다.

-   **기대 효과:** 데이터의 흐름이 단방향(읽기 전용)으로 명확해지고, 각 상태 관리 객체의 책임이 분리되어 시스템 전체의 복잡성이 감소하며, 상태 불일치 버그를 원천적으로 방지합니다.

## 최종 결론

`proposal6.md`에서 제안된 **상태 저장 백엔드로의 전환은 전적으로 올바른 방향**입니다. 여기에 위에서 제안된 **1) 캐시 유효 시간 도입, 2) 사용자 경험 흐름 통일, 3) 상태 관리 역할 명확화**를 추가로 보완한다면, 우리의 아키텍처는 단순히 동작하는 것을 넘어, 어떤 상황에서도 안정적이고 예측 가능하게 동작하는 **"프로덕션 레디(Production-Ready)"** 수준의 견고함을 갖추게 될 것입니다.
