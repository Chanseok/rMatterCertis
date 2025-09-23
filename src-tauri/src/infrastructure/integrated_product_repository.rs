//! Repository implementation for integrated product operations
//!
//! This module provides database operations that combine product and `product_detail`
//! tables to provide comprehensive product information with advanced search and
//! filtering capabilities.

#![allow(clippy::uninlined_format_args)]
#![allow(missing_docs)]
#![allow(clippy::unnecessary_operation)]
#![allow(unused_must_use)]

use crate::domain::integrated_product::DatabaseStatistics;
use crate::domain::product::{
    Product, ProductDetail, ProductSearchCriteria, ProductSearchResult, ProductWithDetails, Vendor,
};
use crate::domain::session_manager::CrawlingResult;
use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{Row, sqlite::SqlitePool};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info};
use super::write_lock_tracker; // for snapshot during tx begin

// Lightweight retry helper for transient SQLITE_BUSY / locked situations.
async fn retry_sqlite_busy<F, Fut, T>(label: &str, mut op: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut attempt = 0u32;
    let max_attempts = 5u32;
    let mut backoff = 80u64; // ms
    let started = Instant::now();
    loop {
        match op().await {
            Ok(v) => {
                let elapsed_ms = started.elapsed().as_millis() as u64;
                if attempt > 0 {
                    tracing::info!(target="persistence", label, attempts=attempt+1, elapsed_ms, "Operation succeeded after retries");
                } else {
                    tracing::debug!(target="persistence", label, elapsed_ms, "Operation succeeded (no retries)");
                }
                return Ok(v);
            }
            Err(e) => {
                let msg = format!("{e}");
                if (msg.contains("database is locked") || msg.contains("SQLITE_BUSY")) && attempt + 1 < max_attempts {
                    attempt += 1;
                    tracing::warn!(target="persistence", attempt, label, backoff_ms=backoff, "Retrying after SQLITE_BUSY");
                    tokio::time::sleep(std::time::Duration::from_millis(backoff)).await;
                    backoff = (backoff * 2).min(1500);
                    continue;
                }
                if msg.contains("database is locked") || msg.contains("SQLITE_BUSY") {
                    let elapsed_ms = started.elapsed().as_millis() as u64;
                    tracing::error!(target="persistence", attempts=attempt+1, label, elapsed_ms, "Retry exhaustion for SQLITE_BUSY");
                    // Best-effort: try to increment global lock counter if AppState accessible via once_cell
                    if let Some(app_state) = crate::application::app_handle_access::get_app_state() {
                        // fire and forget
                        let cloned = app_state.clone();
                        tokio::spawn(async move { cloned.incr_lock_errors().await; });
                    }
                }
                return Err(e);
            }
        }
    }
}

