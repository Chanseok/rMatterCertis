//! Repository implementation for integrated product operations
//!
//! This module provides database operations that combine product and product_detail
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
use tracing::{debug, info};

/// Repository for the integrated schema (products + product_details + vendors + crawling_results)
#[derive(Clone)]
pub struct IntegratedProductRepository {
    pool: Arc<SqlitePool>,
}

impl IntegratedProductRepository {
    /// Normalize URL for consistent storage and comparison
    /// - Trims whitespace
    /// - Lowercases the hostname
    /// - Leaves path/query as-is (CSA URLs are case-sensitive there)
    fn normalize_url(url: &str) -> String {
        let trimmed = url.trim();
        if let Ok(mut parsed) = url::Url::parse(trimmed) {
            if let Some(host) = parsed.host_str() {
                let lower = host.to_ascii_lowercase();
                // Only replace host if changed
                if lower != host {
                    // Rebuild with lowercased host
                    let _ = parsed.set_host(Some(&lower));
                }
            }
            parsed.to_string()
        } else {
            // Fallback to trimmed original
            trimmed.to_string()
        }
    }
    /// Expose underlying pool reference (read-only operations convenience)
    pub fn pool(&self) -> &sqlx::SqlitePool {
        &self.pool
    }
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool: Arc::new(pool),
        }
    }

    /// Danger zone: completely remove all product related data so we can rebuild
    /// indexing semantics (Plan B). This is intentionally explicit and NOT called
    /// automatically; the frontend must invoke the dedicated reset command.
    /// Returns (products_deleted, product_details_deleted).
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
    /// Returns: (was_updated: bool, was_created: bool)
    pub async fn create_or_update_product(&self, product: &Product) -> Result<(bool, bool)> {
        let now = chrono::Utc::now();
    // Normalize URL to ensure consistent storage and matching
    let normalized_url = Self::normalize_url(&product.url);

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
                // 📝 실제 변경사항이 있을 때만 업데이트
                sqlx::query(
                    r"
                    UPDATE products 
                    SET manufacturer = ?, model = ?, certificate_id = ?, page_id = ?, index_in_page = ?, updated_at = ?
                    WHERE url = ?
                    ",
                )
                .bind(&product.manufacturer)
                .bind(&product.model)
                .bind(&product.certificate_id)
                .bind(product.page_id)
                .bind(product.index_in_page)
                .bind(now)
                .bind(&normalized_url)
                .execute(&*self.pool)
                .await?;

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
            .bind(product.page_id)
            .bind(product.index_in_page)
            .bind(now)
            .bind(now)
            .execute(&*self.pool)
            .await?;

            // info!("🆕 New product created: {}", product.model.as_deref().unwrap_or("Unknown"));
            Ok((false, true)) // updated=false, created=true
        }
    }

    /// Insert or update detailed product specifications
    /// 🎯 지능적 비교: 빈 필드 채움 및 실제 변경사항만 업데이트
    /// Returns: (was_updated: bool, was_created: bool)
    pub async fn create_or_update_product_detail(
        &self,
        detail: &ProductDetail,
    ) -> Result<(bool, bool)> {
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
            enum BindValue<'a> {
                OptStr(&'a Option<String>),
                OptI32(&'a Option<i32>),
            }
            let mut binds: Vec<BindValue> = Vec::new();
            let mut change_kinds: Vec<String> = Vec::new(); // human readable change descriptors
            let verbose = std::env::var("MC_PERSIST_VERBOSE")
                .ok()
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);

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
            fill_or_change_opt_str!(primary_device_type_id, "primary_device_type_id");
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

            if !updates.is_empty() {
                // Optional verbose diff logging before executing update
                if verbose {
                    // helper closures for formatting
                    fn fmt_opt_str(v: &Option<String>) -> String {
                        v.as_deref()
                            .map(|s| if s.is_empty() { "" } else { s })
                            .unwrap_or("∅")
                            .to_string()
                    }
                    fn fmt_opt_i32(v: &Option<i32>) -> String {
                        v.map(|n| n.to_string()).unwrap_or_else(|| "∅".to_string())
                    }
                    let mut diffs: Vec<String> = Vec::new();
                    for kind in &change_kinds {
                        if let Some((k, col)) = kind.split_once(':') {
                            let (old_v, new_v) = match col {
                                "device_type" => (
                                    fmt_opt_str(&existing_detail.device_type),
                                    fmt_opt_str(&detail.device_type),
                                ),
                                "certification_date" => (
                                    fmt_opt_str(&existing_detail.certification_date),
                                    fmt_opt_str(&detail.certification_date),
                                ),
                                "software_version" => (
                                    fmt_opt_str(&existing_detail.software_version),
                                    fmt_opt_str(&detail.software_version),
                                ),
                                "hardware_version" => (
                                    fmt_opt_str(&existing_detail.hardware_version),
                                    fmt_opt_str(&detail.hardware_version),
                                ),
                                "description" => (
                                    fmt_opt_str(&existing_detail.description),
                                    fmt_opt_str(&detail.description),
                                ),
                                "firmware_version" => (
                                    fmt_opt_str(&existing_detail.firmware_version),
                                    fmt_opt_str(&detail.firmware_version),
                                ),
                                "specification_version" => (
                                    fmt_opt_str(&existing_detail.specification_version),
                                    fmt_opt_str(&detail.specification_version),
                                ),
                                "transport_interface" => (
                                    fmt_opt_str(&existing_detail.transport_interface),
                                    fmt_opt_str(&detail.transport_interface),
                                ),
                                "application_categories" => (
                                    fmt_opt_str(&existing_detail.application_categories),
                                    fmt_opt_str(&detail.application_categories),
                                ),
                                "compliance_document_url" => (
                                    fmt_opt_str(&existing_detail.compliance_document_url),
                                    fmt_opt_str(&detail.compliance_document_url),
                                ),
                                "program_type" => (
                                    fmt_opt_str(&existing_detail.program_type),
                                    fmt_opt_str(&detail.program_type),
                                ),
                                "family_sku" => (
                                    fmt_opt_str(&existing_detail.family_sku),
                                    fmt_opt_str(&detail.family_sku),
                                ),
                                "family_variant_sku" => (
                                    fmt_opt_str(&existing_detail.family_variant_sku),
                                    fmt_opt_str(&detail.family_variant_sku),
                                ),
                                "family_id" => (
                                    fmt_opt_str(&existing_detail.family_id),
                                    fmt_opt_str(&detail.family_id),
                                ),
                                "tis_trp_tested" => (
                                    fmt_opt_str(&existing_detail.tis_trp_tested),
                                    fmt_opt_str(&detail.tis_trp_tested),
                                ),
                                "primary_device_type_id" => (
                                    fmt_opt_str(&existing_detail.primary_device_type_id),
                                    fmt_opt_str(&detail.primary_device_type_id),
                                ),
                                "certificate_id" => (
                                    fmt_opt_str(&existing_detail.certificate_id),
                                    fmt_opt_str(&detail.certificate_id),
                                ),
                                "vid" => {
                                    (fmt_opt_i32(&existing_detail.vid), fmt_opt_i32(&detail.vid))
                                }
                                "pid" => {
                                    (fmt_opt_i32(&existing_detail.pid), fmt_opt_i32(&detail.pid))
                                }
                                "manufacturer" => (
                                    fmt_opt_str(&existing_detail.manufacturer),
                                    fmt_opt_str(&detail.manufacturer),
                                ),
                                "model" => (
                                    fmt_opt_str(&existing_detail.model),
                                    fmt_opt_str(&detail.model),
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

                let mut sql_query = sqlx::query(&query);
                for bind in binds {
                    sql_query = match bind {
                        BindValue::OptStr(v) => sql_query.bind(v),
                        BindValue::OptI32(v) => sql_query.bind(v),
                    };
                }
                sql_query = sql_query.bind(now).bind(&detail.url);

                sql_query.execute(&*self.pool).await?;
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
                Ok((true, false)) // updated=true, created=false
            } else {
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

                        sqlx::query(
                                r"
                                INSERT INTO product_details 
                                (url, page_id, index_in_page, id, manufacturer, model, device_type,
                                 certificate_id, certification_date, software_version, hardware_version,
                                 vid, pid, family_sku, family_variant_sku, firmware_version, family_id,
                                 tis_trp_tested, specification_version, transport_interface, 
                                 primary_device_type_id, application_categories, description,
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
                                    primary_device_type_id = excluded.primary_device_type_id,
                                    application_categories = excluded.application_categories,
                                    description = excluded.description,
                                    compliance_document_url = excluded.compliance_document_url,
                                    program_type = excluded.program_type,
                                    updated_at = excluded.updated_at
                                ",
                        )
            .bind(&detail.url)
            .bind(detail.page_id)
            .bind(detail.index_in_page)
            .bind(&generated_id)  // Use generated_id instead of detail.id
            .bind(&detail.manufacturer)
            .bind(&detail.model)
            .bind(&detail.device_type)
            .bind(&detail.certificate_id)
            .bind(&detail.certification_date)
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
            .bind(detail.primary_device_type_id.clone())
            .bind(&detail.application_categories)
            .bind(&detail.description)
            .bind(&detail.compliance_document_url)
            .bind(&detail.program_type)
            .bind(now)
            .bind(now)
            .execute(&*self.pool)
            .await?;

            let verbose = std::env::var("MC_PERSIST_VERBOSE")
                .ok()
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
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
            Ok((false, true)) // updated=false, created=true
        }
    }

    /// 빠른 통계: product_details 전체 개수, page_id 범위, 마지막 업데이트 시각
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

        match row {
            Some(row) => Ok(Some(Product {
                id: None, // products 테이블에는 id 컬럼이 없음
                url: row.get("url"),
                manufacturer: row.get("manufacturer"),
                model: row.get("model"),
                certificate_id: row.get("certificate_id"),
                page_id: row.get("page_id"),
                index_in_page: row.get("index_in_page"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })),
            None => Ok(None),
        }
    }

    /// Get product with details by URL
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
    pub async fn get_product_detail_by_url(&self, url: &str) -> Result<Option<ProductDetail>> {
    let normalized_url = Self::normalize_url(url);
    let row = sqlx::query(
            r"
            SELECT url, page_id, index_in_page, id, manufacturer, model, device_type,
                   certificate_id, certification_date, software_version, hardware_version,
                   vid, pid, family_sku, family_variant_sku, firmware_version, family_id,
                   tis_trp_tested, specification_version, transport_interface, 
                   primary_device_type_id, application_categories, description,
                   compliance_document_url, program_type, created_at, updated_at
            FROM product_details WHERE url = ?
            ",
        )
    .bind(&normalized_url)
        .fetch_optional(&*self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(ProductDetail {
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
                primary_device_type_id: row.get("primary_device_type_id"),
                application_categories: row.get("application_categories"),
                description: row.get("description"),
                compliance_document_url: row.get("compliance_document_url"),
                program_type: row.get("program_type"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })),
            None => Ok(None),
        }
    }

    /// Search products with criteria and pagination
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
            "".to_string()
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
                   pd.primary_device_type_id, pd.application_categories, pd.description,
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
                        primary_device_type_id: row.get("primary_device_type_id"),
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
            manufacturer: product_json["manufacturer"].as_str().map(|s| s.to_string()),
            model: product_json["model"].as_str().map(|s| s.to_string()),
            certificate_id: product_json["certification_id"]
                .as_str()
                .map(|s| s.to_string()),
            page_id: product_json["page_id"].as_i64().map(|i| i as i32),
            index_in_page: product_json["index_in_page"].as_i64().map(|i| i as i32),
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
            let vid = product_json["vid"].as_i64().map(|i| i as i32);
            let pid = product_json["pid"].as_i64().map(|i| i as i32);

            let detail = ProductDetail {
                url: url.clone(),
                page_id: basic_product.page_id,
                index_in_page: basic_product.index_in_page,
                id: product_json["id"].as_str().map(|s| s.to_string()),
                manufacturer: basic_product.manufacturer.clone(),
                model: basic_product.model.clone(),
                device_type: product_json["device_type"].as_str().map(|s| s.to_string()),
                certificate_id: basic_product.certificate_id.clone(),
                certification_date: product_json["certification_date"]
                    .as_str()
                    .map(|s| s.to_string()),
                software_version: product_json["software_version"]
                    .as_str()
                    .map(|s| s.to_string()),
                hardware_version: product_json["hardware_version"]
                    .as_str()
                    .map(|s| s.to_string()),
                vid,
                pid,
                family_sku: product_json["family_sku"].as_str().map(|s| s.to_string()),
                family_variant_sku: product_json["family_variant_sku"]
                    .as_str()
                    .map(|s| s.to_string()),
                firmware_version: product_json["firmware_version"]
                    .as_str()
                    .map(|s| s.to_string()),
                family_id: product_json["family_id"].as_str().map(|s| s.to_string()),
                tis_trp_tested: product_json["tis_trp_tested"]
                    .as_str()
                    .map(|s| s.to_string()),
                specification_version: product_json["specification_version"]
                    .as_str()
                    .map(|s| s.to_string()),
                transport_interface: product_json["transport_interface"]
                    .as_str()
                    .map(|s| s.to_string()),
                primary_device_type_id: product_json["primary_device_type_id"]
                    .as_str()
                    .map(|s| s.to_string()),
                application_categories: product_json["application_categories"].as_array().map(
                    |arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    },
                ),
                description: product_json["description"].as_str().map(|s| s.to_string()),
                compliance_document_url: product_json["compliance_document_url"]
                    .as_str()
                    .map(|s| s.to_string()),
                program_type: product_json["program_type"].as_str().map(|s| s.to_string()),
                created_at: basic_product.created_at,
                updated_at: basic_product.updated_at,
            };

            self.create_or_update_product_detail(&detail).await?;
        }

        Ok(is_new)
    }

    /// Get all vendors
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
            (total_details as f32 / total_products as f32) * 100.0
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
            completion_rate: completion_rate as f64,
        })
    }

    /// Get products without details (for crawling prioritization)
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

    /// Get only product URLs (no details yet) within a specific set of page_ids
    /// Returns at most `limit` URLs. If `pages` is empty returns empty Vec.
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
                placeholders.push_str(",");
            }
            placeholders.push_str("?");
        }
        let query = format!(
            "SELECT p.url FROM products p LEFT JOIN product_details pd ON p.url = pd.url \
             WHERE pd.url IS NULL AND p.page_id IN ({}) ORDER BY p.page_id DESC, p.index_in_page ASC LIMIT ?",
            placeholders
        );
        let mut q = sqlx::query(&query);
        for p in pages {
            q = q.bind(*p as i32);
        }
        q = q.bind(limit);
        let rows = q.fetch_all(&*self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|r| r.get::<String, _>("url"))
            .collect())
    }

    /// Get all products from the database
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

        if let Some(row) = row {
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
        } else {
            Ok(None)
        }
    }

    // ===============================
    // CRAWLING RANGE CALCULATION
    // ===============================

    /// Get the maximum page_id and index_in_page from the database
    /// Returns (max_page_id, max_index_in_page) or (None, None) if no data
    ///
    /// This finds the product with the highest page_id, and among those products,
    /// the one with the highest index_in_page. This represents the "last" product
    /// we've crawled in the reverse chronological order.
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

        if let Some(row) = row {
            let max_page_id: Option<i32> = row.get("page_id");
            let max_index_in_page: Option<i32> = row.get("index_in_page");
            tracing::debug!(
                "📊 Found last saved product: page_id={:?}, index_in_page={:?}",
                max_page_id,
                max_index_in_page
            );
            Ok((max_page_id, max_index_in_page))
        } else {
            tracing::debug!("📊 No products found with page_id and index_in_page");
            Ok((None, None))
        }
    }

    /// Get the count of products stored in the database
    pub async fn get_product_count(&self) -> Result<i32> {
        let count: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
            .fetch_one(&*self.pool)
            .await?;
        Ok(count)
    }

    /// Calculate the next crawling range based on local DB state and site information
    /// Returns (start_page, end_page) for the next crawling range
    ///
    /// This implements the logic from prompts6:
    /// 1. Get the last saved product's reverse absolute index
    /// 2. Calculate the next product index to crawl
    /// 3. Convert to website page numbers
    /// 4. Apply crawl page limit (respecting user settings)
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
        let last_saved_index = (max_page_id as u32 * products_per_page) + max_index_in_page as u32;

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

    /// Check if a given site page (1..=total_pages_on_site) is fully stored in product_details
    /// Uses product_details table as source of truth for deduplication decisions.
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
        let page_id: i32 = (total_pages_on_site - site_page) as i32;
        // Expected count
        let expected: i32 = if site_page == total_pages_on_site {
            products_on_last_page as i32
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
        .bind(start_page_id as i32)
        .bind(end_page_id as i32)
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

        Ok(count >= expected_products as i32)
    }

    /// Analyze database state for system diagnostics
    pub async fn analyze_database_state(
        &self,
    ) -> Result<crate::application::shared_state::DbAnalysisResult> {
        let stats = self.get_database_statistics().await?;

        // Calculate quality score based on available data
        let quality_score = if stats.total_products > 0 { 0.8 } else { 0.0 };

        Ok(crate::application::shared_state::DbAnalysisResult {
            total_products: stats.total_products as u32,
            max_page_id: Some(stats.total_products as i32 / 12), // Assuming 12 products per page
            max_index_in_page: Some(11),                         // 0-indexed, so max is 11
            quality_score,
            analyzed_at: chrono::Utc::now(),
            cached_at: std::time::Instant::now(),
            is_empty: stats.total_products == 0,
            is_valid: true,
        })
    }

    /// 제품 총 개수 조회 (Backend-Only CRUD 패턴)
    pub async fn count_products(&self) -> Result<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
            .fetch_one(&*self.pool)
            .await?;
        Ok(count)
    }

    /// 최근 업데이트된 제품들 조회 (Backend-Only CRUD 패턴)
    pub async fn get_latest_updated_products(&self, limit: u32) -> Result<Vec<Product>> {
        let _offset = 0;
        let rows = sqlx::query(
            r"
            SELECT url, manufacturer, model, certificate_id, page_id, index_in_page, 
                   created_at, updated_at
            FROM products 
            ORDER BY updated_at DESC 
            LIMIT ?
            ",
        )
        .bind(limit as i32)
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
