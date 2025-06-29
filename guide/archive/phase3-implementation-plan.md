# Phase 3: 크롤링 엔진 + 프론트엔드 완성 구현 계획

**📅 계획 수립일**: 2025년 6월 28일  
**🎯 목표**: Phase 3 완료 (크롤링 엔진 + SolidJS UI 완성)  
**⏰ 예상 기간**: 3-4주  
**📊 현재 진행률**: 60% → 100%

## 🎯 Phase 3 완료 목표

### ✅ 이미 완료된 기능 (60%)
- ✅ **백엔드 아키텍처**: Clean Architecture, Repository Pattern, Use Cases, DTO
- ✅ **메모리 기반 세션 관리**: SessionManager 완전 구현
- ✅ **기본 프론트엔드**: 벤더 관리 시스템, DB 모니터링
- ✅ **Tauri API**: 15개 백엔드 명령어 구현 완료
- ✅ **테스트 인프라**: 인메모리 데이터베이스, TestUtils

### 🚧 구현해야 할 기능 (40%)
- 🚧 **크롤링 엔진**: HTTP 클라이언트, HTML 파싱, Matter 데이터 추출
- 🚧 **제품 관리 UI**: 검색, 필터링, 상세 보기
- 🚧 **크롤링 대시보드**: 실시간 모니터링, 세션 제어
- 🚧 **라우팅 시스템**: SolidJS Router, 멀티페이지

---

## 📋 Week 1-2: 크롤링 엔진 구현 (백엔드)

### 🎯 **Week 1 목표: 크롤링 인프라 구축**

#### **Day 1-2: HTTP 클라이언트 & 기본 크롤러**

**구현 파일:**
```
src-tauri/src/infrastructure/
├── crawler.rs          // 메인 크롤링 엔진
├── http_client.rs      // HTTP 요청 관리
└── rate_limiter.rs     // 요청 속도 제한
```

**주요 구현 내용:**
```rust
// src-tauri/src/infrastructure/crawler.rs
pub struct WebCrawler {
    client: reqwest::Client,
    rate_limiter: RateLimiter,
    session_manager: Arc<SessionManager>,
}

impl WebCrawler {
    pub async fn start_crawling(&self, config: CrawlingConfig) -> Result<String>;
    pub async fn crawl_page(&self, url: &str) -> Result<CrawledPage>;
    pub async fn extract_product_links(&self, html: &str) -> Result<Vec<String>>;
}
```

**기술 스택:**
- `reqwest`: HTTP 클라이언트
- `tokio`: 비동기 처리
- `tokio::time`: Rate limiting

#### **Day 3-4: HTML 파서 & Matter 데이터 추출**

**구현 파일:**
```
src-tauri/src/infrastructure/
├── html_parser.rs      // HTML 파싱 로직
├── matter_scraper.rs   // Matter 특화 데이터 추출
└── selectors.rs        // CSS 셀렉터 설정
```

**주요 구현 내용:**
```rust
// src-tauri/src/infrastructure/matter_scraper.rs
pub struct MatterDataExtractor {
    selectors: SelectorConfig,
}

impl MatterDataExtractor {
    pub fn extract_product_data(&self, html: &str) -> Result<ProductData>;
    pub fn extract_matter_details(&self, html: &str) -> Result<MatterProductData>;
    pub fn extract_vendor_info(&self, html: &str) -> Result<VendorData>;
}
```

**기술 스택:**
- `scraper`: HTML 파싱
- `select`: CSS 셀렉터
- `regex`: 텍스트 정리

#### **Day 5: 크롤링 Use Cases & 통합**

**구현 파일:**
```
src-tauri/src/application/
└── crawling_use_cases.rs  // 크롤링 비즈니스 로직
```

**주요 구현 내용:**
```rust
// src-tauri/src/application/crawling_use_cases.rs
pub struct CrawlingUseCases {
    crawler: Arc<WebCrawler>,
    session_manager: Arc<SessionManager>,
    product_repository: Arc<dyn ProductRepository>,
}

impl CrawlingUseCases {
    pub async fn start_matter_crawling(&self, dto: StartCrawlingDto) -> Result<String>;
    pub async fn pause_crawling(&self, session_id: &str) -> Result<()>;
    pub async fn resume_crawling(&self, session_id: &str) -> Result<()>;
    pub async fn stop_crawling(&self, session_id: &str) -> Result<()>;
}
```