// Helper enum for heterogeneous SQL binds in update operations
enum BindValue<'a> {
    OptStr(&'a Option<String>),
    OptI32(&'a Option<i32>),
    OwnedStr(String),
}

/// Outcome of an upsert operation (vendors)
#[derive(Debug, Clone, Copy)]
pub struct UpsertOutcome {
    pub inserted: bool,
    pub updated: bool,
}

/// Repository for the integrated schema (products + `product_details` + vendors + `crawling_results`)
#[derive(Clone)]
pub struct IntegratedProductRepository {
    pool: Arc<SqlitePool>,
    // Optional global write serialization (enabled via env MC_SERIALIZE_WRITES=1)
    write_mutex: Option<Arc<tokio::sync::Mutex<()>>>,
}

impl IntegratedProductRepository {
    /// Normalize date strings to ISO YYYY-MM-DD if in MM/DD/YYYY format.
    fn normalize_cert_date(raw: &Option<String>) -> Option<String> {
        raw.as_ref().and_then(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() { return None; }
            // Already ISO (simple check)
            if trimmed.len() == 10 && trimmed.chars().nth(4) == Some('-') && trimmed.chars().nth(7) == Some('-') {
                return Some(trimmed.to_string());
            }
            // MM/DD/YYYY pattern
            if trimmed.len() == 10 && trimmed.chars().nth(2) == Some('/') && trimmed.chars().nth(5) == Some('/') {
                let mm = &trimmed[0..2];
                let dd = &trimmed[3..5];
                let yyyy = &trimmed[6..10];
                if mm.chars().all(|c| c.is_ascii_digit()) && dd.chars().all(|c| c.is_ascii_digit()) && yyyy.chars().all(|c| c.is_ascii_digit()) {
                    return Some(format!("{}-{}-{}", yyyy, mm, dd));
                }
            }
            Some(trimmed.to_string())
        })
    }
    /// Count vendors in the local database
    /// # Errors
    /// Returns an error if the query fails.
    pub async fn count_vendors(&self) -> Result<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM vendors")
            .fetch_one(&*self.pool)
            .await?;
        Ok(count)
    }

    /// Upsert a vendor by vendor_number.
    /// - Inserts when vendor_number doesn't exist.
    /// - Updates name/legal name if changed.
    /// Returns whether an insert or update occurred.
    /// # Errors
    /// Returns an error if database operations fail.
    pub async fn upsert_vendor_by_number(
        &self,
        vendor_number: i32,
        vendor_name: &str,
        company_legal_name: Option<&str>,
    ) -> Result<UpsertOutcome> {
        // Check existing record
        let existing = sqlx::query(
            r"SELECT vendor_name, company_legal_name FROM vendors WHERE vendor_number = ?",
        )
        .bind(vendor_number)
        .fetch_optional(&*self.pool)
        .await?;

        if let Some(row) = existing {
            let cur_name: String = row.get("vendor_name");
            let cur_legal: Option<String> = row.get("company_legal_name");

            let name_changed = cur_name != vendor_name;
            let legal_changed = cur_legal.as_deref() != company_legal_name;

            if name_changed || legal_changed {
                sqlx::query(
                    r"
                    UPDATE vendors
                    SET vendor_name = ?, company_legal_name = ?, updated_at = CURRENT_TIMESTAMP
                    WHERE vendor_number = ?
                    ",
                )
                .bind(vendor_name)
                .bind(company_legal_name)
                .bind(vendor_number)
                .execute(&*self.pool)
                .await?;

                Ok(UpsertOutcome {
                    inserted: false,
                    updated: true,
                })
            } else {
                Ok(UpsertOutcome {
                    inserted: false,
                    updated: false,
                })
            }
        } else {
            sqlx::query(
                r"
                INSERT INTO vendors (vendor_number, vendor_name, company_legal_name, created_at, updated_at)
                VALUES (?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                ",
            )
            .bind(vendor_number)
            .bind(vendor_name)
            .bind(company_legal_name)
            .execute(&*self.pool)
            .await?;

            Ok(UpsertOutcome {
                inserted: true,
                updated: false,
            })
        }
    }
    /// Vacate an occupied (`page_id`, `index_in_page`) slot if it's taken by a different URL.
    /// This helps avoid UNIQUE constraint violations when moving an item to a new position.
    /// Returns the URL that was occupying the slot, if any, after setting its position to NULL.
    async fn vacate_position_if_occupied(
        &self,
        page_id: i32,
        index_in_page: i32,
        keep_url: &str,
    ) -> Result<Option<String>> {
        let now = chrono::Utc::now();
        let keep_norm = Self::normalize_url(keep_url);

        // Find occupant different from the target URL
        // First check product_details (source of truth), then fallback to products
        let mut occupant_url: Option<String> = sqlx::query_scalar(
            r"SELECT url FROM product_details
               WHERE page_id = ? AND index_in_page = ? AND url != ?
               LIMIT 1",
        )
        .bind(page_id)
        .bind(index_in_page)
        .bind(&keep_norm)
        .fetch_optional(&*self.pool)
        .await?;
        if occupant_url.is_none() {
            occupant_url = sqlx::query_scalar(
                r"SELECT url FROM products
                   WHERE page_id = ? AND index_in_page = ? AND url != ?
                   LIMIT 1",
            )
            .bind(page_id)
            .bind(index_in_page)
            .bind(&keep_norm)
            .fetch_optional(&*self.pool)
            .await?;
        }

        if let Some(occ_url) = occupant_url.clone() {
            // Set occupant's position to NULL to free the slot
            let _ = sqlx::query(
                r"UPDATE product_details
                   SET page_id = NULL, index_in_page = NULL, id = NULL, updated_at = ?
                   WHERE url = ?",
            )
            .bind(now)
            .bind(&occ_url)
            .execute(&*self.pool)
            .await?;

            // Keep products table in sync
            let _ = sqlx::query(
                r"UPDATE products
                   SET page_id = NULL, index_in_page = NULL, id = NULL, updated_at = ?
                   WHERE url = ?",
            )
            .bind(now)
            .bind(&occ_url)
            .execute(&*self.pool)
            .await?;

            debug!(
                "[Vacate] Freed slot p{:04}i{:02} from url={} (kept url={})",
                page_id, index_in_page, occ_url, keep_norm
            );
            Ok(Some(occ_url))
        } else {
            Ok(None)
        }
    }
    /// 강제 위치 업데이트: URL이 존재하면 products, `product_details에` 대해
    /// `page_id`, `index_in_page`, id 세 필드만 업데이트합니다. 존재하지 않으면 0 리턴.
    /// # Errors
    /// Returns an error if database updates fail or the vacate operation errors.
    pub async fn force_update_position_by_url(
        &self,
        url: &str,
        page_id: i32,
        index_in_page: i32,
    ) -> Result<(u32, u32)> {
        let normalized = Self::normalize_url(url);
        let now = chrono::Utc::now();

        // Pre-vacate target slot to avoid UNIQUE(page_id, index_in_page) violations
        // if another record currently occupies the desired position.
        let _ = self
            .vacate_position_if_occupied(page_id, index_in_page, &normalized)
            .await?;

        // products 테이블 업데이트 (include id derived from position)
        let forced_id = format!("p{:04}i{:02}", page_id, index_in_page);
        let prod_res = sqlx::query(
            r"UPDATE products
         SET page_id = ?, index_in_page = ?, id = ?, updated_at = ?
               WHERE url = ?",
        )
        .bind(page_id)
        .bind(index_in_page)
        .bind(&forced_id)
        .bind(now)
        .bind(&normalized)
        .execute(&*self.pool)
        .await?;
        let prod_rows = u32::try_from(prod_res.rows_affected()).unwrap_or(u32::MAX);

        // product_details 테이블 업데이트 (id 포함)
        let det_res = sqlx::query(
            r"UPDATE product_details
               SET page_id = ?, index_in_page = ?, id = ?, updated_at = ?
               WHERE url = ?",
        )
        .bind(page_id)
        .bind(index_in_page)
        .bind(&forced_id)
        .bind(now)
        .bind(&normalized)
        .execute(&*self.pool)
        .await?;
        let det_rows = u32::try_from(det_res.rows_affected()).unwrap_or(u32::MAX);

        Ok((prod_rows, det_rows))
    }
    /// Normalize URL for consistent storage and comparison
    /// - Trims whitespace
    /// - Lowercases the hostname
    /// - Leaves path/query as-is (CSA URLs are case-sensitive there)
    fn normalize_url(url: &str) -> String {
        let trimmed = url.trim();
        url::Url::parse(trimmed).map_or_else(
            |_| trimmed.to_string(),
            |mut parsed| {
                if let Some(host) = parsed.host_str() {
                    let lower = host.to_ascii_lowercase();
                    if lower != host {
                        let _ = parsed.set_host(Some(&lower));
                    }
                }
                parsed.to_string()
            },
        )
    }
    /// Expose underlying pool reference (read-only operations convenience)
    #[must_use]
    pub fn pool(&self) -> &sqlx::SqlitePool {
        &self.pool
    }
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        let serialize = std::env::var("MC_SERIALIZE_WRITES").ok().is_some_and(|v| v=="1" || v.eq_ignore_ascii_case("true"));
        Self {
            pool: Arc::new(pool),
            write_mutex: if serialize { Some(Arc::new(tokio::sync::Mutex::new(()))) } else { None },
        }
    }

    /// Danger zone: completely remove all product related data so we can rebuild
    /// indexing semantics (Plan B). This is intentionally explicit and NOT called
    /// automatically; the frontend must invoke the dedicated reset command.
    /// Returns (`products_deleted`, `product_details_deleted`).
    /// # Errors
    /// Returns an error if deletion queries fail unexpectedly.
    pub async fn clear_all_products_and_details(&self) -> Result<(u64, u64)> {
        // Foreign key constraints: ensure ON DELETE CASCADE or delete child first.
        // We optimistically attempt child table deletion then parent.
        // NOTE: We don't wrap in a transaction intentionally to allow partial
        // progress visibility even if second step fails; caller can retry.
        let mut details_deleted: u64 = 0;
        let mut products_deleted: u64 = 0;
        // product_details may not exist in early DB versions; ignore errors gracefully.
        if let Ok(res) = sqlx::query("DELETE FROM product_details")
            .execute(&*self.pool)
            .await
        {
            details_deleted = res.rows_affected();
        }
        if let Ok(res) = sqlx::query("DELETE FROM products")
            .execute(&*self.pool)
            .await
        {
            products_deleted = res.rows_affected();
        }
        // Optionally reclaim space (best-effort)
        let _ = sqlx::query("VACUUM").execute(&*self.pool).await; // ignore errors
        info!(
            "🧹 Cleared product data: products_deleted={}, product_details_deleted={}",
            products_deleted, details_deleted
        );
        Ok((products_deleted, details_deleted))
    }

    // ===============================
    // PRODUCT OPERATIONS
    // ===============================

    /// Insert or update basic product information from listing page
    /// 🎯 지능적 비교: 실제로 변경된 필드가 있을 때만 업데이트
    /// Returns: (`was_updated`: bool, `was_created`: bool)
    /// # Errors
    /// Returns an error if database access fails during read or write.
    pub async fn create_or_update_product(&self, product: &Product) -> Result<(bool, bool)> {
        let now = chrono::Utc::now();
        // Normalize URL to ensure consistent storage and matching
        let normalized_url = Self::normalize_url(&product.url);

        // Basic validation to prevent blocking rows
        // - page_id/index_in_page must be non-negative if provided (0 is valid for oldest page / 0-based index)
        // - allow NULL for manufacturer/model/certificate_id temporarily, but prefer non-empty strings
        if let Some(pid) = product.page_id {
            if pid < 0 {
                anyhow::bail!("Invalid page_id: {}", pid);
            }
        }
        if let Some(idx) = product.index_in_page {
            if idx < 0 {
                anyhow::bail!("Invalid index_in_page: {}", idx);
            }
        }

        // Optional: quick trace for incoming coords when verbose
        if std::env::var("MC_PERSIST_VERBOSE")
            .ok()
            .as_deref()
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        {
            if let (Some(pid), Some(idx)) = (product.page_id, product.index_in_page) {
                debug!(
                    "[PersistTrace] incoming coords url={} pid={} idx={}",
                    normalized_url, pid, idx
                );
            }
        }

        // Check if product already exists
        let existing = self.get_product_by_url(&normalized_url).await?;

        if let Some(existing_product) = existing {
            // 🔍 지능적 비교: 실제 변경사항이 있는지 확인
            let needs_update = existing_product.manufacturer != product.manufacturer
                || existing_product.model != product.model
                || existing_product.certificate_id != product.certificate_id
                || existing_product.page_id != product.page_id
                || existing_product.index_in_page != product.index_in_page;

            if needs_update {
                // Derive id depending on page fields: set p####i## if both present, else NULL
                let new_id: Option<String> = match (product.page_id, product.index_in_page) {
                    (Some(pid), Some(idx)) => Some(format!("p{:04}i{:02}", pid, idx)),
                    _ => None,
                };
                // 📝 실제 변경사항이 있을 때만 업데이트
                sqlx::query(
                    r"
                    UPDATE products 
                    SET manufacturer = ?, model = ?, certificate_id = ?, page_id = ?, index_in_page = ?, id = ?, updated_at = ?
                    WHERE url = ?
                    ",
                )
                .bind(&product.manufacturer)
                .bind(&product.model)
                .bind(&product.certificate_id)
                .bind(product.page_id)
                .bind(product.index_in_page)
                .bind(new_id)
                .bind(now)
                .bind(&normalized_url)
                .execute(&*self.pool)
                .await?;

                info!(
                    "[Persist] products: url={} pid={:?} idx={:?} action=update",
                    normalized_url, product.page_id, product.index_in_page
                );
                info!(
                    "📝 Product updated: {} (changes detected)",
                    product.model.as_deref().unwrap_or("Unknown")
                );
                Ok((true, false)) // updated=true, created=false
            } else {
                // ✅ 변경사항 없음 - 불필요한 업데이트 스킵
                debug!(
                    "✅ Product unchanged: {} (skipping update)",
                    product.model.as_deref().unwrap_or("Unknown")
                );
                Ok((false, false)) // updated=false, created=false
            }
        } else {
            // Insert new product
            // Generate ID if not already set
            let generated_id = product.id.clone().unwrap_or_else(|| {
                if let (Some(page_id), Some(index_in_page)) =
                    (product.page_id, product.index_in_page)
                {
                    format!("p{:04}i{:02}", page_id, index_in_page)
                } else {
                    // Fallback ID generation
                    format!(
                        "product_{}",
                        product
                            .url
                            .chars()
                            .map(|c| if c.is_alphanumeric() { c } else { '_' })
                            .collect::<String>()
                    )
                }
            });

            sqlx::query(
                r"
                INSERT INTO products 
                (id, url, manufacturer, model, certificate_id, page_id, index_in_page, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                ",
            )
            .bind(&generated_id)  // Add generated_id
            .bind(&normalized_url)
            .bind(&product.manufacturer)
            .bind(&product.model)
            .bind(&product.certificate_id)
            .bind(product.page_id.map(i64::from))
            .bind(product.index_in_page.map(i64::from))
            .bind(now)
            .bind(now)
            .execute(&*self.pool)
            .await?;

            info!(
                "[Persist] products: url={} pid={:?} idx={:?} action=insert",
                normalized_url, product.page_id, product.index_in_page
            );
            // info!("🆕 New product created: {}", product.model.as_deref().unwrap_or("Unknown"));
            Ok((false, true)) // updated=false, created=true
        }
    }

    /// Insert or update detailed product specifications
    /// � 지능적 비교: 빈 필드 채움 및 실제 변경사항만 업데이트
    /// Returns: (`was_updated`: bool, `was_created`: bool)
    ///
    /// # Errors
    /// Returns an error if database operations fail or if serialization of update statements
    /// encounters an error.
    pub async fn create_or_update_product_detail(
        &self,
        detail: &ProductDetail,
    ) -> Result<(bool, bool)> {
        let _guard = if let Some(m) = &self.write_mutex { Some(m.lock().await) } else { None }; // serialize writes if enabled
        let now = chrono::Utc::now();

        // 기존 ProductDetail 확인
        // Apply normalization to reduce accidental duplicates
        let normalized_url = Self::normalize_url(&detail.url);
        let mut detail = detail.clone();
        detail.url = normalized_url;

        let existing = self.get_product_detail_by_url(&detail.url).await?;

        if let Some(existing_detail) = existing {
            // 🔍 지능적 비교: 빈 필드 채우기 + 실제 변경사항 확인
            let mut updates = Vec::new();
            // Heterogeneous bind values (different Option<T> types) captured via enum to avoid type mismatch
            let mut binds: Vec<BindValue> = Vec::new();
            let mut change_kinds: Vec<String> = Vec::new(); // human readable change descriptors
            let verbose = std::env::var("MC_PERSIST_VERBOSE")
                .ok()
                .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));

            // === Expanded fill/change detection (P1) ===
            macro_rules! fill_or_change_opt_str {
                ($field:ident, $col:expr) => {
                    if existing_detail.$field.is_none() && detail.$field.is_some() {
                        updates.push(concat!($col, " = ?"));
                        binds.push(BindValue::OptStr(&detail.$field));
                        change_kinds.push(format!("fill:{}", $col));
                    } else if existing_detail.$field.is_some()
                        && detail.$field.is_some()
                        && existing_detail.$field != detail.$field
                    {
                        updates.push(concat!($col, " = ?"));
                        binds.push(BindValue::OptStr(&detail.$field));
                        change_kinds.push(format!("change:{}", $col));
                    }
                };
            }

            // device_type handled manually for legacy diff
            if existing_detail.device_type.is_none() && detail.device_type.is_some() {
                updates.push("device_type = ?");
                binds.push(BindValue::OptStr(&detail.device_type));
                change_kinds.push("fill:device_type".to_string());
            } else if existing_detail.device_type.is_some()
                && detail.device_type.is_some()
                && existing_detail.device_type != detail.device_type
            {
                updates.push("device_type = ?");
                binds.push(BindValue::OptStr(&detail.device_type));
                change_kinds.push("change:device_type".to_string());
            }
            fill_or_change_opt_str!(certification_date, "certification_date");
            fill_or_change_opt_str!(software_version, "software_version");
            fill_or_change_opt_str!(hardware_version, "hardware_version");
            fill_or_change_opt_str!(description, "description");
            fill_or_change_opt_str!(firmware_version, "firmware_version");
            fill_or_change_opt_str!(specification_version, "specification_version");
            fill_or_change_opt_str!(transport_interface, "transport_interface");
            fill_or_change_opt_str!(application_categories, "application_categories");
            fill_or_change_opt_str!(compliance_document_url, "compliance_document_url");
            fill_or_change_opt_str!(program_type, "program_type");
            fill_or_change_opt_str!(family_sku, "family_sku");
            fill_or_change_opt_str!(family_variant_sku, "family_variant_sku");
            fill_or_change_opt_str!(family_id, "family_id");
            fill_or_change_opt_str!(tis_trp_tested, "tis_trp_tested");
            // primary_device_type_id removed; normalization uses primary_device_type_ids only
            fill_or_change_opt_str!(certificate_id, "certificate_id");
            // numeric
            if existing_detail.vid != detail.vid && detail.vid.is_some() {
                updates.push("vid = ?");
                binds.push(BindValue::OptI32(&detail.vid));
                change_kinds.push("change:vid".to_string());
            }
            if existing_detail.pid != detail.pid && detail.pid.is_some() {
                updates.push("pid = ?");
                binds.push(BindValue::OptI32(&detail.pid));
                change_kinds.push("change:pid".to_string());
            }

            // 실제 변경사항 확인 (기존 값과 다른 경우)
            if existing_detail.manufacturer != detail.manufacturer {
                updates.push("manufacturer = ?");
                binds.push(BindValue::OptStr(&detail.manufacturer));
                change_kinds.push("change:manufacturer".to_string());
            }
            if existing_detail.model != detail.model {
                updates.push("model = ?");
                binds.push(BindValue::OptStr(&detail.model));
                change_kinds.push("change:model".to_string());
            }

            // ✅ Ensure pagination coordinates are persisted on update as well
            if existing_detail.page_id != detail.page_id {
                updates.push("page_id = ?");
                binds.push(BindValue::OptI32(&detail.page_id));
                change_kinds.push("change:page_id".to_string());
            }
            if existing_detail.index_in_page != detail.index_in_page {
                updates.push("index_in_page = ?");
                binds.push(BindValue::OptI32(&detail.index_in_page));
                change_kinds.push("change:index_in_page".to_string());
            }

            // Derive the canonical id from target coordinates (or None if clearing)
            let derived_id: Option<String> = match (detail.page_id, detail.index_in_page) {
                (Some(pid), Some(idx)) => Some(format!("p{:04}i{:02}", pid, idx)),
                _ => None,
            };
            // If only id is stale (coords unchanged) we still need to update id
            let id_mismatch = existing_detail.id != derived_id;

            if !updates.is_empty() || id_mismatch {
                // Reconciliation: ensure products table coords are updated when detail has coords and product is stale or NULL
                if detail.page_id.is_some() && detail.index_in_page.is_some() {
                    // Fetch current product row coords
                    if let Ok(row) = sqlx::query(r"SELECT page_id, index_in_page FROM products WHERE url = ?")
                        .bind(&detail.url)
                        .fetch_optional(&*self.pool).await {
                        if let Some(r) = row {
                            let p_pid: Option<i32> = r.get("page_id");
                            let p_idx: Option<i32> = r.get("index_in_page");
                            if p_pid != detail.page_id || p_idx != detail.index_in_page {
                                tracing::info!(target="persist_detail_decision", url=%detail.url, old_p_pid=?p_pid, old_p_idx=?p_idx, new_p_pid=?detail.page_id, new_p_idx=?detail.index_in_page, "reconciling_products_coords");
                                let _ = sqlx::query(r"UPDATE products SET page_id = ?, index_in_page = ?, updated_at = ? WHERE url = ?")
                                    .bind(detail.page_id)
                                    .bind(detail.index_in_page)
                                    .bind(now)
                                    .bind(&detail.url)
                                    .execute(&*self.pool).await;
                            }
                        }
                    }
                }
                tracing::info!(target="persist_detail_decision", url=%detail.url, id_mismatch, changes=?change_kinds, clearing_position = (change_kinds.iter().any(|k| k=="change:page_id") && detail.page_id.is_none()) || (change_kinds.iter().any(|k| k=="change:index_in_page") && detail.index_in_page.is_none()), "detail_update_applying");
                // Before applying updates that change the position, vacate target slot to avoid UNIQUE violation
                if (change_kinds.iter().any(|k| k == "change:page_id")
                    || change_kinds.iter().any(|k| k == "change:index_in_page"))
                    && detail.page_id.is_some()
                    && detail.index_in_page.is_some()
                {
                    if let (Some(pid), Some(idx)) = (detail.page_id, detail.index_in_page) {
                        let _ = self
                            .vacate_position_if_occupied(pid, idx, &detail.url)
                            .await?;
                        tracing::debug!(target="persist_detail_decision", url=%detail.url, pid, idx, "vacated_target_slot_before_update");
                    }
                }

                // If page_id/index_in_page are being cleared to NULL, also clear id to NULL for consistency
                let clearing_position = (change_kinds.iter().any(|k| k == "change:page_id")
                    && detail.page_id.is_none())
                    || (change_kinds.iter().any(|k| k == "change:index_in_page")
                        && detail.index_in_page.is_none());
                if clearing_position {
                    updates.push("id = NULL");
                    // No bind needed for NULL literal
                }
                // If only id is mismatched (no page coordinate changes captured), fix id explicitly
                if !clearing_position && id_mismatch {
                    if let Some(ref forced) = derived_id {
                        updates.push("id = ?");
                        binds.push(BindValue::OwnedStr(forced.clone()));
                        change_kinds.push("fix:id".to_string());
                    } else {
                        updates.push("id = NULL");
                        change_kinds.push("fix:id(NULL)".to_string());
                    }
                }
                // Optional verbose diff logging before executing update
                if verbose {
                    // helper closures for formatting
                    fn fmt_opt_str(v: Option<&String>) -> String {
                        v.map_or_else(
                            || "∅".to_string(),
                            |s| {
                                if s.is_empty() {
                                    String::new()
                                } else {
                                    s.clone()
                                }
                            },
                        )
                    }
                    fn fmt_opt_i32(v: Option<i32>) -> String {
                        v.map_or_else(|| "∅".to_string(), |n| n.to_string())
                    }
                    let mut diffs: Vec<String> = Vec::new();
                    for kind in &change_kinds {
                        if let Some((k, col)) = kind.split_once(':') {
                            let (old_v, new_v) = match col {
                                "device_type" => (
                                    fmt_opt_str(existing_detail.device_type.as_ref()),
                                    fmt_opt_str(detail.device_type.as_ref()),
                                ),
                                "certification_date" => (
                                    fmt_opt_str(existing_detail.certification_date.as_ref()),
                                    fmt_opt_str(detail.certification_date.as_ref()),
                                ),
                                "software_version" => (
                                    fmt_opt_str(existing_detail.software_version.as_ref()),
                                    fmt_opt_str(detail.software_version.as_ref()),
                                ),
                                "hardware_version" => (
                                    fmt_opt_str(existing_detail.hardware_version.as_ref()),
                                    fmt_opt_str(detail.hardware_version.as_ref()),
                                ),
                                "description" => (
                                    fmt_opt_str(existing_detail.description.as_ref()),
                                    fmt_opt_str(detail.description.as_ref()),
                                ),
                                "firmware_version" => (
                                    fmt_opt_str(existing_detail.firmware_version.as_ref()),
                                    fmt_opt_str(detail.firmware_version.as_ref()),
                                ),
                                "specification_version" => (
                                    fmt_opt_str(existing_detail.specification_version.as_ref()),
                                    fmt_opt_str(detail.specification_version.as_ref()),
                                ),
                                "transport_interface" => (
                                    fmt_opt_str(existing_detail.transport_interface.as_ref()),
                                    fmt_opt_str(detail.transport_interface.as_ref()),
                                ),
                                "application_categories" => (
                                    fmt_opt_str(existing_detail.application_categories.as_ref()),
                                    fmt_opt_str(detail.application_categories.as_ref()),
                                ),
                                "compliance_document_url" => (
                                    fmt_opt_str(existing_detail.compliance_document_url.as_ref()),
                                    fmt_opt_str(detail.compliance_document_url.as_ref()),
                                ),
                                "program_type" => (
                                    fmt_opt_str(existing_detail.program_type.as_ref()),
                                    fmt_opt_str(detail.program_type.as_ref()),
                                ),
                                "family_sku" => (
                                    fmt_opt_str(existing_detail.family_sku.as_ref()),
                                    fmt_opt_str(detail.family_sku.as_ref()),
                                ),
                                "family_variant_sku" => (
                                    fmt_opt_str(existing_detail.family_variant_sku.as_ref()),
                                    fmt_opt_str(detail.family_variant_sku.as_ref()),
                                ),
                                "family_id" => (
                                    fmt_opt_str(existing_detail.family_id.as_ref()),
                                    fmt_opt_str(detail.family_id.as_ref()),
                                ),
                                "tis_trp_tested" => (
                                    fmt_opt_str(existing_detail.tis_trp_tested.as_ref()),
                                    fmt_opt_str(detail.tis_trp_tested.as_ref()),
                                ),
                                // primary_device_type_id removed; normalized list is primary_device_type_ids
                                "certificate_id" => (
                                    fmt_opt_str(existing_detail.certificate_id.as_ref()),
                                    fmt_opt_str(detail.certificate_id.as_ref()),
                                ),
                                "vid" => {
                                    (fmt_opt_i32(existing_detail.vid), fmt_opt_i32(detail.vid))
                                }
                                "pid" => {
                                    (fmt_opt_i32(existing_detail.pid), fmt_opt_i32(detail.pid))
                                }
                                "manufacturer" => (
                                    fmt_opt_str(existing_detail.manufacturer.as_ref()),
                                    fmt_opt_str(detail.manufacturer.as_ref()),
                                ),
                                "model" => (
                                    fmt_opt_str(existing_detail.model.as_ref()),
                                    fmt_opt_str(detail.model.as_ref()),
                                ),
                                other => ("?".to_string(), format!("(unmapped:{})", other)),
                            };
                            diffs.push(format!("{} {}: '{}' -> '{}'", k, col, old_v, new_v));
                        }
                    }

                    if !diffs.is_empty() {
                        info!(
                            "[PersistDiff] ProductDetail url={} changes: {}",
                            detail.url,
                            diffs.join("; ")
                        );
                    }
                }

                let query = format!(
                    "UPDATE product_details SET {}, updated_at = ? WHERE url = ?",
                    updates.join(", ")
                );

                let query_clone = query.clone();
                // Retry execution building binds in the same order each attempt
                retry_sqlite_busy("update_product_details", || {
                    let mut q = sqlx::query(&query_clone);
                    for bind in &binds {
                        q = match bind {
                            BindValue::OptStr(v) => q.bind(*v),
                            BindValue::OptI32(v) => q.bind(*v),
                            BindValue::OwnedStr(s) => q.bind(s),
                        };
                    }
                    q = q.bind(now).bind(&detail.url);
                    async move { q.execute(&*self.pool).await.map(|_| ()).map_err(|e| anyhow::Error::new(e)) }
                }).await?;

                // Keep products table in sync for pagination coordinates and id
                if detail.page_id.is_some() || detail.index_in_page.is_some() || id_mismatch {
                    // Update products when page fields changed OR id needed correction
                    let changed_page = change_kinds
                        .iter()
                        .any(|k| k == "change:page_id" || k == "change:index_in_page");
                    if changed_page || id_mismatch {
                        let _ = sqlx::query(
                            r"
                            UPDATE products
                            SET page_id = ?,
                                index_in_page = ?,
                                id = CASE WHEN ? IS NULL THEN NULL ELSE ? END,
                                updated_at = ?
                            WHERE url = ?
                            ",
                        )
                        .bind(detail.page_id)
                        .bind(detail.index_in_page)
                        .bind(derived_id.clone())
                        .bind(derived_id.clone())
                        .bind(now)
                        .bind(&detail.url)
                        .execute(&*self.pool)
                        .await; // best-effort
                    }
                }
                if verbose {
                    info!(
                        "📝 ProductDetail updated: {} ({} fields) kinds={:?}",
                        detail.model.as_deref().unwrap_or("Unknown"),
                        updates.len(),
                        change_kinds
                    );
                } else {
                    info!(
                        "📝 ProductDetail updated: {} ({} fields)",
                        detail.model.as_deref().unwrap_or("Unknown"),
                        updates.len()
                    );
                }
                info!(
                    "[Persist] product_details: url={} pid={:?} idx={:?} action=update",
                    detail.url, detail.page_id, detail.index_in_page
                );
                Ok((true, false)) // updated=true, created=false
            } else {
                tracing::info!(target="persist_detail_decision", url=%detail.url, "detail_noop_no_changes_post_compare");
                // 완전 no-op: 변경할 필드도 없고 id도 좌표와 일치 → diagnostics에서 계속 hole로 나타날 수 있는 잠재 지점
                tracing::debug!(target: "persist_detail", url=%detail.url, page_id=?detail.page_id, index_in_page=?detail.index_in_page, id_mismatch, "detail_noop_no_changes");
                if verbose {
                    debug!(
                        "✅ ProductDetail unchanged(noop): {} manufacturer={:?} model={:?} device_type={:?}",
                        detail.model.as_deref().unwrap_or("Unknown"),
                        detail.manufacturer,
                        detail.model,
                        detail.device_type
                    );
                } else {
                    debug!(
                        "✅ ProductDetail unchanged: {} (skipping update)",
                        detail.model.as_deref().unwrap_or("Unknown")
                    );
                }
                Ok((false, false)) // updated=false, created=false
            }
        } else {
            // 🆕 새로운 ProductDetail 삽입
            // ✅ Foreign Key 제약 해결: products 테이블에 먼저 기본 정보 삽입
            let basic_product = Product {
                url: detail.url.clone(),
                manufacturer: detail.manufacturer.clone(),
                model: detail.model.clone(),
                certificate_id: detail.certificate_id.clone(),
                page_id: detail.page_id,
                index_in_page: detail.index_in_page,
                id: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };

            // If target position is provided, pre-vacate to avoid UNIQUE violation BEFORE inserting into products
            if let (Some(pid), Some(idx)) = (detail.page_id, detail.index_in_page) {
                let _ = self
                    .vacate_position_if_occupied(pid, idx, &detail.url)
                    .await?;
            }

            // 기본 제품 정보 삽입 (UPSERT 방식)
            self.create_or_update_product(&basic_product)
                .await
                .map_err(|e| {
                    anyhow::anyhow!("Failed to create/update basic product info: {}", e)
                })?;

            // Generate ID if not already set
            let generated_id = detail.id.clone().unwrap_or_else(|| {
                if let (Some(page_id), Some(index_in_page)) = (detail.page_id, detail.index_in_page)
                {
                    format!("p{:04}i{:02}", page_id, index_in_page)
                } else {
                    // Fallback ID generation
                    format!(
                        "detail_{}",
                        detail
                            .url
                            .chars()
                            .map(|c| if c.is_alphanumeric() { c } else { '_' })
                            .collect::<String>()
                    )
                }
            });

            let insert_sql = r"
                                INSERT INTO product_details 
                                (url, page_id, index_in_page, id, manufacturer, model, device_type,
                                 certificate_id, certification_date, software_version, hardware_version,
                                 vid, pid, family_sku, family_variant_sku, firmware_version, family_id,
                                 tis_trp_tested, specification_version, transport_interface, 
                                 application_categories, description,
                                 compliance_document_url, program_type, created_at, updated_at)
                                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                                ON CONFLICT(url) DO UPDATE SET
                                    page_id = excluded.page_id,
                                    index_in_page = excluded.index_in_page,
                                    id = excluded.id,
                                    manufacturer = excluded.manufacturer,
                                    model = excluded.model,
                                    device_type = excluded.device_type,
                                    certificate_id = excluded.certificate_id,
                                    certification_date = excluded.certification_date,
                                    software_version = excluded.software_version,
                                    hardware_version = excluded.hardware_version,
                                    vid = excluded.vid,
                                    pid = excluded.pid,
                                    family_sku = excluded.family_sku,
                                    family_variant_sku = excluded.family_variant_sku,
                                    firmware_version = excluded.firmware_version,
                                    family_id = excluded.family_id,
                                    tis_trp_tested = excluded.tis_trp_tested,
                                    specification_version = excluded.specification_version,
                                    transport_interface = excluded.transport_interface,
                                    application_categories = excluded.application_categories,
                                    description = excluded.description,
                                    compliance_document_url = excluded.compliance_document_url,
                                    program_type = excluded.program_type,
                                    updated_at = excluded.updated_at
                                ";
            let normalized_cert_date = Self::normalize_cert_date(&detail.certification_date);
            retry_sqlite_busy("insert_product_details", || {
                let q = sqlx::query(insert_sql)
                    .bind(&detail.url)
                    .bind(detail.page_id)
                    .bind(detail.index_in_page)
                    .bind(&generated_id)
                    .bind(&detail.manufacturer)
                    .bind(&detail.model)
                    .bind(&detail.device_type)
                    .bind(&detail.certificate_id)
                    .bind(&normalized_cert_date)
                    .bind(&detail.software_version)
                    .bind(&detail.hardware_version)
                    .bind(detail.vid)
                    .bind(detail.pid)
                    .bind(&detail.family_sku)
                    .bind(&detail.family_variant_sku)
                    .bind(&detail.firmware_version)
                    .bind(&detail.family_id)
                    .bind(detail.tis_trp_tested.clone())
                    .bind(&detail.specification_version)
                    .bind(&detail.transport_interface)
                    .bind(&detail.application_categories)
                    .bind(&detail.description)
                    .bind(&detail.compliance_document_url)
                    .bind(&detail.program_type)
                    .bind(now)
                    .bind(now);
                async move { q.execute(&*self.pool).await.map(|_| ()).map_err(|e| anyhow::Error::new(e)) }
            }).await?;

            let verbose = std::env::var("MC_PERSIST_VERBOSE")
                .ok()
                .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
            if verbose {
                info!(
                    "🆕 New ProductDetail created: {} url={} page_id={:?} idx={:?}",
                    detail.model.as_deref().unwrap_or("Unknown"),
                    detail.url,
                    detail.page_id,
                    detail.index_in_page
                );
            } else {
                info!(
                    "🆕 New ProductDetail created: {}",
                    detail.model.as_deref().unwrap_or("Unknown")
                );
            }
            info!(
                "[Persist] product_details: url={} pid={:?} idx={:?} action=insert",
                detail.url, detail.page_id, detail.index_in_page
            );
            Ok((false, true)) // updated=false, created=true
        }
    }

    /// Bulk create or update multiple ProductDetails in a single transaction
    /// This method processes all details together to avoid SQLITE_BUSY errors
    /// Returns total counts: (`total_updated`: usize, `total_created`: usize)
    ///
    /// # Errors
    /// Returns an error if the transaction or any database operations fail.
    pub async fn bulk_create_or_update_product_details(
        &self,
        details: &[ProductDetail],
    ) -> Result<(usize, usize)> {
        if details.is_empty() {
            return Ok((0, 0));
        }

        let _guard = if let Some(m) = &self.write_mutex { Some(m.lock().await) } else { None };
        
        // Use retry logic for the entire bulk operation to handle SQLITE_BUSY
        retry_sqlite_busy("bulk_create_or_update_product_details", || async {
            self.bulk_create_or_update_product_details_impl(details).await
        }).await
    }

    /// Internal implementation of bulk operations (wrapped by retry logic)
    async fn bulk_create_or_update_product_details_impl(
        &self,
        details: &[ProductDetail],
    ) -> Result<(usize, usize)> {
        let chunk_size: usize = std::env::var("MC_BULK_TX_CHUNK_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(100);

        let mut total_updated = 0;
        let mut total_created = 0;

        let overall_started = Instant::now();
        for (chunk_index, chunk) in details.chunks(chunk_size).enumerate() {
            // Transaction begin with focused retry (instead of retrying whole bulk logic)
            let chunk_started = Instant::now();
            let begin_max_attempts: u32 = std::env::var("MC_TX_BEGIN_MAX_ATTEMPTS").ok().and_then(|v| v.parse().ok()).unwrap_or(6);
            let initial_backoff_ms: u64 = std::env::var("MC_TX_BEGIN_INITIAL_BACKOFF_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(30);
            let backoff_factor: u64 = std::env::var("MC_TX_BEGIN_BACKOFF_FACTOR").ok().and_then(|v| v.parse().ok()).unwrap_or(2);
            let mut attempt: u32 = 0;
            let mut backoff = initial_backoff_ms;
            let mut tx_opt = None;
            while attempt < begin_max_attempts {
                let lock_snapshot = write_lock_tracker::snapshot();
                let snapshot_count = lock_snapshot.len();
                // Try to begin
                match self.pool.begin().await {
                    Ok(t) => {
                        tracing::info!(target="bulk_persist", chunk_index, attempt, snapshot_count, "tx_begin_success");
                        tx_opt = Some(t);
                        break;
                    }
                    Err(e) => {
                        let now = Utc::now();
                        let ls: Vec<String> = lock_snapshot.into_iter().map(|i| {
                            let age_ms = (now - i.started_at).num_milliseconds();
                            format!("id={} label={} age_ms={}", i.id, i.label, age_ms)
                        }).collect();
                        tracing::warn!(target="bulk_persist", chunk_index, attempt, error=%e, snapshot_count, active_writes=%ls.join(";"), backoff_ms=backoff, "tx_begin_busy_or_error");
                        tokio::time::sleep(std::time::Duration::from_millis(backoff)).await;
                        backoff = backoff.saturating_mul(backoff_factor).min(1500);
                    }
                }
                attempt += 1;
            }
            let mut tx = match tx_opt { Some(t) => t, None => {
                return Err(anyhow::anyhow!("Failed to begin transaction after {} attempts", attempt));
            }};
            // Register per-chunk write transaction for diagnostics (dropped when chunk scope ends)
            let tx_guard = write_lock_tracker::register("bulk_chunk", "product detail bulk chunk tx");
            tracing::info!(target="bulk_persist", chunk_index, count=chunk.len(), guard_id=%tx_guard.id(), "Starting bulk product detail persistence chunk");

            let before_updated = total_updated;
            let before_created = total_created;

            for detail in chunk {
                // Apply URL normalization
                let normalized_url = Self::normalize_url(&detail.url);
                let mut detail = detail.clone();
                detail.url = normalized_url;

                // Check if record exists using a more efficient EXISTS query
                let existing: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM product_details WHERE url = ?)")
                    .bind(&detail.url)
                    .fetch_one(&mut *tx)
                    .await?;

                let now = chrono::Utc::now();

                if existing {
                    // Update existing record
                    let certification_date = Self::normalize_cert_date(&detail.certification_date);
                    let primary_device_type_ids_json = detail.primary_device_type_ids.as_ref()
                        .map(|ids| serde_json::to_string(ids).unwrap_or_default());

                    let derived_id: Option<String> = match (detail.page_id, detail.index_in_page) {
                        (Some(pid), Some(idx)) => Some(format!("p{:04}i{:02}", pid, idx)),
                        _ => None,
                    };

                    if let (Some(pid), Some(idx)) = (detail.page_id, detail.index_in_page) {
                        let occupant: Option<String> = sqlx::query_scalar(
                            "SELECT url FROM product_details WHERE page_id = ? AND index_in_page = ? AND url != ?"
                        )
                        .bind(pid)
                        .bind(idx)
                        .bind(&detail.url)
                        .fetch_optional(&mut *tx)
                        .await?;

                        if let Some(occupant_url) = occupant {
                            sqlx::query(
                                "UPDATE product_details SET page_id = NULL, index_in_page = NULL, id = NULL, updated_at = ? WHERE url = ?"
                            )
                            .bind(now)
                            .bind(&occupant_url)
                            .execute(&mut *tx)
                            .await?;
                            tracing::debug!(target="bulk_persist", occupant=%occupant_url, pid, idx, "vacated_slot_for_bulk_update");
                        }
                    }

                    sqlx::query(
                        r"
                        UPDATE product_details SET
                            page_id = ?, index_in_page = ?, id = ?, manufacturer = ?, model = ?, device_type = ?,
                            certificate_id = ?, certification_date = ?, software_version = ?, hardware_version = ?,
                            vid = ?, pid = ?, family_sku = ?, family_variant_sku = ?, firmware_version = ?, family_id = ?,
                            tis_trp_tested = ?, specification_version = ?, transport_interface = ?,
                            primary_device_type_ids = ?, application_categories = ?, description = ?,
                            compliance_document_url = ?, program_type = ?, updated_at = ?
                        WHERE url = ?
                        "
                    )
                    .bind(detail.page_id)
                    .bind(detail.index_in_page)
                    .bind(derived_id)
                    .bind(&detail.manufacturer)
                    .bind(&detail.model)
                    .bind(&detail.device_type)
                    .bind(&detail.certificate_id)
                    .bind(certification_date)
                    .bind(&detail.software_version)
                    .bind(&detail.hardware_version)
                    .bind(detail.vid)
                    .bind(detail.pid)
                    .bind(&detail.family_sku)
                    .bind(&detail.family_variant_sku)
                    .bind(&detail.firmware_version)
                    .bind(&detail.family_id)
                    .bind(&detail.tis_trp_tested)
                    .bind(&detail.specification_version)
                    .bind(&detail.transport_interface)
                    .bind(primary_device_type_ids_json)
                    .bind(&detail.application_categories)
                    .bind(&detail.description)
                    .bind(&detail.compliance_document_url)
                    .bind(&detail.program_type)
                    .bind(now)
                    .bind(&detail.url)
                    .execute(&mut *tx)
                    .await?;

                    if detail.page_id.is_some() && detail.index_in_page.is_some() {
                        sqlx::query(
                            "UPDATE products SET page_id = ?, index_in_page = ?, updated_at = ? WHERE url = ?"
                        )
                        .bind(detail.page_id)
                        .bind(detail.index_in_page)
                        .bind(now)
                        .bind(&detail.url)
                        .execute(&mut *tx)
                        .await
                        .ok();
                    }

                    total_updated += 1;
                } else {
                    // Insert new record
                    let certification_date = Self::normalize_cert_date(&detail.certification_date);
                    let primary_device_type_ids_json = detail.primary_device_type_ids.as_ref()
                        .map(|ids| serde_json::to_string(ids).unwrap_or_default());

                    let derived_id: Option<String> = match (detail.page_id, detail.index_in_page) {
                        (Some(pid), Some(idx)) => Some(format!("p{:04}i{:02}", pid, idx)),
                        _ => None,
                    };

                    if let (Some(pid), Some(idx)) = (detail.page_id, detail.index_in_page) {
                        let occupant: Option<String> = sqlx::query_scalar(
                            "SELECT url FROM product_details WHERE page_id = ? AND index_in_page = ?"
                        )
                        .bind(pid)
                        .bind(idx)
                        .fetch_optional(&mut *tx)
                        .await?;

                        if let Some(occupant_url) = occupant {
                            sqlx::query(
                                "UPDATE product_details SET page_id = NULL, index_in_page = NULL, id = NULL, updated_at = ? WHERE url = ?"
                            )
                            .bind(now)
                            .bind(&occupant_url)
                            .execute(&mut *tx)
                            .await?;
                            tracing::debug!(target="bulk_persist", occupant=%occupant_url, pid, idx, "vacated_slot_for_bulk_insert");
                        }
                    }

                    sqlx::query(
                        r"
                        INSERT INTO product_details (
                            url, page_id, index_in_page, id, manufacturer, model, device_type,
                            certificate_id, certification_date, software_version, hardware_version,
                            vid, pid, family_sku, family_variant_sku, firmware_version, family_id,
                            tis_trp_tested, specification_version, transport_interface,
                            primary_device_type_ids, application_categories, description,
                            compliance_document_url, program_type, created_at, updated_at
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                        "
                    )
                    .bind(&detail.url)
                    .bind(detail.page_id)
                    .bind(detail.index_in_page)
                    .bind(derived_id)
                    .bind(&detail.manufacturer)
                    .bind(&detail.model)
                    .bind(&detail.device_type)
                    .bind(&detail.certificate_id)
                    .bind(certification_date)
                    .bind(&detail.software_version)
                    .bind(&detail.hardware_version)
                    .bind(detail.vid)
                    .bind(detail.pid)
                    .bind(&detail.family_sku)
                    .bind(&detail.family_variant_sku)
                    .bind(&detail.firmware_version)
                    .bind(&detail.family_id)
                    .bind(&detail.tis_trp_tested)
                    .bind(&detail.specification_version)
                    .bind(&detail.transport_interface)
                    .bind(primary_device_type_ids_json)
                    .bind(&detail.application_categories)
                    .bind(&detail.description)
                    .bind(&detail.compliance_document_url)
                    .bind(&detail.program_type)
                    .bind(now)
                    .bind(now)
                    .execute(&mut *tx)
                    .await?;

                    total_created += 1;
                }
            }

            // Commit the transaction for the chunk
            if let Err(e) = tx.commit().await {
                let chunk_elapsed_ms = chunk_started.elapsed().as_millis() as u64;
                tracing::error!(target="bulk_persist", chunk_index, count=chunk.len(), guard_id=%tx_guard.id(), updated_in_chunk=total_updated-before_updated, created_in_chunk=total_created-before_created, chunk_elapsed_ms, error=%e, "Chunk commit failed");
                return Err(e.into());
            } else {
                let chunk_elapsed_ms = chunk_started.elapsed().as_millis() as u64;
                tracing::info!(target="bulk_persist", chunk_index, count=chunk.len(), guard_id=%tx_guard.id(), updated_in_chunk=total_updated-before_updated, created_in_chunk=total_created-before_created, chunk_elapsed_ms, "Chunk committed");
            }
        }
        
        let overall_elapsed_ms = overall_started.elapsed().as_millis() as u64;
        tracing::info!(target="bulk_persist", updated=total_updated, created=total_created, total=details.len(), overall_elapsed_ms, "Bulk product detail persistence completed");

        Ok((total_updated, total_created))
    }

    /// Bulk repair: synchronize products.page_id/index_in_page from product_details when detail has coords and product is NULL or mismatched.
    pub async fn repair_product_coordinates(&self, limit: Option<i64>) -> Result<u64> {
        let _guard = if let Some(m) = &self.write_mutex { Some(m.lock().await) } else { None };
        let lim_clause = limit.map(|_| "LIMIT ?").unwrap_or("");
        let sql = format!(r"
            SELECT d.url, d.page_id, d.index_in_page FROM product_details d
            JOIN products p ON p.url = d.url
            WHERE d.page_id IS NOT NULL AND d.index_in_page IS NOT NULL
              AND (p.page_id IS NULL OR p.index_in_page IS NULL OR p.page_id != d.page_id OR p.index_in_page != d.index_in_page)
            {}
        ", lim_clause);
        let rows = if let Some(l) = limit {
            sqlx::query(&sql).bind(l).fetch_all(&*self.pool).await?
        } else { sqlx::query(&sql).fetch_all(&*self.pool).await? };
        let mut fixed = 0u64;
        for row in &rows {
            let url: String = row.get("url");
            let pid: i32 = row.get("page_id");
            let idx: i32 = row.get("index_in_page");
            if let Err(e) = sqlx::query(r"UPDATE products SET page_id = ?, index_in_page = ?, updated_at = ? WHERE url = ?")
                .bind(pid)
                .bind(idx)
                .bind(chrono::Utc::now())
                .bind(&url)
                .execute(&*self.pool).await {
                tracing::warn!(target="repair_coords", url=%url, error=%e, "failed_to_update_product_coords");
            } else {
                fixed += 1;
            }
        }
        tracing::info!(target="repair_coords", fixed, total_rows=rows.len(), "repair_product_coordinates_completed");
        Ok(fixed)
    }

    /// 빠른 통계: `product_details` 전체 개수, `page_id` 범위, 마지막 업데이트 시각
    /// # Errors
    /// Returns an error if the query fails.
    pub async fn get_product_detail_stats(
        &self,
    ) -> Result<(i64, Option<i32>, Option<i32>, Option<DateTime<Utc>>)> {
        let row = sqlx::query(
            r"SELECT COUNT(*) as cnt, MIN(page_id) as min_page, MAX(page_id) as max_page, MAX(updated_at) as last_updated FROM product_details"
        ).fetch_one(&*self.pool).await?;
        let cnt: i64 = row.get("cnt");
        let min_page: Option<i32> = row.get("min_page");
        let max_page: Option<i32> = row.get("max_page");
        let last_updated: Option<DateTime<Utc>> = row.get("last_updated");
        Ok((cnt, min_page, max_page, last_updated))
    }

    /// Get all products with pagination
    /// # Errors
    /// Returns an error if the query or row decoding fails.
    pub async fn get_products_paginated(&self, page: i32, limit: i32) -> Result<Vec<Product>> {
        let offset = (page - 1) * limit;
        let rows = sqlx::query(
            r"
            SELECT url, manufacturer, model, certificate_id, page_id, index_in_page, created_at, updated_at
            FROM products 
            ORDER BY page_id DESC, index_in_page ASC 
            LIMIT ? OFFSET ?
            ",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&*self.pool)
        .await?;

        let products = rows
            .into_iter()
            .map(|row| Product {
                id: None, // products 테이블에는 id 컬럼이 없음
                url: row.get("url"),
                manufacturer: row.get("manufacturer"),
                model: row.get("model"),
                certificate_id: row.get("certificate_id"),
                page_id: row.get("page_id"),
                index_in_page: row.get("index_in_page"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

        Ok(products)
    }

    /// Get product by URL
    /// # Errors
    /// Returns an error if the query fails.
    pub async fn get_product_by_url(&self, url: &str) -> Result<Option<Product>> {
        let normalized_url = Self::normalize_url(url);
        let row = sqlx::query(
            r"
            SELECT url, manufacturer, model, certificate_id, page_id, index_in_page, created_at, updated_at
            FROM products WHERE url = ?
            ",
        )
    .bind(&normalized_url)
        .fetch_optional(&*self.pool)
        .await?;

        row.map_or_else(
            || Ok(None),
            |row| {
                Ok(Some(Product {
                    id: None, // products 테이블에는 id 컬럼이 없음
                    url: row.get("url"),
                    manufacturer: row.get("manufacturer"),
                    model: row.get("model"),
                    certificate_id: row.get("certificate_id"),
                    page_id: row.get("page_id"),
                    index_in_page: row.get("index_in_page"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }))
            },
        )
    }

    /// Get product with details by URL
    /// # Errors
    /// Returns an error if either underlying product/detail retrieval fails.
    pub async fn get_product_with_details(&self, url: &str) -> Result<Option<ProductWithDetails>> {
        let product = self.get_product_by_url(url).await?;

        if let Some(product) = product {
            let detail = self.get_product_detail_by_url(url).await?;
            Ok(Some(ProductWithDetails {
                product,
                details: detail,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get product detail by URL
    /// # Errors
    /// Returns an error if the query fails.
    pub async fn get_product_detail_by_url(&self, url: &str) -> Result<Option<ProductDetail>> {
        let normalized_url = Self::normalize_url(url);
        let row = sqlx::query(
            r"
         SELECT url, page_id, index_in_page, id, manufacturer, model, device_type,
                   certificate_id, certification_date, software_version, hardware_version,
                   vid, pid, family_sku, family_variant_sku, firmware_version, family_id,
                   tis_trp_tested, specification_version, transport_interface, 
             primary_device_type_ids, application_categories, description,
                   compliance_document_url, program_type, created_at, updated_at
            FROM product_details WHERE url = ?
            ",
        )
        .bind(&normalized_url)
        .fetch_optional(&*self.pool)
        .await?;

        row.map_or_else(
            || Ok(None),
            |row| {
                Ok(Some(ProductDetail {
                    url: row.get("url"),
                    page_id: row.get("page_id"),
                    index_in_page: row.get("index_in_page"),
                    id: row.get("id"),
                    manufacturer: row.get("manufacturer"),
                    model: row.get("model"),
                    device_type: row.get("device_type"),
                    certificate_id: row.get("certificate_id"),
                    certification_date: row.get("certification_date"),
                    software_version: row.get("software_version"),
                    hardware_version: row.get("hardware_version"),
                    vid: row.get("vid"),
                    pid: row.get("pid"),
                    family_sku: row.get("family_sku"),
                    family_variant_sku: row.get("family_variant_sku"),
                    firmware_version: row.get("firmware_version"),
                    family_id: row.get("family_id"),
                    tis_trp_tested: row.get("tis_trp_tested"),
                    specification_version: row.get("specification_version"),
                    transport_interface: row.get("transport_interface"),
                    primary_device_type_ids: {
                        let json: Option<String> = row.get("primary_device_type_ids");
                        json.and_then(|s| serde_json::from_str::<Vec<i32>>(&s).ok())
                    },
                    application_categories: row.get("application_categories"),
                    description: row.get("description"),
                    compliance_document_url: row.get("compliance_document_url"),
                    program_type: row.get("program_type"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }))
            },
        )
    }

    /// Search products with criteria and pagination
    /// # Errors
    /// Returns an error if counting or data queries fail.
    pub async fn search_products(
        &self,
        criteria: &ProductSearchCriteria,
    ) -> Result<ProductSearchResult> {
        let mut conditions = Vec::new();
        let mut bind_values = Vec::new();

        // Build WHERE clause based on criteria
        if let Some(manufacturer) = &criteria.manufacturer {
            conditions.push("p.manufacturer LIKE ?");
            bind_values.push(format!("%{}%", manufacturer));
        }

        if let Some(device_type) = &criteria.device_type {
            conditions.push("pd.device_type LIKE ?");
            bind_values.push(format!("%{}%", device_type));
        }

        if let Some(certificate_id) = &criteria.certificate_id {
            conditions.push("p.certificate_id LIKE ?");
            bind_values.push(format!("%{}%", certificate_id));
        }

        if let Some(specification_version) = &criteria.specification_version {
            conditions.push("pd.specification_version = ?");
            bind_values.push(specification_version.clone());
        }

        if let Some(program_type) = &criteria.program_type {
            conditions.push("pd.program_type = ?");
            bind_values.push(program_type.clone());
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let page = criteria.page.unwrap_or(1);
        let limit = criteria.limit.unwrap_or(50);
        let offset = (page - 1) * limit;

        // Get total count
        let count_query = format!(
            r"
            SELECT COUNT(*) as total
            FROM products p
            LEFT JOIN product_details pd ON p.url = pd.url
            {}
            ",
            where_clause
        );

        let mut count_query_builder = sqlx::query_scalar::<_, i32>(&count_query);
        for value in &bind_values {
            count_query_builder = count_query_builder.bind(value);
        }
        let total_count = count_query_builder.fetch_one(&*self.pool).await?;

        // Get paginated results
        let data_query = format!(
            r"
            SELECT p.url, p.manufacturer, p.model, p.certificate_id, p.page_id, p.index_in_page, 
                   p.created_at as p_created_at, p.updated_at as p_updated_at,
                   pd.id, pd.device_type as pd_device_type, pd.certification_date as pd_certification_date, pd.software_version, pd.hardware_version,
                   pd.vid, pd.pid, pd.family_sku, pd.family_variant_sku, pd.firmware_version, pd.family_id,
                   pd.tis_trp_tested, pd.specification_version, pd.transport_interface, 
                   pd.primary_device_type_ids, pd.application_categories, pd.description,
                   pd.compliance_document_url, pd.program_type,
                   pd.created_at as pd_created_at, pd.updated_at as pd_updated_at
            FROM products p
            LEFT JOIN product_details pd ON p.url = pd.url
            {}
            ORDER BY p.page_id DESC, p.index_in_page ASC
            LIMIT ? OFFSET ?
            ",
            where_clause
        );

        let mut data_query_builder = sqlx::query(&data_query);
        for value in &bind_values {
            data_query_builder = data_query_builder.bind(value);
        }
        data_query_builder = data_query_builder.bind(limit).bind(offset);
        let rows = data_query_builder.fetch_all(&*self.pool).await?;

        let products = rows
            .into_iter()
            .map(|row| {
                let product = Product {
                    id: row.get("id"),
                    url: row.get("url"),
                    manufacturer: row.get("manufacturer"),
                    model: row.get("model"),
                    certificate_id: row.get("certificate_id"),
                    page_id: row.get("page_id"),
                    index_in_page: row.get("index_in_page"),
                    created_at: row.get("p_created_at"),
                    updated_at: row.get("p_updated_at"),
                };

                let details = if row.get::<Option<String>, _>("id").is_some() {
                    Some(ProductDetail {
                        url: row.get("url"),
                        page_id: row.get("page_id"),
                        index_in_page: row.get("index_in_page"),
                        id: row.get("id"),
                        manufacturer: row.get("manufacturer"),
                        model: row.get("model"),
                        device_type: row.get("pd_device_type"),
                        certificate_id: row.get("certificate_id"),
                        certification_date: row.get("pd_certification_date"),
                        software_version: row.get("software_version"),
                        hardware_version: row.get("hardware_version"),
                        vid: row.get("vid"),
                        pid: row.get("pid"),
                        family_sku: row.get("family_sku"),
                        family_variant_sku: row.get("family_variant_sku"),
                        firmware_version: row.get("firmware_version"),
                        family_id: row.get("family_id"),
                        tis_trp_tested: row.get("tis_trp_tested"),
                        specification_version: row.get("specification_version"),
                        transport_interface: row.get("transport_interface"),
                        primary_device_type_ids: {
                            let json: Option<String> = row.get("primary_device_type_ids");
                            json.and_then(|s| serde_json::from_str::<Vec<i32>>(&s).ok())
                        },
                        application_categories: row.get("application_categories"),
                        description: row.get("description"),
                        compliance_document_url: row.get("compliance_document_url"),
                        program_type: row.get("program_type"),
                        created_at: row.get("pd_created_at"),
                        updated_at: row.get("pd_updated_at"),
                    })
                } else {
                    None
                };

                ProductWithDetails { product, details }
            })
            .collect();

        let total_pages = (total_count + limit - 1) / limit;

        Ok(ProductSearchResult {
            products,
            total_count,
            page,
            limit,
            total_pages,
        })
    }

    /// JSON 형식의 제품 데이터를 DB에 추가 또는 업데이트
    ///
    /// 새로운 제품인 경우 true, 기존 제품 업데이트인 경우 false 반환
    /// # Errors
    /// Returns an error if required fields are missing or DB upserts fail.
    pub async fn upsert_product(&self, product_json: serde_json::Value) -> Result<bool> {
        let url = product_json["url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Product JSON must contain a url field"))?
            .to_string();

        // 기존 제품 확인
        let existing_product = self.get_product_by_url(&url).await?;
        let is_new = existing_product.is_none();

        // 기본 제품 정보 추출
        let mut basic_product = Product {
            id: None, // Will be generated in create_or_update_product
            url: url.clone(),
            manufacturer: product_json["manufacturer"]
                .as_str()
                .map(std::string::ToString::to_string),
            model: product_json["model"]
                .as_str()
                .map(std::string::ToString::to_string),
            certificate_id: product_json["certification_id"]
                .as_str()
                .map(std::string::ToString::to_string),
            page_id: product_json["page_id"]
                .as_i64()
                .and_then(|i| i32::try_from(i).ok()),
            index_in_page: product_json["index_in_page"]
                .as_i64()
                .and_then(|i| i32::try_from(i).ok()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        // Generate ID based on page_id and index_in_page
        basic_product.generate_id();

        // 기본 제품 정보 저장
        self.create_or_update_product(&basic_product).await?;

        // 상세 정보가 있는 경우 저장
        if product_json.get("device_type").is_some()
            || product_json.get("hardware_version").is_some()
        {
            let vid = product_json["vid"]
                .as_i64()
                .and_then(|i| i32::try_from(i).ok());
            let pid = product_json["pid"]
                .as_i64()
                .and_then(|i| i32::try_from(i).ok());

            let detail = ProductDetail {
                url: url.clone(),
                page_id: basic_product.page_id,
                index_in_page: basic_product.index_in_page,
                id: product_json["id"]
                    .as_str()
                    .map(std::string::ToString::to_string),
                manufacturer: basic_product.manufacturer.clone(),
                model: basic_product.model.clone(),
                device_type: product_json["device_type"]
                    .as_str()
                    .map(std::string::ToString::to_string),
                certificate_id: basic_product.certificate_id.clone(),
                certification_date: product_json["certification_date"]
                    .as_str()
                    .map(std::string::ToString::to_string),
                software_version: product_json["software_version"]
                    .as_str()
                    .map(std::string::ToString::to_string),
                hardware_version: product_json["hardware_version"]
                    .as_str()
                    .map(std::string::ToString::to_string),
                vid,
                pid,
                family_sku: product_json["family_sku"]
                    .as_str()
                    .map(std::string::ToString::to_string),
                family_variant_sku: product_json["family_variant_sku"]
                    .as_str()
                    .map(std::string::ToString::to_string),
                firmware_version: product_json["firmware_version"]
                    .as_str()
                    .map(std::string::ToString::to_string),
                family_id: product_json["family_id"]
                    .as_str()
                    .map(std::string::ToString::to_string),
                tis_trp_tested: product_json["tis_trp_tested"]
                    .as_str()
                    .map(std::string::ToString::to_string),
                specification_version: product_json["specification_version"]
                    .as_str()
                    .map(std::string::ToString::to_string),
                transport_interface: product_json["transport_interface"]
                    .as_str()
                    .map(std::string::ToString::to_string),
                // legacy field removed; keep only normalized list
                primary_device_type_ids: None,
                application_categories: product_json["application_categories"].as_array().map(
                    |arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(std::string::ToString::to_string))
                            .collect()
                    },
                ),
                description: product_json["description"]
                    .as_str()
                    .map(std::string::ToString::to_string),
                compliance_document_url: product_json["compliance_document_url"]
                    .as_str()
                    .map(std::string::ToString::to_string),
                program_type: product_json["program_type"]
                    .as_str()
                    .map(std::string::ToString::to_string),
                created_at: basic_product.created_at,
                updated_at: basic_product.updated_at,
            };

            self.create_or_update_product_detail(&detail).await?;
        }

        Ok(is_new)
    }

    /// Get all vendors
    /// # Errors
    /// Returns an error if the query fails.
    pub async fn get_vendors(&self) -> Result<Vec<Vendor>> {
        let rows = sqlx::query(
            "SELECT vendor_id, vendor_number, vendor_name, company_legal_name, created_at, updated_at FROM vendors ORDER BY vendor_name"
        )
        .fetch_all(&*self.pool)
        .await?;

        let vendors = rows
            .into_iter()
            .map(|row| Vendor {
                vendor_id: row.get("vendor_id"),
                vendor_number: row.get("vendor_number"),
                vendor_name: row.get("vendor_name"),
                company_legal_name: row.get("company_legal_name"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

        Ok(vendors)
    }

    /// Create a new vendor
    /// # Errors
    /// Returns an error if insert or id retrieval fails.
    pub async fn create_vendor(&self, vendor: &crate::domain::product::Vendor) -> Result<i32> {
        let vendor_id = vendor.vendor_id.max(0); // 새 ID인 경우 자동 생성

        sqlx::query(
            r"
            INSERT INTO vendors 
            (vendor_id, vendor_number, vendor_name, company_legal_name, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)
            ",
        )
        .bind(vendor_id)
        .bind(vendor.vendor_number)
        .bind(&vendor.vendor_name)
        .bind(&vendor.company_legal_name)
        .bind(vendor.created_at)
        .bind(vendor.updated_at)
        .execute(&*self.pool)
        .await?;

        // 새 벤더 ID 반환 (자동 생성된 경우 조회)
        if vendor_id <= 0 {
            let row = sqlx::query("SELECT last_insert_rowid() as id")
                .fetch_one(&*self.pool)
                .await?;

            let new_id: i32 = row.get("id");
            Ok(new_id)
        } else {
            Ok(vendor_id)
        }
    }

    // ===============================
    // CRAWLING RESULTS OPERATIONS
    // ===============================

    /// Save crawling result
    /// # Errors
    /// Returns an error if the upsert query fails.
    pub async fn save_crawling_result(&self, result: &CrawlingResult) -> Result<()> {
        sqlx::query(
            r"
            INSERT OR REPLACE INTO crawling_results 
            (session_id, status, stage, total_pages, products_found, details_fetched, errors_count,
             started_at, completed_at, execution_time_seconds, config_snapshot, error_details, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ",
        )
        .bind(&result.session_id)
        .bind(&result.status)
        .bind(&result.stage)
        .bind(result.total_pages)
        .bind(result.products_found)
        .bind(result.details_fetched)
        .bind(result.errors_count)
        .bind(result.started_at)
        .bind(result.completed_at)
        .bind(result.execution_time_seconds)
        .bind(&result.config_snapshot)
        .bind(&result.error_details)
        .bind(result.created_at)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    /// Get crawling results with pagination
    /// # Errors
    /// Returns an error if the query or row decoding fails.
    pub async fn get_crawling_results(&self, page: i32, limit: i32) -> Result<Vec<CrawlingResult>> {
        let offset = (page - 1) * limit;
        let rows = sqlx::query(
            r"
            SELECT session_id, status, stage, total_pages, products_found, details_fetched, errors_count,
                   started_at, completed_at, execution_time_seconds, config_snapshot, error_details, created_at
            FROM crawling_results 
            ORDER BY started_at DESC 
            LIMIT ? OFFSET ?
            ",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&*self.pool)
        .await?;

        let results = rows
            .into_iter()
            .map(|row| CrawlingResult {
                session_id: row.get("session_id"),
                status: row.get("status"),
                stage: row.get("stage"),
                total_pages: row.get("total_pages"),
                products_found: row.get("products_found"),
                details_fetched: row.get("details_fetched"),
                errors_count: row.get("errors_count"),
                started_at: row.get("started_at"),
                completed_at: row.get("completed_at"),
                execution_time_seconds: row.get("execution_time_seconds"),
                config_snapshot: row.get("config_snapshot"),
                error_details: row.get("error_details"),
                created_at: row.get("created_at"),
            })
            .collect();

        Ok(results)
    }

    // ===============================
    // STATISTICS AND ANALYTICS
    // ===============================

    /// Get database statistics
    ///
    /// # Errors
    /// Returns an error if any of the required queries fail.
    pub async fn get_database_statistics(&self) -> Result<DatabaseStatistics> {
        let total_products: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
            .fetch_one(&*self.pool)
            .await?;

        let total_details: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM product_details")
            .fetch_one(&*self.pool)
            .await?;

        let unique_manufacturers: i32 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT manufacturer) FROM products WHERE manufacturer IS NOT NULL",
        )
        .fetch_one(&*self.pool)
        .await?;

        let unique_device_types: i32 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT device_type) FROM product_details WHERE device_type IS NOT NULL",
        )
        .fetch_one(&*self.pool)
        .await?;

        let latest_crawl_date: Option<String> =
            sqlx::query_scalar("SELECT MAX(started_at) FROM crawling_results")
                .fetch_optional(&*self.pool)
                .await?;

        let matter_products_count: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM product_details WHERE program_type = 'Matter' OR program_type IS NULL")
            .fetch_one(&*self.pool)
            .await?;

        let completion_rate = if total_products > 0 {
            #[allow(clippy::cast_precision_loss)]
            {
                (total_details as f32 / total_products as f32) * 100.0
            }
        } else {
            0.0
        };

        Ok(DatabaseStatistics {
            total_products: total_products.into(),
            active_products: total_products.into(), // TODO: Add proper active_products query
            unique_vendors: unique_manufacturers.into(),
            unique_categories: unique_device_types.into(),
            avg_rating: None, // TODO: Add rating calculation
            total_reviews: 0, // TODO: Add review count
            last_crawled: latest_crawl_date.as_ref().and_then(|s| {
                DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            }),
            total_details: total_details.into(),
            unique_manufacturers: unique_manufacturers.into(),
            unique_device_types: unique_device_types.into(),
            latest_crawl_date: latest_crawl_date.as_ref().and_then(|s| {
                DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            }),
            matter_products_count: matter_products_count.into(),
            completion_rate: f64::from(completion_rate),
        })
    }

    /// Get products without details (for crawling prioritization)
    ///
    /// # Errors
    /// Returns an error if the query or decoding fails.
    pub async fn get_products_without_details(&self, limit: i32) -> Result<Vec<Product>> {
        let rows = sqlx::query(
            r"
            SELECT p.id, p.url, p.manufacturer, p.model, p.certificate_id, p.page_id, p.index_in_page, p.created_at, p.updated_at
            FROM products p
            LEFT JOIN product_details pd ON p.url = pd.url
            WHERE pd.url IS NULL
            ORDER BY p.page_id DESC, p.index_in_page ASC
            LIMIT ?
            ",
        )
        .bind(limit)
        .fetch_all(&*self.pool)
        .await?;

        let products = rows
            .into_iter()
            .map(|row| Product {
                id: row.get("id"),
                url: row.get("url"),
                manufacturer: row.get("manufacturer"),
                model: row.get("model"),
                certificate_id: row.get("certificate_id"),
                page_id: row.get("page_id"),
                index_in_page: row.get("index_in_page"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

        Ok(products)
    }

    /// Get only product URLs (no details yet) within a specific set of `page_ids`
    /// Returns at most `limit` URLs. If `pages` is empty returns empty Vec.
    /// Get product URLs without details in specific pages
    ///
    /// # Errors
    /// Returns an error if the query fails.
    pub async fn get_product_urls_without_details_in_pages(
        &self,
        pages: &[u32],
        limit: i32,
    ) -> Result<Vec<String>> {
        if pages.is_empty() {
            return Ok(Vec::new());
        }
        // Build dynamic IN clause safely
        let mut placeholders = String::new();
        for i in 0..pages.len() {
            if i > 0 {
                placeholders.push(',');
            }
            placeholders.push('?');
        }
        let query = format!(
            "SELECT p.url FROM products p LEFT JOIN product_details pd ON p.url = pd.url \
             WHERE pd.url IS NULL AND p.page_id IN ({}) ORDER BY p.page_id DESC, p.index_in_page ASC LIMIT ?",
            placeholders
        );
        let mut q = sqlx::query(&query);
        for p in pages {
            let p_i32 = i32::try_from(*p).unwrap_or(i32::MAX);
            q = q.bind(p_i32);
        }
        q = q.bind(limit);
        let rows = q.fetch_all(&*self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|r| r.get::<String, _>("url"))
            .collect())
    }

    /// Get all products from the database
    ///
    /// # Errors
    /// Returns an error if the query or decoding fails.
    pub async fn get_all_products(&self) -> Result<Vec<Product>> {
        let rows = sqlx::query(
            r"
            SELECT url, manufacturer, model, certificate_id, 
                   page_id, index_in_page, created_at, updated_at
            FROM products
            ORDER BY page_id DESC, index_in_page ASC
            ",
        )
        .fetch_all(&*self.pool)
        .await?;

        let products = rows
            .into_iter()
            .map(|row| Product {
                id: None, // products 테이블에는 id 컬럼이 없음
                url: row.get("url"),
                manufacturer: row.get("manufacturer"),
                model: row.get("model"),
                certificate_id: row.get("certificate_id"),
                page_id: row.get("page_id"),
                index_in_page: row.get("index_in_page"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

        Ok(products)
    }

    /// Get the latest updated product
    ///
    /// # Errors
    /// Returns an error if the query fails.
    pub async fn get_latest_updated_product(&self) -> Result<Option<Product>> {
        let row = sqlx::query(
            r"
            SELECT url, manufacturer, model, certificate_id, 
                   page_id, index_in_page, created_at, updated_at
            FROM products
            ORDER BY updated_at DESC
            LIMIT 1
            ",
        )
        .fetch_optional(&*self.pool)
        .await?;

        row.map_or_else(
            || Ok(None),
            |row| {
                Ok(Some(Product {
                    id: None, // products 테이블에는 id 컬럼이 없음
                    url: row.get("url"),
                    manufacturer: row.get("manufacturer"),
                    model: row.get("model"),
                    certificate_id: row.get("certificate_id"),
                    page_id: row.get("page_id"),
                    index_in_page: row.get("index_in_page"),
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at"),
                }))
            },
        )
    }

    // ===============================
    // CRAWLING RANGE CALCULATION
    // ===============================

    /// Get the maximum `page_id` and `index_in_page` from the database
    /// Returns (`max_page_id`, `max_index_in_page`) or (None, None) if no data
    ///
    /// This finds the product with the highest `page_id`, and among those products,
    /// the one with the highest `index_in_page`. This represents the "last" product
    /// we've crawled in the reverse chronological order.
    /// Get the maximum `page_id` and `index_in_page`
    ///
    /// # Errors
    /// Returns an error if the query fails.
    pub async fn get_max_page_id_and_index(&self) -> Result<(Option<i32>, Option<i32>)> {
        let row = sqlx::query(
            r"
            SELECT page_id, index_in_page
            FROM products
            WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL
            ORDER BY page_id DESC, index_in_page DESC
            LIMIT 1
            ",
        )
        .fetch_optional(&*self.pool)
        .await?;

        row.map_or_else(
            || {
                tracing::debug!("📊 No products found with page_id and index_in_page");
                Ok((None, None))
            },
            |row| {
                let max_page_id: Option<i32> = row.get("page_id");
                let max_index_in_page: Option<i32> = row.get("index_in_page");
                tracing::debug!(
                    "📊 Found last saved product: page_id={:?}, index_in_page={:?}",
                    max_page_id,
                    max_index_in_page
                );
                Ok((max_page_id, max_index_in_page))
            },
        )
    }

    /// Get the count of products stored in the database
    ///
    /// # Errors
    /// Returns an error if the query fails.
    pub async fn get_product_count(&self) -> Result<i32> {
        let count: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
            .fetch_one(&*self.pool)
            .await?;
        Ok(count)
    }

    /// Calculate the next crawling range based on local DB state and site information
    /// Returns (`start_page`, `end_page`) for the next crawling range
    ///
    /// This implements the logic from prompts6:
    /// 1. Get the last saved product's reverse absolute index
    /// 2. Calculate the next product index to crawl
    /// 3. Convert to website page numbers
    /// 4. Apply crawl page limit (respecting user settings)
    ///    Calculate the next crawling page range based on DB state and site info
    ///
    /// # Errors
    /// Returns an error if database access or range analysis fails.
    ///
    /// # Panics
    /// Panics if internal DB state is inconsistent and `max_page_id`/`max_index_in_page` are `None`
    /// after earlier checks. This should not occur in normal operation.
    pub async fn calculate_next_crawling_range(
        &self,
        total_pages_on_site: u32,
        products_on_last_page: u32,
        crawl_page_limit: u32,
    ) -> Result<Option<(u32, u32)>> {
        // Load user configuration to respect intelligent mode settings
        let user_config = match crate::infrastructure::config::ConfigManager::new() {
            Ok(config_manager) => match config_manager.load_config().await {
                Ok(config) => config,
                Err(e) => {
                    tracing::warn!("⚠️ Failed to load user config: {}", e);
                    crate::infrastructure::config::AppConfig::default()
                }
            },
            Err(e) => {
                tracing::warn!("⚠️ Failed to create config manager: {}", e);
                crate::infrastructure::config::AppConfig::default()
            }
        };

        // Determine effective page limit based on user settings
        let effective_crawl_limit = if user_config.user.crawling.intelligent_mode.enabled
            && user_config
                .user
                .crawling
                .intelligent_mode
                .override_config_limit
        {
            // In intelligent mode with override enabled, use the minimum of user setting and max limit
            crawl_page_limit.min(user_config.user.crawling.intelligent_mode.max_range_limit)
        } else {
            // Otherwise, use user's configured page_range_limit
            user_config.user.crawling.page_range_limit
        };

        tracing::info!(
            "📊 Range calculation settings: input_limit={}, user_limit={}, intelligent_mode={}, effective_limit={}",
            crawl_page_limit,
            user_config.user.crawling.page_range_limit,
            user_config.user.crawling.intelligent_mode.enabled,
            effective_crawl_limit
        );

        // Use site constants for products per page - no more hardcoding!
        let products_per_page = crate::domain::constants::site::PRODUCTS_PER_PAGE as u32;
        // Step 1: Get the last saved product's page_id and index_in_page
        let (max_page_id, max_index_in_page) = self.get_max_page_id_and_index().await?;

        // 상세한 디버깅 로그 추가
        tracing::info!(
            "🔍 DB state check: max_page_id={:?}, max_index_in_page={:?}",
            max_page_id,
            max_index_in_page
        );

        // If no data exists, start from the oldest page (highest page number)
        if max_page_id.is_none() || max_index_in_page.is_none() {
            let start_page = total_pages_on_site;
            let end_page = if start_page >= effective_crawl_limit {
                start_page - effective_crawl_limit + 1
            } else {
                1
            };
            tracing::info!(
                "🆕 No existing data, starting from page {} to {} (limit: {})",
                start_page,
                end_page,
                effective_crawl_limit
            );
            return Ok(Some((start_page, end_page)));
        }

        let max_page_id = max_page_id.unwrap();
        let max_index_in_page = max_index_in_page.unwrap();

        // Step 2: Calculate the last saved product's reverse absolute index
        // Formula: lastSavedIndex = (max_page_id * productsPerPage) + max_index_in_page
        let safe_page_id = u32::try_from(max_page_id).unwrap_or_else(|_| {
            tracing::warn!("negative page_id detected ({}), clamping to 0", max_page_id);
            0
        });
        let safe_index = u32::try_from(max_index_in_page).unwrap_or_else(|_| {
            tracing::warn!(
                "negative index_in_page detected ({}), clamping to 0",
                max_index_in_page
            );
            0
        });
        let last_saved_index = (safe_page_id * products_per_page) + safe_index;

        // Step 3: Calculate the next product index to crawl
        // Formula: nextProductIndex = lastSavedIndex + 1
        let next_product_index = last_saved_index + 1;

        // Step 4: Calculate total products on the site
        // Formula: totalProducts = ((totalPagesOnSite - 1) * productsPerPage) + productsOnLastPage
        let total_products =
            ((total_pages_on_site - 1) * products_per_page) + products_on_last_page;

        // Check if we've already crawled all products
        if next_product_index >= total_products {
            tracing::info!(
                "🏁 All products have been crawled (next_index: {}, total: {})",
                next_product_index,
                total_products
            );
            return Ok(None);
        }

        // Step 5: Convert next product index to website page number
        // Formula: forwardIndex = (totalProducts - 1) - nextProductIndex
        let forward_index = (total_products - 1) - next_product_index;

        // Formula: targetPageNumber = floor(forwardIndex / productsPerPage) + 1
        let target_page_number = (forward_index / products_per_page) + 1;

        // Step 6: Adjust start page to the nearest page that is NOT fully detailed
        // Crawling goes from older pages (higher numbers) to newer pages (lower numbers)
        let mut start_page = target_page_number;
        // Scan backwards (older pages) until we find a page that is not fully detailed.
        // Cap scan to a reasonable window (effective_crawl_limit * 3) to avoid long loops on pathological DBs.
        let scan_cap = (effective_crawl_limit.saturating_mul(3)).max(10);
        let mut scanned: u32 = 0;
        while start_page >= 1 && scanned < scan_cap {
            if !self
                .is_site_page_fully_detailed(start_page, total_pages_on_site, products_on_last_page)
                .await
                .unwrap_or(false)
            {
                break;
            }
            if start_page == 1 {
                break;
            }
            start_page -= 1;
            scanned += 1;
        }
        if scanned > 0 {
            tracing::info!(
                "🔎 Adjusted start_page backward by {} to skip fully detailed pages → {}",
                scanned,
                start_page
            );
        }
        let end_page = if start_page >= effective_crawl_limit {
            start_page - effective_crawl_limit + 1
        } else {
            1
        };

        tracing::info!("📊 Crawling range calculation:");
        tracing::info!(
            "  Last saved: page_id={}, index_in_page={}",
            max_page_id,
            max_index_in_page
        );
        tracing::info!("  Last saved index: {}", last_saved_index);
        tracing::info!("  Next product index: {}", next_product_index);
        tracing::info!("  Total products on site: {}", total_products);
        tracing::info!("  Forward index: {}", forward_index);
        tracing::info!("  Target page: {}", target_page_number);
        tracing::info!(
            "  Crawl range: {} to {} (effective limit: {})",
            start_page,
            end_page,
            effective_crawl_limit
        );

        Ok(Some((start_page, end_page)))
    }

    /// Check if a given site page (`1..=total_pages_on_site`) is fully stored in `product_details`
    /// Uses `product_details` table as source of truth for deduplication decisions.
    /// Check if a given site page (`1..=total_pages_on_site`) is fully stored in `product_details`.
    ///
    /// # Errors
    /// Returns an error if database access fails while counting details.
    pub async fn is_site_page_fully_detailed(
        &self,
        site_page: u32,
        total_pages_on_site: u32,
        products_on_last_page: u32,
    ) -> Result<bool> {
        if site_page == 0 || site_page > total_pages_on_site {
            return Ok(true);
        }
        // Convert site page (1=newest, total_pages_on_site=oldest) to our 0-based page_id
        let page_id: i32 =
            i32::try_from(total_pages_on_site.saturating_sub(site_page)).unwrap_or(i32::MAX);
        // Expected count
        let expected: i32 = if site_page == total_pages_on_site {
            i32::try_from(products_on_last_page).unwrap_or(i32::MAX)
        } else {
            crate::domain::constants::site::PRODUCTS_PER_PAGE
        };
        // Count details for this page_id
        let count: i32 =
            sqlx::query_scalar(r"SELECT COUNT(*) FROM product_details WHERE page_id = ?")
                .bind(page_id)
                .fetch_one(&*self.pool)
                .await
                .unwrap_or(0);
        tracing::debug!(
            "[DetailCoverage] site_page={} (page_id={}) details_count={} expected={}",
            site_page,
            page_id,
            count,
            expected
        );
        Ok(count >= expected && expected > 0)
    }

    /// Check if a specific page range has already been crawled
    /// Check if a specific page range has already been crawled
    ///
    /// # Errors
    /// Returns an error if database access fails while counting products.
    pub async fn is_page_range_crawled(&self, start_page: u32, end_page: u32) -> Result<bool> {
        // Use site constants instead of parameter
        let products_per_page = crate::domain::constants::site::PRODUCTS_PER_PAGE as u32;
        // Convert website page numbers to our internal page_id system
        // Website pages are in reverse order (1 = newest, high number = oldest)
        // Our page_id starts from 0 for the oldest products

        // For the check, we need to see if we have products in the corresponding page_id range
        let start_page_id = if start_page > end_page {
            // Normal case: start_page is higher (older) than end_page
            end_page - 1 // Convert to 0-based page_id
        } else {
            start_page - 1
        };

        let end_page_id = if start_page > end_page {
            start_page - 1
        } else {
            end_page - 1
        };

        let count: i32 = sqlx::query_scalar(
            r"
            SELECT COUNT(*) 
            FROM products 
            WHERE page_id >= ? AND page_id <= ?
            ",
        )
        .bind(i32::try_from(start_page_id).unwrap_or(i32::MAX))
        .bind(i32::try_from(end_page_id).unwrap_or(i32::MAX))
        .fetch_one(&*self.pool)
        .await?;

        // Expected number of products in this range
        let expected_products = (end_page_id - start_page_id + 1) * products_per_page;

        tracing::debug!(
            "Range check: page_id {} to {} has {} products (expected: {})",
            start_page_id,
            end_page_id,
            count,
            expected_products
        );

        let expected_i32 = i32::try_from(expected_products).unwrap_or(i32::MAX);
        Ok(count >= expected_i32)
    }

    /// Analyze database state for system diagnostics
    ///
    /// # Errors
    /// Returns an error if database statistics retrieval fails.
    pub async fn analyze_database_state(
        &self,
    ) -> Result<crate::application::shared_state::DbAnalysisResult> {
        let stats = self.get_database_statistics().await?;

        // Calculate quality score based on available data
        let quality_score = if stats.total_products > 0 { 0.8 } else { 0.0 };

        Ok(crate::application::shared_state::DbAnalysisResult {
            total_products: u32::try_from(stats.total_products).unwrap_or(u32::MAX),
            max_page_id: Some(i32::try_from(stats.total_products).unwrap_or(i32::MAX) / 12), // Assuming 12 products per page
            max_index_in_page: Some(11), // 0-indexed, so max is 11
            quality_score,
            analyzed_at: chrono::Utc::now(),
            cached_at: std::time::Instant::now(),
            is_empty: stats.total_products == 0,
            is_valid: true,
        })
    }

    /// 제품 총 개수 조회 (Backend-Only CRUD 패턴)
    ///
    /// # Errors
    /// Returns an error if the query fails.
    pub async fn count_products(&self) -> Result<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
            .fetch_one(&*self.pool)
            .await?;
        Ok(count)
    }

    /// 최근 업데이트된 제품들 조회 (Backend-Only CRUD 패턴)
    ///
    /// # Errors
    /// Returns an error if the query or row decoding fails.
    pub async fn get_latest_updated_products(&self, limit: u32) -> Result<Vec<Product>> {
        let rows = sqlx::query(
            r"
            SELECT url, manufacturer, model, certificate_id, page_id, index_in_page, 
                   created_at, updated_at
            FROM products 
            ORDER BY updated_at DESC 
            LIMIT ?
            ",
        )
        .bind(i32::try_from(limit).unwrap_or(i32::MAX))
        .fetch_all(&*self.pool)
        .await?;

        let products = rows
            .into_iter()
            .map(|row| Product {
                id: None, // products 테이블에는 id 컬럼이 없음
                url: row.get("url"),
                manufacturer: row.get("manufacturer"),
                model: row.get("model"),
                certificate_id: row.get("certificate_id"),
                page_id: row.get("page_id"),
                index_in_page: row.get("index_in_page"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

        Ok(products)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::product::ProductDetail;
    use chrono::Utc;
    use sqlx::SqlitePool;

    async fn create_test_db() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        
        // Create the product_details table for testing
        sqlx::query(r"
            CREATE TABLE IF NOT EXISTS product_details (
                url TEXT PRIMARY KEY,
                page_id INTEGER,
                index_in_page INTEGER,
                id TEXT,
                manufacturer TEXT,
                model TEXT,
                device_type TEXT,
                certificate_id TEXT,
                certification_date TEXT,
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
                primary_device_type_ids TEXT,
                application_categories TEXT,
                description TEXT,
                compliance_document_url TEXT,
                program_type TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(page_id, index_in_page)
            )
        ")
        .execute(&pool)
        .await
        .unwrap();

        // Create the products table for testing (order must match INSERT statement)
        sqlx::query(r"
            CREATE TABLE IF NOT EXISTS products (
                id TEXT,
                url TEXT PRIMARY KEY,
                manufacturer TEXT,
                model TEXT,
                certificate_id TEXT,
                page_id INTEGER,
                index_in_page INTEGER,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(page_id, index_in_page)
            )
        ")
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    fn create_test_product_detail(url: &str, page_id: Option<i32>, index_in_page: Option<i32>) -> ProductDetail {
        ProductDetail {
            url: url.to_string(),
            page_id,
            index_in_page,
            id: None,
            manufacturer: Some("Test Manufacturer".to_string()),
            model: Some("Test Model".to_string()),
            device_type: Some("Light Bulb".to_string()),
            certificate_id: Some("TEST123".to_string()),
            certification_date: Some("2024-01-01".to_string()),
            software_version: Some("1.0.0".to_string()),
            hardware_version: Some("1.0.0".to_string()),
            vid: Some(1234),
            pid: Some(5678),
            family_sku: Some("FAM-SKU".to_string()),
            family_variant_sku: Some("FAM-VAR-SKU".to_string()),
            firmware_version: Some("1.0.0".to_string()),
            family_id: Some("FAM-ID".to_string()),
            tis_trp_tested: Some("Yes".to_string()),
            specification_version: Some("1.0".to_string()),
            transport_interface: Some("WiFi".to_string()),
            primary_device_type_ids: Some(vec![256, 257]),
            application_categories: Some("Lighting".to_string()),
            description: Some("Test product description".to_string()),
            compliance_document_url: Some("https://example.com/doc.pdf".to_string()),
            program_type: Some("Matter".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_bulk_create_product_details() {
        let pool = create_test_db().await;
        let repo = IntegratedProductRepository::new(pool);
        
        // Create test products
        let products = vec![
            create_test_product_detail("https://example.com/product1", Some(1), Some(0)),
            create_test_product_detail("https://example.com/product2", Some(1), Some(1)),
            create_test_product_detail("https://example.com/product3", Some(1), Some(2)),
        ];
        
        // Test bulk insert
        let (updated, created) = repo.bulk_create_or_update_product_details(&products).await.unwrap();
        
        assert_eq!(updated, 0, "Should have 0 updates for new records");
        assert_eq!(created, 3, "Should have created 3 new records");
        
        // Verify records were inserted
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_details")
            .fetch_one(&*repo.pool)
            .await
            .unwrap();
        assert_eq!(count, 3, "Should have 3 records in the database");
    }

    #[tokio::test]
    async fn test_bulk_update_product_details() {
        let pool = create_test_db().await;
        let repo = IntegratedProductRepository::new(pool);
        
        // First, insert some products
        let initial_products = vec![
            create_test_product_detail("https://example.com/product1", Some(1), Some(0)),
            create_test_product_detail("https://example.com/product2", Some(1), Some(1)),
        ];
        
        let (_, created) = repo.bulk_create_or_update_product_details(&initial_products).await.unwrap();
        assert_eq!(created, 2);
        
        // Now update the same products with different data
        let mut updated_products = initial_products.clone();
        updated_products[0].manufacturer = Some("Updated Manufacturer 1".to_string());
        updated_products[1].manufacturer = Some("Updated Manufacturer 2".to_string());
        
        let (updated, created) = repo.bulk_create_or_update_product_details(&updated_products).await.unwrap();
        
        assert_eq!(updated, 2, "Should have updated 2 existing records");
        assert_eq!(created, 0, "Should have created 0 new records");
        
        // Verify the updates
        let manufacturer: String = sqlx::query_scalar("SELECT manufacturer FROM product_details WHERE url = ?")
            .bind("https://example.com/product1")
            .fetch_one(&*repo.pool)
            .await
            .unwrap();
        assert_eq!(manufacturer, "Updated Manufacturer 1");
    }

    #[tokio::test] 
    async fn test_bulk_empty_list() {
        let pool = create_test_db().await;
        let repo = IntegratedProductRepository::new(pool);
        
        // Test empty list
        let (updated, created) = repo.bulk_create_or_update_product_details(&[]).await.unwrap();
        
        assert_eq!(updated, 0, "Should have 0 updates for empty list");
        assert_eq!(created, 0, "Should have 0 creations for empty list");
        
        // Verify no records were created
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_details")
            .fetch_one(&*repo.pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "Should have 0 records in the database");
    }
}
