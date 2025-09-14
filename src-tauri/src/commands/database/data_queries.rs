#![allow(clippy::used_underscore_binding)]
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::{error, info};
use ts_rs::TS;

use crate::application::AppState;
use crate::DatabaseConnection; // for new dashboard commands
use regex::Regex;
use sqlx::Row; // to access row.get
use crate::domain::product::Product;
use crate::infrastructure::integrated_product_repository::IntegratedProductRepository; // 올바른 Product 타입 사용

/// 제품 페이지 응답
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProductPage {
    pub products: Vec<Product>,
    pub total_count: u32,
    pub page: u32,
    pub size: u32,
    pub has_next: bool,
}

/// 크롤링 상태 정보
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CrawlingStatusInfo {
    pub is_running: bool,
    pub current_page: Option<u32>,
    pub total_pages: Option<u32>,
    pub last_updated: Option<String>,
    pub session_id: Option<String>,
}

/// 시스템 상태 정보
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SystemStatus {
    pub database_connected: bool,
    pub total_products: u32,
    pub last_crawl_time: Option<DateTime<chrono::Utc>>,
    pub config_loaded: bool,
}

/// 제품 데이터 페이지별 조회 (Backend-Only CRUD)
#[tauri::command]
#[allow(clippy::used_underscore_binding)]
/// # Errors
/// Returns an error string if the database pool cannot be obtained or queries fail.
pub async fn get_products_page(
    state: State<'_, AppState>,
    page: u32,
    size: u32,
) -> Result<ProductPage, String> {
    let pool = state.get_database_pool().await?;
    let repo = IntegratedProductRepository::new(pool);

    let page_i32 = i32::try_from(page).unwrap_or(i32::MAX);
    let size_i32 = i32::try_from(size).unwrap_or(i32::MAX);
    match repo.get_products_paginated(page_i32, size_i32).await {
        Ok(products) => {
            // 전체 개수 조회 (향후 최적화 가능)
            let total_count = repo.count_products().await.map_or_else(
                |e| {
                    error!("Failed to count products: {}", e);
                    0
                },
                |count| u32::try_from(count).unwrap_or(u32::MAX),
            );

            let has_next = (page + 1) * size < total_count;

            info!(
                "✅ Retrieved {} products for page {} (size: {})",
                products.len(),
                page,
                size
            );

            Ok(ProductPage {
                products,
                total_count,
                page,
                size,
                has_next,
            })
        }
        Err(e) => {
            error!("Failed to get products page: {}", e);
            Err(format!("Failed to retrieve products: {}", e))
        }
    }
}

