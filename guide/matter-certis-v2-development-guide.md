# Matter Certis v2 - 단계별 개발 가이드

## 🗓️ 전체 개발 일정 (8주)

### Phase 1: 프로젝트 초기화 및 핵심 아키텍처 (2주)
### Phase 2: 백엔드 도메인 구현 (2주)
### Phase 3: 크롤링 엔진 구현 (2주)
### Phase 4: 프론트엔드 구현 (1.5주)
### Phase 5: 통합 테스트 및 최적화 (0.5주)

---

## 📅 Phase 1: 프로젝트 초기화 및 핵심 아키텍처 (2주)

### 🎯 목표
- Tauri 프로젝트 초기화
- 기본 프로젝트 구조 구축
- 핵심 데이터 모델 정의
- 기본 Tauri Commands 구현

### 📋 작업 목록

#### Week 1.1: 프로젝트 셋업 (3-4일)

**1일차: 프로젝트 초기화**
```bash
# 새 Tauri 프로젝트 생성
npm create tauri-app@latest matter-certis-v2
cd matter-certis-v2

# 프로젝트 템플릿 선택
# - Package manager: npm
# - Frontend template: Vanilla
# - TypeScript: Yes
```

**프로젝트 구조 생성**
```
matter-certis-v2/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── domain/
│   │   │   ├── mod.rs
│   │   │   ├── entities/
│   │   │   ├── repositories/
│   │   │   └── services/
│   │   ├── application/
│   │   │   ├── mod.rs
│   │   │   ├── use_cases/
│   │   │   └── dto/
│   │   ├── infrastructure/
│   │   │   ├── mod.rs
│   │   │   ├── database/
│   │   │   ├── http/
│   │   │   └── config/
│   │   └── commands/
│   │       └── mod.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/
│   ├── main.tsx
│   ├── App.tsx
│   ├── stores/
│   ├── components/
│   ├── services/
│   ├── types/
│   └── utils/
├── package.json
├── vite.config.ts
└── tsconfig.json
```

**2일차: Cargo.toml 설정**
```toml
[package]
name = "matter-certis-v2"
version = "0.1.0"
edition = "2021"

[dependencies]
tauri = { version = "2.0", features = ["api-all"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
reqwest = { version = "0.11", features = ["json", "cookies", "gzip"] }
sqlx = { version = "0.7", features = ["sqlite", "runtime-tokio-rustls", "chrono"] }
scraper = "0.18"
anyhow = "1.0"
thiserror = "1.0"
rayon = "1.7"
futures = "0.3"
config = "0.13"
tracing = "0.1"
tracing-subscriber = "0.3"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.0", features = ["v4", "serde"] }

[build-dependencies]
tauri-build = { version = "2.0", features = [] }
```

**3일차: TypeScript 환경 구성**
```json
// package.json
{
  "name": "matter-certis-v2",
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "tauri": "tauri"
  },
  "dependencies": {
    "solid-js": "^1.8.0",
    "@solidjs/router": "^0.10.0",
    "@tauri-apps/api": "^2.0.0",
    "@kobalte/core": "^0.12.0",
    "solid-primitives": "^1.8.0",
    "date-fns": "^2.30.0",
    "nanoid": "^5.0.0"
  },
  "devDependencies": {
    "@types/node": "^20.0.0",
    "typescript": "^5.3.0",
    "vite": "^5.0.0",
    "vite-plugin-solid": "^2.8.0",
    "vite-tsconfig-paths": "^4.2.0",
    "vitest": "^1.0.0"
  }
}
```

**4일차: 기본 Vite 및 SolidJS 설정**
```typescript
// vite.config.ts
import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';
import tsconfigPaths from 'vite-tsconfig-paths';

export default defineConfig({
  plugins: [solid(), tsconfigPaths()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    target: process.env.TAURI_PLATFORM == 'windows' ? 'chrome105' : 'safari13',
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
});
```

#### Week 1.2: 핵심 데이터 모델 구현 (3-4일)

