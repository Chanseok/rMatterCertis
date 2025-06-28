# Matter Certis 도메인 지식 - Rust 개발자를 위한 핵심 가이드

> 이 문서는 기존 TypeScript/Electron 구현체에서 추출한 핵심 도메인 지식을 Rust 개발자가 Tauri로 재구현할 수 있도록 정리한 가이드입니다.

## 📋 목차

1. [비즈니스 도메인 개요](#비즈니스-도메인-개요)
2. [핵심 엔터티 모델](#핵심-엔터티-모델)
3. [크롤링 워크플로우](#크롤링-워크플로우)
4. [아키텍처 패턴](#아키텍처-패턴)
5. [배치 처리 시스템](#배치-처리-시스템)
6. [설정 관리](#설정-관리)
7. [이벤트 시스템](#이벤트-시스템)
8. [데이터베이스 설계](#데이터베이스-설계)
9. [진행 상황 추적](#진행-상황-추적)
10. [에러 처리 패턴](#에러-처리-패턴)
11. [Rust 구현 고려사항](#rust-구현-고려사항)

---

## 비즈니스 도메인 개요

### 핵심 목적
**CSA-IoT Matter Certification 데이터베이스에서 인증된 제품 정보를 체계적으로 수집하고 관리하는 시스템**

### 주요 기능
1. **제품 목록 수집**: 사이트에서 페이지별 제품 목록 추출
2. **제품 상세 정보 수집**: 개별 제품 페이지에서 상세 정보 추출
3. **중복 검증**: 로컬 DB와 비교하여 신규/기존 제품 구분
4. **배치 처리**: 대용량 데이터 처리를 위한 배치 단위 작업
5. **진행 추적**: 실시간 진행 상황 모니터링 및 UI 업데이트

### 데이터 흐름
```
사이트 페이지 → 제품 목록 추출 → 중복 검증 → 상세 정보 수집 → 로컬 DB 저장
```

---

## 핵심 엔터티 모델

### Product (기본 제품 정보)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub url: String,                    // 제품 상세 페이지 URL (Primary Key)
    pub manufacturer: Option<String>,   // 제조사명
    pub model: Option<String>,          // 모델명
    pub certificate_id: Option<String>, // 인증서 ID
    pub page_id: Option<u32>,          // 수집된 페이지 번호
    pub index_in_page: Option<u32>,    // 페이지 내 순서
}
```

### MatterProduct (완전한 제품 정보)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatterProduct {
    // Product 기본 필드
    pub url: String,
    pub page_id: Option<u32>,
    pub index_in_page: Option<u32>,
    pub id: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    
    // 상세 정보 필드
    pub device_type: Option<String>,
    pub certificate_id: Option<String>,
    pub certification_date: Option<String>,
    pub software_version: Option<String>,
    pub hardware_version: Option<String>,
    pub vid: Option<String>,              // Vendor ID
    pub pid: Option<String>,              // Product ID
    pub family_sku: Option<String>,
    pub family_variant_sku: Option<String>,
    pub firmware_version: Option<String>,
    pub family_id: Option<String>,
    pub tis_trp_tested: Option<String>,
    pub specification_version: Option<String>,
    pub transport_interface: Option<String>,
    pub primary_device_type_id: Option<String>,
    pub application_categories: Vec<String>, // JSON 배열을 Vec로
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}
```

### CrawlerConfig (크롤러 설정)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlerConfig {
    // 핵심 설정
    pub page_range_limit: u32,           // 크롤링할 최대 페이지 수
    pub product_list_retry_count: u32,   // 목록 수집 재시도 횟수
    pub product_detail_retry_count: u32, // 상세 정보 재시도 횟수
    pub products_per_page: u32,          // 페이지당 제품 수 (기본 12)
    pub auto_add_to_local_db: bool,      // 자동 DB 저장 여부
    pub auto_status_check: bool,         // 자동 상태 체크 여부
    
    // 브라우저 설정
    pub headless_browser: Option<bool>,
    pub crawler_type: Option<CrawlerType>, // "reqwest" 또는 "playwright"
    pub user_agent: Option<String>,
    
    // 성능 설정
    pub max_concurrent_tasks: Option<u32>,
    pub request_delay: Option<u64>,       // 요청 간 지연 (ms)
    pub request_timeout: Option<u64>,     // 요청 타임아웃 (ms)
    
    // 배치 처리 설정
    pub enable_batch_processing: Option<bool>,
    pub batch_size: Option<u32>,          // 배치당 페이지 수 (기본 30)
    pub batch_delay_ms: Option<u64>,      // 배치 간 지연 (기본 2000ms)
    pub batch_retry_limit: Option<u32>,   // 배치 재시도 제한
    
    // URL 설정
    pub base_url: Option<String>,
    pub matter_filter_url: Option<String>, // Matter 필터 적용 URL
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrawlerType {
    Reqwest,
    Playwright,
}
```

### Vendor (벤더 정보)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vendor {
    pub vendor_id: String,
    pub vendor_number: u32,
    pub vendor_name: String,
    pub company_legal_name: String,
}
```

---

## 크롤링 워크플로우

### 3단계 크롤링 프로세스

#### 1단계: 제품 목록 수집 (Product List Collection)
```rust
pub async fn collect_product_list(&self, page_limit: u32) -> Result<Vec<Product>, CrawlingError> {
    // 1. 사이트 총 페이지 수 확인
    let total_pages = self.get_site_total_pages().await?;
    let pages_to_crawl = std::cmp::min(page_limit, total_pages);
    
    // 2. 배치 처리 여부 결정
    if self.config.enable_batch_processing && pages_to_crawl > self.config.batch_size {
        self.collect_in_batches(pages_to_crawl).await
    } else {
        self.collect_sequential(pages_to_crawl).await
    }
}
```

**핵심 로직:**
- 각 페이지에서 제품 URL, 제조사, 모델명, 인증서 ID 추출
- 페이지당 12개 제품이 기본 (설정 가능)
- 실패한 페이지는 재시도 큐에 추가
- 진행 상황을 실시간으로 UI에 전송

#### 1.5단계: 중복 검증 (Validation)
```rust
pub async fn validate_products(&self, products: Vec<Product>) -> ValidationResult {
    // 1. 로컬 DB에서 기존 제품 URL 조회
    let existing_urls = self.db.get_existing_product_urls().await?;
    
    // 2. 제품 분류
    let mut new_products = Vec::new();
    let mut existing_products = Vec::new();
    let mut duplicate_products = Vec::new();
    
    let mut seen_urls = HashSet::new();
    
    for product in products {
        // 1단계 수집 과정에서의 중복 감지
        if seen_urls.contains(&product.url) {
            duplicate_products.push(product);
            continue;
        }
        seen_urls.insert(product.url.clone());
        
        // 로컬 DB와의 중복 확인
        if existing_urls.contains(&product.url) {
            existing_products.push(product);
        } else {
            new_products.push(product);
        }
    }
    
    ValidationResult {
        new_products,
        existing_products,
        duplicate_products,
        summary: ValidationSummary {
            total_products: products.len(),
            new_products: new_products.len(),
            // ... 기타 통계
        }
    }
}
```

#### 2단계: 제품 상세 정보 수집 (Product Detail Collection)
```rust
pub async fn collect_product_details(&self, products: Vec<Product>) -> Result<Vec<MatterProduct>, CrawlingError> {
    // 1. 동시성 풀 생성
    let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent_tasks));
    let mut tasks = Vec::new();
    
    // 2. 각 제품에 대해 상세 정보 수집 태스크 생성
    for product in products {
        let permit = semaphore.clone().acquire_owned().await?;
        let task = tokio::spawn(async move {
            let _permit = permit; // 태스크 완료시 자동 해제
            self.crawl_single_product_detail(product).await
        });
        tasks.push(task);
    }
    
    // 3. 모든 태스크 완료 대기
    let results = futures::future::join_all(tasks).await;
    
    // 4. 성공한 결과만 수집
    let mut matter_products = Vec::new();
    for result in results {
        match result? {
            Ok(matter_product) => matter_products.push(matter_product),
            Err(e) => self.log_error(e), // 실패한 제품은 로깅만
        }
    }
    
    Ok(matter_products)
}
```

**상세 정보 추출 로직:**
- 제품 페이지 HTML 파싱
- Matter 인증 관련 필드 추출 (VID, PID, 애플리케이션 카테고리 등)
- 제조사 정보 보강 (벤더 DB와 매칭)
- 실패 시 재시도 메커니즘

---

## 아키텍처 패턴

### Clean Architecture 기반 레이어 구조

```
src/
├── domain/
│   ├── entities/
│   │   ├── product.rs
│   │   ├── vendor.rs
│   │   └── crawling_session.rs
│   ├── repositories/
│   │   ├── product_repository.rs
│   │   └── vendor_repository.rs
│   └── services/
│       ├── crawling_service.rs
│       └── validation_service.rs
├── application/
│   ├── use_cases/
│   │   ├── start_crawling.rs
│   │   ├── collect_product_list.rs
│   │   └── validate_products.rs
│   └── dto/
│       ├── crawling_request.rs
│       └── crawling_response.rs
├── infrastructure/
│   ├── database/
│   │   ├── sqlite_repository.rs
│   │   └── migrations/
│   ├── http/
│   │   ├── reqwest_client.rs
│   │   └── html_parser.rs
│   └── config/
│       └── config_manager.rs
└── commands/
    ├── crawling_commands.rs
    └── config_commands.rs
```

### 핵심 서비스 패턴

#### CrawlingService (도메인 서비스)
```rust
pub struct CrawlingService {
    product_repository: Arc<dyn ProductRepository>,
    vendor_repository: Arc<dyn VendorRepository>,
    http_client: Arc<dyn HttpClient>,
    config: CrawlerConfig,
}

impl CrawlingService {
    pub async fn start_crawling(&self, config: CrawlerConfig) -> Result<CrawlingSession, ServiceError> {
        // 1. 크롤링 세션 초기화
        let session = CrawlingSession::new(config.clone());
        
        // 2. 1단계: 제품 목록 수집
        let products = self.collect_product_list(config.page_range_limit).await?;
        session.emit_stage_complete(CrawlingStage::ProductList, products.len());
        
        // 3. 1.5단계: 중복 검증
        let validation_result = self.validate_products(products).await?;
        session.emit_stage_complete(CrawlingStage::Validation, validation_result.new_products.len());
        
        // 4. 2단계: 상세 정보 수집
        let matter_products = self.collect_product_details(validation_result.new_products).await?;
        session.emit_stage_complete(CrawlingStage::ProductDetail, matter_products.len());
        
        // 5. DB 저장 (설정에 따라)
        if config.auto_add_to_local_db {
            self.save_products_to_db(matter_products).await?;
        }
        
        Ok(session)
    }
}
```

#### ProgressTracker (진행 상황 추적)
```rust
#[derive(Debug, Clone)]
pub struct CrawlingProgress {
    pub current: u32,
    pub total: u32,
    pub percentage: f32,
    pub current_step: String,
    pub current_stage: CrawlingStage,
    pub elapsed_time: Duration,
    pub remaining_time: Option<Duration>,
    pub status: CrawlingStatus,
    pub message: Option<String>,
    // 배치 처리 정보
    pub current_batch: Option<u32>,
    pub total_batches: Option<u32>,
    // 오류 정보
    pub retry_count: u32,
    pub failed_items: u32,
}

pub struct ProgressTracker {
    progress: Arc<RwLock<CrawlingProgress>>,
    event_sender: EventSender,
}

impl ProgressTracker {
    pub async fn update_progress(&self, update: ProgressUpdate) {
        let mut progress = self.progress.write().await;
        
        // 진행률 계산
        progress.current = update.current;
        progress.total = update.total;
        progress.percentage = if progress.total > 0 {
            (progress.current as f32 / progress.total as f32) * 100.0
        } else {
            0.0
        };
        
        // 남은 시간 예측
        if progress.current > 0 && progress.elapsed_time.as_secs() > 0 {
            let avg_time_per_item = progress.elapsed_time.as_secs_f32() / progress.current as f32;
            let remaining_items = progress.total - progress.current;
            progress.remaining_time = Some(Duration::from_secs_f32(avg_time_per_item * remaining_items as f32));
        }
        
        // 이벤트 발송
        self.event_sender.send(Event::CrawlingProgress(progress.clone())).await;
    }
}
```

---

## 배치 처리 시스템

### 배치 처리 핵심 로직

```rust
pub struct BatchProcessor {
    config: CrawlerConfig,
    progress_tracker: Arc<ProgressTracker>,
}

impl BatchProcessor {
    pub async fn process_in_batches(&self, total_pages: u32) -> Result<Vec<Product>, BatchError> {
        let batch_size = self.config.batch_size.unwrap_or(30);
        let total_batches = (total_pages + batch_size - 1) / batch_size;
        
        let mut all_products = Vec::new();
        
        for batch_num in 1..=total_batches {
            // 배치 범위 계산
            let start_page = (batch_num - 1) * batch_size + 1;
            let end_page = std::cmp::min(batch_num * batch_size, total_pages);
            
            // 배치 처리 시작 이벤트
            self.progress_tracker.update_batch_start(batch_num, total_batches).await;
            
            // 배치 수집 실행 (재시도 포함)
            let batch_products = self.collect_batch_with_retry(start_page, end_page, batch_num).await?;
            
            all_products.extend(batch_products);
            
            // 배치 완료 이벤트
            self.progress_tracker.update_batch_complete(batch_num, all_products.len()).await;
            
            // 배치 간 지연
            if batch_num < total_batches {
                let delay = Duration::from_millis(self.config.batch_delay_ms.unwrap_or(2000));
                tokio::time::sleep(delay).await;
            }
        }
        
        Ok(all_products)
    }
    
    async fn collect_batch_with_retry(&self, start_page: u32, end_page: u32, batch_num: u32) -> Result<Vec<Product>, BatchError> {
        let retry_limit = self.config.batch_retry_limit.unwrap_or(3);
        
        for attempt in 1..=retry_limit {
            match self.collect_single_batch(start_page, end_page).await {
                Ok(products) => return Ok(products),
                Err(e) if attempt < retry_limit => {
                    // 재시도 전 지연
                    let delay = Duration::from_millis(2000 * attempt as u64);
                    tokio::time::sleep(delay).await;
                    
                    self.progress_tracker.update_batch_retry(batch_num, attempt).await;
                },
                Err(e) => return Err(e),
            }
        }
        
        unreachable!()
    }
}
```

### 배치 처리 장점
1. **메모리 효율성**: 대용량 데이터를 작은 단위로 처리
2. **오류 격리**: 한 배치 실패가 전체에 영향을 주지 않음
3. **진행 추적**: 배치별 진행 상황을 세밀하게 추적
4. **자원 관리**: 시스템 자원 사용량 제어

---

## 설정 관리

### 설정 관리자 구현

```rust
pub struct ConfigManager {
    config_path: PathBuf,
    config: Arc<RwLock<CrawlerConfig>>,
}

impl ConfigManager {
    pub fn new(app_handle: &tauri::AppHandle) -> Result<Self, ConfigError> {
        let config_dir = app_handle
            .path_resolver()
            .app_data_dir()
            .ok_or(ConfigError::InvalidPath)?;
        
        let config_path = config_dir.join("crawler-config.json");
        
        // 디렉토리 생성
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        // 설정 로드
        let config = Self::load_config(&config_path)?;
        
        Ok(Self {
            config_path,
            config: Arc::new(RwLock::new(config)),
        })
    }
    
    fn load_config(path: &Path) -> Result<CrawlerConfig, ConfigError> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let mut config: CrawlerConfig = serde_json::from_str(&content)?;
            
            // 기본값으로 누락된 필드 채우기
            Self::apply_defaults(&mut config);
            
            Ok(config)
        } else {
            Ok(Self::default_config())
        }
    }
    
    pub async fn get_config(&self) -> CrawlerConfig {
        self.config.read().await.clone()
    }
    
    pub async fn update_config(&self, updates: PartialCrawlerConfig) -> Result<CrawlerConfig, ConfigError> {
        let mut config = self.config.write().await;
        
        // 부분 업데이트 적용
        Self::merge_config(&mut config, updates);
        
        // 유효성 검증
        Self::validate_config(&config)?;
        
        // 파일 저장
        self.save_config(&config).await?;
        
        Ok(config.clone())
    }
    
    fn validate_config(config: &CrawlerConfig) -> Result<(), ConfigError> {
        if config.page_range_limit == 0 {
            return Err(ConfigError::InvalidValue("page_range_limit must be greater than 0".into()));
        }
        
        if config.products_per_page == 0 {
            return Err(ConfigError::InvalidValue("products_per_page must be greater than 0".into()));
        }
        
        // 배치 설정 검증
        if let Some(batch_size) = config.batch_size {
            if batch_size == 0 {
                return Err(ConfigError::InvalidValue("batch_size must be greater than 0".into()));
            }
        }
        
        Ok(())
    }
    
    fn default_config() -> CrawlerConfig {
        CrawlerConfig {
            page_range_limit: 10,
            product_list_retry_count: 9,
            product_detail_retry_count: 9,
            products_per_page: 12,
            auto_add_to_local_db: false,
            auto_status_check: true,
            enable_batch_processing: Some(true),
            batch_size: Some(30),
            batch_delay_ms: Some(2000),
            crawler_type: Some(CrawlerType::Reqwest),
            // ... 기타 기본값
        }
    }
}
```

---

## 이벤트 시스템

### 이벤트 기반 아키텍처

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum CrawlingEvent {
    ProgressUpdate {
        progress: CrawlingProgress,
    },
    StageComplete {
        stage: CrawlingStage,
        items_processed: u32,
        duration: Duration,
    },
    BatchStart {
        batch_number: u32,
        total_batches: u32,
        pages_in_batch: u32,
    },
    BatchComplete {
        batch_number: u32,
        products_collected: u32,
        failed_pages: u32,
    },
    CrawlingComplete {
        success: bool,
        total_products: u32,
        duration: Duration,
        auto_saved_to_db: bool,
    },
    CrawlingError {
        error: String,
        stage: CrawlingStage,
        recoverable: bool,
    },
    StatusSummary {
        summary: CrawlingStatusSummary,
    },
}

pub struct EventBus {
    sender: broadcast::Sender<CrawlingEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1000);
        Self { sender }
    }
    
    pub fn emit(&self, event: CrawlingEvent) {
        if let Err(e) = self.sender.send(event) {
            eprintln!("Failed to emit event: {:?}", e);
        }
    }
    
    pub fn subscribe(&self) -> broadcast::Receiver<CrawlingEvent> {
        self.sender.subscribe()
    }
}

// Tauri 이벤트 연동
#[tauri::command]
pub async fn subscribe_to_crawling_events(app_handle: tauri::AppHandle, event_bus: tauri::State<'_, EventBus>) -> Result<(), String> {
    let mut receiver = event_bus.subscribe();
    
    tokio::spawn(async move {
        while let Ok(event) = receiver.recv().await {
            let event_name = match &event {
                CrawlingEvent::ProgressUpdate { .. } => "crawling-progress",
                CrawlingEvent::StageComplete { .. } => "stage-complete",
                CrawlingEvent::CrawlingComplete { .. } => "crawling-complete",
                CrawlingEvent::CrawlingError { .. } => "crawling-error",
                _ => continue,
            };
            
            if let Err(e) = app_handle.emit_all(event_name, &event) {
                eprintln!("Failed to emit event to frontend: {:?}", e);
            }
        }
    });
    
    Ok(())
}
```

---

## 데이터베이스 설계

### SQLite 테이블 구조

**📋 최신 업데이트: 2025-01-15 - 메모리 기반 세션 관리로 아키텍처 최적화**

```sql
-- Vendor table for CSA-IoT Matter certification database vendors
CREATE TABLE IF NOT EXISTS vendors (
    vendor_id TEXT PRIMARY KEY,              -- UUID-based ID for better distribution
    vendor_number INTEGER NOT NULL UNIQUE,   -- Matter vendor number
    vendor_name TEXT NOT NULL,
    company_legal_name TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Product table for basic product information (Stage 1 collection)
CREATE TABLE IF NOT EXISTS products (
    url TEXT PRIMARY KEY,              -- Product detail page URL
    manufacturer TEXT,                 -- Manufacturer name
    model TEXT,                       -- Model name
    certificate_id TEXT,              -- Certificate ID
    page_id INTEGER,                  -- Collected page number
    index_in_page INTEGER,           -- Order within page
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Matter products table for complete Matter certification info (Stage 2 collection)
CREATE TABLE IF NOT EXISTS matter_products (
    url TEXT PRIMARY KEY,                    -- Product detail page URL (FK to products)
    page_id INTEGER,                        -- Collected page number
    index_in_page INTEGER,                 -- Order within page
    id TEXT,                               -- Matter product ID
    manufacturer TEXT,                     -- Manufacturer name
    model TEXT,                           -- Model name
    device_type TEXT,                     -- Device type
    certificate_id TEXT,                  -- Certificate ID
    certification_date TEXT,             -- Certification date
    software_version TEXT,               -- Software version
    hardware_version TEXT,               -- Hardware version
    vid TEXT,                            -- Vendor ID (hex string)
    pid TEXT,                            -- Product ID (hex string)
    family_sku TEXT,                     -- Family SKU
    family_variant_sku TEXT,             -- Family variant SKU
    firmware_version TEXT,               -- Firmware version
    family_id TEXT,                      -- Family ID
    tis_trp_tested TEXT,                 -- TIS/TRP tested
    specification_version TEXT,          -- Specification version
    transport_interface TEXT,            -- Transport interface
    primary_device_type_id TEXT,         -- Primary device type ID
    application_categories TEXT,         -- JSON array as TEXT
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Crawling results table for final session outcomes only
-- NOTE: Session state is now managed in-memory for better performance
CREATE TABLE IF NOT EXISTS crawling_results (
    session_id TEXT PRIMARY KEY,
    status TEXT NOT NULL,                    -- Completed, Failed, Stopped
    stage TEXT NOT NULL,                     -- products, matter_products, details
    total_pages INTEGER NOT NULL,
    products_found INTEGER NOT NULL,
    errors_count INTEGER NOT NULL,
    started_at DATETIME NOT NULL,
    completed_at DATETIME NOT NULL,
    execution_time_seconds INTEGER NOT NULL,
    config_snapshot TEXT,                    -- JSON configuration used
    error_details TEXT                       -- Detailed error information if failed
);

-- Performance-optimized indexes
CREATE INDEX IF NOT EXISTS idx_products_manufacturer ON products (manufacturer);
CREATE INDEX IF NOT EXISTS idx_products_page_id ON products (page_id);
CREATE INDEX IF NOT EXISTS idx_matter_products_manufacturer ON matter_products (manufacturer);
CREATE INDEX IF NOT EXISTS idx_matter_products_device_type ON matter_products (device_type);
CREATE INDEX IF NOT EXISTS idx_matter_products_vid ON matter_products (vid);
CREATE INDEX IF NOT EXISTS idx_matter_products_certification_date ON matter_products (certification_date);
CREATE INDEX IF NOT EXISTS idx_crawling_results_status ON crawling_results (status);
CREATE INDEX IF NOT EXISTS idx_crawling_results_started_at ON crawling_results (started_at);
CREATE INDEX IF NOT EXISTS idx_vendors_vendor_number ON vendors (vendor_number);
```

### 아키텍처 최적화: 메모리 기반 세션 관리

**2025년 1월 업데이트: 산업 표준 접근법 도입**

크롤링 세션 관리를 기존의 데이터베이스 중심에서 메모리 기반으로 전환했습니다:

- **AS-IS**: `crawling_sessions` 테이블에 모든 세션 상태 저장
- **TO-BE**: 메모리에서 세션 상태 관리, 최종 결과만 `crawling_results` 테이블에 저장

이 변경으로 다음과 같은 이점을 얻었습니다:
- 🚀 **성능 향상**: 실시간 상태 업데이트시 DB I/O 제거
- 🔄 **확장성 개선**: 다중 세션 동시 처리 최적화
- 🧹 **단순화**: 세션 정리 로직 불필요
- 📊 **안정성**: 메모리 기반 상태로 락(lock) 경합 제거
```

### Repository 패턴 구현

```rust
#[async_trait]
pub trait ProductRepository: Send + Sync {
    async fn save_product(&self, product: &Product) -> Result<(), RepositoryError>;
    async fn save_matter_product(&self, product: &MatterProduct) -> Result<(), RepositoryError>;
    async fn get_existing_urls(&self) -> Result<HashSet<String>, RepositoryError>;
    async fn get_products_paginated(&self, page: u32, limit: u32) -> Result<(Vec<MatterProduct>, u32), RepositoryError>;
    async fn search_products(&self, query: &str) -> Result<Vec<MatterProduct>, RepositoryError>;
    async fn get_database_summary(&self) -> Result<DatabaseSummary, RepositoryError>;
}

pub struct SqliteProductRepository {
    pool: Pool<Sqlite>,
}

impl SqliteProductRepository {
    pub async fn new(database_url: &str) -> Result<Self, RepositoryError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;
        
        // 마이그레이션 실행
        sqlx::migrate!("./migrations").run(&pool).await?;
        
        Ok(Self { pool })
    }
}

#[async_trait]
impl ProductRepository for SqliteProductRepository {
    async fn save_matter_product(&self, product: &MatterProduct) -> Result<(), RepositoryError> {
        let application_categories_json = serde_json::to_string(&product.application_categories)?;
        
        sqlx::query!(
            r#"
            INSERT OR REPLACE INTO product_details (
                url, page_id, index_in_page, manufacturer, model, device_type,
                certification_id, certification_date, software_version, hardware_version,
                vid, pid, family_sku, family_variant_sku, firmware_version,
                family_id, tis_trp_tested, specification_version, transport_interface,
                primary_device_type_id, application_categories, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            "#,
            product.url,
            product.page_id,
            product.index_in_page,
            product.manufacturer,
            product.model,
            product.device_type,
            product.certificate_id,
            product.certification_date,
            product.software_version,
            product.hardware_version,
            product.vid,
            product.pid,
            product.family_sku,
            product.family_variant_sku,
            product.firmware_version,
            product.family_id,
            product.tis_trp_tested,
            product.specification_version,
            product.transport_interface,
            product.primary_device_type_id,
            application_categories_json
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn get_existing_urls(&self) -> Result<HashSet<String>, RepositoryError> {
        let rows = sqlx::query!("SELECT url FROM products")
            .fetch_all(&self.pool)
            .await?;
        
        Ok(rows.into_iter().map(|row| row.url).collect())
    }
}
```

---

## 진행 상황 추적

### 멀티 스테이지 진행 추적

```rust
#[derive(Debug, Clone, Serialize)]
pub enum CrawlingStage {
    Idle,
    ProductList,      // 1단계: 제품 목록 수집
    Validation,       // 1.5단계: 중복 검증
    ProductDetail,    // 2단계: 상세 정보 수집
    DatabaseSave,     // 3단계: DB 저장
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub enum CrawlingStatus {
    Idle,
    Initializing,
    Running,
    Paused,
    Completed,
    Error,
    Stopped,
}

pub struct StageProgress {
    pub stage: CrawlingStage,
    pub status: CrawlingStatus,
    pub current: u32,
    pub total: u32,
    pub percentage: f32,
    pub start_time: Option<Instant>,
    pub estimated_end_time: Option<Instant>,
    pub retry_count: u32,
    pub failed_items: u32,
}

pub struct CrawlingSession {
    pub session_id: String,
    pub overall_status: CrawlingStatus,
    pub current_stage: CrawlingStage,
    pub stages: HashMap<CrawlingStage, StageProgress>,
    pub start_time: Instant,
    pub total_elapsed: Duration,
    pub config: CrawlerConfig,
}

impl CrawlingSession {
    pub fn new(config: CrawlerConfig) -> Self {
        let session_id = uuid::Uuid::new_v4().to_string();
        let start_time = Instant::now();
        
        // 모든 스테이지 초기화
        let mut stages = HashMap::new();
        for stage in [CrawlingStage::ProductList, CrawlingStage::Validation, CrawlingStage::ProductDetail] {
            stages.insert(stage.clone(), StageProgress {
                stage: stage.clone(),
                status: CrawlingStatus::Idle,
                current: 0,
                total: 0,
                percentage: 0.0,
                start_time: None,
                estimated_end_time: None,
                retry_count: 0,
                failed_items: 0,
            });
        }
        
        Self {
            session_id,
            overall_status: CrawlingStatus::Initializing,
            current_stage: CrawlingStage::ProductList,
            stages,
            start_time,
            total_elapsed: Duration::ZERO,
            config,
        }
    }
    
    pub fn start_stage(&mut self, stage: CrawlingStage, total_items: u32) {
        self.current_stage = stage.clone();
        
        if let Some(stage_progress) = self.stages.get_mut(&stage) {
            stage_progress.status = CrawlingStatus::Running;
            stage_progress.total = total_items;
            stage_progress.current = 0;
            stage_progress.start_time = Some(Instant::now());
        }
    }
    
    pub fn update_stage_progress(&mut self, stage: CrawlingStage, current: u32) {
        if let Some(stage_progress) = self.stages.get_mut(&stage) {
            stage_progress.current = current;
            stage_progress.percentage = if stage_progress.total > 0 {
                (current as f32 / stage_progress.total as f32) * 100.0
            } else {
                0.0
            };
            
            // 완료 시간 예측
            if let Some(start_time) = stage_progress.start_time {
                let elapsed = start_time.elapsed();
                if current > 0 {
                    let avg_time_per_item = elapsed.as_secs_f32() / current as f32;
                    let remaining_items = stage_progress.total - current;
                    let estimated_remaining = Duration::from_secs_f32(avg_time_per_item * remaining_items as f32);
                    stage_progress.estimated_end_time = Some(Instant::now() + estimated_remaining);
                }
            }
        }
    }
}
```

---

## 에러 처리 패턴

### 계층화된 에러 처리

```rust
#[derive(Debug, thiserror::Error)]
pub enum CrawlingError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    
    #[error("HTML parsing failed: {0}")]
    HtmlParsing(String),
    
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    
    #[error("Network timeout")]
    Timeout,
    
    #[error("Crawling was aborted")]
    Aborted,
    
    #[error("Too many retries: {max_retries}")]
    TooManyRetries { max_retries: u32 },
    
    #[error("Batch processing failed: batch {batch_number}, error: {source}")]
    BatchFailed { batch_number: u32, source: Box<CrawlingError> },
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid configuration value: {0}")]
    InvalidValue(String),
    
    #[error("Configuration file not found: {0}")]
    FileNotFound(String),
    
    #[error("JSON parsing error: {0}")]
    JsonParsing(#[from] serde_json::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// 에러 복구 전략
pub struct ErrorRecoveryStrategy {
    pub max_retries: u32,
    pub retry_delay: Duration,
    pub backoff_multiplier: f32,
    pub recoverable_errors: HashSet<String>,
}

impl ErrorRecoveryStrategy {
    pub fn should_retry(&self, error: &CrawlingError, current_attempt: u32) -> bool {
        if current_attempt >= self.max_retries {
            return false;
        }
        
        match error {
            CrawlingError::Http(_) => true,
            CrawlingError::Timeout => true,
            CrawlingError::HtmlParsing(_) => false, // 파싱 에러는 재시도 불가
            CrawlingError::Database(_) => false,   // DB 에러는 재시도 불가
            _ => false,
        }
    }
    
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        let multiplier = self.backoff_multiplier.powi(attempt as i32);
        Duration::from_millis((self.retry_delay.as_millis() as f32 * multiplier) as u64)
    }
}

// 재시도 매크로
macro_rules! retry_with_strategy {
    ($strategy:expr, $operation:expr) => {{
        let mut attempt = 0;
        loop {
            match $operation {
                Ok(result) => break Ok(result),
                Err(error) => {
                    attempt += 1;
                    if !$strategy.should_retry(&error, attempt) {
                        break Err(error);
                    }
                    
                    let delay = $strategy.calculate_delay(attempt);
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }};
}
```

---

## Rust 구현 고려사항

### 1. 의존성 관리 (Cargo.toml)

```toml
[dependencies]
# Tauri 관련
tauri = { version = "2.0", features = ["api-all"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# 비동기 런타임
tokio = { version = "1.0", features = ["full"] }

# HTTP 클라이언트
reqwest = { version = "0.11", features = ["json", "cookies", "gzip"] }

# HTML 파싱
scraper = "0.18"
select = "0.6"

# 데이터베이스
sqlx = { version = "0.7", features = ["sqlite", "runtime-tokio-rustls", "chrono", "migrate"] }

# 동시성
futures = "0.3"
rayon = "1.7"

# 에러 처리
anyhow = "1.0"
thiserror = "1.0"

# 설정 관리
config = "0.13"

# 로깅
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# 시간 처리
chrono = { version = "0.4", features = ["serde"] }

# UUID
uuid = { version = "1.0", features = ["v4", "serde"] }

# 비동기 트레이트
async-trait = "0.1"
```

### 2. 성능 최적화 포인트

#### HTTP 클라이언트 최적화
```rust
pub struct OptimizedHttpClient {
    client: reqwest::Client,
    rate_limiter: Arc<RateLimiter>,
}

impl OptimizedHttpClient {
    pub fn new(config: &CrawlerConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(config.request_timeout.unwrap_or(30000)))
            .user_agent(config.user_agent.as_deref().unwrap_or("Matter-Certis-Crawler/2.0"))
            .gzip(true)
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        
        let rate_limiter = Arc::new(RateLimiter::new(
            config.request_delay.unwrap_or(1000)
        ));
        
        Self { client, rate_limiter }
    }
    
    pub async fn get_with_rate_limit(&self, url: &str) -> Result<String, reqwest::Error> {
        // 요청 전 레이트 리미터 대기
        self.rate_limiter.wait().await;
        
        let response = self.client
            .get(url)
            .send()
            .await?;
        
        response.text().await
    }
}
```

#### 동시성 제어
```rust
pub struct ConcurrencyController {
    semaphore: Arc<Semaphore>,
    active_tasks: Arc<AtomicU32>,
    max_concurrent: u32,
}

impl ConcurrencyController {
    pub fn new(max_concurrent: u32) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent as usize)),
            active_tasks: Arc::new(AtomicU32::new(0)),
            max_concurrent,
        }
    }
    
    pub async fn execute<F, T, E>(&self, task: F) -> Result<T, E>
    where
        F: Future<Output = Result<T, E>>,
    {
        let _permit = self.semaphore.acquire().await.unwrap();
        let _guard = TaskGuard::new(&self.active_tasks);
        
        task.await
    }
}

struct TaskGuard<'a> {
    counter: &'a AtomicU32,
}

impl<'a> TaskGuard<'a> {
    fn new(counter: &'a AtomicU32) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self { counter }
    }
}

impl<'a> Drop for TaskGuard<'a> {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}
```

### 3. Tauri 명령어 구현

```rust
#[tauri::command]
pub async fn start_crawling(
    config: CrawlerConfig,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    let crawling_service = &state.crawling_service;
    
    match crawling_service.start_crawling(config).await {
        Ok(_) => Ok(true),
        Err(e) => {
            // 에러 이벤트 발송
            app_handle.emit_all("crawling-error", &format!("{}", e))
                .map_err(|e| format!("Failed to emit error event: {}", e))?;
            Err(format!("Crawling failed: {}", e))
        }
    }
}

#[tauri::command]
pub async fn get_crawling_status(
    state: tauri::State<'_, AppState>,
) -> Result<CrawlingProgress, String> {
    state.progress_tracker
        .get_current_progress()
        .await
        .map_err(|e| format!("Failed to get progress: {}", e))
}

#[tauri::command]
pub async fn update_config(
    updates: PartialCrawlerConfig,
    state: tauri::State<'_, AppState>,
) -> Result<CrawlerConfig, String> {
    state.config_manager
        .update_config(updates)
        .await
        .map_err(|e| format!("Failed to update config: {}", e))
}
```

### 4. 테스트 전략

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;
    
    #[tokio::test]
    async fn test_product_collection() {
        let config = CrawlerConfig::default();
        let service = CrawlingService::new_for_test(config);
        
        let products = service.collect_product_list(5).await.unwrap();
        assert!(!products.is_empty());
        assert!(products.len() <= 5 * 12); // 페이지당 12개 제품
    }
    
    #[tokio::test]
    async fn test_batch_processing() {
        let config = CrawlerConfig {
            enable_batch_processing: Some(true),
            batch_size: Some(2),
            ..Default::default()
        };
        
        let processor = BatchProcessor::new(config);
        let products = processor.process_in_batches(5).await.unwrap();
        
        // 배치 처리가 정상적으로 작동했는지 확인
        assert!(!products.is_empty());
    }
    
    #[test]
    fn test_config_validation() {
        let mut config = CrawlerConfig::default();
        config.page_range_limit = 0;
        
        assert!(ConfigManager::validate_config(&config).is_err());
    }
}
```

---

## 마이그레이션 가이드

### TypeScript → Rust 변환 매핑

| TypeScript | Rust | 노트 |
|------------|------|------|
| `interface Product` | `struct Product` | `#[derive(Serialize, Deserialize)]` 추가 |
| `readonly` 필드 | 기본 필드 | Rust는 기본적으로 immutable |
| `Array<T>` | `Vec<T>` | |
| `Record<K, V>` | `HashMap<K, V>` | |
| `Promise<T>` | `Future<Output = T>` | async/await 사용 |
| `setTimeout` | `tokio::time::sleep` | |
| `EventEmitter` | `broadcast::channel` | |
| SQLite 쿼리 | `sqlx::query!` 매크로 | 컴파일 타임 검증 |

### 주요 아키텍처 결정

1. **HTTP 클라이언트**: reqwest 사용 (chromiumoxide 대신)
2. **데이터베이스**: SQLx + SQLite (타입 안전성)
3. **동시성**: Tokio + async/await (성능 우선)
4. **설정 관리**: serde + JSON (호환성 유지)
5. **에러 처리**: thiserror + Result<T, E> (Rust 관례)

### 성능 개선 목표

- **메모리 사용량**: 70% 감소
- **시작 시간**: 66% 단축 (3초 → 1초)
- **CPU 사용률**: 50% 감소
- **배터리 수명**: 40% 향상

이 문서는 기존 TypeScript 구현체의 핵심 도메인 로직을 Rust로 재구현하기 위한 완전한 가이드를 제공합니다. 각 섹션의 코드 예제는 실제 구현 가능한 형태로 작성되었으며, 기존 시스템의 검증된 패턴을 Rust 생태계에 맞게 변환했습니다.