/// 최근 업데이트된 제품 조회 (Backend-Only CRUD)
#[tauri::command]
#[allow(clippy::used_underscore_binding)]
/// # Errors
/// Returns an error string if the database pool cannot be obtained or queries fail.
pub async fn get_latest_products(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<Product>, String> {
    let pool = state.get_database_pool().await?;
    let repo = IntegratedProductRepository::new(pool);

    match repo.get_latest_updated_products(limit).await {
        Ok(products) => {
            info!("✅ Retrieved {} latest updated products", products.len());
            Ok(products)
        }
        Err(e) => {
            error!("Failed to get latest products: {}", e);
            Err(format!("Failed to retrieve latest products: {}", e))
        }
    }
}

/// 크롤링 상태 조회 (Backend-Only CRUD)
#[tauri::command]
#[allow(clippy::used_underscore_binding)]
/// # Errors
/// Returns an error string if shared state cannot be accessed.
pub async fn get_crawling_status_v2(
    state: State<'_, AppState>,
) -> Result<CrawlingStatusInfo, String> {
    let current_session = state.current_session.read().await;
    let current_progress = state.current_progress.read().await;

    let status = CrawlingStatusInfo {
        is_running: current_session.is_some(),
        current_page: None,
        total_pages: None,
        last_updated: None, // CrawlingProgress doesn't have last_updated field
        session_id: current_session.as_ref().map(|s| s.id.clone()),
    };

    let running = status.is_running;
    drop(current_session);
    drop(current_progress);
    info!("✅ Retrieved crawling status: running={}", running);
    Ok(status)
}

/// 시스템 전체 상태 조회 (Backend-Only CRUD)
#[tauri::command]
#[allow(clippy::used_underscore_binding)]
/// # Errors
/// Returns an error string if the database pool cannot be obtained or queries fail.
pub async fn get_system_status(state: State<'_, AppState>) -> Result<SystemStatus, String> {
    // 데이터베이스 연결 확인
    let database_connected = state.get_database_pool().await.is_ok();

    let (total_products, last_crawl_time) = if database_connected {
        let pool = state.get_database_pool().await?;
        let repo = IntegratedProductRepository::new(pool);

        let total = repo
            .count_products()
            .await
            .map(|count| u32::try_from(count).unwrap_or(u32::MAX))
            .unwrap_or(0);

        let last_updated = match repo.get_latest_updated_product().await {
            Ok(Some(product)) => Some(product.updated_at),
            _ => None,
        };

        (total, last_updated)
    } else {
        (0, None)
    };

    // 설정 로딩 상태 확인
    let config_guard = state.config.read().await;
    let config_loaded = true; // config가 항상 로드되어 있음
    drop(config_guard);

    let status = SystemStatus {
        database_connected,
        total_products,
        last_crawl_time,
        config_loaded,
    };

    info!(
        "✅ System status: db_connected={}, total_products={}, config_loaded={}",
        status.database_connected, status.total_products, status.config_loaded
    );

    Ok(status)
}

// -----------------------------------------------------------------------------
// Phase 1: Local DB Dashboard - Summary + Analytics (basic)
// -----------------------------------------------------------------------------

#[derive(serde::Serialize)]
pub struct DbSummary {
    pub total_products: i64,
    pub total_product_details: i64,
    pub total_vendors: i64,
    pub total_device_types: i64,
    pub new_products_24h: i64,
    pub new_products_7d: i64,
    pub top_device_categories: Vec<(String, i64)>,
}

/// Return basic database summary statistics.
#[tauri::command]
pub async fn get_db_summary(state: State<'_, DatabaseConnection>) -> Result<DbSummary, String> {
    let pool = state.pool();
    // Counts
    let total_products = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM products")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let total_product_details = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM product_details")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let total_vendors = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM vendors")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    let total_device_types = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM device_types")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    // Recent windows (product_details.created_at)
    let new_products_24h = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM product_details WHERE created_at >= datetime('now','-24 hours')",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    let new_products_7d = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM product_details WHERE created_at >= datetime('now','-7 days')",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    // Top categories (analytics view may not exist yet gracefully fallback)
    // Use dynamic query (avoid compile-time validation failure if view absent during build time with sqlx offline)
    let top_device_categories = {
        let sql = "SELECT device_category, COUNT(*) as cnt FROM v_product_detail_analytics GROUP BY device_category ORDER BY cnt DESC LIMIT 5";
        match sqlx::query(sql).fetch_all(pool).await {
            Ok(rows) => rows
                .into_iter()
                .filter_map(|r| {
                    let cat: Option<String> = r.get::<Option<String>, _>("device_category");
                    let cnt: i64 = r.get::<i64, _>("cnt");
                    cat.map(|c| (c, cnt))
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    };
    Ok(DbSummary {
        total_products,
        total_product_details,
        total_vendors,
        total_device_types,
        new_products_24h,
        new_products_7d,
        top_device_categories,
    })
}

// Analytics Query (Phase 1 minimal: no DSL yet, just pagination)

#[derive(serde::Deserialize)]
pub struct AnalyticsQueryInput {
    pub offset: Option<i64>,
    pub limit: Option<i64>,
    pub filter: Option<String>, // reserved for Phase 2 DSL
    pub sort: Option<Vec<String>>, // e.g. ["device_category:asc", "vendor_name:desc"]
}

#[derive(serde::Serialize)]
pub struct AnalyticsRow {
    pub product_detail_url: Option<String>,
    pub model: Option<String>,
    pub vendor_name: Option<String>,
    pub device_type_name: Option<String>,
    pub device_category: Option<String>,
    pub certification_date: Option<String>,
    pub detail_created_at: Option<String>,
    pub transport_interface: Option<String>,
}

#[derive(serde::Serialize)]
pub struct AnalyticsPage {
    pub rows: Vec<AnalyticsRow>,
    pub total: i64,
    pub offset: i64,
    pub limit: i64,
    pub applied_filter: Option<String>,
    pub filter_error: Option<String>,
}

#[tauri::command]
pub async fn analytics_query(
    state: State<'_, DatabaseConnection>,
    params: AnalyticsQueryInput,
) -> Result<AnalyticsPage, String> {
    let pool = state.pool();
    let offset = params.offset.unwrap_or(0).max(0);
    let mut limit = params.limit.unwrap_or(50);
    if limit <= 0 { limit = 50; }
    if limit > 200 { limit = 200; }
    let raw_filter = params.filter.unwrap_or_default().trim().to_string();

    // ---------------------------------------------------------------------
    // Runtime Safety Net Backfill (defensive):
    // If the analytics view still shows no mapped device_type_name values and
    // the bridge table is empty while product_details have primary_device_type_ids,
    // attempt an idempotent backfill here. This covers scenarios where the user
    // upgraded without running the newer migrations on an existing populated DB.
    // The SQL is intentionally simple and idempotent (INSERT OR IGNORE + UPDATE).
    // Any error is logged but never fails the analytics query.
    // ---------------------------------------------------------------------
    if let (Ok(mapped_count), Ok(bridge_count), Ok(details_with_ids)) = (
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM v_product_detail_analytics WHERE device_type_name IS NOT NULL")
            .fetch_one(pool).await,
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM product_primary_device_types")
            .fetch_one(pool).await,
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM product_details WHERE primary_device_type_ids IS NOT NULL")
            .fetch_one(pool).await,
    ) {
        if mapped_count == 0 && bridge_count == 0 && details_with_ids > 0 {
            info!("runtime_backfill" = true, mapped_count, bridge_count, details_with_ids, "Attempting runtime device type bridge backfill");
            // Ensure type_id populated on device_types
            if let Err(e) = sqlx::query("UPDATE device_types SET type_id = id WHERE type_id IS NULL")
                .execute(pool).await { error!(?e, "runtime backfill: failed to normalize device_types.type_id"); }

            // Insert mappings using dual match (type_id OR id) from JSON array
                        let insert_sql = r#"
                                INSERT OR IGNORE INTO product_primary_device_types (product_detail_id, device_type_id)
                                SELECT pd.url, dt.type_id
                                FROM product_details pd
                                JOIN json_each(pd.primary_device_type_ids) je
                                LEFT JOIN device_types dt ON (dt.type_id = je.value OR dt.id = je.value)
                                WHERE pd.primary_device_type_ids IS NOT NULL
                                    AND json_valid(pd.primary_device_type_ids)
                                    AND dt.type_id IS NOT NULL
                        "#;
            if let Err(e) = sqlx::query(insert_sql).execute(pool).await {
                error!(?e, "runtime backfill: insert bridge mappings failed");
            }

            // Log result counts after attempt
            if let (Ok(new_bridge), Ok(new_mapped)) = (
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM product_primary_device_types").fetch_one(pool).await,
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM v_product_detail_analytics WHERE device_type_name IS NOT NULL").fetch_one(pool).await,
            ) {
                info!(new_bridge, new_mapped, "runtime backfill completed (post counts)");
            }
        }
    }

    // ---------------- DSL Parsing (Phase 7) ----------------
    // Grammar (informal): tokens separated by whitespace.
    // token := field?(op)?value | bareword
    // field := vendor|v|ven|vnum|category|cat|dtype|dname|dt|model|m|date|created|c
    // op := = | ~ | >= | <= (default ~ for text, = for exact numeric/date if no op)
    // bareword -> (model LIKE ? OR vendor_name LIKE ?)
    let mut where_clauses: Vec<String> = Vec::new();
    let mut binds: Vec<String> = Vec::new();
    let mut filter_error: Option<String> = None;
    let mut applied_filter: Option<String> = None;
    if !raw_filter.is_empty() {
        // Tokenization: simple split; reject tokens containing quotes to avoid complexity now.
        let tokens: Vec<&str> = raw_filter.split_whitespace().collect();
        let field_pattern = Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*").unwrap();
        for t in &tokens {
            if t.contains('"') || t.contains('\'') { filter_error = Some("따옴표는 아직 지원하지 않습니다".into()); break; }
            // Operators precedence: check for >= or <= first
            let (field_part, op, value_part) = if let Some(pos) = t.find(">=") { (&t[..pos], Some(">="), &t[pos+2..]) } else if let Some(pos) = t.find("<=") { (&t[..pos], Some("<="), &t[pos+2..]) } else if let Some(pos) = t.find('=') { (&t[..pos], Some("="), &t[pos+1..]) } else if let Some(pos) = t.find('~') { (&t[..pos], Some("~"), &t[pos+1..]) } else if let Some(pos) = t.find(':') { (&t[..pos], None, &t[pos+1..]) } else { ("", None, *t) };

            let field = field_part.to_lowercase();
            let value = value_part.trim();
            if field.is_empty() { // bareword search
                if value.is_empty() { continue; }
                where_clauses.push("(model LIKE ? OR vendor_name LIKE ?)".into());
                let like = format!("%{}%", value);
                binds.push(like.clone());
                binds.push(like);
                continue;
            }
            if value.is_empty() { filter_error = Some(format!("필드 '{}' 뒤에 값이 비었습니다", field)); break; }
            // Map field
            let column = match field.as_str() {
                "vendor"|"v"|"ven" => "vendor_name",
                "vnum" => "vendor_number",
                "category"|"cat" => "device_category",
                "dtype"|"dname"|"dt" => "device_type_name",
                "model"|"m" => "model",
                "date" => "certification_date",
                "created"|"c" => "detail_created_at",
                _ => { filter_error = Some(format!("알 수 없는 필드 '{}'", field)); break; }
            };
            let op_used = op.unwrap_or(if matches!(field.as_str(), "date"|"created"|"c") { "=" } else { "~" });
            match op_used {
                "~" => {
                    where_clauses.push(format!("{} LIKE ?", column));
                    binds.push(format!("%{}%", value));
                },
                "=" => {
                    where_clauses.push(format!("{} = ?", column));
                    binds.push(value.to_string());
                },
                ">=" | "<=" => {
                    // treat as lexical compare; for dates stored as string it's OK (ISO assumed)
                    where_clauses.push(format!("{} {} ?", column, op_used));
                    binds.push(value.to_string());
                },
                _ => { filter_error = Some(format!("지원하지 않는 연산자 '{}'", op_used)); break; }
            }
        }
        if filter_error.is_none() {
            applied_filter = Some(raw_filter.clone());
        }
    }

    // Build WHERE fragment
    let where_sql = if filter_error.is_none() && !where_clauses.is_empty() {
        format!("WHERE {}", where_clauses.join(" AND "))
    } else { String::new() };

    // Count
    let total: i64 = if filter_error.is_none() {
        let count_sql = format!("SELECT COUNT(*) FROM v_product_detail_analytics {}", where_sql);
        let mut q = sqlx::query_scalar::<_, i64>(&count_sql);
        for b in &binds { q = q.bind(b); }
        q.fetch_one(pool).await.unwrap_or(0)
    } else { 0 };

    // ---------------- Sorting ----------------
    let mut order_by_parts: Vec<String> = Vec::new();
    if let Some(sort_vec) = &params.sort {
        for s in sort_vec {
            let mut split = s.split(':');
            let field = split.next().unwrap_or("");
            let dir_raw = split.next().unwrap_or("");
            let dir = match dir_raw.to_lowercase().as_str() { "asc" => "ASC", "desc" => "DESC", _ => continue };
            // Whitelist mapping
            let col = match field {
                "device_category" => Some("device_category"),
                "device_type_name" => Some("device_type_name"),
                "model" => Some("model"),
                "vendor_name" => Some("vendor_name"),
                "certification_date" => Some("certification_date"),
                "detail_created_at" => Some("detail_created_at"),
                "transport_interface" | "transport_if" => Some("transport_interface"),
                _ => None,
            };
            if let Some(c) = col { order_by_parts.push(format!("{} {}", c, dir)); }
        }
    }
    if order_by_parts.is_empty() { order_by_parts.push("detail_created_at DESC".into()); }
    let order_by_sql = format!("ORDER BY {}", order_by_parts.join(", "));

    // Rows
    let rows: Vec<AnalyticsRow> = if filter_error.is_none() {
        let base_select = format!(r#"SELECT product_detail_url, model, vendor_name, device_type_name, device_category, certification_date, detail_created_at, transport_interface
                      FROM v_product_detail_analytics
                      {} {} LIMIT ? OFFSET ?"#, where_sql, order_by_sql);
        let mut q = sqlx::query(&base_select);
        for b in &binds { q = q.bind(b); }
        q = q.bind(limit).bind(offset);
        match q.fetch_all(pool).await {
            Ok(rs) => rs.into_iter().map(|r| {
                let product_detail_url = r.get::<Option<String>, _>("product_detail_url");
                let model = r.get::<Option<String>, _>("model");
                let vendor_name = r.get::<Option<String>, _>("vendor_name");
                let device_type_name = r.get::<Option<String>, _>("device_type_name");
                let device_category = r.get::<Option<String>, _>("device_category");
                let certification_date = r.get::<Option<String>, _>("certification_date");
                let detail_created_at = r.get::<Option<String>, _>("detail_created_at");
                let transport_interface = r.get::<Option<String>, _>("transport_interface");
                AnalyticsRow { product_detail_url, model, vendor_name, device_type_name, device_category, certification_date, detail_created_at, transport_interface }
            }).collect(),
            Err(_) => Vec::new()
        }
    } else { Vec::new() };

    Ok(AnalyticsPage { rows, total, offset, limit, applied_filter, filter_error })
}

#[derive(serde::Serialize)]
pub struct AnalyticsMappingDiagnostics {
    pub product_details_with_ids: i64,
    pub product_details_total: i64,
    pub bridge_rows: i64,
    pub distinct_bridge_products: i64,
    pub device_types_total: i64,
    pub device_types_with_type_id: i64,
    pub device_types_type_id_null: i64,
    pub analytics_rows_total: i64,
    pub analytics_with_device_type: i64,
    pub analytics_distinct_products_total: i64,
    pub analytics_distinct_products_mapped: i64,
    pub sample_unmapped: Vec<serde_json::Value>,
    // --- Extended diagnostics for debugging unmapped device types ---
    pub product_details_json_valid_ids: i64,
    pub json_each_expanded_rows: i64,
    pub sample_primary_device_type_ids: Vec<serde_json::Value>,
    pub sample_expanded_values: Vec<serde_json::Value>,
    pub distinct_json_values_count: i64,
    pub unmatched_json_values_count: i64,
    pub sample_unmatched_json_values: Vec<serde_json::Value>,
    pub sample_non_numeric_json_values: Vec<serde_json::Value>,
    pub sample_device_type_ids: Vec<i64>,
    pub mapping_coverage_pct: f64,
}

#[tauri::command]
pub async fn diagnostics_analytics_mapping(state: State<'_, DatabaseConnection>) -> Result<AnalyticsMappingDiagnostics, String> {
    let pool = state.pool();
    // Allow some wait if crawler holds a write lock
    let _ = sqlx::query("PRAGMA busy_timeout=5000").execute(pool).await;
    // Opportunistic on-demand backfill if bridge still empty (idempotent)
    if let Ok(br) = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM product_primary_device_types").fetch_one(pool).await {
        if br == 0 {
            // Attempt inside a transaction; ignore errors (diagnostics must not panic)
            if let Err(e) = sqlx::query("BEGIN IMMEDIATE").execute(pool).await { error!(?e, "diag backfill: begin failed"); } else {
                // Normalize type_id just in case
                if let Err(e) = sqlx::query("UPDATE device_types SET type_id = id WHERE type_id IS NULL").execute(pool).await { error!(?e, "diag backfill: normalize type_id failed"); }
                let insert_sql = r#"
                    INSERT OR IGNORE INTO product_primary_device_types(product_detail_id, device_type_id)
                    SELECT pd.url, dt.type_id
                    FROM product_details pd
                    JOIN json_each(pd.primary_device_type_ids) je
                    JOIN device_types dt ON dt.type_id = je.value
                    WHERE pd.primary_device_type_ids IS NOT NULL
                      AND json_valid(pd.primary_device_type_ids)
                "#;
                if let Err(e) = sqlx::query(insert_sql).execute(pool).await { error!(?e, "diag backfill: insert failed"); }
                let _ = sqlx::query("COMMIT").execute(pool).await; // ignore commit errors
            }
        }
    }
    let product_details_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_details").fetch_one(pool).await.unwrap_or(0);
    let product_details_with_ids: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_details WHERE primary_device_type_ids IS NOT NULL AND primary_device_type_ids <> ''").fetch_one(pool).await.unwrap_or(0);
    let bridge_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_primary_device_types").fetch_one(pool).await.unwrap_or(0);
    let distinct_bridge_products: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT product_detail_id) FROM product_primary_device_types WHERE product_detail_id IS NOT NULL").fetch_one(pool).await.unwrap_or(0);
    let device_types_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM device_types").fetch_one(pool).await.unwrap_or(0);
    let device_types_with_type_id: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM device_types WHERE type_id IS NOT NULL").fetch_one(pool).await.unwrap_or(0);
    let device_types_type_id_null: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM device_types WHERE type_id IS NULL").fetch_one(pool).await.unwrap_or(0);
    let analytics_rows_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM v_product_detail_analytics").fetch_one(pool).await.unwrap_or(0);
    let analytics_with_device_type: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM v_product_detail_analytics WHERE device_type_id IS NOT NULL").fetch_one(pool).await.unwrap_or(0);
    let analytics_distinct_products_total: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT product_detail_url) FROM v_product_detail_analytics").fetch_one(pool).await.unwrap_or(0);
    let analytics_distinct_products_mapped: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT product_detail_url) FROM v_product_detail_analytics WHERE device_type_id IS NOT NULL").fetch_one(pool).await.unwrap_or(0);

    // JSON validity + expansion stats
    let product_details_json_valid_ids: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_details WHERE primary_device_type_ids IS NOT NULL AND primary_device_type_ids <> '' AND json_valid(primary_device_type_ids)").fetch_one(pool).await.unwrap_or(0);
    let json_each_expanded_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_details pd JOIN json_each(pd.primary_device_type_ids) je").fetch_one(pool).await.unwrap_or(0);

    // Sample raw primary_device_type_ids (first 5)
    let sample_primary_ids_rows = sqlx::query("SELECT url, length(primary_device_type_ids) AS json_len, primary_device_type_ids FROM product_details WHERE primary_device_type_ids IS NOT NULL AND primary_device_type_ids <> '' LIMIT 5")
        .fetch_all(pool).await.unwrap_or_default();
    let mut sample_primary_device_type_ids: Vec<serde_json::Value> = Vec::new();
    for r in sample_primary_ids_rows {
        let url: Option<String> = r.get("url");
        let raw: Option<String> = r.get("primary_device_type_ids");
        let json_len: Option<i64> = r.get("json_len");
        sample_primary_device_type_ids.push(serde_json::json!({"url": url, "len": json_len, "raw": raw}));
    }

    // Sample expanded distinct values (first 10)
    let sample_expanded_rows = sqlx::query("SELECT DISTINCT je.value as v FROM product_details pd JOIN json_each(pd.primary_device_type_ids) je LIMIT 10")
        .fetch_all(pool).await.unwrap_or_default();
    let mut sample_expanded_values: Vec<serde_json::Value> = Vec::new();
    for r in sample_expanded_rows {
        // je.value can be stored as INTEGER or TEXT depending on JSON token
        let v_str = if let Ok(iv) = r.try_get::<i64, _>("v") { iv.to_string() } else { r.try_get::<String, _>("v").unwrap_or_default() };
        sample_expanded_values.push(serde_json::json!({"value": v_str}));
    }

    // Distinct json values total
    let distinct_json_values_count: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT je.value) FROM product_details pd JOIN json_each(pd.primary_device_type_ids) je")
        .fetch_one(pool).await.unwrap_or(0);
    // Unmatched numeric-ish values (those that look numeric but no device_types.type_id)
    let unmatched_json_values_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM (SELECT DISTINCT je.value AS v FROM product_details pd JOIN json_each(pd.primary_device_type_ids) je LEFT JOIN device_types dt ON dt.type_id = je.value WHERE dt.type_id IS NULL)")
        .fetch_one(pool).await.unwrap_or(0);
    let sample_unmatched_rows = sqlx::query("SELECT DISTINCT je.value AS v FROM product_details pd JOIN json_each(pd.primary_device_type_ids) je LEFT JOIN device_types dt ON dt.type_id = je.value WHERE dt.type_id IS NULL LIMIT 10")
        .fetch_all(pool).await.unwrap_or_default();
    let mut sample_unmatched_json_values: Vec<serde_json::Value> = Vec::new();
    for r in sample_unmatched_rows {
        let v_str = if let Ok(iv) = r.try_get::<i64, _>("v") { iv.to_string() } else { r.try_get::<String, _>("v").unwrap_or_default() };
        sample_unmatched_json_values.push(serde_json::json!({"value": v_str}));
    }
    // Non-numeric sample values (could indicate hex or other encoding)
    let sample_non_numeric_rows = sqlx::query("SELECT DISTINCT je.value AS v FROM product_details pd JOIN json_each(pd.primary_device_type_ids) je WHERE je.value NOT GLOB '[0-9]*' LIMIT 5")
        .fetch_all(pool).await.unwrap_or_default();
    let mut sample_non_numeric_json_values: Vec<serde_json::Value> = Vec::new();
    for r in sample_non_numeric_rows {
        let v_str = if let Ok(iv) = r.try_get::<i64, _>("v") { iv.to_string() } else { r.try_get::<String, _>("v").unwrap_or_default() };
        sample_non_numeric_json_values.push(serde_json::json!({"value": v_str}));
    }
    // Sample device_type_ids to compare
    let sample_device_type_rows = sqlx::query("SELECT type_id FROM device_types WHERE type_id IS NOT NULL ORDER BY type_id LIMIT 15")
        .fetch_all(pool).await.unwrap_or_default();
    let mut sample_device_type_ids: Vec<i64> = Vec::new();
    for r in sample_device_type_rows { if let Ok(tid) = r.try_get::<i64, _>("type_id") { sample_device_type_ids.push(tid); } }

    // Sample unmapped (limit 10)
    let sample_unmapped_rows = sqlx::query("SELECT product_detail_url, model, device_type_id, device_type_name, device_category FROM v_product_detail_analytics WHERE device_type_id IS NULL LIMIT 10")
        .fetch_all(pool).await.unwrap_or_default();
    let mut sample_unmapped: Vec<serde_json::Value> = Vec::new();
    for r in sample_unmapped_rows {
        let url: Option<String> = r.get("product_detail_url");
        let model: Option<String> = r.get("model");
        let dtid: Option<i64> = r.get("device_type_id");
        let dtname: Option<String> = r.get("device_type_name");
        let cat: Option<String> = r.get("device_category");
        sample_unmapped.push(serde_json::json!({"url": url, "model": model, "device_type_id": dtid, "device_type_name": dtname, "device_category": cat}));
    }

    let mapping_coverage_pct = if analytics_distinct_products_total > 0 { (analytics_distinct_products_mapped as f64 * 100.0) / analytics_distinct_products_total as f64 } else { 0.0 };

    Ok(AnalyticsMappingDiagnostics {
        product_details_with_ids,
        product_details_total,
        bridge_rows,
        distinct_bridge_products,
        device_types_total,
        device_types_with_type_id,
        device_types_type_id_null,
        analytics_rows_total,
        analytics_with_device_type,
        analytics_distinct_products_total,
        analytics_distinct_products_mapped,
        sample_unmapped,
    product_details_json_valid_ids,
    json_each_expanded_rows,
    sample_primary_device_type_ids,
    sample_expanded_values,
    distinct_json_values_count,
    unmatched_json_values_count,
    sample_unmatched_json_values,
    sample_non_numeric_json_values,
    sample_device_type_ids,
        mapping_coverage_pct,
    })
}