**5일차: Rust 도메인 엔티티 정의**
```rust
// src-tauri/src/domain/entities/mod.rs
pub mod product;
pub mod vendor;
pub mod crawling_session;

pub use product::*;
pub use vendor::*;
pub use crawling_session::*;
```

```rust
// src-tauri/src/domain/entities/product.rs
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Product {
    pub id: String,
    pub name: String,
    pub price: Option<f64>,
    pub currency: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub product_url: String,
    pub vendor_id: String,
    pub category: Option<String>,
    pub in_stock: bool,
    pub collected_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Product {
    pub fn new(
        name: String,
        product_url: String,
        vendor_id: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            price: None,
            currency: "USD".to_string(),
            description: None,
            image_url: None,
            product_url,
            vendor_id,
            category: None,
            in_stock: true,
            collected_at: now,
            updated_at: now,
        }
    }
}
```

**6일차: TypeScript 타입 정의**
```typescript
// src/types/domain.ts
export interface Product {
  id: string;
  name: string;
  price?: number;
  currency: string;
  description?: string;
  imageUrl?: string;
  productUrl: string;
  vendorId: string;
  category?: string;
  inStock: boolean;
  collectedAt: string;
  updatedAt: string;
}

export interface Vendor {
  id: string;
  name: string;
  baseUrl: string;
  crawlingConfig: CrawlingConfig;
  isActive: boolean;
  lastCrawledAt?: string;
  createdAt: string;
  updatedAt: string;
}

export interface CrawlingConfig {
  maxPages?: number;
  delayBetweenRequests: number;
  maxConcurrentRequests: number;
  selectors: ProductSelectors;
  pagination?: PaginationConfig;
}

export interface ProductSelectors {
  productContainer: string;
  name: string;
  price: string;
  imageUrl?: string;
  productUrl?: string;
  inStock?: string;
}
```

**7일차: 기본 Repository 인터페이스**
```rust
// src-tauri/src/domain/repositories/mod.rs
pub mod product_repository;
pub mod vendor_repository;
pub mod crawling_session_repository;

pub use product_repository::*;
pub use vendor_repository::*;
pub use crawling_session_repository::*;
```

```rust
// src-tauri/src/domain/repositories/product_repository.rs
use async_trait::async_trait;
use crate::domain::entities::Product;
use anyhow::Result;

#[async_trait]
pub trait ProductRepository: Send + Sync {
    async fn save(&self, product: &Product) -> Result<()>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Product>>;
    async fn find_by_vendor(&self, vendor_id: &str) -> Result<Vec<Product>>;
    async fn update(&self, product: &Product) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<()>;
    async fn find_all(&self) -> Result<Vec<Product>>;
}
```

### 📋 Week 1 완료 체크리스트
- [ ] Tauri 프로젝트 초기화 완료
- [ ] 프로젝트 구조 생성 완료
- [ ] Rust 의존성 설정 완료
- [ ] TypeScript 환경 구성 완료
- [ ] SolidJS 기본 설정 완료
- [ ] 핵심 도메인 엔티티 정의 완료
- [ ] TypeScript 타입 정의 완료
- [ ] Repository 인터페이스 정의 완료

---

## 📅 Phase 2: 백엔드 도메인 구현 (2주)

### 🎯 목표
- SQLite 데이터베이스 설정
- Repository 구현
- 기본 Use Cases 구현
- Tauri Commands 기본 구조

### 📋 작업 목록

#### Week 2.1: 데이터베이스 및 Infrastructure (3-4일)

**8일차: SQLite 데이터베이스 설정**
```rust
// src-tauri/src/infrastructure/database/mod.rs
pub mod connection;
pub mod migrations;
pub mod repositories;

pub use connection::*;
```

```rust
// src-tauri/src/infrastructure/database/connection.rs
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use anyhow::Result;
use std::path::Path;

pub struct DatabaseConnection {
    pool: SqlitePool,
}

impl DatabaseConnection {
    pub async fn new(database_url: &str) -> Result<Self> {
        // 데이터베이스 파일이 없으면 생성
        if let Some(parent) = Path::new(database_url.trim_start_matches("sqlite://")).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }
}
```

