# Matter Certis v2 - 크롤링 엔진 세부 워크플로우

> 크롤링 엔진의 단계별 세부 동작과 최적화 전략을 상세히 분석합니다.

## 📋 목차

1. [크롤링 엔진 개요](#크롤링-엔진-개요)
2. [세부 워크플로우](#세부-워크플로우)
3. [병렬 처리 전략](#병렬-처리-전략)
4. [데이터 추출 및 검증](#데이터-추출-및-검증)
5. [에러 처리 및 복구](#에러-처리-및-복구)
6. [성능 최적화](#성능-최적화)

## 크롤링 엔진 개요

### 현재 구현 상태
- ✅ **기본 크롤링 인프라**: HTTP 클라이언트, 세션 관리, 데이터베이스 연결
- ✅ **데이터 추출 로직**: HTML 파싱, 제품 정보 추출, Matter 인증 데이터 파싱
- ✅ **저장소 패턴**: Repository 패턴으로 데이터 저장
- ✅ **세션 관리**: 진행 상황 추적, 오류 로깅, 상태 관리

### 핵심 컴포넌트
```rust
// src-tauri/src/infrastructure/crawler.rs
pub struct WebCrawler {
    http_client: HttpClient,                    // HTTP 요청 처리
    session_manager: Arc<SessionManager>,       // 세션 상태 관리
    visited_urls: Arc<Mutex<HashSet<String>>>, // 중복 방문 방지
    product_repo: Arc<SqliteProductRepository>, // 제품 데이터 저장
    vendor_repo: Arc<SqliteVendorRepository>,   // 벤더 데이터 저장
}
```

## 세부 워크플로우

### Phase 1: 크롤링 세션 시작
```
1. 세션 초기화
   ├── 세션 ID 생성 (UUID)
   ├── 초기 설정 검증 (URL, 도메인, 제한값)
   ├── 세션 매니저에 등록
   └── 상태를 'Active'로 설정

2. 초기 URL 검증
   ├── robots.txt 확인
   ├── 도메인 허용 목록 검사
   ├── URL 형식 유효성 검증
   └── 시작 페이지 접근 가능성 확인
```

### Phase 2: 페이지 크롤링 루프
```
메인 크롤링 루프:
while (urls_to_crawl.len() > 0 && pages_crawled < max_pages) {
    current_url = urls_to_crawl.pop()
    
    1. 중복 방문 확인
       ├── visited_urls 체크
       ├── 이미 방문한 경우 스킵
       └── 방문 기록에 추가
    
    2. 페이지 요청 및 다운로드
       ├── HTTP GET 요청
       ├── 응답 상태 코드 확인 (200, 300번대 처리)
       ├── Content-Type 검증 (HTML 여부)
       ├── 페이지 크기 제한 확인
       └── 응답 본문 텍스트 변환
    
    3. 페이지 내용 분석
       ├── HTML 파싱 (scraper crate)
       ├── 제목 추출
       ├── 링크 추출 및 필터링
       └── 구조화된 데이터 식별
    
    4. 제품 데이터 추출
       ├── Matter 인증 테이블 탐지
       ├── 제품 정보 파싱
       ├── 인증 데이터 검증
       └── 추출된 데이터 정규화
    
    5. 데이터베이스 저장
       ├── 중복 제품 검사
       ├── 데이터 유효성 검증
       ├── 트랜잭션 시작
       ├── 제품/벤더 데이터 저장
       └── 트랜잭션 커밋
    
    6. 새로운 URL 발견
       ├── 현재 페이지에서 링크 추출
       ├── 제품 관련 URL 필터링
       ├── 도메인 허용 목록 확인
       ├── 우선순위 큐에 추가
       └── 방문 예정 목록 업데이트
    
    7. 세션 상태 업데이트
       ├── 진행률 계산 (crawled/total)
       ├── 현재 페이지 정보 업데이트
       ├── 오류 카운트 증가 (실패 시)
       └── 통계 정보 갱신
    
    8. 요청 간 지연
       ├── 설정된 delay_ms 대기
       ├── 서버 부하 고려 조절
       └── Rate limiting 준수
}
```

### Phase 3: 세션 완료 및 정리
```
1. 세션 상태 최종 업데이트
   ├── 상태를 'Completed' 또는 'Failed'로 설정
   ├── 최종 통계 계산
   ├── 처리 시간 기록
   └── 오류 요약 생성

2. 리소스 정리
   ├── HTTP 클라이언트 정리
   ├── 메모리 해제
   ├── 임시 파일 삭제
   └── 로그 파일 압축

3. 결과 요약 생성
   ├── 크롤링된 페이지 수
   ├── 추출된 제품 수
   ├── 발생한 오류 수
   └── 소요 시간 및 성능 지표
```

## 병렬 처리 전략

### 현재 구현: 순차 처리
```rust
// 현재는 단일 스레드 순차 처리
for url in urls_to_crawl {
    let page = self.crawl_page(&url).await?;
    self.extract_and_save_products(&page, &session_id, page_count).await?;
}
```

### 향후 병렬 처리 계획
```rust
use tokio::sync::Semaphore;
use futures::stream::{self, StreamExt};

impl WebCrawler {
    pub async fn crawl_parallel(&self, config: CrawlingConfig) -> Result<()> {
        let semaphore = Arc::new(Semaphore::new(config.concurrent_requests as usize));
        let session_id = &config.session_id;
        
        // URL 스트림을 병렬로 처리
        stream::iter(urls_to_crawl)
            .map(|url| {
                let crawler = self.clone();
                let permit = semaphore.clone();
                let session_id = session_id.clone();
                
                async move {
                    let _permit = permit.acquire().await?;
                    crawler.process_single_url(url, &session_id).await
                }
            })
            .buffer_unordered(config.concurrent_requests as usize)
            .collect::<Vec<_>>()
            .await;
            
        Ok(())
    }
    
    async fn process_single_url(&self, url: String, session_id: &str) -> Result<()> {
        // 1. 중복 확인 (thread-safe)
        {
            let mut visited = self.visited_urls.lock().await;
            if visited.contains(&url) {
                return Ok(());
            }
            visited.insert(url.clone());
        }
        
        // 2. 페이지 크롤링
        let page = self.crawl_page(&url).await?;
        
        // 3. 데이터 추출 및 저장
        self.extract_and_save_products(&page, session_id, 0).await?;
        
        // 4. 세션 진행률 업데이트 (thread-safe)
        self.session_manager.increment_progress(session_id).await?;
        
        Ok(())
    }
}
```

## 데이터 추출 및 검증

### HTML 구조 분석 및 적응형 파싱
```rust
impl WebCrawler {
    fn extract_matter_products(&self, html: &str, page_id: u32) -> Result<Vec<ExtractedMatterProduct>> {
        let document = Html::parse_document(html);
        let mut products = Vec::new();
        
        // 1. 테이블 기반 데이터 추출 시도
        if let Some(table_products) = self.extract_from_tables(&document, page_id)? {
            products.extend(table_products);
        }
        
        // 2. JSON-LD 구조화 데이터 추출 시도
        if products.is_empty() {
            if let Some(json_products) = self.extract_from_json_ld(&document, page_id)? {
                products.extend(json_products);
            }
        }
        
        // 3. CSS 클래스 기반 추출 시도
        if products.is_empty() {
            if let Some(css_products) = self.extract_from_css_selectors(&document, page_id)? {
                products.extend(css_products);
            }
        }
        
        // 4. 텍스트 패턴 매칭 추출 (최후 수단)
        if products.is_empty() {
            if let Some(pattern_products) = self.extract_from_text_patterns(&document, page_id)? {
                products.extend(pattern_products);
            }
        }
        
        // 5. 추출된 데이터 검증 및 정규화
        let validated_products = products.into_iter()
            .filter_map(|p| self.validate_and_normalize_product(p))
            .collect();
            
        Ok(validated_products)
    }
    
    fn validate_and_normalize_product(&self, mut product: ExtractedMatterProduct) -> Option<ExtractedMatterProduct> {
        // 필수 필드 검증
        if product.manufacturer.is_none() && product.model.is_none() && product.certificate_id.is_none() {
            return None;
        }
        
        // 데이터 정규화
        if let Some(ref mut manufacturer) = product.manufacturer {
            *manufacturer = manufacturer.trim().to_string();
            if manufacturer.is_empty() {
                product.manufacturer = None;
            }
        }
        
        // 인증 ID 형식 검증
        if let Some(ref cert_id) = product.certificate_id {
            if !self.is_valid_certificate_id(cert_id) {
                tracing::warn!("Invalid certificate ID format: {}", cert_id);
                product.certificate_id = None;
            }
        }
        
        Some(product)
    }
    
    fn is_valid_certificate_id(&self, cert_id: &str) -> bool {
        // CSA Matter 인증 ID 패턴 검증
        // 예: CSA-IOT-12345, CERT-MATTER-67890
        let patterns = [
            regex::Regex::new(r"^CSA-[A-Z]+-\d+$").unwrap(),
            regex::Regex::new(r"^CERT-[A-Z]+-\d+$").unwrap(),
            regex::Regex::new(r"^[A-Z0-9]{8,20}$").unwrap(),
        ];
        
        patterns.iter().any(|pattern| pattern.is_match(cert_id))
    }
}
```

### 데이터 품질 보장
```rust
#[derive(Debug, Clone)]
pub struct DataQualityMetrics {
    pub total_extracted: u32,
    pub validated_products: u32,
    pub duplicate_products: u32,
    pub missing_critical_fields: u32,
    pub invalid_formats: u32,
}

impl WebCrawler {
    async fn save_with_quality_check(&self, products: Vec<ExtractedMatterProduct>, session_id: &str) -> Result<DataQualityMetrics> {
        let mut metrics = DataQualityMetrics::default();
        metrics.total_extracted = products.len() as u32;
        
        for product in products {
            // 1. 중복 확인
            if self.is_duplicate_product(&product).await? {
                metrics.duplicate_products += 1;
                continue;
            }
            
            // 2. 필수 필드 검증
            if !self.has_critical_fields(&product) {
                metrics.missing_critical_fields += 1;
                continue;
            }
            
            // 3. 형식 검증
            if !self.validate_product_format(&product) {
                metrics.invalid_formats += 1;
                continue;
            }
            
            // 4. 저장
            self.save_matter_product(&product).await?;
            metrics.validated_products += 1;
        }
        
        // 품질 메트릭을 세션에 기록
        self.session_manager.update_quality_metrics(session_id, metrics.clone()).await?;
        
        Ok(metrics)
    }
}
```

## 에러 처리 및 복구

### 계층별 에러 처리
```rust
#[derive(Debug, thiserror::Error)]
pub enum CrawlingError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),
    
    #[error("HTML parsing failed: {0}")]
    ParseError(String),
    
    #[error("Database operation failed: {0}")]
    DatabaseError(#[from] sqlx::Error),
    
    #[error("Session management error: {0}")]
    SessionError(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Rate limit exceeded for domain: {0}")]
    RateLimitError(String),
    
    #[error("Maximum retries exceeded for URL: {0}")]
    MaxRetriesError(String),
}

impl WebCrawler {
    async fn crawl_with_retry(&self, url: &str, max_retries: u32) -> Result<CrawledPage, CrawlingError> {
        let mut last_error = None;
        
        for attempt in 1..=max_retries {
            match self.crawl_page(url).await {
                Ok(page) => return Ok(page),
                Err(e) => {
                    last_error = Some(e);
                    
                    // 재시도 간격 (exponential backoff)
                    let delay = std::time::Duration::from_millis(1000 * 2_u64.pow(attempt - 1));
                    tokio::time::sleep(delay).await;
                    
                    tracing::warn!("Retry {}/{} for URL {}: {:?}", attempt, max_retries, url, last_error);
                }
            }
        }
        
        Err(CrawlingError::MaxRetriesError(url.to_string()))
    }
    
    async fn handle_crawling_error(&self, error: &CrawlingError, url: &str, session_id: &str) -> Result<()> {
        match error {
            CrawlingError::HttpError(e) if e.is_timeout() => {
                // 타임아웃 에러: URL을 재시도 큐에 추가
                self.session_manager.add_retry_url(session_id, url).await?;
            },
            CrawlingError::RateLimitError(_) => {
                // Rate limit: 지연 후 재시도
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                self.session_manager.add_retry_url(session_id, url).await?;
            },
            CrawlingError::DatabaseError(_) => {
                // DB 에러: 데이터 손실 방지를 위해 임시 파일에 저장
                self.save_to_temp_file(url, &error.to_string()).await?;
            },
            _ => {
                // 일반 에러: 로그만 기록
                self.session_manager.add_error(session_id, error.to_string()).await?;
            }
        }
        
        Ok(())
    }
}
```

### 세션 복구 메커니즘
```rust
impl WebCrawler {
    pub async fn resume_session(&self, session_id: &str) -> Result<()> {
        // 1. 세션 상태 복구
        let session = self.session_manager.get_session(session_id).await?;
        if session.status != SessionStatus::Paused && session.status != SessionStatus::Failed {
            anyhow::bail!("Session cannot be resumed from status: {:?}", session.status);
        }
        
        // 2. 미처리 URL 목록 복구
        let remaining_urls = self.session_manager.get_remaining_urls(session_id).await?;
        let retry_urls = self.session_manager.get_retry_urls(session_id).await?;
        
        // 3. 방문한 URL 목록 복구
        let visited_urls = self.session_manager.get_visited_urls(session_id).await?;
        {
            let mut visited = self.visited_urls.lock().await;
            visited.extend(visited_urls);
        }
        
        // 4. 세션 재시작
        self.session_manager.set_status(session_id, SessionStatus::Active).await?;
        
        // 5. 크롤링 재개
        let mut all_urls = remaining_urls;
        all_urls.extend(retry_urls);
        
        self.crawl_url_list(all_urls, session_id).await?;
        
        Ok(())
    }
}
```

## 성능 최적화

### 메모리 관리 최적화
```rust
impl WebCrawler {
    fn optimize_memory_usage(&self, html: &str) -> String {
        // 1. 불필요한 HTML 태그 제거 (스크립트, 스타일 등)
        let cleaned = self.remove_unnecessary_tags(html);
        
        // 2. 압축
        if cleaned.len() > 10_000 {
            self.compress_html(&cleaned)
        } else {
            cleaned
        }
    }
    
    async fn manage_url_queue(&self, urls: &mut Vec<String>, session_id: &str) -> Result<()> {
        // URL 큐 크기 제한 (메모리 사용량 제어)
        const MAX_QUEUE_SIZE: usize = 10_000;
        
        if urls.len() > MAX_QUEUE_SIZE {
            // 우선순위가 낮은 URL들을 데이터베이스에 저장
            let excess_urls = urls.split_off(MAX_QUEUE_SIZE);
            self.session_manager.save_pending_urls(session_id, &excess_urls).await?;
            
            tracing::info!("Saved {} URLs to database to manage memory", excess_urls.len());
        }
        
        Ok(())
    }
}
```

### 데이터베이스 성능 최적화
```rust
impl WebCrawler {
    async fn batch_save_products(&self, products: Vec<ExtractedMatterProduct>) -> Result<()> {
        const BATCH_SIZE: usize = 100;
        
        for chunk in products.chunks(BATCH_SIZE) {
            let mut transaction = self.product_repo.begin_transaction().await?;
            
            for product in chunk {
                self.product_repo.save_matter_product_in_transaction(&mut transaction, product).await?;
            }
            
            transaction.commit().await?;
        }
        
        Ok(())
    }
    
    async fn optimize_database_performance(&self) -> Result<()> {
        // 1. 인덱스 최적화
        self.product_repo.ensure_indexes().await?;
        
        // 2. 통계 업데이트
        self.product_repo.update_statistics().await?;
        
        // 3. 테이블 분석
        self.product_repo.analyze_tables().await?;
        
        Ok(())
    }
}
```

## 모니터링 및 로깅

### 실시간 진행 상황 추적
```rust
#[derive(Debug, Clone, Serialize)]
pub struct CrawlingProgress {
    pub session_id: String,
    pub pages_crawled: u32,
    pub total_estimated: u32,
    pub products_found: u32,
    pub errors_count: u32,
    pub current_url: String,
    pub stage: CrawlingStage,
    pub elapsed_time: std::time::Duration,
    pub estimated_remaining: std::time::Duration,
    pub crawl_rate: f64, // pages per minute
}

impl WebCrawler {
    async fn update_progress_metrics(&self, session_id: &str) -> Result<()> {
        let session = self.session_manager.get_session(session_id).await?;
        let start_time = session.created_at;
        let elapsed = chrono::Utc::now().signed_duration_since(start_time);
        
        let progress = CrawlingProgress {
            session_id: session_id.to_string(),
            pages_crawled: session.current_page,
            total_estimated: self.estimate_total_pages(session_id).await?,
            products_found: self.count_session_products(session_id).await?,
            errors_count: session.error_count,
            current_url: session.current_url.clone(),
            stage: session.stage,
            elapsed_time: elapsed.to_std().unwrap_or_default(),
            estimated_remaining: self.estimate_remaining_time(session_id).await?,
            crawl_rate: self.calculate_crawl_rate(session_id).await?,
        };
        
        // 프론트엔드로 진행 상황 브로드캐스트
        self.session_manager.broadcast_progress(progress).await?;
        
        Ok(())
    }
}
```

이 세부 워크플로우 문서는 크롤링 엔진의 모든 주요 기능과 최적화 전략을 포함하고 있으며, 현재 구현된 코드를 기반으로 향후 개선 방향을 제시합니다.
