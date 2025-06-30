# Matter Certis v2.0 배치 크롤링 시스템: Chapter 5 - 데이터베이스 통합 및 배치 최적화

## 목차
- [5.1 데이터베이스 스키마 및 모델 정의](#51-데이터베이스-스키마-및-모델-정의)
- [5.2 배치 저장 및 업데이트 로직](#52-배치-저장-및-업데이트-로직)
- [5.3 배치 진행 상태 영속화](#53-배치-진행-상태-영속화)
- [5.4 트랜잭션 관리 및 에러 복구](#54-트랜잭션-관리-및-에러-복구)
- [5.5 데이터베이스 성능 최적화](#55-데이터베이스-성능-최적화)

---

## 5.1 데이터베이스 스키마 및 모델 정의

### 5.1.1 Rust/Tauri 데이터베이스 엔티티

TypeScript/Electron의 SQLite 스키마를 Rust의 강타입 시스템에 맞게 재설계합니다.

```rust
// src/database/entities/product.rs
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Product {
    pub url: String,                    // PRIMARY KEY
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub certificate_id: Option<String>,
    pub page_id: Option<i32>,
    pub index_in_page: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProductDetail {
    pub url: String,                        // PRIMARY KEY, FOREIGN KEY -> products.url
    pub page_id: Option<i32>,
    pub index_in_page: Option<i32>,
    pub id: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub device_type: Option<String>,
    pub certification_id: Option<String>,
    pub certification_date: Option<DateTime<Utc>>,
    pub software_version: Option<String>,
    pub hardware_version: Option<String>,
    pub vid: Option<i32>,
    pub pid: Option<i32>,
    pub family_sku: Option<String>,
    pub family_variant_sku: Option<String>,
    pub firmware_version: Option<String>,
    pub family_id: Option<String>,
    pub tis_trp_tested: Option<String>,
    pub specification_version: Option<String>,
    pub transport_interface: Option<String>,
    pub primary_device_type_id: Option<String>,
    pub application_categories: Option<String>, // JSON string
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Vendor {
    pub vendor_id: i32,                 // PRIMARY KEY
    pub vendor_name: Option<String>,
    pub company_legal_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// 배치 처리 관련 엔티티
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BatchProgress {
    pub id: String,                     // PRIMARY KEY (UUID)
    pub session_id: String,
    pub batch_number: i32,
    pub total_batches: i32,
    pub status: BatchStatus,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub items_processed: i32,
    pub items_total: i32,
    pub retry_count: i32,
    pub error_message: Option<String>,
    pub metadata: Option<String>,       // JSON string
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "batch_status")]
pub enum BatchStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Retrying,
    Cancelled,
}
```

### 5.1.2 데이터베이스 마이그레이션 스키마

```rust
// src/database/migrations/mod.rs
use sqlx::{Sqlite, SqlitePool};
use anyhow::Result;

pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    // 마이그레이션 실행
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

// migrations/001_initial_schema.sql
/*
-- Products 테이블
CREATE TABLE IF NOT EXISTS products (
    url TEXT PRIMARY KEY NOT NULL,
    manufacturer TEXT,
    model TEXT,
    certificate_id TEXT,
    page_id INTEGER,
    index_in_page INTEGER,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Product Details 테이블
CREATE TABLE IF NOT EXISTS product_details (
    url TEXT PRIMARY KEY NOT NULL,
    page_id INTEGER,
    index_in_page INTEGER,
    id TEXT,
    manufacturer TEXT,
    model TEXT,
    device_type TEXT,
    certification_id TEXT,
    certification_date DATETIME,
    software_version TEXT,
    hardware_version TEXT,
    vid INTEGER,
    pid INTEGER,
    family_sku TEXT,
    family_variant_sku TEXT,
    firmware_version TEXT,
    family_id TEXT,
    tis_trp_tested TEXT,
    specification_version TEXT,
    transport_interface TEXT,
    primary_device_type_id TEXT,
    application_categories TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (url) REFERENCES products(url) ON DELETE CASCADE
);

-- Vendors 테이블
CREATE TABLE IF NOT EXISTS vendors (
    vendor_id INTEGER PRIMARY KEY AUTOINCREMENT,
    vendor_name TEXT,
    company_legal_name TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Batch Progress 테이블
CREATE TABLE IF NOT EXISTS batch_progress (
    id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL,
    batch_number INTEGER NOT NULL,
    total_batches INTEGER NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'failed', 'retrying', 'cancelled')),
    start_time DATETIME NOT NULL,
    end_time DATETIME,
    items_processed INTEGER NOT NULL DEFAULT 0,
    items_total INTEGER NOT NULL DEFAULT 0,
    retry_count INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    metadata TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 인덱스 생성
CREATE INDEX IF NOT EXISTS idx_products_page_id ON products(page_id);
CREATE INDEX IF NOT EXISTS idx_products_manufacturer ON products(manufacturer);
CREATE INDEX IF NOT EXISTS idx_product_details_page_id ON product_details(page_id);
CREATE INDEX IF NOT EXISTS idx_product_details_manufacturer ON product_details(manufacturer);
CREATE INDEX IF NOT EXISTS idx_batch_progress_session ON batch_progress(session_id);
CREATE INDEX IF NOT EXISTS idx_batch_progress_status ON batch_progress(status);
*/
```

### 5.1.3 데이터베이스 컨넥션 및 풀 관리

```rust
// src/database/connection.rs
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use std::str::FromStr;
use anyhow::Result;
use tauri::api::path::app_data_dir;
use std::path::PathBuf;

pub struct DatabaseManager {
    pool: SqlitePool,
}

impl DatabaseManager {
    pub async fn new() -> Result<Self> {
        let db_path = Self::get_database_path()?;
        
        // 데이터베이스 연결 옵션 설정
        let connect_options = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path.display()))?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .pragma("cache_size", "1000")
            .pragma("temp_store", "memory")
            .pragma("mmap_size", "268435456"); // 256MB

        // 연결 풀 생성
        let pool = SqlitePool::connect_with(connect_options).await?;
        
        // 마이그레이션 실행
        crate::database::migrations::run_migrations(&pool).await?;
        
        Ok(Self { pool })
    }

    fn get_database_path() -> Result<PathBuf> {
        let app_data = app_data_dir(&tauri::Config::default())
            .ok_or_else(|| anyhow::anyhow!("앱 데이터 디렉토리를 찾을 수 없습니다"))?;
        
        std::fs::create_dir_all(&app_data)?;
        Ok(app_data.join("matter-certis.db"))
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }

    // 데이터베이스 상태 확인
    pub async fn health_check(&self) -> Result<bool> {
        let result = sqlx::query("SELECT 1 as test")
            .fetch_one(&self.pool)
            .await?;
        
        Ok(result.get::<i32, _>("test") == 1)
    }
}
```

---

## 5.2 배치 저장 및 업데이트 로직

### 5.2.1 배치 단위 제품 저장 서비스

```rust
// src/services/product_batch_service.rs
use crate::database::{entities::*, DatabaseManager};
use crate::crawling::entities::*;
use sqlx::{Sqlite, Transaction, QueryBuilder};
use anyhow::Result;
use uuid::Uuid;
use chrono::Utc;
use std::collections::HashMap;

pub struct ProductBatchService {
    db: DatabaseManager,
}

#[derive(Debug, Clone)]
pub struct BatchSaveResult {
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub failed: usize,
    pub duplicate_info: Vec<DuplicateInfo>,
}

#[derive(Debug, Clone)]
pub struct DuplicateInfo {
    pub url: String,
    pub changes: Vec<String>,
}

impl ProductBatchService {
    pub fn new(db: DatabaseManager) -> Self {
        Self { db }
    }

    // 메인 배치 저장 메서드
    pub async fn save_products_batch(&self, products: Vec<MatterProduct>) -> Result<BatchSaveResult> {
        if products.is_empty() {
            return Ok(BatchSaveResult::default());
        }

        let mut tx = self.db.pool().begin().await?;
        
        // 1. 기본 제품 정보 저장/업데이트
        let basic_result = self.save_basic_products(&mut tx, &products).await?;
        
        // 2. 상세 제품 정보 저장/업데이트  
        let detail_result = self.save_product_details(&mut tx, &products).await?;
        
        // 3. 트랜잭션 커밋
        tx.commit().await?;
        
        // 4. 결과 병합
        Ok(self.merge_save_results(basic_result, detail_result))
    }

    // 기본 제품 정보 저장 (products 테이블)
    async fn save_basic_products(
        &self, 
        tx: &mut Transaction<'_, Sqlite>, 
        products: &[MatterProduct]
    ) -> Result<BatchSaveResult> {
        let mut result = BatchSaveResult::default();
        
        // 기존 제품 URLs 조회 (벌크 조회로 성능 최적화)
        let urls: Vec<String> = products.iter().map(|p| p.url.clone()).collect();
        let existing_products = self.get_existing_basic_products(tx, &urls).await?;
        
        // UPSERT 쿼리 준비
        let mut query_builder = QueryBuilder::new(
            "INSERT INTO products (url, manufacturer, model, certificate_id, page_id, index_in_page, updated_at) "
        );
        
        query_builder.push("VALUES ");
        
        let mut separated = query_builder.separated(", ");
        
        for product in products {
            let is_existing = existing_products.contains_key(&product.url);
            
            separated.push("(")
                .push_bind(&product.url)
                .push(", ")
                .push_bind(&product.manufacturer)
                .push(", ")
                .push_bind(&product.model)
                .push(", ")
                .push_bind(&product.certificate_id)
                .push(", ")
                .push_bind(product.page_id)
                .push(", ")
                .push_bind(product.index_in_page)
                .push(", ")
                .push_bind(Utc::now())
                .push(")");
                
            if is_existing {
                result.updated += 1;
            } else {
                result.added += 1;
            }
        }
        
        query_builder.push(
            " ON CONFLICT(url) DO UPDATE SET
             manufacturer = excluded.manufacturer,
             model = excluded.model,
             certificate_id = excluded.certificate_id,
             page_id = excluded.page_id,
             index_in_page = excluded.index_in_page,
             updated_at = excluded.updated_at"
        );
        
        let query = query_builder.build();
        query.execute(&mut **tx).await?;
        
        Ok(result)
    }

    // 상세 제품 정보 저장 (product_details 테이블)
    async fn save_product_details(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        products: &[MatterProduct]
    ) -> Result<BatchSaveResult> {
        let mut result = BatchSaveResult::default();
        
        // 기존 상세 제품 정보 조회
        let urls: Vec<String> = products.iter().map(|p| p.url.clone()).collect();
        let existing_details = self.get_existing_product_details(tx, &urls).await?;
        
        // 각 제품별로 변경사항 확인 및 저장
        for product in products {
            match self.save_single_product_detail(tx, product, &existing_details).await {
                Ok(single_result) => {
                    result.added += single_result.added;
                    result.updated += single_result.updated;
                    result.unchanged += single_result.unchanged;
                    result.duplicate_info.extend(single_result.duplicate_info);
                }
                Err(e) => {
                    log::error!("제품 상세 정보 저장 실패 (URL: {}): {}", product.url, e);
                    result.failed += 1;
                }
            }
        }
        
        Ok(result)
    }

    // 단일 제품 상세 정보 저장
    async fn save_single_product_detail(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        product: &MatterProduct,
        existing_details: &HashMap<String, ProductDetail>
    ) -> Result<BatchSaveResult> {
        let mut result = BatchSaveResult::default();
        
        // 기존 제품 확인
        if let Some(existing) = existing_details.get(&product.url) {
            // 변경사항 확인
            let changes = self.detect_product_changes(product, existing);
            
            if !changes.is_empty() {
                // 업데이트 수행
                self.update_product_detail(tx, product).await?;
                result.updated = 1;
                result.duplicate_info.push(DuplicateInfo {
                    url: product.url.clone(),
                    changes,
                });
            } else {
                result.unchanged = 1;
            }
        } else {
            // 새 제품 삽입
            self.insert_product_detail(tx, product).await?;
            result.added = 1;
        }
        
        Ok(result)
    }

    // 제품 변경사항 감지
    fn detect_product_changes(&self, new: &MatterProduct, existing: &ProductDetail) -> Vec<String> {
        let mut changes = Vec::new();
        
        // 필드별 변경사항 확인
        if new.manufacturer != existing.manufacturer {
            changes.push(format!("manufacturer: {:?} → {:?}", existing.manufacturer, new.manufacturer));
        }
        
        if new.model != existing.model {
            changes.push(format!("model: {:?} → {:?}", existing.model, new.model));
        }
        
        if new.device_type != existing.device_type {
            changes.push(format!("device_type: {:?} → {:?}", existing.device_type, new.device_type));
        }
        
        // VID/PID 정수 변환 및 비교
        let new_vid = new.vid.and_then(|v| self.parse_hex_to_int(&v));
        if new_vid != existing.vid {
            changes.push(format!("vid: {:?} → {:?}", existing.vid, new_vid));
        }
        
        let new_pid = new.pid.and_then(|v| self.parse_hex_to_int(&v));
        if new_pid != existing.pid {
            changes.push(format!("pid: {:?} → {:?}", existing.pid, new_pid));
        }
        
        // 다른 필드들도 동일한 방식으로 비교...
        
        changes
    }

    // 16진수 문자열을 정수로 변환
    fn parse_hex_to_int(&self, hex_str: &str) -> Option<i32> {
        if hex_str.is_empty() {
            return None;
        }
        
        let cleaned = hex_str.trim_start_matches("0x").trim_start_matches("0X");
        i32::from_str_radix(cleaned, 16).ok()
    }

    // 기존 기본 제품 정보 조회
    async fn get_existing_basic_products(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        urls: &[String]
    ) -> Result<HashMap<String, Product>> {
        if urls.is_empty() {
            return Ok(HashMap::new());
        }
        
        let mut query_builder = QueryBuilder::new("SELECT * FROM products WHERE url IN (");
        let mut separated = query_builder.separated(", ");
        
        for url in urls {
            separated.push_bind(url);
        }
        
        query_builder.push(")");
        
        let products: Vec<Product> = query_builder
            .build_query_as::<Product>()
            .fetch_all(&mut **tx)
            .await?;
            
        Ok(products.into_iter().map(|p| (p.url.clone(), p)).collect())
    }

    // 기존 상세 제품 정보 조회
    async fn get_existing_product_details(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        urls: &[String]
    ) -> Result<HashMap<String, ProductDetail>> {
        if urls.is_empty() {
            return Ok(HashMap::new());
        }
        
        let mut query_builder = QueryBuilder::new("SELECT * FROM product_details WHERE url IN (");
        let mut separated = query_builder.separated(", ");
        
        for url in urls {
            separated.push_bind(url);
        }
        
        query_builder.push(")");
        
        let details: Vec<ProductDetail> = query_builder
            .build_query_as::<ProductDetail>()
            .fetch_all(&mut **tx)
            .await?;
            
        Ok(details.into_iter().map(|d| (d.url.clone(), d)).collect())
    }

    // 제품 상세 정보 업데이트
    async fn update_product_detail(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        product: &MatterProduct
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE product_details SET
                page_id = ?,
                index_in_page = ?,
                id = ?,
                manufacturer = ?,
                model = ?,
                device_type = ?,
                certification_id = ?,
                certification_date = ?,
                software_version = ?,
                hardware_version = ?,
                vid = ?,
                pid = ?,
                family_sku = ?,
                family_variant_sku = ?,
                firmware_version = ?,
                family_id = ?,
                tis_trp_tested = ?,
                specification_version = ?,
                transport_interface = ?,
                primary_device_type_id = ?,
                application_categories = ?,
                updated_at = ?
            WHERE url = ?
            "#,
            product.page_id,
            product.index_in_page,
            product.id,
            product.manufacturer,
            product.model,
            product.device_type,
            product.certificate_id,
            product.certification_date,
            product.software_version,
            product.hardware_version,
            product.vid.as_ref().and_then(|v| self.parse_hex_to_int(v)),
            product.pid.as_ref().and_then(|v| self.parse_hex_to_int(v)),
            product.family_sku,
            product.family_variant_sku,
            product.firmware_version,
            product.family_id,
            product.tis_trp_tested,
            product.specification_version,
            product.transport_interface,
            product.primary_device_type_id,
            serde_json::to_string(&product.application_categories).ok(),
            Utc::now(),
            product.url
        )
        .execute(&mut **tx)
        .await?;
        
        Ok(())
    }

    // 제품 상세 정보 삽입
    async fn insert_product_detail(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        product: &MatterProduct
    ) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO product_details (
                url, page_id, index_in_page, id, manufacturer, model, device_type,
                certification_id, certification_date, software_version, hardware_version,
                vid, pid, family_sku, family_variant_sku, firmware_version, family_id,
                tis_trp_tested, specification_version, transport_interface,
                primary_device_type_id, application_categories, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            product.url,
            product.page_id,
            product.index_in_page,
            product.id,
            product.manufacturer,
            product.model,
            product.device_type,
            product.certificate_id,
            product.certification_date,
            product.software_version,
            product.hardware_version,
            product.vid.as_ref().and_then(|v| self.parse_hex_to_int(v)),
            product.pid.as_ref().and_then(|v| self.parse_hex_to_int(v)),
            product.family_sku,
            product.family_variant_sku,
            product.firmware_version,
            product.family_id,
            product.tis_trp_tested,
            product.specification_version,
            product.transport_interface,
            product.primary_device_type_id,
            serde_json::to_string(&product.application_categories).ok(),
            Utc::now(),
            Utc::now()
        )
        .execute(&mut **tx)
        .await?;
        
        Ok(())
    }

    // 저장 결과 병합
    fn merge_save_results(&self, basic: BatchSaveResult, detail: BatchSaveResult) -> BatchSaveResult {
        BatchSaveResult {
            added: detail.added,      // 상세 정보 기준
            updated: detail.updated,
            unchanged: detail.unchanged,
            failed: basic.failed + detail.failed,
            duplicate_info: detail.duplicate_info,
        }
    }
}

impl Default for BatchSaveResult {
    fn default() -> Self {
        Self {
            added: 0,
            updated: 0,
            unchanged: 0,
            failed: 0,
            duplicate_info: Vec::new(),
        }
    }
}
```

### 5.2.2 배치 단위 삭제 및 정리 서비스

```rust
// src/services/product_cleanup_service.rs
use crate::database::{entities::*, DatabaseManager};
use sqlx::{Sqlite, Transaction};
use anyhow::Result;

pub struct ProductCleanupService {
    db: DatabaseManager,
}

#[derive(Debug, Clone)]
pub struct CleanupResult {
    pub deleted_products: usize,
    pub deleted_details: usize,
    pub pages_cleaned: Vec<i32>,
}

impl ProductCleanupService {
    pub fn new(db: DatabaseManager) -> Self {
        Self { db }
    }

    // 페이지 범위별 삭제
    pub async fn delete_by_page_range(&self, start_page: i32, end_page: i32) -> Result<CleanupResult> {
        let mut tx = self.db.pool().begin().await?;
        
        // 삭제할 페이지 목록 수집
        let pages: Vec<i32> = sqlx::query_scalar!(
            "SELECT DISTINCT page_id FROM products WHERE page_id >= ? AND page_id <= ? AND page_id IS NOT NULL",
            start_page,
            end_page
        )
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .filter_map(|p| p)
        .collect();

        // product_details 삭제 (외래키 제약으로 먼저 삭제)
        let deleted_details = sqlx::query!(
            "DELETE FROM product_details WHERE page_id >= ? AND page_id <= ?",
            start_page,
            end_page
        )
        .execute(&mut *tx)
        .await?
        .rows_affected() as usize;

        // products 삭제
        let deleted_products = sqlx::query!(
            "DELETE FROM products WHERE page_id >= ? AND page_id <= ?",
            start_page,
            end_page
        )
        .execute(&mut *tx)
        .await?
        .rows_affected() as usize;

        tx.commit().await?;

        Ok(CleanupResult {
            deleted_products,
            deleted_details,
            pages_cleaned: pages,
        })
    }

    // 중복 URL 정리
    pub async fn cleanup_duplicate_urls(&self) -> Result<CleanupResult> {
        let mut tx = self.db.pool().begin().await?;
        
        // 중복 URL 찾기 (가장 최근 updated_at 기준으로 유지)
        let duplicate_urls: Vec<String> = sqlx::query_scalar!(
            r#"
            SELECT url FROM (
                SELECT url, ROW_NUMBER() OVER (PARTITION BY url ORDER BY updated_at DESC) as rn
                FROM products
            ) WHERE rn > 1
            "#
        )
        .fetch_all(&mut *tx)
        .await?;

        if duplicate_urls.is_empty() {
            return Ok(CleanupResult::default());
        }

        // 오래된 중복 항목 삭제
        let deleted_details = sqlx::query!(
            r#"
            DELETE FROM product_details WHERE url IN (
                SELECT url FROM (
                    SELECT url, ROW_NUMBER() OVER (PARTITION BY url ORDER BY updated_at DESC) as rn
                    FROM product_details
                ) WHERE rn > 1
            )
            "#
        )
        .execute(&mut *tx)
        .await?
        .rows_affected() as usize;

        let deleted_products = sqlx::query!(
            r#"
            DELETE FROM products WHERE url IN (
                SELECT url FROM (
                    SELECT url, ROW_NUMBER() OVER (PARTITION BY url ORDER BY updated_at DESC) as rn
                    FROM products
                ) WHERE rn > 1
            )
            "#
        )
        .execute(&mut *tx)
        .await?
        .rows_affected() as usize;

        tx.commit().await?;

        Ok(CleanupResult {
            deleted_products,
            deleted_details,
            pages_cleaned: Vec::new(),
        })
    }

    // NULL page_id 정리
    pub async fn cleanup_null_page_ids(&self) -> Result<CleanupResult> {
        let mut tx = self.db.pool().begin().await?;
        
        let deleted_details = sqlx::query!(
            "DELETE FROM product_details WHERE page_id IS NULL"
        )
        .execute(&mut *tx)
        .await?
        .rows_affected() as usize;

        let deleted_products = sqlx::query!(
            "DELETE FROM products WHERE page_id IS NULL"
        )
        .execute(&mut *tx)
        .await?
        .rows_affected() as usize;

        tx.commit().await?;

        Ok(CleanupResult {
            deleted_products,
            deleted_details,
            pages_cleaned: Vec::new(),
        })
    }

    // 배치 진행 상태 정리 (완료된 세션)
    pub async fn cleanup_completed_batch_progress(&self, days_to_keep: i64) -> Result<usize> {
        let cutoff_date = chrono::Utc::now() - chrono::Duration::days(days_to_keep);
        
        let deleted = sqlx::query!(
            "DELETE FROM batch_progress WHERE status = 'completed' AND end_time < ?",
            cutoff_date
        )
        .execute(self.db.pool())
        .await?
        .rows_affected() as usize;

        Ok(deleted)
    }
}

impl Default for CleanupResult {
    fn default() -> Self {
        Self {
            deleted_products: 0,
            deleted_details: 0,
            pages_cleaned: Vec::new(),
        }
    }
}
```

이번 Chapter 5의 첫 번째 섹션에서는 Rust/Tauri 환경에서의 데이터베이스 스키마 정의, 강타입 엔티티 모델, 그리고 배치 단위의 효율적인 저장 및 업데이트 로직을 다뤘습니다. 

다음 섹션들에서는 배치 진행 상태 영속화, 트랜잭션 관리 및 에러 복구, 그리고 성능 최적화에 대해 다루겠습니다. 다음 섹션을 생성할까요?