**9일차: 데이터베이스 마이그레이션 스크립트**
```sql
-- migrations/001_initial.sql
CREATE TABLE IF NOT EXISTS vendors (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    base_url TEXT NOT NULL,
    crawling_config TEXT NOT NULL, -- JSON
    is_active BOOLEAN NOT NULL DEFAULT 1,
    last_crawled_at DATETIME,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS products (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    price REAL,
    currency TEXT NOT NULL DEFAULT 'USD',
    description TEXT,
    image_url TEXT,
    product_url TEXT NOT NULL,
    vendor_id TEXT NOT NULL,
    category TEXT,
    in_stock BOOLEAN NOT NULL DEFAULT 1,
    collected_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (vendor_id) REFERENCES vendors (id)
);

CREATE TABLE IF NOT EXISTS crawling_sessions (
    id TEXT PRIMARY KEY,
    vendor_id TEXT NOT NULL,
    status TEXT NOT NULL,
    total_pages INTEGER,
    processed_pages INTEGER NOT NULL DEFAULT 0,
    products_found INTEGER NOT NULL DEFAULT 0,
    errors_count INTEGER NOT NULL DEFAULT 0,
    started_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at DATETIME,
    FOREIGN KEY (vendor_id) REFERENCES vendors (id)
);

CREATE INDEX IF NOT EXISTS idx_products_vendor_id ON products (vendor_id);
CREATE INDEX IF NOT EXISTS idx_products_collected_at ON products (collected_at);
CREATE INDEX IF NOT EXISTS idx_crawling_sessions_vendor_id ON crawling_sessions (vendor_id);
```

**10일차: Repository 구현**
```rust
// src-tauri/src/infrastructure/database/repositories/product_repository_impl.rs
use async_trait::async_trait;
use sqlx::SqlitePool;
use anyhow::Result;
use crate::domain::{entities::Product, repositories::ProductRepository};

pub struct ProductRepositoryImpl {
    pool: SqlitePool,
}

impl ProductRepositoryImpl {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProductRepository for ProductRepositoryImpl {
    async fn save(&self, product: &Product) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO products (
                id, name, price, currency, description, 
                image_url, product_url, vendor_id, category, 
                in_stock, collected_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            product.id,
            product.name,
            product.price,
            product.currency,
            product.description,
            product.image_url,
            product.product_url,
            product.vendor_id,
            product.category,
            product.in_stock,
            product.collected_at,
            product.updated_at
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<Product>> {
        let product = sqlx::query_as!(
            Product,
            "SELECT * FROM products WHERE id = ?",
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(product)
    }

    async fn find_by_vendor(&self, vendor_id: &str) -> Result<Vec<Product>> {
        let products = sqlx::query_as!(
            Product,
            "SELECT * FROM products WHERE vendor_id = ? ORDER BY collected_at DESC",
            vendor_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(products)
    }

    // ... 다른 메서드들 구현
}
```

**11일차: HTTP 클라이언트 구현**
```rust
// src-tauri/src/infrastructure/http/mod.rs
pub mod client;
pub mod rate_limiter;

pub use client::*;
pub use rate_limiter::*;
```

```rust
// src-tauri/src/infrastructure/http/client.rs
use reqwest::{Client, ClientBuilder, Response};
use anyhow::Result;
use std::time::Duration;
use tokio::time;

pub struct HttpClient {
    client: Client,
    rate_limiter: RateLimiter,
}

impl HttpClient {
    pub fn new(
        timeout: Duration,
        max_requests_per_second: u32,
    ) -> Result<Self> {
        let client = ClientBuilder::new()
            .timeout(timeout)
            .user_agent("MatterCertis/2.0")
            .gzip(true)
            .build()?;

        let rate_limiter = RateLimiter::new(max_requests_per_second);

        Ok(Self {
            client,
            rate_limiter,
        })
    }

    pub async fn get(&self, url: &str) -> Result<Response> {
        self.rate_limiter.wait().await;
        
        let response = self.client
            .get(url)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("HTTP request failed: {}", response.status());
        }

        Ok(response)
    }
}
```

#### Week 2.2: Use Cases 및 Application 계층 (3-4일)

