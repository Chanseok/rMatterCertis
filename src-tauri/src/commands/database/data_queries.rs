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

    // Rows
    let rows: Vec<AnalyticsRow> = if filter_error.is_none() {
        let base_select = format!(r#"SELECT product_detail_url, model, vendor_name, device_type_name, device_category, certification_date, detail_created_at
                      FROM v_product_detail_analytics
                      {} ORDER BY detail_created_at DESC LIMIT ? OFFSET ?"#, where_sql);
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
                AnalyticsRow { product_detail_url, model, vendor_name, device_type_name, device_category, certification_date, detail_created_at }
            }).collect(),
            Err(_) => Vec::new()
        }
    } else { Vec::new() };

    Ok(AnalyticsPage { rows, total, offset, limit, applied_filter, filter_error })
}
