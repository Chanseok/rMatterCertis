# 이벤트 기반 크롤링 아키텍처 제안 (상세)

이 문서는 rMatterCertis 크롤링 엔진의 차세대 아키텍처를 제안합니다. 기존의 순차적인 단계별(Stage) 모델에서 벗어나, **작업 큐(Work Queue)에 기반한 유연하고 제어 가능한 이벤트 기반 시스템**으로 전환하는 것을 목표로 합니다.

---

## 1. 핵심 설계 원칙

새로운 아키텍처는 다음 네 가지 핵심 원칙을 기반으로 합니다.

- **명확한 분리 (Decoupling):** '어떤 일'을 할 것인가(작업의 정의)와 '누가' 그 일을 할 것인가(작업자)를 명확하게 분리합니다. 작업은 독립적인 데이터 구조(`enum`)로 정의되며, 작업자는 이 작업을 처리하는 역할만 수행합니다.

- **전문화 (Specialization):** 특정 유형의 작업만 전문적으로 처리하는 '작업자 풀(Worker Pool)'을 구성합니다. 예를 들어, HTML을 가져오는 작업자, HTML을 파싱하는 작업자, 데이터베이스에 저장하는 작업자를 각각 따로 둡니다.

- **비동기 통신 (Asynchronous Communication):** 작업자들은 서로 직접 통신하지 않습니다. 대신, 처리 결과를 다음 단계의 작업 큐에 넣는 방식으로 통신합니다. 이는 각 작업자 그룹이 독립적으로, 비동기적으로 동작할 수 있게 합니다.

- **중앙 집중 제어 (Centralized Control):** 크롤링 세션의 시작, 종료, 취소 등 전체 흐름은 '오케스트레이터(Orchestrator)'가 전담합니다. 제어 신호는 모든 작업자가 공유하는 '공유 상태 객체'를 통해 한 번에 전파됩니다.

---

## 2. 아키텍처 구성 요소

### 2.1. `CrawlingTask`: 모든 작업의 명세서

크롤러가 수행하는 모든 개별 행동은 `CrawlingTask` 열거형(enum)의 한 종류로 정의됩니다. 이는 시스템 내에서 수행될 수 있는 모든 작업을 명확하고 타입-안전하게 관리할 수 있게 해줍니다.

```rust
/// 크롤링 프로세스에서 수행되는 개별적이고 독립적인 작업 단위를 정의합니다.
pub enum CrawlingTask {
    // 1단계: 제품 목록 페이지의 HTML 콘텐츠를 가져옵니다.
    FetchListPage { page: u32 },
    
    // 2단계: 목록 페이지의 HTML을 파싱하여 개별 제품의 URL들을 추출합니다.
    ParseListPageHtml { page: u32, html_content: String },
    
    // 3단계: 개별 제품 상세 페이지의 HTML 콘텐츠를 가져옵니다.
    FetchProductDetailPage { product_url: String },
    
    // 4단계: 상세 페이지의 HTML을 파싱하여 구조화된 제품 정보를 추출합니다.
    ParseProductDetailHtml { product_url: String, html_content: String },
    
    // 5단계: 추출된 제품 정보를 데이터베이스에 저장합니다.
    SaveProduct { product_detail: ProductDetail },
}
```

### 2.2. 작업 큐 (Work Queues)

`tokio::sync::mpsc` (Multi-Producer, Single-Consumer) 채널을 사용하여 각 작업자 풀을 연결하는 파이프라인을 구성합니다. 각 큐는 특정 종류의 `CrawlingTask`만 전달합니다.

- `list_page_fetch_queue`: `FetchListPage` 작업을 전달하는 큐
- `list_page_parse_queue`: `ParseListPageHtml` 작업을 전달하는 큐
- `detail_page_fetch_queue`: `FetchProductDetailPage` 작업을 전달하는 큐
- `detail_page_parse_queue`: `ParseProductDetailHtml` 작업을 전달하는 큐
- `db_save_queue`: `SaveProduct` 작업을 전달하는 큐

### 2.3. `SharedState`: 중앙 제어 및 공유 상태

`Arc`로 감싸진 공유 상태 객체는 크롤링 세션 전반에 걸쳐 제어 메커니즘과 통계 데이터에 대한 접근을 제공합니다.