**12일차: 기본 Use Cases 구현**
```rust
// src-tauri/src/application/use_cases/mod.rs
pub mod manage_vendors;
pub mod start_crawling;
pub mod get_products;

pub use manage_vendors::*;
pub use start_crawling::*;
pub use get_products::*;
```

```rust
// src-tauri/src/application/use_cases/manage_vendors.rs
use crate::domain::{entities::Vendor, repositories::VendorRepository};
use anyhow::Result;
use std::sync::Arc;

pub struct ManageVendorsUseCase {
    vendor_repository: Arc<dyn VendorRepository>,
}

impl ManageVendorsUseCase {
    pub fn new(vendor_repository: Arc<dyn VendorRepository>) -> Self {
        Self { vendor_repository }
    }

    pub async fn create_vendor(&self, vendor: Vendor) -> Result<()> {
        self.vendor_repository.save(&vendor).await
    }

    pub async fn get_all_vendors(&self) -> Result<Vec<Vendor>> {
        self.vendor_repository.find_all().await
    }

    pub async fn update_vendor(&self, vendor: Vendor) -> Result<()> {
        self.vendor_repository.update(&vendor).await
    }

    pub async fn delete_vendor(&self, id: &str) -> Result<()> {
        self.vendor_repository.delete(id).await
    }
}
```

**13일차: Tauri Commands 기본 구조**
```rust
// src-tauri/src/commands/mod.rs
pub mod vendor_commands;
pub mod crawling_commands;
pub mod product_commands;

pub use vendor_commands::*;
pub use crawling_commands::*;
pub use product_commands::*;
```

```rust
// src-tauri/src/commands/vendor_commands.rs
use tauri::State;
use crate::application::use_cases::ManageVendorsUseCase;
use crate::domain::entities::Vendor;
use anyhow::Result;

#[tauri::command]
pub async fn create_vendor(
    vendor: Vendor,
    use_case: State<'_, ManageVendorsUseCase>,
) -> Result<(), String> {
    use_case
        .create_vendor(vendor)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_all_vendors(
    use_case: State<'_, ManageVendorsUseCase>,
) -> Result<Vec<Vendor>, String> {
    use_case
        .get_all_vendors()
        .await
        .map_err(|e| e.to_string())
}
```

**14일차: 애플리케이션 상태 관리 및 의존성 주입**
```rust
// src-tauri/src/lib.rs
use tauri::Manager;
use std::sync::Arc;

pub mod domain;
pub mod application;
pub mod infrastructure;
pub mod commands;

use infrastructure::database::DatabaseConnection;
use infrastructure::database::repositories::*;
use application::use_cases::*;

pub struct AppState {
    pub database: Arc<DatabaseConnection>,
    pub manage_vendors_use_case: Arc<ManageVendorsUseCase>,
    // ... 다른 use cases
}

impl AppState {
    pub async fn new() -> anyhow::Result<Self> {
        // 데이터베이스 연결
        let database = Arc::new(
            DatabaseConnection::new("sqlite://data/matter_certis.db").await?
        );
        
        // 마이그레이션 실행
        database.migrate().await?;

        // Repository 생성
        let vendor_repository = Arc::new(
            VendorRepositoryImpl::new(database.pool().clone())
        );

        // Use Case 생성
        let manage_vendors_use_case = Arc::new(
            ManageVendorsUseCase::new(vendor_repository)
        );

        Ok(Self {
            database,
            manage_vendors_use_case,
        })
    }
}
```

### 📋 Week 2 완료 체크리스트
- [ ] SQLite 데이터베이스 설정 완료
- [ ] 데이터베이스 마이그레이션 스크립트 작성
- [ ] Repository 구현 완료
- [ ] HTTP 클라이언트 기본 구현
- [ ] 기본 Use Cases 구현
- [ ] Tauri Commands 기본 구조 완성
- [ ] 의존성 주입 설정 완료

---

## 📅 Phase 3: 크롤링 엔진 구현 (2주)

### 🎯 목표
- reqwest 기반 크롤링 엔진 구현
- HTML 파싱 및 데이터 추출
- 병렬 처리 및 Rate Limiting
- 실시간 진행 상황 업데이트

