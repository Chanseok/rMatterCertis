# Matter Certis v2.0 배치 크롤링 시스템: Chapter 5.2 - 배치 처리 및 저장 로직

## 목차
- [5.2.1 배치 단위 저장 서비스](#521-배치-단위-저장-서비스)
- [5.2.2 배치 업데이트 및 충돌 해결](#522-배치-업데이트-및-충돌-해결)
- [5.2.3 중복 데이터 정리 서비스](#523-중복-데이터-정리-서비스)
- [5.2.4 데이터 검증 및 무결성 보장](#524-데이터-검증-및-무결성-보장)

---

## 5.2.1 배치 단위 저장 서비스

### Rust/Tauri 배치 저장 아키텍처

TypeScript/Electron의 배치 처리를 Rust의 강타입 시스템과 병렬 처리 능력을 활용해 재구현합니다.

```rust
// src/database/services/batch_service.rs
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use sqlx::{Pool, Sqlite, Transaction};
use crate::database::entities::{Product, ProductDetail, BatchProgress};
use crate::errors::{BatchError, Result};

#[derive(Clone)]
pub struct BatchService {
    pool: Pool<Sqlite>,
    progress_tracker: Arc<RwLock<BatchProgress>>,
    batch_config: BatchConfig,
}

#[derive(Debug, Clone)]
pub struct BatchConfig {
    pub batch_size: usize,
    pub max_retry_attempts: usize,
    pub retry_delay_ms: u64,
    pub transaction_timeout_ms: u64,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            max_retry_attempts: 3,
            retry_delay_ms: 1000,
            transaction_timeout_ms: 30000,
        }
    }
}

impl BatchService {
    pub fn new(pool: Pool<Sqlite>, config: BatchConfig) -> Self {
        Self {
            pool,
            progress_tracker: Arc::new(RwLock::new(BatchProgress::new())),
            batch_config: config,
        }
    }

    /// 제품 리스트를 배치 단위로 저장
    pub async fn save_products_batch(
        &self,
        products: Vec<Product>,
        batch_id: &str,
    ) -> Result<BatchSaveResult> {
        let chunks: Vec<_> = products
            .chunks(self.batch_config.batch_size)
            .collect();

        let mut results = Vec::new();
        let mut total_saved = 0;
        let mut total_failed = 0;

        for (chunk_index, chunk) in chunks.iter().enumerate() {
            match self.save_single_batch(chunk, batch_id, chunk_index).await {
                Ok(result) => {
                    total_saved += result.saved_count;
                    results.push(result);
                }
                Err(e) => {
                    total_failed += chunk.len();
                    log::error!("Batch {} chunk {} failed: {}", batch_id, chunk_index, e);
                    
                    // 재시도 로직
                    if let Ok(retry_result) = self.retry_failed_batch(chunk, batch_id, chunk_index).await {
                        total_saved += retry_result.saved_count;
                        total_failed -= retry_result.saved_count;
                        results.push(retry_result);
                    }
                }
            }

            // 진행 상태 업데이트
            self.update_progress(batch_id, chunk_index + 1, chunks.len()).await?;
        }

        Ok(BatchSaveResult {
            batch_id: batch_id.to_string(),
            total_processed: products.len(),
            total_saved,
            total_failed,
            chunk_results: results,
        })
    }

    /// 단일 배치 청크 저장
    async fn save_single_batch(
        &self,
        products: &[Product],
        batch_id: &str,
        chunk_index: usize,
    ) -> Result<ChunkSaveResult> {
        let mut tx = self.pool.begin().await
            .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        let mut saved_count = 0;
        let mut failed_urls = Vec::new();

        for product in products {
            match self.insert_or_update_product(&mut tx, product).await {
                Ok(_) => saved_count += 1,
                Err(e) => {
                    log::warn!("Failed to save product {}: {}", product.url, e);
                    failed_urls.push(product.url.clone());
                }
            }
        }

        // 트랜잭션 커밋
        tx.commit().await
            .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        log::info!(
            "Batch {} chunk {} saved: {}/{} products",
            batch_id, chunk_index, saved_count, products.len()
        );

        Ok(ChunkSaveResult {
            chunk_index,
            saved_count,
            failed_count: failed_urls.len(),
            failed_urls,
        })
    }

    /// UPSERT 로직: 제품 데이터 삽입 또는 업데이트
    async fn insert_or_update_product(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        product: &Product,
    ) -> Result<()> {
        // 중복 체크 및 업데이트 결정
        let existing = sqlx::query_as!(
            Product,
            "SELECT * FROM products WHERE url = ?",
            product.url
        )
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        if let Some(existing_product) = existing {
            // 업데이트가 필요한지 확인
            if self.should_update_product(&existing_product, product) {
                self.update_existing_product(tx, product).await?;
            }
        } else {
            // 새로운 제품 삽입
            self.insert_new_product(tx, product).await?;
        }

        Ok(())
    }

    /// 새 제품 데이터 삽입
    async fn insert_new_product(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        product: &Product,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO products (
                url, manufacturer, model, certificate_id, 
                page_id, index_in_page, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            product.url,
            product.manufacturer,
            product.model,
            product.certificate_id,
            product.page_id,
            product.index_in_page,
            product.created_at,
            product.updated_at
        )
        .execute(&mut **tx)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// 기존 제품 데이터 업데이트
    async fn update_existing_product(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        product: &Product,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE products SET 
                manufacturer = ?, model = ?, certificate_id = ?,
                page_id = ?, index_in_page = ?, updated_at = ?
            WHERE url = ?
            "#,
            product.manufacturer,
            product.model,
            product.certificate_id,
            product.page_id,
            product.index_in_page,
            product.updated_at,
            product.url
        )
        .execute(&mut **tx)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// 업데이트 필요성 판단
    fn should_update_product(&self, existing: &Product, new: &Product) -> bool {
        existing.manufacturer != new.manufacturer
            || existing.model != new.model
            || existing.certificate_id != new.certificate_id
            || existing.page_id != new.page_id
            || existing.index_in_page != new.index_in_page
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchSaveResult {
    pub batch_id: String,
    pub total_processed: usize,
    pub total_saved: usize,
    pub total_failed: usize,
    pub chunk_results: Vec<ChunkSaveResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChunkSaveResult {
    pub chunk_index: usize,
    pub saved_count: usize,
    pub failed_count: usize,
    pub failed_urls: Vec<String>,
}
```

---

## 5.2.2 배치 업데이트 및 충돌 해결

### 병렬 처리 및 충돌 방지

```rust
// src/database/services/conflict_resolver.rs
use std::collections::HashMap;
use tokio::sync::Semaphore;
use std::sync::Arc;

#[derive(Clone)]
pub struct ConflictResolver {
    pool: Pool<Sqlite>,
    semaphore: Arc<Semaphore>, // 동시성 제어
    conflict_strategy: ConflictStrategy,
}

#[derive(Debug, Clone)]
pub enum ConflictStrategy {
    LatestWins,           // 최신 데이터가 우선
    MergeFields,          // 필드별 병합
    PreserveExisting,     // 기존 데이터 유지
    PromptUser,           // 사용자 선택
}

impl ConflictResolver {
    pub fn new(pool: Pool<Sqlite>, max_concurrent: usize, strategy: ConflictStrategy) -> Self {
        Self {
            pool,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            conflict_strategy: strategy,
        }
    }

    /// 배치 업데이트와 충돌 해결
    pub async fn resolve_and_update_batch(
        &self,
        updates: Vec<Product>,
        batch_id: &str,
    ) -> Result<ConflictResolutionResult> {
        let mut resolved_updates = Vec::new();
        let mut conflicts = Vec::new();
        let mut resolution_stats = ConflictStats::new();

        // 동시성 제어를 위한 세마포어 사용
        let tasks: Vec<_> = updates
            .into_iter()
            .map(|product| {
                let resolver = self.clone();
                let batch_id = batch_id.to_string();
                tokio::spawn(async move {
                    let _permit = resolver.semaphore.acquire().await.unwrap();
                    resolver.resolve_single_conflict(product, &batch_id).await
                })
            })
            .collect();

        // 모든 태스크 완료 대기
        for task in tasks {
            match task.await.unwrap() {
                Ok(resolution) => {
                    match resolution.action {
                        ConflictAction::Update(product) => {
                            resolved_updates.push(product);
                            resolution_stats.resolved_count += 1;
                        }
                        ConflictAction::Skip => {
                            resolution_stats.skipped_count += 1;
                        }
                        ConflictAction::Conflict(conflict) => {
                            conflicts.push(conflict);
                            resolution_stats.conflict_count += 1;
                        }
                    }
                }
                Err(e) => {
                    log::error!("Conflict resolution failed: {}", e);
                    resolution_stats.error_count += 1;
                }
            }
        }

        // 해결된 업데이트 적용
        if !resolved_updates.is_empty() {
            self.apply_resolved_updates(resolved_updates, batch_id).await?;
        }

        Ok(ConflictResolutionResult {
            batch_id: batch_id.to_string(),
            stats: resolution_stats,
            unresolved_conflicts: conflicts,
        })
    }

    /// 단일 충돌 해결
    async fn resolve_single_conflict(
        &self,
        new_product: Product,
        batch_id: &str,
    ) -> Result<ConflictResolution> {
        // 기존 데이터 조회
        let existing = sqlx::query_as!(
            Product,
            "SELECT * FROM products WHERE url = ?",
            new_product.url
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        let Some(existing_product) = existing else {
            // 신규 데이터이므로 충돌 없음
            return Ok(ConflictResolution {
                url: new_product.url.clone(),
                action: ConflictAction::Update(new_product),
            });
        };

        // 충돌 해결 전략 적용
        let action = match self.conflict_strategy {
            ConflictStrategy::LatestWins => {
                if new_product.updated_at > existing_product.updated_at {
                    ConflictAction::Update(new_product)
                } else {
                    ConflictAction::Skip
                }
            }
            ConflictStrategy::MergeFields => {
                let merged = self.merge_product_fields(&existing_product, &new_product);
                ConflictAction::Update(merged)
            }
            ConflictStrategy::PreserveExisting => {
                ConflictAction::Skip
            }
            ConflictStrategy::PromptUser => {
                ConflictAction::Conflict(ProductConflict {
                    url: new_product.url.clone(),
                    existing: existing_product,
                    incoming: new_product,
                    conflict_fields: self.identify_conflict_fields(&existing_product, &new_product),
                })
            }
        };

        Ok(ConflictResolution {
            url: new_product.url,
            action,
        })
    }

    /// 필드별 병합 로직
    fn merge_product_fields(&self, existing: &Product, new: &Product) -> Product {
        Product {
            url: existing.url.clone(),
            manufacturer: new.manufacturer.clone().or(existing.manufacturer.clone()),
            model: new.model.clone().or(existing.model.clone()),
            certificate_id: new.certificate_id.clone().or(existing.certificate_id.clone()),
            page_id: new.page_id.or(existing.page_id),
            index_in_page: new.index_in_page.or(existing.index_in_page),
            created_at: existing.created_at,
            updated_at: new.updated_at.max(existing.updated_at),
        }
    }

    /// 충돌 필드 식별
    fn identify_conflict_fields(&self, existing: &Product, new: &Product) -> Vec<String> {
        let mut conflicts = Vec::new();

        if existing.manufacturer != new.manufacturer && 
           existing.manufacturer.is_some() && new.manufacturer.is_some() {
            conflicts.push("manufacturer".to_string());
        }
        if existing.model != new.model && 
           existing.model.is_some() && new.model.is_some() {
            conflicts.push("model".to_string());
        }
        if existing.certificate_id != new.certificate_id && 
           existing.certificate_id.is_some() && new.certificate_id.is_some() {
            conflicts.push("certificate_id".to_string());
        }

        conflicts
    }

    /// 해결된 업데이트 적용
    async fn apply_resolved_updates(
        &self,
        updates: Vec<Product>,
        batch_id: &str,
    ) -> Result<()> {
        let batch_service = BatchService::new(self.pool.clone(), BatchConfig::default());
        batch_service.save_products_batch(updates, batch_id).await?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct ConflictResolution {
    pub url: String,
    pub action: ConflictAction,
}

#[derive(Debug)]
pub enum ConflictAction {
    Update(Product),
    Skip,
    Conflict(ProductConflict),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProductConflict {
    pub url: String,
    pub existing: Product,
    pub incoming: Product,
    pub conflict_fields: Vec<String>,
}

#[derive(Debug, Default)]
pub struct ConflictStats {
    pub resolved_count: usize,
    pub skipped_count: usize,
    pub conflict_count: usize,
    pub error_count: usize,
}

#[derive(Debug)]
pub struct ConflictResolutionResult {
    pub batch_id: String,
    pub stats: ConflictStats,
    pub unresolved_conflicts: Vec<ProductConflict>,
}
```

---

## 5.2.3 중복 데이터 정리 서비스

### 스마트 중복 제거 및 데이터 정규화

```rust
// src/database/services/deduplication_service.rs
use std::collections::{HashMap, HashSet};
use regex::Regex;
use fuzzy_matcher::{FuzzyMatcher, SkimMatcher};

#[derive(Clone)]
pub struct DeduplicationService {
    pool: Pool<Sqlite>,
    similarity_threshold: f64,
    batch_size: usize,
}

impl DeduplicationService {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self {
            pool,
            similarity_threshold: 0.85, // 85% 유사도 기준
            batch_size: 1000,
        }
    }

    /// 전체 데이터베이스 중복 제거
    pub async fn deduplicate_all_products(&self) -> Result<DeduplicationResult> {
        let mut result = DeduplicationResult::new();

        // 1단계: URL 기반 정확한 중복 제거
        let exact_duplicates = self.find_exact_url_duplicates().await?;
        if !exact_duplicates.is_empty() {
            let removed = self.remove_exact_duplicates(exact_duplicates).await?;
            result.exact_duplicates_removed = removed;
        }

        // 2단계: 유사도 기반 중복 제거
        let similar_groups = self.find_similar_products().await?;
        if !similar_groups.is_empty() {
            let merged = self.merge_similar_products(similar_groups).await?;
            result.similar_products_merged = merged;
        }

        // 3단계: 불완전한 데이터 정리
        let incomplete_cleaned = self.clean_incomplete_products().await?;
        result.incomplete_products_cleaned = incomplete_cleaned;

        // 4단계: 데이터 정규화
        let normalized = self.normalize_product_data().await?;
        result.products_normalized = normalized;

        Ok(result)
    }

    /// URL 기반 정확한 중복 찾기
    async fn find_exact_url_duplicates(&self) -> Result<Vec<DuplicateGroup>> {
        let duplicates = sqlx::query!(
            r#"
            SELECT url, COUNT(*) as count
            FROM products 
            GROUP BY url 
            HAVING COUNT(*) > 1
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        let mut duplicate_groups = Vec::new();

        for duplicate in duplicates {
            let products = sqlx::query_as!(
                Product,
                "SELECT * FROM products WHERE url = ? ORDER BY updated_at DESC",
                duplicate.url
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

            if products.len() > 1 {
                duplicate_groups.push(DuplicateGroup {
                    primary: products[0].clone(),
                    duplicates: products[1..].to_vec(),
                    similarity_score: 1.0, // 정확한 중복
                });
            }
        }

        Ok(duplicate_groups)
    }

    /// 유사한 제품 찾기 (퍼지 매칭)
    async fn find_similar_products(&self) -> Result<Vec<SimilarGroup>> {
        let all_products = sqlx::query_as!(
            Product,
            "SELECT * FROM products ORDER BY url"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        let mut similar_groups = Vec::new();
        let mut processed_urls = HashSet::new();
        let matcher = SkimMatcher::default();

        for (i, product) in all_products.iter().enumerate() {
            if processed_urls.contains(&product.url) {
                continue;
            }

            let mut similar_products = vec![product.clone()];
            processed_urls.insert(product.url.clone());

            // 나머지 제품들과 유사도 비교
            for other_product in &all_products[i + 1..] {
                if processed_urls.contains(&other_product.url) {
                    continue;
                }

                let similarity = self.calculate_product_similarity(
                    product,
                    other_product,
                    &matcher,
                ).await;

                if similarity >= self.similarity_threshold {
                    similar_products.push(other_product.clone());
                    processed_urls.insert(other_product.url.clone());
                }
            }

            if similar_products.len() > 1 {
                similar_groups.push(SimilarGroup {
                    products: similar_products,
                    average_similarity: self.similarity_threshold,
                });
            }
        }

        Ok(similar_groups)
    }

    /// 제품 간 유사도 계산
    async fn calculate_product_similarity(
        &self,
        product1: &Product,
        product2: &Product,
        matcher: &SkimMatcher,
    ) -> f64 {
        let mut similarity_scores = Vec::new();

        // URL 유사도 (도메인 제외)
        if let (Some(path1), Some(path2)) = (
            self.extract_url_path(&product1.url),
            self.extract_url_path(&product2.url),
        ) {
            if let Some(score) = matcher.fuzzy_match(&path1, &path2) {
                similarity_scores.push(score as f64 / 100.0);
            }
        }

        // 제조사 유사도
        if let (Some(mfg1), Some(mfg2)) = (&product1.manufacturer, &product2.manufacturer) {
            if let Some(score) = matcher.fuzzy_match(mfg1, mfg2) {
                similarity_scores.push(score as f64 / 100.0);
            }
        }

        // 모델명 유사도
        if let (Some(model1), Some(model2)) = (&product1.model, &product2.model) {
            if let Some(score) = matcher.fuzzy_match(model1, model2) {
                similarity_scores.push(score as f64 / 100.0);
            }
        }

        // 인증 ID 유사도
        if let (Some(cert1), Some(cert2)) = (&product1.certificate_id, &product2.certificate_id) {
            if cert1 == cert2 {
                similarity_scores.push(1.0);
            } else if let Some(score) = matcher.fuzzy_match(cert1, cert2) {
                similarity_scores.push(score as f64 / 100.0);
            }
        }

        // 가중 평균 계산
        if similarity_scores.is_empty() {
            0.0
        } else {
            similarity_scores.iter().sum::<f64>() / similarity_scores.len() as f64
        }
    }

    /// URL에서 경로 추출
    fn extract_url_path(&self, url: &str) -> Option<String> {
        if let Ok(parsed) = url::Url::parse(url) {
            Some(parsed.path().to_string())
        } else {
            None
        }
    }

    /// 정확한 중복 제거
    async fn remove_exact_duplicates(&self, duplicates: Vec<DuplicateGroup>) -> Result<usize> {
        let mut removed_count = 0;
        let mut tx = self.pool.begin().await
            .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        for group in duplicates {
            // 가장 최신 데이터를 제외하고 나머지 삭제
            for duplicate in group.duplicates {
                sqlx::query!(
                    "DELETE FROM products WHERE url = ? AND created_at = ?",
                    duplicate.url,
                    duplicate.created_at
                )
                .execute(&mut *tx)
                .await
                .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

                removed_count += 1;
            }
        }

        tx.commit().await
            .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        log::info!("Removed {} exact duplicates", removed_count);
        Ok(removed_count)
    }

    /// 유사한 제품 병합
    async fn merge_similar_products(&self, similar_groups: Vec<SimilarGroup>) -> Result<usize> {
        let mut merged_count = 0;
        let mut tx = self.pool.begin().await
            .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        for group in similar_groups {
            if group.products.len() < 2 {
                continue;
            }

            // 가장 완전한 데이터를 기준으로 병합
            let master_product = self.select_master_product(&group.products);
            let merged_product = self.merge_product_data(&group.products, &master_product);

            // 마스터 제품 업데이트
            self.update_existing_product(&mut tx, &merged_product).await?;

            // 나머지 제품들 삭제
            for product in &group.products {
                if product.url != master_product.url {
                    sqlx::query!(
                        "DELETE FROM products WHERE url = ?",
                        product.url
                    )
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

                    merged_count += 1;
                }
            }
        }

        tx.commit().await
            .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        log::info!("Merged {} similar products", merged_count);
        Ok(merged_count)
    }

    /// 마스터 제품 선택 (가장 완전한 데이터)
    fn select_master_product(&self, products: &[Product]) -> Product {
        products
            .iter()
            .max_by_key(|p| {
                let mut score = 0;
                if p.manufacturer.is_some() { score += 1; }
                if p.model.is_some() { score += 1; }
                if p.certificate_id.is_some() { score += 1; }
                if p.page_id.is_some() { score += 1; }
                if p.index_in_page.is_some() { score += 1; }
                score
            })
            .unwrap()
            .clone()
    }

    /// 제품 데이터 병합
    fn merge_product_data(&self, products: &[Product], master: &Product) -> Product {
        let mut merged = master.clone();

        // 각 필드에서 가장 완전한 데이터 선택
        for product in products {
            if merged.manufacturer.is_none() && product.manufacturer.is_some() {
                merged.manufacturer = product.manufacturer.clone();
            }
            if merged.model.is_none() && product.model.is_some() {
                merged.model = product.model.clone();
            }
            if merged.certificate_id.is_none() && product.certificate_id.is_some() {
                merged.certificate_id = product.certificate_id.clone();
            }
            if merged.page_id.is_none() && product.page_id.is_some() {
                merged.page_id = product.page_id;
            }
            if merged.index_in_page.is_none() && product.index_in_page.is_some() {
                merged.index_in_page = product.index_in_page;
            }
        }

        merged.updated_at = chrono::Utc::now();
        merged
    }

    /// 불완전한 제품 데이터 정리
    async fn clean_incomplete_products(&self) -> Result<usize> {
        let cleaned = sqlx::query!(
            r#"
            DELETE FROM products 
            WHERE manufacturer IS NULL 
            AND model IS NULL 
            AND certificate_id IS NULL
            "#
        )
        .execute(&self.pool)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        log::info!("Cleaned {} incomplete products", cleaned.rows_affected());
        Ok(cleaned.rows_affected() as usize)
    }

    /// 데이터 정규화
    async fn normalize_product_data(&self) -> Result<usize> {
        let products = sqlx::query_as!(
            Product,
            "SELECT * FROM products"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        let mut normalized_count = 0;
        let mut tx = self.pool.begin().await
            .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        for product in products {
            let mut updated = false;
            let mut normalized_product = product.clone();

            // 제조사명 정규화
            if let Some(manufacturer) = &product.manufacturer {
                let normalized_mfg = self.normalize_manufacturer_name(manufacturer);
                if normalized_mfg != *manufacturer {
                    normalized_product.manufacturer = Some(normalized_mfg);
                    updated = true;
                }
            }

            // 모델명 정규화
            if let Some(model) = &product.model {
                let normalized_model = self.normalize_model_name(model);
                if normalized_model != *model {
                    normalized_product.model = Some(normalized_model);
                    updated = true;
                }
            }

            if updated {
                self.update_existing_product(&mut tx, &normalized_product).await?;
                normalized_count += 1;
            }
        }

        tx.commit().await
            .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        log::info!("Normalized {} products", normalized_count);
        Ok(normalized_count)
    }

    /// 제조사명 정규화
    fn normalize_manufacturer_name(&self, name: &str) -> String {
        // 공통 약어 및 변형 정규화
        let normalized = name
            .trim()
            .replace("Corp.", "Corporation")
            .replace("Inc.", "Incorporated")
            .replace("Ltd.", "Limited")
            .replace("Co.", "Company");

        // 대소문자 정규화
        self.title_case(&normalized)
    }

    /// 모델명 정규화
    fn normalize_model_name(&self, name: &str) -> String {
        // 불필요한 공백 제거 및 정규화
        let re = Regex::new(r"\s+").unwrap();
        re.replace_all(name.trim(), " ").to_string()
    }

    /// 타이틀 케이스 변환
    fn title_case(&self, text: &str) -> String {
        text.split_whitespace()
            .map(|word| {
                if word.is_empty() {
                    String::new()
                } else {
                    format!("{}{}", 
                        word.chars().next().unwrap().to_uppercase(),
                        word.chars().skip(1).collect::<String>().to_lowercase()
                    )
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Debug)]
pub struct DuplicateGroup {
    pub primary: Product,
    pub duplicates: Vec<Product>,
    pub similarity_score: f64,
}

#[derive(Debug)]
pub struct SimilarGroup {
    pub products: Vec<Product>,
    pub average_similarity: f64,
}

#[derive(Debug, Default)]
pub struct DeduplicationResult {
    pub exact_duplicates_removed: usize,
    pub similar_products_merged: usize,
    pub incomplete_products_cleaned: usize,
    pub products_normalized: usize,
}
```

---

## 5.2.4 데이터 검증 및 무결성 보장

### 데이터 품질 검증 시스템

```rust
// src/database/services/validation_service.rs
use validator::{Validate, ValidationError};
use url::Url;

#[derive(Clone)]
pub struct ValidationService {
    pool: Pool<Sqlite>,
    validation_rules: ValidationRules,
}

#[derive(Debug, Clone)]
pub struct ValidationRules {
    pub require_manufacturer: bool,
    pub require_model: bool,
    pub require_certificate_id: bool,
    pub max_url_length: usize,
    pub max_text_field_length: usize,
    pub allowed_domains: Option<Vec<String>>,
}

impl Default for ValidationRules {
    fn default() -> Self {
        Self {
            require_manufacturer: false,
            require_model: false,
            require_certificate_id: false,
            max_url_length: 2048,
            max_text_field_length: 500,
            allowed_domains: None,
        }
    }
}

impl ValidationService {
    pub fn new(pool: Pool<Sqlite>, rules: ValidationRules) -> Self {
        Self {
            pool,
            validation_rules: rules,
        }
    }

    /// 배치 데이터 검증
    pub async fn validate_batch(
        &self,
        products: &[Product],
        batch_id: &str,
    ) -> Result<ValidationReport> {
        let mut report = ValidationReport::new(batch_id.to_string());
        
        for (index, product) in products.iter().enumerate() {
            match self.validate_single_product(product).await {
                Ok(_) => report.valid_count += 1,
                Err(errors) => {
                    report.invalid_count += 1;
                    report.validation_errors.push(ProductValidationError {
                        index,
                        url: product.url.clone(),
                        errors,
                    });
                }
            }
        }

        // 중복 검사
        let duplicates = self.find_batch_duplicates(products).await?;
        report.duplicate_count = duplicates.len();
        report.duplicates = duplicates;

        // 참조 무결성 검사
        let orphaned = self.find_orphaned_details(products).await?;
        report.orphaned_details = orphaned;

        Ok(report)
    }

    /// 단일 제품 검증
    async fn validate_single_product(&self, product: &Product) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // URL 검증
        if let Err(e) = self.validate_url(&product.url) {
            errors.push(format!("Invalid URL: {}", e));
        }

        // 필수 필드 검증
        if self.validation_rules.require_manufacturer && product.manufacturer.is_none() {
            errors.push("Manufacturer is required".to_string());
        }

        if self.validation_rules.require_model && product.model.is_none() {
            errors.push("Model is required".to_string());
        }

        if self.validation_rules.require_certificate_id && product.certificate_id.is_none() {
            errors.push("Certificate ID is required".to_string());
        }

        // 텍스트 필드 길이 검증
        if let Some(manufacturer) = &product.manufacturer {
            if manufacturer.len() > self.validation_rules.max_text_field_length {
                errors.push(format!(
                    "Manufacturer name too long: {} > {}",
                    manufacturer.len(),
                    self.validation_rules.max_text_field_length
                ));
            }
        }

        if let Some(model) = &product.model {
            if model.len() > self.validation_rules.max_text_field_length {
                errors.push(format!(
                    "Model name too long: {} > {}",
                    model.len(),
                    self.validation_rules.max_text_field_length
                ));
            }
        }

        // 도메인 검증
        if let Some(allowed_domains) = &self.validation_rules.allowed_domains {
            if let Ok(url) = Url::parse(&product.url) {
                if let Some(domain) = url.host_str() {
                    if !allowed_domains.contains(&domain.to_string()) {
                        errors.push(format!("Domain not allowed: {}", domain));
                    }
                }
            }
        }

        // 데이터 일관성 검증
        if let Err(consistency_errors) = self.validate_data_consistency(product).await {
            errors.extend(consistency_errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// URL 검증
    fn validate_url(&self, url: &str) -> Result<(), String> {
        if url.len() > self.validation_rules.max_url_length {
            return Err(format!(
                "URL too long: {} > {}",
                url.len(),
                self.validation_rules.max_url_length
            ));
        }

        match Url::parse(url) {
            Ok(parsed_url) => {
                if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
                    Err("URL must use HTTP or HTTPS scheme".to_string())
                } else {
                    Ok(())
                }
            }
            Err(e) => Err(format!("Invalid URL format: {}", e)),
        }
    }

    /// 데이터 일관성 검증
    async fn validate_data_consistency(&self, product: &Product) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // 페이지 ID와 인덱스 일관성
        match (product.page_id, product.index_in_page) {
            (Some(page_id), Some(index)) => {
                if page_id < 0 {
                    errors.push("Page ID cannot be negative".to_string());
                }
                if index < 0 {
                    errors.push("Index in page cannot be negative".to_string());
                }
            }
            (Some(_), None) => {
                errors.push("Index in page required when page ID is provided".to_string());
            }
            (None, Some(_)) => {
                errors.push("Page ID required when index in page is provided".to_string());
            }
            (None, None) => {} // 둘 다 없는 것은 허용
        }

        // 인증 ID 형식 검증
        if let Some(cert_id) = &product.certificate_id {
            if !self.is_valid_certificate_id(cert_id) {
                errors.push("Invalid certificate ID format".to_string());
            }
        }

        // 제조사-모델 조합 검증
        if let (Some(manufacturer), Some(model)) = (&product.manufacturer, &product.model) {
            if let Err(e) = self.validate_manufacturer_model_combination(manufacturer, model).await {
                errors.extend(e);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// 인증 ID 형식 검증
    fn is_valid_certificate_id(&self, cert_id: &str) -> bool {
        // 일반적인 인증 ID 패턴 (예: ABC-123456)
        let re = Regex::new(r"^[A-Z]{2,5}-\d{5,8}$").unwrap();
        re.is_match(cert_id)
    }

    /// 제조사-모델 조합 검증
    async fn validate_manufacturer_model_combination(
        &self,
        manufacturer: &str,
        model: &str,
    ) -> Result<(), Vec<String>> {
        // 기존 데이터베이스에서 동일한 조합 확인
        let existing_count = sqlx::query!(
            "SELECT COUNT(*) as count FROM products WHERE manufacturer = ? AND model = ?",
            manufacturer,
            model
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| vec![format!("Database validation error: {}", e)])?;

        // 동일한 조합이 너무 많으면 의심스러운 데이터
        if existing_count.count > 100 {
            Err(vec![format!(
                "Suspicious manufacturer-model combination: {} occurrences",
                existing_count.count
            )])
        } else {
            Ok(())
        }
    }

    /// 배치 내 중복 검사
    async fn find_batch_duplicates(&self, products: &[Product]) -> Result<Vec<DuplicateInfo>> {
        let mut url_counts = HashMap::new();
        let mut duplicates = Vec::new();

        // URL 카운트
        for (index, product) in products.iter().enumerate() {
            url_counts
                .entry(product.url.clone())
                .or_insert_with(Vec::new)
                .push(index);
        }

        // 중복 찾기
        for (url, indices) in url_counts {
            if indices.len() > 1 {
                duplicates.push(DuplicateInfo {
                    url,
                    indices,
                    count: indices.len(),
                });
            }
        }

        Ok(duplicates)
    }

    /// 고아 상세 정보 찾기
    async fn find_orphaned_details(&self, products: &[Product]) -> Result<Vec<String>> {
        let product_urls: HashSet<_> = products.iter().map(|p| &p.url).collect();
        let mut orphaned = Vec::new();

        // 데이터베이스의 상세 정보 중 부모 제품이 없는 것들 찾기
        let all_details = sqlx::query!(
            "SELECT url FROM product_details"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        for detail in all_details {
            if !product_urls.contains(&detail.url) {
                // 실제 products 테이블에도 없는지 확인
                let exists = sqlx::query!(
                    "SELECT COUNT(*) as count FROM products WHERE url = ?",
                    detail.url
                )
                .fetch_one(&self.pool)
                .await
                .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

                if exists.count == 0 {
                    orphaned.push(detail.url);
                }
            }
        }

        Ok(orphaned)
    }

    /// 데이터 정합성 복구
    pub async fn repair_data_integrity(&self) -> Result<IntegrityRepairResult> {
        let mut result = IntegrityRepairResult::new();

        // 1. 고아 상세 정보 삭제
        let orphaned_deleted = sqlx::query!(
            r#"
            DELETE FROM product_details 
            WHERE url NOT IN (SELECT url FROM products)
            "#
        )
        .execute(&self.pool)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        result.orphaned_details_removed = orphaned_deleted.rows_affected() as usize;

        // 2. NULL 값 정리
        let null_cleaned = self.clean_null_values().await?;
        result.null_values_cleaned = null_cleaned;

        // 3. 잘못된 날짜 수정
        let dates_fixed = self.fix_invalid_dates().await?;
        result.invalid_dates_fixed = dates_fixed;

        Ok(result)
    }

    /// NULL 값 정리
    async fn clean_null_values(&self) -> Result<usize> {
        let mut cleaned_count = 0;

        // 빈 문자열을 NULL로 변환
        let updated = sqlx::query!(
            r#"
            UPDATE products SET 
                manufacturer = CASE WHEN trim(manufacturer) = '' THEN NULL ELSE manufacturer END,
                model = CASE WHEN trim(model) = '' THEN NULL ELSE model END,
                certificate_id = CASE WHEN trim(certificate_id) = '' THEN NULL ELSE certificate_id END
            "#
        )
        .execute(&self.pool)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        cleaned_count += updated.rows_affected() as usize;
        Ok(cleaned_count)
    }

    /// 잘못된 날짜 수정
    async fn fix_invalid_dates(&self) -> Result<usize> {
        let now = chrono::Utc::now();
        let mut fixed_count = 0;

        // 미래 날짜를 현재 시간으로 수정
        let updated = sqlx::query!(
            r#"
            UPDATE products SET 
                created_at = ?,
                updated_at = ?
            WHERE created_at > ? OR updated_at > ?
            "#,
            now,
            now,
            now,
            now
        )
        .execute(&self.pool)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        fixed_count += updated.rows_affected() as usize;

        // created_at > updated_at인 경우 수정
        let fixed_order = sqlx::query!(
            r#"
            UPDATE products SET updated_at = created_at 
            WHERE created_at > updated_at
            "#
        )
        .execute(&self.pool)
        .await
        .map_err(|e| BatchError::DatabaseError(e.to_string()))?;

        fixed_count += fixed_order.rows_affected() as usize;
        Ok(fixed_count)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidationReport {
    pub batch_id: String,
    pub total_count: usize,
    pub valid_count: usize,
    pub invalid_count: usize,
    pub duplicate_count: usize,
    pub validation_errors: Vec<ProductValidationError>,
    pub duplicates: Vec<DuplicateInfo>,
    pub orphaned_details: Vec<String>,
}

impl ValidationReport {
    fn new(batch_id: String) -> Self {
        Self {
            batch_id,
            total_count: 0,
            valid_count: 0,
            invalid_count: 0,
            duplicate_count: 0,
            validation_errors: Vec::new(),
            duplicates: Vec::new(),
            orphaned_details: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProductValidationError {
    pub index: usize,
    pub url: String,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DuplicateInfo {
    pub url: String,
    pub indices: Vec<usize>,
    pub count: usize,
}

#[derive(Debug, Default)]
pub struct IntegrityRepairResult {
    pub orphaned_details_removed: usize,
    pub null_values_cleaned: usize,
    pub invalid_dates_fixed: usize,
}
```

---

## 마무리

Chapter 5.2에서는 Rust/Tauri 기반의 배치 처리 및 저장 시스템의 핵심 구성 요소들을 다뤘습니다:

### 주요 구현 내용

1. **배치 단위 저장 서비스**: 대용량 데이터를 효율적으로 처리하는 배치 시스템
2. **충돌 해결 메커니즘**: 동시성 제어와 데이터 충돌 방지
3. **중복 데이터 정리**: 스마트한 중복 제거 및 데이터 병합
4. **데이터 검증 시스템**: 포괄적인 데이터 품질 보장

### Rust의 장점 활용

- **메모리 안전성**: 소유권 시스템으로 안전한 동시성 처리
- **성능**: Zero-cost abstractions와 효율적인 메모리 관리
- **타입 안전성**: 컴파일 타임 오류 검출로 런타임 안정성 향상
- **병렬 처리**: Tokio를 활용한 비동기 배치 처리

다음 섹션에서는 배치 진행 상태 영속화와 트랜잭션 관리에 대해 다루겠습니다.