```rust
/// 크롤링 세션 전체에서 공유되는 상태와 제어 메커니즘을 포함합니다.
pub struct SharedState {
    /// 모든 실행 중인 작업자에게 정상 종료(graceful shutdown) 신호를 보내기 위한 토큰입니다.
    pub cancellation_token: CancellationToken,
    
    /// 동시 실행되는 아웃바운드 HTTP 요청의 최대 개수를 제어하는 세마포입니다.
    pub http_semaphore: Arc<Semaphore>,
    
    /// 실시간 크롤링 통계를 위한 스레드-안전(thread-safe) 컨테이너입니다.
    pub stats: Arc<Mutex<CrawlingStats>>,
}

/// 크롤링 진행 상황에 대한 실시간 통계 정보를 저장합니다.
#[derive(Default)]
pub struct CrawlingStats {
    pub list_pages_fetched: u32,    // 가져온 목록 페이지 수
    pub product_urls_found: u32,    // 찾은 제품 URL 수
    pub detail_pages_fetched: u32,  // 가져온 상세 페이지 수
    pub products_saved: u32,        // DB에 저장된 제품 수
    pub failed_tasks: u32,          // 실패한 작업 수
}
```

### 2.4. 오케스트레이터 (Orchestrator)

오케스트레이터는 전체 크롤링 세션의 생명주기를 관리하는 진입점입니다.

**주요 역할:**
1.  **초기화:** 모든 작업 큐와 `SharedState` 객체를 생성합니다.
2.  **작업자 생성 및 실행:** 각 전문화된 작업자 풀을 생성하고, 필요한 큐의 송신자(Sender)와 `SharedState`의 참조(`Arc`)를 전달하여 비동기 태스크로 실행시킵니다.
3.  **초기 작업 주입 (Seeding):** 첫 번째 작업 큐에 초기 작업을 주입합니다. (예: 1페이지부터 50페이지까지의 `FetchListPage` 작업을 `list_page_fetch_queue`에 넣음)
4.  **생명주기 관리:** 외부로부터의 취소 명령을 감지하고, `CancellationToken`을 통해 모든 작업자에게 중단 신호를 보냅니다. 또한, 모든 큐가 비고 작업자들이 유휴 상태가 되면 크롤링이 완료되었음을 판단하고 시스템을 정상 종료합니다.

### 2.5. 작업자 풀 (Worker Pools)

각 작업자 풀은 동일한 로직을 수행하는 비동기 태스크의 집합입니다. 특정 큐에서 작업을 받아 처리한 후, 그 결과를 다음 단계의 큐로 보냅니다.

- **`ListPageFetcher` 풀:** `list_page_fetch_queue`에서 `FetchListPage` 작업을 가져옵니다. `http_semaphore`를 사용해 동시 요청 수를 제어하며 페이지 HTML을 가져온 후, `ParseListPageHtml` 작업을 생성하여 `list_page_parse_queue`에 보냅니다.
- **`ListPageParser` 풀:** `list_page_parse_queue`에서 `ParseListPageHtml` 작업을 가져옵니다. HTML을 파싱하여 제품 URL들을 추출하고, 추출된 **각각의 URL에 대해** `FetchProductDetailPage` 작업을 생성하여 `detail_page_fetch_queue`에 보냅니다. (하나의 입력이 여러 개의 출력 생성)
- **`ProductDetailFetcher` 풀:** `detail_page_fetch_queue`에서 `FetchProductDetailPage` 작업을 가져옵니다. `http_semaphore`를 사용해 상세 페이지 HTML을 가져온 후, `ParseProductDetailHtml` 작업을 생성하여 `detail_page_parse_queue`에 보냅니다.
- **`ProductDetailParser` 풀:** `detail_page_parse_queue`에서 `ParseProductDetailHtml` 작업을 가져옵니다. HTML을 파싱하여 최종 `ProductDetail` 구조체를 만든 후, `SaveProduct` 작업을 생성하여 `db_save_queue`에 보냅니다.
- **`DbSaver` 풀:** `db_save_queue`에서 `SaveProduct` 작업을 가져옵니다. 최종 결과를 데이터베이스에 저장하며, 이 작업 흐름의 마지막 단계입니다.

---

## 3. 워크플로우 예시

### 3.1. 데이터 흐름 (정상 경로)

하나의 초기 작업이 시스템을 통과하며 변환되는 과정입니다.

1.  **오케스트레이터** -> `list_page_fetch_queue` : `FetchListPage { page: 1 }`
2.  **ListPageFetcher**가 작업을 소비, 페이지 HTML을 가져옴.
3.  **ListPageFetcher** -> `list_page_parse_queue` : `ParseListPageHtml { page: 1, html_content: "..." }`
4.  **ListPageParser**가 작업을 소비, 12개의 제품 URL 추출.
5.  **ListPageParser** -> `detail_page_fetch_queue` : **12개**의 `FetchProductDetailPage { product_url: "..." }` 작업을 보냄.
6.  **ProductDetailFetcher**가 12개의 작업 중 하나를 소비, 상세 페이지 HTML을 가져옴.
7.  **ProductDetailFetcher** -> `detail_page_parse_queue` : `ParseProductDetailHtml { product_url: "...", html_content: "..." }`
8.  **ProductDetailParser**가 작업을 소비, 구조화된 `ProductDetail` 데이터를 추출.
9.  **ProductDetailParser** -> `db_save_queue` : `SaveProduct { product_detail: ProductDetail { ... } }`
10. **DbSaver**가 작업을 소비, DB에 저장. 이 특정 URL에 대한 작업 흐름이 완료됨.