### 📋 작업 목록

#### Week 3.1: 크롤링 엔진 핵심 구현 (3-4일)

**15일차: 크롤링 서비스 도메인 구현**
```rust
// src-tauri/src/domain/services/crawling_service.rs
use async_trait::async_trait;
use crate::domain::entities::{CrawlingSession, Product, Vendor};
use anyhow::Result;

#[async_trait]
pub trait CrawlingService: Send + Sync {
    async fn start_crawling(&self, vendor: &Vendor) -> Result<CrawlingSession>;
    async fn pause_crawling(&self, session_id: &str) -> Result<()>;
    async fn resume_crawling(&self, session_id: &str) -> Result<()>;
    async fn stop_crawling(&self, session_id: &str) -> Result<()>;
    async fn get_crawling_progress(&self, session_id: &str) -> Result<CrawlingProgress>;
}

#[derive(Debug, Clone)]
pub struct CrawlingProgress {
    pub session_id: String,
    pub total_pages: Option<u32>,
    pub processed_pages: u32,
    pub products_found: u32,
    pub errors_count: u32,
    pub current_url: Option<String>,
    pub status: CrawlingStatus,
}
```

**16일차: HTML 파싱 구현**
```rust
// src-tauri/src/infrastructure/crawling/mod.rs
pub mod html_parser;
pub mod product_extractor;
pub mod crawler_engine;

pub use html_parser::*;
pub use product_extractor::*;
pub use crawler_engine::*;
```

```rust
// src-tauri/src/infrastructure/crawling/html_parser.rs
use scraper::{Html, Selector};
use anyhow::Result;
use crate::domain::entities::{Product, CrawlingConfig};

pub struct HtmlParser {
    config: CrawlingConfig,
}

impl HtmlParser {
    pub fn new(config: CrawlingConfig) -> Self {
        Self { config }
    }

    pub fn extract_products(&self, html: &str, base_url: &str, vendor_id: &str) -> Result<Vec<Product>> {
        let document = Html::parse_document(html);
        let mut products = Vec::new();

        // 제품 컨테이너 선택자
        let container_selector = Selector::parse(&self.config.selectors.product_container)
            .map_err(|e| anyhow::anyhow!("Invalid container selector: {}", e))?;

        // 각 제품 요소에 대해 데이터 추출
        for element in document.select(&container_selector) {
            if let Ok(product) = self.extract_single_product(element, base_url, vendor_id) {
                products.push(product);
            }
        }

        Ok(products)
    }

    fn extract_single_product(
        &self,
        element: scraper::ElementRef,
        base_url: &str,
        vendor_id: &str,
    ) -> Result<Product> {
        // 제품명 추출
        let name_selector = Selector::parse(&self.config.selectors.name)?;
        let name = element
            .select(&name_selector)
            .next()
            .and_then(|e| e.text().next())
            .ok_or_else(|| anyhow::anyhow!("Product name not found"))?
            .trim()
            .to_string();

        // 가격 추출
        let price_selector = Selector::parse(&self.config.selectors.price)?;
        let price = element
            .select(&price_selector)
            .next()
            .and_then(|e| e.text().next())
            .and_then(|text| self.parse_price(text));

        // 제품 URL 추출
        let product_url = if let Some(url_selector) = &self.config.selectors.product_url {
            let selector = Selector::parse(url_selector)?;
            element
                .select(&selector)
                .next()
                .and_then(|e| e.value().attr("href"))
                .map(|href| self.resolve_url(base_url, href))
                .unwrap_or_else(|| base_url.to_string())
        } else {
            base_url.to_string()
        };

        // 이미지 URL 추출
        let image_url = if let Some(img_selector) = &self.config.selectors.image_url {
            let selector = Selector::parse(img_selector)?;
            element
                .select(&selector)
                .next()
                .and_then(|e| e.value().attr("src").or_else(|| e.value().attr("data-src")))
                .map(|src| self.resolve_url(base_url, src))
        } else {
            None
        };

        // 재고 상태 확인
        let in_stock = if let Some(stock_selector) = &self.config.selectors.in_stock {
            let selector = Selector::parse(stock_selector)?;
            element.select(&selector).next().is_some()
        } else {
            true // 기본값: 재고 있음
        };

        let mut product = Product::new(name, product_url, vendor_id.to_string());
        product.price = price;
        product.image_url = image_url;
        product.in_stock = in_stock;

        Ok(product)
    }

    fn parse_price(&self, price_text: &str) -> Option<f64> {
        // 가격 텍스트에서 숫자 추출
        let cleaned = price_text
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
            .collect::<String>()
            .replace(',', "");

        cleaned.parse().ok()
    }

    fn resolve_url(&self, base_url: &str, relative_url: &str) -> String {
        if relative_url.starts_with("http") {
            relative_url.to_string()
        } else if relative_url.starts_with("//") {
            format!("https:{}", relative_url)
        } else if relative_url.starts_with('/') {
            let base = url::Url::parse(base_url).unwrap();
            format!("{}://{}{}", base.scheme(), base.host_str().unwrap(), relative_url)
        } else {
            format!("{}/{}", base_url.trim_end_matches('/'), relative_url)
        }
    }
}
```