### 🎯 **Week 2 목표: 크롤링 완성 & API 연동**

#### **Day 6-7: Tauri Commands 추가**

**구현 파일:**
```
src-tauri/src/commands.rs  // 크롤링 관련 명령어 추가
```

**추가할 Commands:**
```rust
#[tauri::command]
pub async fn start_crawling(dto: StartCrawlingDto) -> Result<String, String>;

#[tauri::command]
pub async fn get_crawling_status(session_id: String) -> Result<SessionStatusDto, String>;

#[tauri::command]
pub async fn pause_crawling(session_id: String) -> Result<(), String>;

#[tauri::command]
pub async fn stop_crawling(session_id: String) -> Result<(), String>;

#[tauri::command]
pub async fn get_crawling_results(session_id: String) -> Result<CrawlingResultDto, String>;
```

#### **Day 8-9: 크롤링 설정 & 에러 처리**

**구현 파일:**
```
src-tauri/src/infrastructure/
├── config.rs           // 크롤링 설정 관리
└── error_handler.rs    // 에러 복구 로직
```

**설정 구조:**
```rust
#[derive(Debug, Deserialize)]
pub struct CrawlingConfig {
    pub max_concurrent_requests: u32,
    pub request_delay_ms: u64,
    pub timeout_seconds: u64,
    pub retry_attempts: u32,
    pub start_url: String,
    pub target_domains: Vec<String>,
}
```

#### **Day 10: 크롤링 테스트 & 검증**

**구현 파일:**
```
src-tauri/src/bin/
└── test_crawler.rs     // 크롤링 테스트 CLI
```

**테스트 내용:**
- Matter 인증 사이트 실제 크롤링
- 세션 관리 동작 확인
- 에러 시나리오 테스트

---

## 📋 Week 3: 프론트엔드 완성

### 🎯 **Week 3 목표: SolidJS UI 완성**

#### **Day 11-12: 제품 관리 UI**

**구현 파일:**
```
src/components/features/products/
├── ProductList.tsx         // 제품 목록 컴포넌트
├── ProductDetail.tsx       // 제품 상세 정보
├── ProductSearch.tsx       // 검색 및 필터링
├── MatterProductCard.tsx   // Matter 제품 카드
└── ProductTable.tsx        // 테이블 뷰
```

**주요 기능:**
```tsx
// src/components/features/products/ProductList.tsx
export function ProductList() {
  const [products, setProducts] = createSignal<MatterProduct[]>([]);
  const [searchQuery, setSearchQuery] = createSignal("");
  const [filters, setFilters] = createSignal<ProductFilters>({});
  
  const searchProducts = async () => {
    const results = await invoke<ProductSearchResultDto>("search_matter_products", {
      dto: { query: searchQuery(), ...filters() }
    });
    setProducts(results.products);
  };
  
  return (
    <div class="product-list">
      <ProductSearch onSearch={searchProducts} />
      <ProductTable products={products()} />
    </div>
  );
}
```

#### **Day 13-14: 크롤링 대시보드**

**구현 파일:**
```
src/components/features/crawling/
├── CrawlingDashboard.tsx   // 크롤링 메인 대시보드
├── SessionControl.tsx      // 세션 시작/중지 컨트롤
├── ProgressMonitor.tsx     // 실시간 진행상황
├── CrawlingHistory.tsx     // 크롤링 이력
└── CrawlingConfig.tsx      // 크롤링 설정
```

