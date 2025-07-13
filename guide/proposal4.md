# 제안: `pageId` 및 `indexInPage` 역산 로직 구현 및 적용 방안

**문서 목적:** 현재 누락된 핵심 비즈니스 로직인 `pageId`와 `indexInPage`의 역산(Inverse) 계산 기능을 정확히 구현하고, 이를 전체 크롤링 파이프라인에 적용하기 위한 구체적인 기술 방안을 제안합니다.

--- 

## 1. 문제 정의: 데이터 좌표계의 불일치

현재 시스템은 수집된 데이터의 좌표(`pageId`, `indexInPage`)를 웹사이트의 순서 그대로 기록하고 있습니다. 이는 "가장 오래된 데이터를 우선하여 `0`번으로 삼는다"는 시스템의 핵심 데이터 정렬 원칙과 정면으로 위배됩니다.

**올바른 좌표계:**
-   **`pageId`:** 가장 오래된 페이지(웹사이트에서 가장 높은 번호의 페이지)가 `pageId = 0`이 되어야 합니다.
-   **`indexInPage`:** 각 페이지 내에서 가장 오래된 제품(HTML 구조상 가장 아래쪽에 있는 제품)이 `indexInPage = 0`이 되어야 합니다.

이 문제를 해결하지 않으면, 증분 수집 로직(`proposal3.md`에서 논의된)이 올바르게 동작할 수 없습니다.

## 2. 근본 원인: 파싱 컨텍스트(Context)의 부재

이 문제의 직접적인 원인은 HTML 파싱을 담당하는 컴포넌트(예: `HtmlParser`)가 계산에 필요한 **맥락 정보 없이** 작업을 수행하기 때문입니다.

-   **`pageId` 계산 불가:** 파서는 `total_pages`와 `current_page_number`를 모르기 때문에 `pageId = total_pages - current_page_number` 수식을 계산할 수 없습니다.
-   **`indexInPage` 계산 불가:** 파서는 페이지 내의 `total_products`를 미리 알지 못하고 각 아이템을 순차적으로 처리하기 때문에, `indexInPage = total_products - 1 - i` 수식을 계산할 수 없습니다.

## 3. 해결 방안: 컨텍스트 주입 및 계산 로직 구현

**핵심 전략:** 파싱을 수행하는 작업자에게 필요한 모든 컨텍스트를 명확하게 전달하고, 그 컨텍스트를 기반으로 정확한 계산 로직을 수행하도록 책임을 부여합니다.

### 3.1. Step 1: `ParsingContext` 구조체 정의

파싱에 필요한 모든 컨텍스트 정보를 담는 구조체를 명시적으로 정의하여, 파이프라인을 통해 안전하게 전달합니다.

-   **위치:** `src-tauri/src/crawling/tasks.rs` 또는 신규 `context.rs`
-   **정의:**
    ```rust
    /// HTML 파싱 시 필요한 모든 컨텍스트 정보
    #[derive(Debug, Clone)]
    pub struct ParsingContext {
        /// 현재 파싱 중인 웹사이트 페이지 번호 (예: 482, 481, ...)
        pub current_page_number: u32,
        /// 사이트의 전체 페이지 수 (예: 482)
        pub total_pages_on_site: u32,
        // 필요시 추가 컨텍스트 (예: 세션 ID, 타임스탬프 등)
    }
    ```

### 3.2. Step 2: 파서(Worker)의 책임 변경

`ListPageParser` 작업자의 `process_task` 메서드 시그니처를 수정하여, `ParsingContext`를 받도록 변경합니다. `CrawlingTask::ParseListPage` 정의 자체에 이 컨텍스트를 포함시키는 것이 가장 이상적입니다.

-   **위치:** `src-tauri/src/crawling/tasks.rs`
-   **수정된 Task 정의:**
    ```rust
    // CrawlingTask enum 내부
    ParseListPage {
        task_id: TaskId,
        html_content: String,
        // context 필드 추가
        context: ParsingContext,
    },
    ```
-   **결과:** `ListPageFetcher`는 `ParseListPage` 태스크를 생성할 때, 자신이 알고 있는 페이지 번호와 전체 페이지 수를 `ParsingContext`에 담아 전달해야 하는 책임이 생깁니다.

### 3.3. Step 3: `ListPageParser` 내 역산 로직 구현

`ListPageParser`의 `process_task` 메서드 내부에서, 전달받은 컨텍스트를 사용하여 정확한 `pageId`와 `indexInPage`를 계산합니다.

-   **위치:** `src-tauri/src/crawling/workers/list_page_parser.rs`
-   **구현 로직:**
    ```rust
    // list_page_parser.rs의 process_task 메서드 내부
    async fn process_task(&self, task: CrawlingTask, ...) -> Result<...> {
        if let CrawlingTask::ParseListPage { html_content, context, .. } = task {
            // 1. HTML 파싱하여 제품 요소(elements) 목록을 먼저 가져옴
            let product_elements = self.parse_product_elements_from_html(&html_content)?;

            // 2. 페이지 내 제품 총 개수 확인
            let num_products_on_page = product_elements.len();

            // 3. pageId 계산 (단 한 번만 수행)
            let page_id = context.total_pages_on_site - context.current_page_number;

            let mut extracted_products = Vec::new();

            // 4. 제품 목록을 순회하며 역산 인덱스 부여
            for (i, element) in product_elements.iter().enumerate() {
                // 5. indexInPage 계산
                let index_in_page = num_products_on_page - 1 - i;

                // 6. 파싱하여 ProductData 생성
                let mut product_data = self.parse_single_product(element)?;

                // 7. 계산된 좌표를 ProductData에 할당
                product_data.page_id = Some(page_id as i32);
                product_data.index_in_page = Some(index_in_page as i32);

                extracted_products.push(product_data);
            }

            // ... 추출된 제품 목록을 다음 단계(TaskOutput)로 반환 ...
            // ...
        }
    }
    ```

## 4. 기대 효과 및 검증

-   **데이터 정합성:** 모든 수집 데이터가 설계된 좌표계에 따라 정확하게 저장됩니다.
-   **증분 수집 가능:** `proposal3.md`에서 설계한 증분 수집 로직이 이 정확한 좌표계를 기반으로 올바르게 동작할 수 있게 됩니다.
-   **로직 중앙화:** 좌표 계산의 책임이 `ListPageParser`로 명확하게 중앙화되어, 코드의 이해와 유지보수가 쉬워집니다.

**검증 방안:**
-   구현 완료 후, 테스트 크롤링을 실행합니다.
-   이후, SQLite DB를 직접 열어 `products` 테이블의 내용을 확인합니다.
-   `SELECT pageId, indexInPage, source_url FROM products ORDER BY pageId ASC, indexInPage ASC` 쿼리를 실행하여, 가장 오래된 페이지(가장 높은 웹 페이지 번호)의 가장 아래쪽 제품이 `pageId=0`, `indexInPage=0`으로 저장되었는지 반드시 검증해야 합니다.