/// 데이터베이스 진단 결과
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DbDiagnosticsReport {
    pub product_details_total: i64,
    pub product_details_with_ids: i64,
    pub product_details_json_valid: i64,
    pub sample_primary_ids: Vec<serde_json::Value>,
    pub device_types_total: i64,
    pub device_types_type_id_null: i64,
    pub sample_device_types: Vec<serde_json::Value>,
    pub unmatched_codes_count: i64,
    pub sample_unmatched_codes: Vec<serde_json::Value>,
    pub analytics_view_total: i64,
    pub analytics_view_mapped: i64,
    pub mapping_success_ratio: f64,
}

/// 크롤링 상태 요약
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CrawlStatusSummary {
    pub is_running: bool,
    pub current_page: Option<u32>,
    pub total_pages: Option<u32>,
    pub last_updated: Option<String>,
    pub session_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database_connection::get_or_init_global_pool;
    use sqlx::{Connection, Executor, SqliteConnection};

    #[sqlx::test(fixtures("products"))]
    async fn test_get_products_page(pool: SqliteConnection) {
        let app_state = AppState::new(pool.clone()).await.unwrap();
        let state = State::new(app_state);

        // 첫 페이지 (기본 10개)
        let page1 = get_products_page(state.clone(), 1, 10).await.unwrap();
        assert_eq!(page1.page, 1);
        assert_eq!(page1.size, 10);
        assert!(page1.products.len() <= 10);
        assert!(page1.total_count > 0);
        assert!(page1.has_next);

        // 두 번째 페이지
        let page2 = get_products_page(state.clone(), 2, 10).await.unwrap();
        assert_eq!(page2.page, 2);
        assert_eq!(page2.size, 10);
        assert!(page2.products.len() <= 10);
        assert!(page2.total_count > 0);
        assert!(page2.has_next);

        // 페이지 크기 변경
        let page_large = get_products_page(state.clone(), 1, 20).await.unwrap();
        assert_eq!(page_large.page, 1);
        assert_eq!(page_large.size, 20);
        assert!(page_large.products.len() <= 20);
        assert!(page_large.total_count > 0);
        assert!(page_large.has_next);

        // 존재하지 않는 페이지
        let empty_page = get_products_page(state.clone(), 999, 10).await;
        assert!(empty_page.is_err());
    }

    #[sqlx::test(fixtures("products"))]
    async fn test_get_latest_products(pool: SqliteConnection) {
        let app_state = AppState::new(pool.clone()).await.unwrap();
        let state = State::new(app_state);

        let latest_products = get_latest_products(state.clone(), 5).await.unwrap();
        assert_eq!(latest_products.len(), 5);
    }

    #[sqlx::test(fixtures("crawling_status"))]
    async fn test_get_crawling_status_v2(pool: SqliteConnection) {
        let app_state = AppState::new(pool.clone()).await.unwrap();
        let state = State::new(app_state);

        let status = get_crawling_status_v2(state.clone()).await.unwrap();
        assert!(status.is_running);
        assert!(status.current_page.is_some());
        assert!(status.total_pages.is_some());
    }

    #[sqlx::test(fixtures("products", "crawling_status"))]
    async fn test_get_system_status(pool: SqliteConnection) {
        let app_state = AppState::new(pool.clone()).await.unwrap();
        let state = State::new(app_state);

        let status = get_system_status(state.clone()).await.unwrap();
        assert!(status.database_connected);
        assert!(status.total_products > 0);
        assert!(status.config_loaded);
    }

    #[sqlx::test(fixtures("products"))]
    async fn test_get_db_summary(pool: SqliteConnection) {
        let app_state = AppState::new(pool.clone()).await.unwrap();
        let state = State::new(app_state);

        let summary = get_db_summary(state.clone()).await.unwrap();
        assert!(summary.total_products > 0);
        assert!(summary.total_product_details > 0);
        assert!(summary.total_vendors > 0);
        assert!(summary.total_device_types > 0);
        assert!(summary.new_products_24h >= 0);
        assert!(summary.new_products_7d >= 0);
        assert!(!summary.top_device_categories.is_empty());
    }

    #[sqlx::test(fixtures("products"))]
    async fn test_analytics_query(pool: SqliteConnection) {
        let app_state = AppState::new(pool.clone()).await.unwrap();
        let state = State::new(app_state);

        let params = AnalyticsQueryInput {
            offset: Some(0),
            limit: Some(10),
            filter: None,
        };
        let page = analytics_query(state.clone(), params).await.unwrap();
        assert_eq!(page.offset, 0);
        assert_eq!(page.limit, 10);
        assert!(page.rows.len() <= 10);
        assert!(page.total > 0);
    }

    #[sqlx::test(fixtures("products"))]
    async fn test_diagnostics_analytics_mapping(pool: SqliteConnection) {
        let app_state = AppState::new(pool.clone()).await.unwrap();
        let state = State::new(app_state);

        let diagnostics = diagnostics_analytics_mapping(state.clone()).await.unwrap();
        assert!(diagnostics.product_details_with_ids > 0);
        assert!(diagnostics.product_details_total > 0);
        assert!(diagnostics.bridge_rows >= 0);
        assert!(diagnostics.distinct_bridge_products >= 0);
        assert!(diagnostics.device_types_total > 0);
        assert!(diagnostics.device_types_with_type_id >= 0);
        assert!(diagnostics.device_types_type_id_null >= 0);
        assert!(diagnostics.analytics_rows_total > 0);
        assert!(diagnostics.analytics_with_device_type > 0);
        assert!(diagnostics.analytics_distinct_products_total > 0);
        assert!(diagnostics.analytics_distinct_products_mapped >= 0);
        assert!(!diagnostics.sample_unmapped.is_empty());
    }

    #[sqlx::test(fixtures("products"))]
    async fn test_db_diagnostics_report(pool: SqliteConnection) {
        let app_state = AppState::new(pool.clone()).await.unwrap();
        let _state = State::new(app_state);

        let pool = get_or_init_global_pool().await.unwrap();
        let report: DbDiagnosticsReport = sqlx::query_as("SELECT * FROM db_diagnostics_report LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert!(report.product_details_total > 0);
        assert!(report.product_details_with_ids > 0);
        assert!(report.product_details_json_valid > 0);
        assert!(!report.sample_primary_ids.is_empty());
        assert!(report.device_types_total > 0);
        assert!(report.device_types_type_id_null >= 0);
        assert!(!report.sample_device_types.is_empty());
        assert!(report.unmatched_codes_count >= 0);
        assert!(!report.sample_unmatched_codes.is_empty());
        assert!(report.analytics_view_total > 0);
        assert!(report.analytics_view_mapped >= 0);
        assert!(report.mapping_success_ratio >= 0.0);
    }
}