**17일차: 크롤링 엔진 구현**
```rust
// src-tauri/src/infrastructure/crawling/crawler_engine.rs
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use futures::stream::{self, StreamExt};
use anyhow::Result;

use crate::domain::{
    entities::{Vendor, Product, CrawlingSession, CrawlingStatus},
    repositories::{ProductRepository, CrawlingSessionRepository},
    services::CrawlingService,
};
use crate::infrastructure::http::HttpClient;
use super::HtmlParser;

pub struct CrawlerEngine {
    http_client: Arc<HttpClient>,
    product_repository: Arc<dyn ProductRepository>,
    session_repository: Arc<dyn CrawlingSessionRepository>,
    progress_sender: Arc<Mutex<Option<mpsc::UnboundedSender<CrawlingProgress>>>>,
}

impl CrawlerEngine {
    pub fn new(
        http_client: Arc<HttpClient>,
        product_repository: Arc<dyn ProductRepository>,
        session_repository: Arc<dyn CrawlingSessionRepository>,
    ) -> Self {
        Self {
            http_client,
            product_repository,
            session_repository,
            progress_sender: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn crawl_vendor(&self, vendor: &Vendor) -> Result<CrawlingSession> {
        let mut session = CrawlingSession::new(vendor.id.clone());
        session.status = CrawlingStatus::Running;
        
        // 세션 저장
        self.session_repository.save(&session).await?;

        // 페이지 URL 생성
        let page_urls = self.generate_page_urls(vendor)?;
        session.total_pages = Some(page_urls.len() as u32);
        self.session_repository.update(&session).await?;

        // 병렬 크롤링 실행
        let max_concurrent = vendor.crawling_config.max_concurrent_requests;
        let parser = HtmlParser::new(vendor.crawling_config.clone());

        let results = stream::iter(page_urls)
            .map(|url| {
                let http_client = self.http_client.clone();
                let parser = parser.clone();
                let vendor_id = vendor.id.clone();
                
                async move {
                    self.crawl_single_page(&http_client, &parser, &url, &vendor_id).await
                }
            })
            .buffer_unordered(max_concurrent as usize)
            .collect::<Vec<_>>()
            .await;

        // 결과 처리
        let mut total_products = 0;
        let mut errors = 0;

        for result in results {
            match result {
                Ok(products) => {
                    for product in products {
                        if let Err(e) = self.product_repository.save(&product).await {
                            tracing::error!("Failed to save product: {}", e);
                            errors += 1;
                        } else {
                            total_products += 1;
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to crawl page: {}", e);
                    errors += 1;
                }
            }

            // 진행 상황 업데이트
            session.processed_pages += 1;
            session.products_found = total_products;
            session.errors_count = errors;
            self.session_repository.update(&session).await?;

            // 진행 상황 이벤트 발송
            self.send_progress_update(&session).await;
        }

        // 세션 완료
        session.status = CrawlingStatus::Completed;
        session.completed_at = Some(chrono::Utc::now());
        self.session_repository.update(&session).await?;

        Ok(session)
    }

    async fn crawl_single_page(
        &self,
        http_client: &HttpClient,
        parser: &HtmlParser,
        url: &str,
        vendor_id: &str,
    ) -> Result<Vec<Product>> {
        // HTTP 요청
        let response = http_client.get(url).await?;
        let html = response.text().await?;

        // HTML 파싱 및 제품 추출
        let products = parser.extract_products(&html, url, vendor_id)?;

        // 딜레이 적용 (Rate Limiting은 HttpClient에서 처리)
        Ok(products)
    }

    fn generate_page_urls(&self, vendor: &Vendor) -> Result<Vec<String>> {
        let mut urls = vec![vendor.base_url.clone()];

        // 페이지네이션 설정이 있는 경우
        if let Some(pagination) = &vendor.crawling_config.pagination {
            if let Some(max_pages) = vendor.crawling_config.max_pages {
                for page in 2..=max_pages {
                    let url = pagination.url_pattern
                        .replace("{page}", &page.to_string());
                    urls.push(url);
                }
            }
        }

        Ok(urls)
    }

    async fn send_progress_update(&self, session: &CrawlingSession) {
        let progress = CrawlingProgress {
            session_id: session.id.clone(),
            total_pages: session.total_pages,
            processed_pages: session.processed_pages,
            products_found: session.products_found,
            errors_count: session.errors_count,
            current_url: None,
            status: session.status.clone(),
        };

        if let Ok(sender_guard) = self.progress_sender.try_lock() {
            if let Some(sender) = sender_guard.as_ref() {
                let _ = sender.send(progress);
            }
        }
    }
}
```