### 3.2. 취소 흐름 (Cancellation Flow)

취소 메커니즘은 즉각적이고 시스템 전체에 걸쳐 우아하게(gracefully) 동작하도록 설계되었습니다.

1.  사용자가 UI 등에서 '취소'를 요청합니다.
2.  **오케스트레이터**는 `shared_state.cancellation_token.cancel()`을 호출합니다.
3.  **모든 작업자**는 메인 루프에서 `tokio::select!` 매크로를 사용하여 자신의 작업 큐와 `cancellation_token`을 **동시에** 기다리고 있습니다.
4.  `cancellation_token.cancelled()` 분기가 즉시 실행됩니다.
5.  작업자는 종료 로그를 남기고 자신의 메인 루프를 `break`하여 태스크를 정상적으로 종료합니다.
6.  결과적으로, 더 이상 큐에서 새로운 작업이 처리되지 않으며, 시스템은 진행 중이던 최소한의 작업만 마친 후 빠르게 비워지고 정지됩니다.

#### 일반적인 작업자 구현 예시

```rust
async fn worker_loop<T>(
    mut input_queue: mpsc::Receiver<T>,
    shared_state: Arc<SharedState>,
    // ... 다음 단계 큐로 작업을 보내기 위한 sender 등
) {
    loop {
        tokio::select! {
            // `biased;`를 사용하여 항상 취소 확인을 최우선으로 처리하도록 합니다.
            biased;

            // 취소 토큰이 활성화되면, 즉시 루프를 탈출하여 작업을 종료합니다.
            _ = shared_state.cancellation_token.cancelled() => {
                info!("취소 신호 감지. 작업자를 종료합니다.");
                break;
            }

            // 취소 신호가 없으면, 입력 큐에서 다음 작업을 기다립니다.
            maybe_task = input_queue.recv() => {
                if let Some(task) = maybe_task {
                    // 실제 작업 처리 로직...
                    // 처리 성공 시, 다음 단계의 작업을 생성하여
                    // 다음 작업 큐로 보냅니다.
                } else {
                    // `None`이 반환되면 채널이 닫혔다는 의미이며, 더 이상 작업이 없으므로 종료합니다.
                    info!("입력 채널이 닫혔습니다. 작업자를 종료합니다.");
                    break;
                }
            }
        }
    }
}
```

---

## 4. 아키텍처의 장점

- **최고의 동시성 및 처리량:** 각 단계의 작업자들이 독립적으로 동작하므로, 특정 단계(예: 네트워크 요청)의 지연이 다른 단계(예: 파싱)를 막지 않습니다. 병목이 발생하는 작업자 풀의 동시성만 선택적으로 늘려(예: `ListPageFetcher` 태스크 수 증가) 전체 성능을 쉽게 튜닝할 수 있습니다.

- **견고하고 즉각적인 제어:** `CancellationToken`을 이용한 중앙 제어 덕분에, 취소는 거의 즉시 시스템 전체에 전파됩니다. 일시정지/재시작과 같은 기능도 유사한 공유 상태 플래그를 통해 쉽게 구현할 수 있습니다.

- **확장성 및 유지보수성:** 크롤링 과정에 새로운 단계를 추가하고 싶을 때(예: 이미지 다운로드, AI 기반 정보 태깅), 새로운 `CrawlingTask` 종류와 해당 작업을 처리할 작업자 풀, 그리고 큐만 추가하면 되므로 기존 코드에 미치는 영향을 최소화할 수 있습니다.

- **회복탄력성 (Resilience):** 단일 작업의 실패(예: 특정 URL의 네트워크 오류)가 전체 시스템을 중단시키지 않습니다. 실패한 작업을 로깅하거나, 별도의 '재시도 큐'에 보내는 등 우아한 오류 처리가 가능합니다.

- **테스트 용이성:** 각 작업자는 독립적인 함수처럼 테스트할 수 있습니다. 모의(mock) 입력 큐를 만들어 작업을 주입하고, 해당 작업자가 올바른 결과물을 출력 큐로 보내는지 확인하는 방식으로 단위 테스트가 매우 용이해집니다.