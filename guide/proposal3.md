# 제안: '오래된 데이터 우선' 증분 크롤링 로직 구현 방안 (수정본)

**문서 목적:** 현재 구현의 잘못된 크롤링 범위 설정 로직을 바로잡고, "로컬 DB에 저장된 가장 오래된 데이터 지점부터 이어서 수집"하는 핵심 비즈니스 요구사항을 정확히 구현하기 위한 기술적 방안을 제안합니다.

--- 

## 1. 문제 재정의: 잘못된 크롤링 목표

현재 시스템은 로컬 DB의 진행 상황을 무시하고 항상 최신 페이지만을 수집하려고 시도합니다. 이는 리소스 낭비이며, 전체 데이터를 순차적으로 확보하려는 원래의 목표에 부합하지 않습니다.

**올바른 목표:**
1.  가장 오래된 데이터(가장 높은 페이지 번호)부터 수집을 시작합니다.
2.  로컬 DB에 저장된 데이터의 끝 지점을 정확히 파악합니다.
3.  다음 크롤링은 그 끝 지점 바로 다음 데이터부터 시작하여, 점진적으로 전체 데이터를 채워나갑니다.

## 2. 올바른 로직 설계: 증분 크롤링 알고리즘

다음과 같은 단계별 알고리즘을 통해 올바른 크롤링 범위를 계산해야 합니다.

**Step 1: 사전 분석 (Prerequisite Analysis)**
-   크롤링 시작 전, 사이트 전체 상태를 분석하여 **`total_pages_on_site`** (예: 482) 값을 먼저 확정합니다. 이 값은 모든 계산의 기준점이 됩니다.

**Step 2: 로컬 DB 커서 조회 (Query Local DB Cursor)**
-   로컬 데이터베이스에 다음을 조회하는 함수를 구현합니다.
    -   `get_last_crawled_cursor() -> Option<(i32, i32)>`
-   이 함수는 `products` 테이블에서 `pageId`가 가장 큰 값을 찾고, 그 `pageId` 내에서 `indexInPage`가 가장 큰 값을 찾아 `(pageId, indexInPage)` 튜플로 반환해야 합니다. DB가 비어있으면 `None`을 반환합니다.

**Step 3: 다음 크롤링 시작 페이지 계산 (Calculate Next Target Page)**
-   `get_last_crawled_cursor()`의 결과에 따라 다음 시작 페이지를 계산합니다.

    -   **Case A: DB가 비어있는 경우 (`None` 반환):**
        -   크롤링을 처음 시작하는 상황입니다.
        -   **`next_start_page = total_pages_on_site`** (예: 482). 가장 오래된 페이지부터 시작합니다.

    -   **Case B: DB에 데이터가 있는 경우 (`Some((last_page_id, last_index_in_page))` 반환):**
        -   `last_page_id` (예: 9), `last_index_in_page` (예: 7)를 가져옵니다.
        -   **`items_per_page`** (예: 12) 설정을 가져옵니다.
        -   **수정된 로직:** 마지막으로 일부라도 수집된 페이지부터 이어서 크롤링을 시작합니다. `indexInPage` 값은 다음 시작 아이템을 찾는 데 사용될 수 있지만, 페이지 단위 크롤링에서는 해당 페이지를 다시 포함하는 것이 가장 간단하고 안전합니다.
        -   **`start_page_from_db = total_pages_on_site - last_page_id`** (예: `482 - 9 = 473`).
        -   **`next_start_page = start_page_from_db`**. 즉, 마지막으로 작업했던 473 페이지부터 다시 시작하여 누락된 데이터가 없도록 보장합니다.

**Step 4: 최종 크롤링 범위 결정 (Determine Final Crawling Range)**
-   **`crawl_batch_size`** (예: 10 페이지) 설정을 가져옵니다.
-   **`final_start_page = next_start_page`** (예: 473)
-   **`final_end_page = (next_start_page - crawl_batch_size + 1).max(1)`** (예: `473 - 10 + 1 = 464`). 1보다 작아지지 않도록 보장합니다.
-   최종 범위는 **`final_start_page`부터 `final_end_page`까지 역순으로** 진행됩니다. (예: 473, 472, ..., 464)

## 3. 구체적인 구현 제안

### 3.1. 데이터베이스 조회 기능 추가

-   **위치:** `src-tauri/src/infrastructure/product_repository.rs` (또는 유사 파일)
-   **추가 함수:**
    ```rust
    // 가장 마지막으로 수집된 데이터의 위치를 반환하는 함수
    pub async fn get_last_crawled_cursor(&self) -> Result<Option<(i32, i32)>> {
        let result = sqlx::query_as(
            "SELECT pageId, MAX(indexInPage) as maxIndex FROM products WHERE pageId = (SELECT MAX(pageId) FROM products)"
        )
        .fetch_optional(&self.pool)
        .await?;
        
        // 실제 반환 타입에 맞게 변환 필요
        // Ok(result.map(|row| (row.pageId, row.maxIndex)))
        unimplemented!();
    }
    ```

### 3.2. 범위 계산 로직 중앙화

-   **위치:** `src-tauri/src/crawling/orchestrator.rs` 또는 신규 모듈 `src-tauri/src/crawling/range_calculator.rs`
-   **추가 함수:** 위에 기술된 **Step 3, 4 알고리즘**을 구현하는 `calculate_next_range` 함수를 작성합니다.

### 3.3. `start_crawling` 커맨드 수정

-   **위치:** `src-tauri/src/commands/crawling_v4.rs`
-   **수정 내용:** 하드코딩된 `start_crawling_session` 호출을 제거하고, 위에서 설계한 알고리즘을 호출하는 로직으로 대체합니다.

    ```rust
    // 개념적 코드 (수정 후)
    #[tauri::command]
    pub async fn start_crawling(...) -> Result<...> {
        // ... 엔진 준비 ...

        // 1. 사이트 분석으로 총 페이지 수 획득
        let total_pages = engine.analyze_site().await?;

        // 2. DB 커서 조회
        let last_cursor = engine.get_db_cursor().await?;

        // 3. 다음 크롤링 범위 계산
        let range = engine.calculate_next_range(total_pages, last_cursor).await?;

        if let Some((start_page, end_page)) = range {
            info!("Intelligent crawling range determined: {} -> {}", start_page, end_page);
            // 4. 계산된 범위로 크롤링 세션 시작
            engine.start_crawling_session(start_page, end_page).await?;
        } else {
            info!("No new pages to crawl. System is up to date.");
            // UI에 "업데이트 완료" 상태 전송
        }

        // ... 상태 브로드캐스팅 시작 ...
        Ok(...)
    }
    ```

## 4. 기대 효과

-   **정확성:** 시스템이 항상 DB 상태를 기반으로 필요한 데이터만 정확히 타겟하여 수집합니다.
-   **효율성:** 불필요한 페이지 재수집을 방지하여 시간과 네트워크 리소스를 크게 절약합니다.
-   **무결성:** 마지막으로 작업한 페이지를 다시 포함하여, 해당 페이지에서 누락된 아이템이 없도록 보장합니다.
-   **자동화:** 수동으로 크롤링 범위를 지정할 필요 없이, 시스템이 스스로 진행 상황을 파악하고 다음 작업을 결정하는 완전 자동화된 증분 수집이 가능해집니다.