**18일차: Rate Limiter 구현**
```rust
// src-tauri/src/infrastructure/http/rate_limiter.rs
use std::time::{Duration, Instant};
use tokio::time;

pub struct RateLimiter {
    max_requests_per_second: u32,
    last_request_time: std::sync::Mutex<Option<Instant>>,
}

impl RateLimiter {
    pub fn new(max_requests_per_second: u32) -> Self {
        Self {
            max_requests_per_second,
            last_request_time: std::sync::Mutex::new(None),
        }
    }

    pub async fn wait(&self) {
        if self.max_requests_per_second == 0 {
            return;
        }

        let min_interval = Duration::from_millis(1000 / self.max_requests_per_second as u64);
        
        let should_wait = {
            let mut last_time = self.last_request_time.lock().unwrap();
            let now = Instant::now();
            
            let should_wait = if let Some(last) = *last_time {
                let elapsed = now.duration_since(last);
                if elapsed < min_interval {
                    Some(min_interval - elapsed)
                } else {
                    None
                }
            } else {
                None
            };
            
            *last_time = Some(now);
            should_wait
        };

        if let Some(wait_duration) = should_wait {
            time::sleep(wait_duration).await;
        }
    }
}
```

#### Week 3.2: 실시간 이벤트 및 최적화 (3-4일)

**19일차: Tauri 이벤트 시스템 구현**
```rust
// src-tauri/src/commands/crawling_commands.rs
use tauri::{State, Window};
use tokio::sync::mpsc;
use std::sync::Arc;

use crate::application::use_cases::StartCrawlingUseCase;
use crate::domain::entities::Vendor;
use crate::infrastructure::crawling::CrawlingProgress;

#[tauri::command]
pub async fn start_crawling_session(
    vendor_id: String,
    window: Window,
    use_case: State<'_, StartCrawlingUseCase>,
) -> Result<String, String> {
    // 진행 상황 채널 생성
    let (tx, mut rx) = mpsc::unbounded_channel::<CrawlingProgress>();
    
    // 백그라운드에서 진행 상황 이벤트 전송
    let window_clone = window.clone();
    tokio::spawn(async move {
        while let Some(progress) = rx.recv().await {
            let _ = window_clone.emit("crawling-progress", &progress);
        }
    });

    // 크롤링 시작
    let session = use_case
        .start_crawling(&vendor_id, Some(tx))
        .await
        .map_err(|e| e.to_string())?;

    Ok(session.id)
}

#[tauri::command]
pub async fn pause_crawling_session(
    session_id: String,
    use_case: State<'_, StartCrawlingUseCase>,
) -> Result<(), String> {
    use_case
        .pause_crawling(&session_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_crawling_progress(
    session_id: String,
    use_case: State<'_, StartCrawlingUseCase>,
) -> Result<CrawlingProgress, String> {
    use_case
        .get_crawling_progress(&session_id)
        .await
        .map_err(|e| e.to_string())
}
```