**주요 기능:**
```tsx
// src/components/features/crawling/CrawlingDashboard.tsx
export function CrawlingDashboard() {
  const [sessions, setSessions] = createSignal<SessionStatusDto[]>([]);
  const [activeSession, setActiveSession] = createSignal<string | null>(null);
  
  const startCrawling = async (config: CrawlingConfig) => {
    const sessionId = await invoke<string>("start_crawling", { dto: config });
    setActiveSession(sessionId);
    startPolling(sessionId);
  };
  
  const startPolling = (sessionId: string) => {
    const interval = setInterval(async () => {
      const status = await invoke<SessionStatusDto>("get_crawling_status", { sessionId });
      updateSessionStatus(status);
    }, 1000);
  };
  
  return (
    <div class="crawling-dashboard">
      <SessionControl onStart={startCrawling} />
      <ProgressMonitor sessionId={activeSession()} />
      <CrawlingHistory sessions={sessions()} />
    </div>
  );
}
```

#### **Day 15: 라우팅 & 내비게이션**

**구현 파일:**
```
src/
├── App.tsx             // 라우터 설정
├── components/layout/
│   ├── Navigation.tsx  // 메인 내비게이션
│   ├── Sidebar.tsx     // 사이드바
│   └── Header.tsx      // 헤더
└── pages/
    ├── Dashboard.tsx   // 대시보드 페이지
    ├── Products.tsx    // 제품 관리 페이지
    ├── Crawling.tsx    // 크롤링 페이지
    └── Settings.tsx    // 설정 페이지
```

**라우터 구조:**
```tsx
// src/App.tsx
import { Router, Route } from "@solidjs/router";

function App() {
  return (
    <Router>
      <div class="app">
        <Navigation />
        <main class="main-content">
          <Route path="/" component={Dashboard} />
          <Route path="/products" component={Products} />
          <Route path="/crawling" component={Crawling} />
          <Route path="/vendors" component={Vendors} />
          <Route path="/settings" component={Settings} />
        </main>
      </div>
    </Router>
  );
}
```

---

## 📋 Week 4: 통합 & 테스트

### 🎯 **Week 4 목표: 시스템 통합 및 최적화**

#### **Day 16-17: 크롤링-UI 연동**

**실시간 업데이트 구현:**
```tsx
// src/services/CrawlingService.ts
export class CrawlingService {
  private pollingInterval: number | null = null;
  
  startStatusPolling(sessionId: string, callback: (status: SessionStatusDto) => void) {
    this.pollingInterval = setInterval(async () => {
      try {
        const status = await invoke<SessionStatusDto>("get_crawling_status", { sessionId });
        callback(status);
      } catch (error) {
        console.error("Failed to get crawling status:", error);
      }
    }, 1000);
  }
  
  stopStatusPolling() {
    if (this.pollingInterval) {
      clearInterval(this.pollingInterval);
      this.pollingInterval = null;
    }
  }
}
```

#### **Day 18-19: 에러 핸들링 & 복구**

**에러 시나리오 처리:**
```rust
// src-tauri/src/infrastructure/error_handler.rs
pub struct CrawlingErrorHandler {
    retry_config: RetryConfig,
}

impl CrawlingErrorHandler {
    pub async fn handle_request_error(&self, error: RequestError) -> RecoveryAction {
        match error {
            RequestError::Timeout => RecoveryAction::Retry,
            RequestError::RateLimited => RecoveryAction::Delay(Duration::from_secs(60)),
            RequestError::Blocked => RecoveryAction::Stop,
            _ => RecoveryAction::Retry,
        }
    }
}
```

#### **Day 20: 성능 최적화 & 테스트**

**성능 최적화:**
```rust
// 메모리 사용량 최적화
pub struct StreamingProductProcessor {
    batch_size: usize,
}

impl StreamingProductProcessor {
    pub async fn process_products_batch(&self, products: Vec<Product>) -> Result<()> {
        // 배치 단위로 처리하여 메모리 사용량 제한
        for chunk in products.chunks(self.batch_size) {
            self.save_products_batch(chunk).await?;
        }
        Ok(())
    }
}
```