**20일차: TypeScript 서비스 레이어**
```typescript
// src/services/crawling-service.ts
import { invoke } from '@tauri-apps/api/tauri';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { CrawlingProgress } from '../types/domain';

export class CrawlingService {
  private progressListeners: Set<(progress: CrawlingProgress) => void> = new Set();
  private unlistenFn: UnlistenFn | null = null;

  async startCrawling(vendorId: string): Promise<string> {
    return await invoke('start_crawling_session', { vendorId });
  }

  async pauseCrawling(sessionId: string): Promise<void> {
    return await invoke('pause_crawling_session', { sessionId });
  }

  async resumeCrawling(sessionId: string): Promise<void> {
    return await invoke('resume_crawling_session', { sessionId });
  }

  async getCrawlingProgress(sessionId: string): Promise<CrawlingProgress> {
    return await invoke('get_crawling_progress', { sessionId });
  }

  async subscribeToProgress(callback: (progress: CrawlingProgress) => void): Promise<void> {
    this.progressListeners.add(callback);

    if (!this.unlistenFn) {
      this.unlistenFn = await listen('crawling-progress', (event) => {
        const progress = event.payload as CrawlingProgress;
        this.progressListeners.forEach(listener => listener(progress));
      });
    }
  }

  unsubscribeFromProgress(callback: (progress: CrawlingProgress) => void): void {
    this.progressListeners.delete(callback);

    if (this.progressListeners.size === 0 && this.unlistenFn) {
      this.unlistenFn();
      this.unlistenFn = null;
    }
  }
}

export const crawlingService = new CrawlingService();
```

**21일차: 에러 처리 및 복구 메커니즘**
```rust
// src-tauri/src/infrastructure/crawling/error_handler.rs
use std::time::Duration;
use tokio::time;
use anyhow::Result;

pub struct ErrorHandler {
    max_retries: u32,
    base_delay: Duration,
}

impl ErrorHandler {
    pub fn new(max_retries: u32, base_delay: Duration) -> Self {
        Self {
            max_retries,
            base_delay,
        }
    }

    pub async fn retry_with_backoff<F, T, E>(&self, mut operation: F) -> Result<T>
    where
        F: FnMut() -> Result<T, E>,
        E: std::fmt::Display,
    {
        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            match operation() {
                Ok(result) => return Ok(result),
                Err(e) => {
                    tracing::warn!("Attempt {} failed: {}", attempt + 1, e);
                    last_error = Some(e);

                    if attempt < self.max_retries {
                        let delay = self.base_delay * (2_u32.pow(attempt));
                        time::sleep(delay).await;
                    }
                }
            }
        }

        Err(anyhow::anyhow!(
            "Operation failed after {} attempts. Last error: {}",
            self.max_retries + 1,
            last_error.map(|e| e.to_string()).unwrap_or_else(|| "Unknown error".to_string())
        ))
    }
}
```

### 📋 Week 3 완료 체크리스트
- [ ] 크롤링 서비스 도메인 인터페이스 정의
- [ ] HTML 파싱 및 데이터 추출 구현
- [ ] reqwest 기반 크롤링 엔진 구현
- [ ] Rate Limiter 구현
- [ ] 병렬 처리 로직 구현
- [ ] 실시간 진행 상황 이벤트 시스템
- [ ] TypeScript 서비스 레이어 구현
- [ ] 에러 처리 및 재시도 메커니즘

---

이제 Phase 4 (프론트엔드 구현)와 Phase 5 (통합 테스트 및 최적화)에 대한 가이드를 계속 작성하시겠습니까?