**E2E 테스트:**
```rust
#[tokio::test]
async fn test_full_crawling_workflow() {
    let ctx = TestContext::new().await.unwrap();
    
    // 1. 크롤링 시작
    let session_id = ctx.crawling_use_cases
        .start_matter_crawling(StartCrawlingDto {
            start_url: "https://certification.csa-iot.org".to_string(),
            target_domains: vec!["csa-iot.org".to_string()],
        })
        .await
        .unwrap();
    
    // 2. 진행상황 확인
    let status = ctx.session_manager.get_session_status(&session_id).unwrap();
    assert_eq!(status.status, SessionStatus::Running);
    
    // 3. 결과 확인
    tokio::time::sleep(Duration::from_secs(10)).await;
    let products = ctx.product_repository.find_all_matter_products().await.unwrap();
    assert!(!products.is_empty());
}
```

---

## 🎯 Phase 3 완료 기준

### ✅ **기능적 요구사항**

#### **크롤링 엔진**
- [ ] CSA-IoT Matter 인증 사이트 완전 크롤링
- [ ] 1000개 이상 제품 처리 가능
- [ ] 실시간 진행상황 모니터링
- [ ] 세션 관리 (시작/일시정지/재시작/중지)
- [ ] 에러 복구 및 재시도 로직

#### **프론트엔드 UI**
- [ ] 직관적인 제품 관리 인터페이스
- [ ] 고급 검색 및 필터링 (제조사, 디바이스 타입, VID 등)
- [ ] 실시간 크롤링 대시보드
- [ ] 반응형 디자인 (데스크톱 최적화)
- [ ] 페이지네이션 및 무한 스크롤

#### **시스템 통합**
- [ ] 크롤링-UI 실시간 연동
- [ ] 에러 상황 사용자 알림
- [ ] 설정 관리 (크롤링 속도, 동시 요청 수 등)

### ✅ **성능 요구사항**

- [ ] **메모리 사용량**: < 500MB (대용량 크롤링 시)
- [ ] **응답 시간**: < 3초 (UI 인터랙션)
- [ ] **크롤링 속도**: 초당 5-10 페이지 처리
- [ ] **데이터베이스**: 10,000개 제품 처리 가능

### ✅ **품질 요구사항**

- [ ] **테스트 커버리지**: 80% 이상
- [ ] **에러 처리**: 모든 실패 시나리오 대응
- [ ] **로깅**: 구조화된 로그 출력
- [ ] **문서화**: API 문서 및 사용자 가이드

---

## 🛠 기술 스택 & 의존성

### **백엔드 추가 의존성**
```toml
# Cargo.toml에 추가
[dependencies]
reqwest = { version = "0.11", features = ["json", "cookies"] }
scraper = "0.18"
select = "0.6"
regex = "1.5"
tokio-util = { version = "0.7", features = ["time"] }
governor = "0.6"  # Rate limiting
```

### **프론트엔드 추가 의존성**
```json
// package.json에 추가
{
  "dependencies": {
    "@solidjs/router": "^0.13.0",
    "solid-icons": "^1.1.0",
    "@solidjs/meta": "^0.29.0"
  }
}
```

---

## 📊 진행상황 추적

### **Week 1 체크포인트**
- [ ] HTTP 클라이언트 기본 동작
- [ ] HTML 파싱 테스트 통과
- [ ] Matter 데이터 추출 검증

### **Week 2 체크포인트**
- [ ] 크롤링 Use Cases 완성
- [ ] Tauri Commands 연동
- [ ] 실제 사이트 크롤링 성공

### **Week 3 체크포인트**
- [ ] 제품 관리 UI 완성
- [ ] 크롤링 대시보드 완성
- [ ] 라우팅 시스템 동작

### **Week 4 체크포인트**
- [ ] 전체 시스템 통합
- [ ] 성능 최적화 완료
- [ ] 모든 테스트 통과

---

## 🚀 Phase 4 준비사항

Phase 3 완료 후 다음 단계:

1. **배포 준비**: Tauri 앱 빌드 최적화
2. **모니터링**: 프로덕션 환경 로깅
3. **문서화**: 사용자 매뉴얼 작성
4. **성능 테스트**: 대규모 데이터 처리 검증

---

**📝 마지막 업데이트**: 2025년 6월 28일  
**👥 담당자**: 개발팀  
**📋 상태**: Phase 3 구현 계획 확정
