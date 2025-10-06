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
// (removed unused imports: IntegratedProductRepository, core_fetch_products_page)

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

/// 최근 제품 몇 개를 단순 반환 (dev tools context)
#[tauri::command]
pub async fn get_latest_products(state: State<'_, AppState>, limit: Option<u32>) -> Result<Vec<Product>, String> {
    let pool = state.get_database_pool().await.map_err(|e| e.to_string())?;
    let l = limit.unwrap_or(20).min(200) as i64;
    let rows = sqlx::query("SELECT id, url, manufacturer, model, certificate_id, page_id, index_in_page, created_at, updated_at FROM products ORDER BY created_at DESC LIMIT ?")
        .bind(l)
        .fetch_all(&pool).await.map_err(|e| e.to_string())?;
    let mut products = Vec::with_capacity(rows.len());
    for r in rows {
        let created_raw: String = r.try_get("created_at").unwrap_or_else(|_| chrono::Utc::now().to_rfc3339());
        let updated_raw: String = r.try_get("updated_at").unwrap_or_else(|_| chrono::Utc::now().to_rfc3339());
        let created_at = chrono::DateTime::parse_from_rfc3339(&created_raw).map(|d| d.with_timezone(&chrono::Utc)).unwrap_or_else(|_| chrono::Utc::now());
        let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_raw).map(|d| d.with_timezone(&chrono::Utc)).unwrap_or_else(|_| chrono::Utc::now());
        products.push(Product {
            id: r.try_get("id").ok(),
            url: r.try_get("url").unwrap_or_default(),
            manufacturer: r.try_get("manufacturer").ok(),
            model: r.try_get("model").ok(),
            certificate_id: r.try_get("certificate_id").ok(),
            page_id: r.try_get("page_id").ok().and_then(|v: Option<i64>| v.map(|x| x as i32)),
            index_in_page: r.try_get("index_in_page").ok().and_then(|v: Option<i64>| v.map(|x| x as i32)),
            created_at,
            updated_at,
        });
    }
    Ok(products)
}

/// 단순 크롤링 상태 (임시 placeholder - 실제 러닝 플래그/페이지는 actor 시스템에서 broadcast)
#[tauri::command]
pub async fn get_crawling_status_v2(state: State<'_, AppState>) -> Result<CrawlingStatusInfo, String> {
    // Minimal implementation: query a small status table if exists; else placeholder
    let pool = state.get_database_pool().await.map_err(|e| e.to_string())?;
    let (is_running, current_page, total_pages, last_updated, session_id) = sqlx::query(
        r#"SELECT is_running, current_page, total_pages, last_updated, session_id FROM crawl_runtime_state LIMIT 1"#)
        .fetch_optional(&pool).await
        .ok()
        .flatten()
        .map(|row| {
            let is_running: Option<i64> = row.get("is_running");
            let current_page: Option<i64> = row.get("current_page");
            let total_pages: Option<i64> = row.get("total_pages");
            let last_updated: Option<String> = row.get("last_updated");
            let session_id: Option<String> = row.get("session_id");
            (
                is_running.unwrap_or(0) != 0,
                current_page.map(|v| v as u32),
                total_pages.map(|v| v as u32),
                last_updated,
                session_id,
            )
        })
        .unwrap_or((false, None, None, None, None));
    Ok(CrawlingStatusInfo { is_running, current_page, total_pages, last_updated, session_id })
}

/// 시스템 상태 (DB 연결, 제품 수 등 간단 메트릭)
#[tauri::command]
pub async fn get_system_status(state: State<'_, AppState>) -> Result<SystemStatus, String> {
    let pool = state.get_database_pool().await.map_err(|e| e.to_string())?;
    let total_products: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
        .fetch_one(&pool).await.unwrap_or(0);
    // last crawl time heuristic: latest product_details created_at
    let last_crawl_time_str: Option<String> = sqlx::query_scalar("SELECT created_at FROM product_details ORDER BY created_at DESC LIMIT 1")
        .fetch_one(&pool).await.ok();
    let last_crawl_time = last_crawl_time_str.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok()).map(|dt| dt.with_timezone(&chrono::Utc));
    Ok(SystemStatus { database_connected: true, total_products: total_products as u32, last_crawl_time, config_loaded: true })
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
    let pool = state.get_database_pool().await.map_err(|e| e.to_string())?;
    let page = page.max(1);
    let size = size.max(1).min(200);
    let offset = (page - 1) * size;
    let rows = sqlx::query("SELECT id, url, manufacturer, model, certificate_id, page_id, index_in_page, created_at, updated_at FROM products ORDER BY id LIMIT ? OFFSET ?")
        .bind(size as i64)
        .bind(offset as i64)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;
    let mut products = Vec::with_capacity(rows.len());
    for r in rows {
        let created_raw: String = r.try_get("created_at").unwrap_or_else(|_| chrono::Utc::now().to_rfc3339());
        let updated_raw: String = r.try_get("updated_at").unwrap_or_else(|_| chrono::Utc::now().to_rfc3339());
        let created_at = chrono::DateTime::parse_from_rfc3339(&created_raw).map(|d| d.with_timezone(&chrono::Utc)).unwrap_or_else(|_| chrono::Utc::now());
        let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_raw).map(|d| d.with_timezone(&chrono::Utc)).unwrap_or_else(|_| chrono::Utc::now());
        products.push(Product {
            id: r.try_get("id").ok(),
            url: r.try_get("url").unwrap_or_default(),
            manufacturer: r.try_get("manufacturer").ok(),
            model: r.try_get("model").ok(),
            certificate_id: r.try_get("certificate_id").ok(),
            page_id: r.try_get("page_id").ok().and_then(|v: Option<i64>| v.map(|x| x as i32)),
            index_in_page: r.try_get("index_in_page").ok().and_then(|v: Option<i64>| v.map(|x| x as i32)),
            created_at,
            updated_at,
        });
    }
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);
    let has_next = (offset as i64 + products.len() as i64) < total;
    Ok(ProductPage { products, total_count: total as u32, page, size, has_next })
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
    pub all_device_categories: Vec<(String, i64)>,
    pub top_device_types: Vec<(String, i64)>,
    pub all_device_types: Vec<(String, i64)>, // 전체 device type 목록 (count와 함께)
    pub top_vendors: Vec<(String, i64)>,
    pub all_vendors: Vec<(String, i64)>, // 전체 벤더 목록 (count와 함께)
    pub all_device_type_names: Vec<String>,
    pub top_transport_interfaces: Vec<(String, i64)>,
    pub all_transport_interfaces: Vec<String>,
    pub device_types_by_category: std::collections::HashMap<String, Vec<String>>,
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
    
    // All categories (for scrollable list)
    let all_device_categories = {
        let sql = "SELECT device_category, COUNT(*) as cnt FROM v_product_detail_analytics GROUP BY device_category ORDER BY cnt DESC";
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
    
    // Top vendors (Top 10 by product count)
    let top_vendors = {
        let sql = "SELECT vendor_name, COUNT(*) as cnt FROM v_product_detail_analytics GROUP BY vendor_name ORDER BY cnt DESC LIMIT 10";
        match sqlx::query(sql).fetch_all(pool).await {
            Ok(rows) => rows
                .into_iter()
                .filter_map(|r| {
                    let vendor: Option<String> = r.get::<Option<String>, _>("vendor_name");
                    let cnt: i64 = r.get::<i64, _>("cnt");
                    vendor.map(|v| (v, cnt))
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    };
    
    // All vendors (전체 벤더 목록 with count)
    let all_vendors = {
        let sql = "SELECT vendor_name, COUNT(*) as cnt FROM v_product_detail_analytics GROUP BY vendor_name ORDER BY cnt DESC";
        match sqlx::query(sql).fetch_all(pool).await {
            Ok(rows) => rows
                .into_iter()
                .filter_map(|r| {
                    let vendor: Option<String> = r.get::<Option<String>, _>("vendor_name");
                    let cnt: i64 = r.get::<i64, _>("cnt");
                    vendor.map(|v| (v, cnt))
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    };
    
    // Top device types (Top 10 by product count)
    let top_device_types = {
        let sql = "SELECT device_type_name, COUNT(*) as cnt FROM v_product_detail_analytics WHERE device_type_name IS NOT NULL GROUP BY device_type_name ORDER BY cnt DESC LIMIT 10";
        match sqlx::query(sql).fetch_all(pool).await {
            Ok(rows) => rows
                .into_iter()
                .filter_map(|r| {
                    let dtype: Option<String> = r.get::<Option<String>, _>("device_type_name");
                    let cnt: i64 = r.get::<i64, _>("cnt");
                    dtype.map(|d| (d, cnt))
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    };
    
    // All device types (전체 device type 목록 with count)
    let all_device_types = {
        let sql = "SELECT device_type_name, COUNT(*) as cnt FROM v_product_detail_analytics WHERE device_type_name IS NOT NULL GROUP BY device_type_name ORDER BY cnt DESC";
        match sqlx::query(sql).fetch_all(pool).await {
            Ok(rows) => rows
                .into_iter()
                .filter_map(|r| {
                    let dtype: Option<String> = r.get::<Option<String>, _>("device_type_name");
                    let cnt: i64 = r.get::<i64, _>("cnt");
                    dtype.map(|d| (d, cnt))
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    };
    
    // All device type names (for device type filter)
    let all_device_type_names = {
        let sql = "SELECT DISTINCT name FROM device_types ORDER BY name ASC";
        match sqlx::query(sql).fetch_all(pool).await {
            Ok(rows) => rows
                .into_iter()
                .filter_map(|r| r.get::<Option<String>, _>("name"))
                .collect(),
            Err(_) => Vec::new(),
        }
    };
    
    // Top transport interfaces (Top 10 by product count)
    let top_transport_interfaces = {
        let sql = "SELECT transport_interface, COUNT(*) as cnt FROM v_product_detail_analytics WHERE transport_interface IS NOT NULL GROUP BY transport_interface ORDER BY cnt DESC LIMIT 10";
        match sqlx::query(sql).fetch_all(pool).await {
            Ok(rows) => rows
                .into_iter()
                .filter_map(|r| {
                    let ti: Option<String> = r.get::<Option<String>, _>("transport_interface");
                    let cnt: i64 = r.get::<i64, _>("cnt");
                    ti.map(|t| (t, cnt))
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    };
    
    // All transport interfaces (for transport interface filter)
    let all_transport_interfaces = {
        let sql = "SELECT DISTINCT transport_interface FROM v_product_detail_analytics WHERE transport_interface IS NOT NULL ORDER BY transport_interface ASC";
        match sqlx::query(sql).fetch_all(pool).await {
            Ok(rows) => rows
                .into_iter()
                .filter_map(|r| r.get::<Option<String>, _>("transport_interface"))
                .collect(),
            Err(_) => Vec::new(),
        }
    };
    
    // Device types by category mapping (for smart filtering)
    let device_types_by_category = {
        let sql = "SELECT device_category, device_type_name FROM v_product_detail_analytics WHERE device_category IS NOT NULL AND device_type_name IS NOT NULL GROUP BY device_category, device_type_name ORDER BY device_category, device_type_name";
        match sqlx::query(sql).fetch_all(pool).await {
            Ok(rows) => {
                let mut map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
                for r in rows {
                    let cat: Option<String> = r.get::<Option<String>, _>("device_category");
                    let dtype: Option<String> = r.get::<Option<String>, _>("device_type_name");
                    if let (Some(c), Some(d)) = (cat, dtype) {
                        map.entry(c).or_insert_with(Vec::new).push(d);
                    }
                }
                map
            },
            Err(_) => std::collections::HashMap::new(),
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
        all_device_categories,
        top_device_types,
        all_device_types,
        top_vendors,
        all_vendors,
        all_device_type_names,
        top_transport_interfaces,
        all_transport_interfaces,
        device_types_by_category,
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
    // Diagnostic columns (added in migration 026)
    pub raw_type_ids: Option<String>,
    pub type_ids_status: Option<String>,
    pub category_missing_reason: Option<String>,
    pub type_match_count: Option<i64>,
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
    
    info!("📊 analytics_query called: offset={}, limit={}, filter_len={}", offset, limit, raw_filter.len());
    if !raw_filter.is_empty() {
        info!("🔍 Raw filter: {}", raw_filter);
    }

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
        // Enhanced tokenization: handle field:in:[...] specially
        let mut tokens: Vec<String> = Vec::new();
        let mut current = String::new();
        let mut in_bracket = false;
        let mut paren_depth = 0;
        
        for ch in raw_filter.chars() {
            match ch {
                '[' if !in_bracket => {
                    in_bracket = true;
                    current.push(ch);
                },
                ']' if in_bracket => {
                    in_bracket = false;
                    current.push(ch);
                },
                '(' => {
                    paren_depth += 1;
                    current.push(ch);
                },
                ')' => {
                    paren_depth -= 1;
                    current.push(ch);
                },
                ' ' | '\t' | '\n' if !in_bracket && paren_depth == 0 => {
                    if !current.is_empty() {
                        tokens.push(current.clone());
                        current.clear();
                    }
                },
                _ => {
                    current.push(ch);
                }
            }
        }
        if !current.is_empty() {
            tokens.push(current);
        }
        
        info!("🔍 Total tokens parsed: {}", tokens.len());
        for (idx, token) in tokens.iter().enumerate() {
            info!("  Token[{}]: '{}'", idx, token);
        }
        
        let field_pattern = Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*").unwrap();
        for t in &tokens {
            // Skip logical connector tokens (prevent accidental bareword searching)
            let upper_tok = t.to_ascii_uppercase();
            if upper_tok == "AND" || upper_tok == "OR" {
                // NOTE: 현재 DSL은 명시적인 논리 연산자를 아직 지원하지 않는다.
                // 사용자가 'date>=2022-10-01 AND date<=2025-10-02' 처럼 입력하면
                // 공백 분리 토큰화 후 AND 가 bareword 로 분류되어
                // (model LIKE '%AND%' OR vendor_name LIKE '%AND%') 조건이 암묵적으로 추가되는
                // 예상치 못한 필터 누락/축소가 발생한다. 이를 방지하기 위해 AND/OR 토큰은 무시한다.
                // 향후 정식 논리식 파서를 도입할 때 여기 로직을 대체/확장해야 한다.
                info!("  Skipping logical operator: {}", upper_tok);
                continue;
            }
            // Operators precedence: check for >= or <= first, then :in:
            let t_str = t.as_str();
            info!("  Processing token: '{}'", t_str);
            
            let (field_part, op, value_part) = if let Some(pos) = t_str.find(">=") {
                info!("    Found >= operator at pos {}", pos);
                (&t_str[..pos], Some(">="), &t_str[pos+2..])
            } else if let Some(pos) = t_str.find("<=") {
                info!("    Found <= operator at pos {}", pos);
                (&t_str[..pos], Some("<="), &t_str[pos+2..])
            } else if let Some(pos) = t_str.find(":in:") {
                info!("    Found :in: operator at pos {}", pos);
                (&t_str[..pos], Some(":in:"), &t_str[pos+4..])
            } else if let Some(pos) = t_str.find('=') {
                info!("    Found = operator at pos {}", pos);
                (&t_str[..pos], Some("="), &t_str[pos+1..])
            } else if let Some(pos) = t_str.find('~') {
                info!("    Found ~ operator at pos {}", pos);
                (&t_str[..pos], Some("~"), &t_str[pos+1..])
            } else if let Some(pos) = t_str.find(':') {
                info!("    Found : operator at pos {}", pos);
                (&t_str[..pos], None, &t_str[pos+1..])
            } else {
                info!("    No operator found, treating as bareword");
                ("", None, t_str)
            };

            info!("    Parsed: field='{}', op={:?}, value='{}'", field_part, op, value_part);

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
                "category"|"cat"|"device_category" => "device_category",
                "dtype"|"dname"|"dt"|"device_type_name" => "device_type_name",
                "vendor_name" => "vendor_name", // 직접 컬럼명 사용 허용
                "model"|"m" => "model",
                "date" => "certification_date",
                "created"|"c" => "detail_created_at",
                _ => { filter_error = Some(format!("알 수 없는 필드 '{}'", field)); break; }
            };
            let op_used = op.unwrap_or(if matches!(field.as_str(), "date"|"created"|"c") { "=" } else { "~" });
            
            // Special handling for null values
            if value.eq_ignore_ascii_case("null") && (op_used == "=" || op_used == "~") {
                where_clauses.push(format!("{} IS NULL", column));
                continue;
            }
            
            match op_used {
                ":in:" => {
                    info!("    Starting :in: operator parsing...");
                    // Parse array: [value1,value2,...] or ["val1","val2",...]
                    if !value.starts_with('[') || !value.ends_with(']') {
                        filter_error = Some(format!("in: 연산자는 배열 형식 [...]이 필요합니다: {}", value));
                        break;
                    }
                    let inner = &value[1..value.len()-1];
                    info!("    Inner content (after removing brackets): '{}'", inner);
                    if inner.is_empty() {
                        filter_error = Some("in: 배열이 비어있습니다".into());
                        break;
                    }
                    
                    // Quote-aware CSV parsing: split by comma only if not inside quotes
                    info!("    Starting quote-aware CSV parsing...");
                    let mut items: Vec<String> = Vec::new();
                    let mut current = String::new();
                    let mut in_quotes = false;
                    let mut escape_next = false;
                    
                    for ch in inner.chars() {
                        if escape_next {
                            current.push(ch);
                            escape_next = false;
                        } else if ch == '\\' {
                            escape_next = true;
                        } else if ch == '"' {
                            in_quotes = !in_quotes;
                            current.push(ch);
                        } else if ch == ',' && !in_quotes {
                            // Comma outside quotes = delimiter
                            let trimmed = current.trim();
                            if !trimmed.is_empty() {
                                // Remove surrounding quotes
                                let unquoted = if (trimmed.starts_with('"') && trimmed.ends_with('"')) || 
                                                  (trimmed.starts_with('\'') && trimmed.ends_with('\'')) {
                                    trimmed[1..trimmed.len()-1].to_string()
                                } else {
                                    trimmed.to_string()
                                };
                                items.push(unquoted);
                            }
                            current.clear();
                        } else {
                            current.push(ch);
                        }
                    }
                    
                    // Don't forget the last item
                    info!("    Processing last item, current buffer: '{}'", current);
                    let trimmed = current.trim();
                    if !trimmed.is_empty() {
                        let unquoted = if (trimmed.starts_with('"') && trimmed.ends_with('"')) || 
                                          (trimmed.starts_with('\'') && trimmed.ends_with('\'')) {
                            trimmed[1..trimmed.len()-1].to_string()
                        } else {
                            trimmed.to_string()
                        };
                        items.push(unquoted);
                    }
                    
                    info!("Parsed IN array for {}: {:?} (count: {})", column, items, items.len());
                    
                    if items.is_empty() {
                        filter_error = Some("in: 배열에 유효한 값이 없습니다".into());
                        info!("    ERROR: items is empty after parsing!");
                        break;
                    }
                    
                    // Generate LIKE clauses for GROUP_CONCAT compatibility
                    // GROUP_CONCAT creates comma-separated values like "Media, Lighting"
                    // So we need LIKE '%Media%' instead of IN ('Media')
                    info!("    Generating LIKE clauses for GROUP_CONCAT compatibility...");
                    let like_clauses: Vec<String> = items.iter().map(|_| format!("{} LIKE ?", column)).collect();
                    where_clauses.push(format!("({})", like_clauses.join(" OR ")));
                    
                    // Bind each item with wildcard for partial matching
                    for item in items {
                        binds.push(format!("%{}%", item));
                    }
                    info!("    :in: operator processing completed (using LIKE for GROUP_CONCAT)");
                },
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
    
    if !where_sql.is_empty() {
        info!("Generated WHERE clause: {} | Binds: {:?}", where_sql, binds);
    }

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
        let base_select = format!(r#"SELECT 
                      product_detail_url, model, vendor_name, device_type_name, device_category, 
                      certification_date, detail_created_at, transport_interface,
                      raw_type_ids, type_ids_status, category_missing_reason, type_match_count
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
                let raw_type_ids = r.get::<Option<String>, _>("raw_type_ids");
                let type_ids_status = r.get::<Option<String>, _>("type_ids_status");
                let category_missing_reason = r.get::<Option<String>, _>("category_missing_reason");
                let type_match_count = r.get::<Option<i64>, _>("type_match_count");
                AnalyticsRow { 
                    product_detail_url, model, vendor_name, device_type_name, device_category, 
                    certification_date, detail_created_at, transport_interface,
                    raw_type_ids, type_ids_status, category_missing_reason, type_match_count
                }
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
    // Detect legacy bridge presence (post-022 migration it should be removed)
    let bridge_exists: bool = sqlx::query_scalar::<_, i64>("SELECT 1 FROM sqlite_master WHERE type='table' AND name='product_primary_device_types' LIMIT 1")
        .fetch_optional(pool).await.unwrap_or(None).is_some();
    let product_details_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_details").fetch_one(pool).await.unwrap_or(0);
    let product_details_with_ids: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_details WHERE primary_device_type_ids IS NOT NULL AND primary_device_type_ids <> ''").fetch_one(pool).await.unwrap_or(0);
    let (bridge_rows, distinct_bridge_products) = if bridge_exists {
        let br: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_primary_device_types").fetch_one(pool).await.unwrap_or(0);
        let dbp: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT product_detail_id) FROM product_primary_device_types WHERE product_detail_id IS NOT NULL").fetch_one(pool).await.unwrap_or(0);
        (br, dbp)
    } else { (0, 0) };
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
    // (정상화 1단계) 기존 인라인 DB 테스트 제거됨.
    // 새 구조: tests/ 디렉토리에 재구성된 통합/쿼리 테스트를 별도 작성 예정.
}

// ==== Added: Coordinate repair & targeted recrawl assistance commands ====
use crate::infrastructure::integrated_product_repository::IntegratedProductRepository as _RepoForRepair; // alias avoid clash
use crate::application::AppState as _AppStateForRepair;
use crate::commands::crawling::actor_system::start_manual_crawl_pages_actor;

#[derive(serde::Serialize)]
pub struct RepairResult { pub fixed: u64 }

#[tauri::command]
pub async fn repair_product_coordinates_cmd(state: State<'_, _AppStateForRepair>, limit: Option<i64>) -> Result<RepairResult, String> {
    let pool = state.get_database_pool().await.map_err(|e| e.to_string())?;
    let repo = _RepoForRepair::new(pool.clone());
    repo.repair_product_coordinates(limit).await
        .map(|fixed| RepairResult { fixed })
        .map_err(|e| e.to_string())
}

// ===================== Lock Error Counter Commands =====================
#[tauri::command(async)]
pub async fn get_lock_error_count(app_state: State<'_, crate::application::AppState>) -> Result<u64, String> {
    Ok(app_state.get_lock_errors().await)
}

#[tauri::command(async)]
pub async fn reset_lock_error_count(app_state: State<'_, crate::application::AppState>) -> Result<(), String> {
    app_state.reset_lock_errors().await;
    Ok(())
}

#[derive(serde::Serialize)]
pub struct PageZeroSample { pub url: String, pub created_at: Option<String>, pub detail_has_coords: bool }

#[tauri::command]
pub async fn list_page_zero_urls(state: State<'_, _AppStateForRepair>, limit: Option<i64>) -> Result<Vec<PageZeroSample>, String> {
    let pool = state.get_database_pool().await.map_err(|e| e.to_string())?;
    let l = limit.unwrap_or(100);
    let sql = r#"SELECT p.url, p.created_at, (d.page_id IS NOT NULL AND d.index_in_page IS NOT NULL) AS detail_has_coords
                 FROM products p LEFT JOIN product_details d ON d.url = p.url
                 WHERE p.page_id = 0 ORDER BY p.created_at DESC LIMIT ?"#;
    let rows = sqlx::query(sql).bind(l).fetch_all(&pool).await.map_err(|e| e.to_string())?;
    Ok(rows.into_iter().map(|r| PageZeroSample {
        url: r.get("url"),
        created_at: r.get("created_at"),
        detail_has_coords: r.get::<i64,_>("detail_has_coords") == 1,
    }).collect())
}

#[derive(serde::Deserialize)]
pub struct RecrawlPagesRequest { pub physical_pages: Vec<i32>, pub max_concurrent: Option<usize> }
#[derive(serde::Serialize)]
pub struct RecrawlPagesResponse { pub accepted: bool, pub count: usize }

#[tauri::command]
pub async fn recrawl_physical_pages(app: tauri::AppHandle, req: RecrawlPagesRequest) -> Result<RecrawlPagesResponse, String> {
    tracing::info!(target="orchestration", pages=?req.physical_pages, "recrawl_physical_pages invoked");
    if req.physical_pages.is_empty() {
        tracing::warn!(target="orchestration", "recrawl_physical_pages rejected: empty pages list");
        return Ok(RecrawlPagesResponse { accepted: false, count: 0 });
    }
    // Convert to u32 (site page numbers assumed >=0)
    let pages: Vec<u32> = req.physical_pages.iter().cloned().filter(|p| *p >= 0).map(|p| p as u32).collect();
    if pages.is_empty() {
        tracing::warn!(target="orchestration", "recrawl_physical_pages rejected: all pages were negative/invalid after filtering");
        return Ok(RecrawlPagesResponse { accepted: false, count: 0 });
    }
    let count = pages.len();
    if let Err(e) = start_manual_crawl_pages_actor(app.clone(), pages.clone(), Some(true)).await {
        tracing::error!(target="orchestration", error=%e, pages=?pages, "recrawl_physical_pages failed to start manual crawl");
        return Err(e);
    }
    Ok(RecrawlPagesResponse { accepted: true, count })
}

// ===================== A: SQL Diagnostic Breakdown =====================
#[derive(serde::Serialize)]
pub struct CoordMismatchBreakdown {
    pub both_null: u64,
    pub products_null_details_filled: u64,
    pub details_null_products_filled: u64,
    pub value_mismatch: u64,
    pub page0_products: u64,
    pub page0_distinct_indices: u64,
    pub page0_total_rows: u64,
}

#[tauri::command]
pub async fn coord_mismatch_breakdown(state: State<'_, _AppStateForRepair>) -> Result<CoordMismatchBreakdown, String> {
    let pool = state.get_database_pool().await.map_err(|e| e.to_string())?;
    macro_rules! q { ($sql:expr) => { sqlx::query_scalar::<_, i64>($sql).fetch_one(&pool).await.unwrap_or(0) as u64 }; }
    let both_null = q!(r"SELECT COUNT(*) FROM products p JOIN product_details d ON d.url = p.url
        WHERE (p.page_id IS NULL OR p.index_in_page IS NULL)
          AND (d.page_id IS NULL OR d.index_in_page IS NULL)");
    let products_null_details_filled = q!(r"SELECT COUNT(*) FROM products p JOIN product_details d ON d.url = p.url
        WHERE (p.page_id IS NULL OR p.index_in_page IS NULL)
          AND d.page_id IS NOT NULL AND d.index_in_page IS NOT NULL");
    let details_null_products_filled = q!(r"SELECT COUNT(*) FROM products p JOIN product_details d ON d.url = p.url
        WHERE (d.page_id IS NULL OR d.index_in_page IS NULL)
          AND p.page_id IS NOT NULL AND p.index_in_page IS NOT NULL");
    let value_mismatch = q!(r"SELECT COUNT(*) FROM product_details d LEFT JOIN products p ON p.url = d.url
        WHERE p.url IS NOT NULL
          AND p.page_id IS NOT NULL AND p.index_in_page IS NOT NULL
          AND d.page_id IS NOT NULL AND d.index_in_page IS NOT NULL
          AND (p.page_id != d.page_id OR p.index_in_page != d.index_in_page)");
    let page0_total_rows = q!(r"SELECT COUNT(*) FROM products WHERE page_id = 0");
    let page0_distinct_indices = q!(r"SELECT COUNT(DISTINCT index_in_page) FROM products WHERE page_id = 0 AND index_in_page IS NOT NULL");
    let page0_products = q!(r"SELECT COUNT(*) FROM products WHERE page_id = 0 AND (index_in_page IS NOT NULL OR index_in_page IS NULL)");
    Ok(CoordMismatchBreakdown { both_null, products_null_details_filled, details_null_products_filled, value_mismatch, page0_products, page0_distinct_indices, page0_total_rows })
}

// ===================== B: List Page Rehydrate (Skeleton) =====================
#[derive(serde::Deserialize)]
pub struct RehydrateListPagesParams {
    pub pages: Option<Vec<u32>>,      // physical page numbers (1-based) to target; None = auto (not yet implemented)
    pub limit: Option<u32>,           // optional cap on number of pages processed
    #[serde(alias="dryRun")] // accept camelCase from old callers
    pub dry_run: Option<bool>,        // if true, compute targets & metrics only
    /// 페이지 범위 구문: "1-50,120,200-205" (pages 가 비어있거나 None 일 때 우선 적용)
    pub page_ranges: Option<String>,
    /// 디버그 통계 확장 출력 여부
    pub debug: Option<bool>,
}

#[derive(serde::Serialize)]
pub struct RehydrateListPagesResult {
    pub pages_targeted: u32,
    pub pages_processed: u32,
    pub products_with_null_coords_before: u64,
    pub products_filled: u64,
    pub products_already_had_coords: u64,
    pub products_still_null_after: u64,
    pub elapsed_ms: u128,
    pub note: String,
    pub pages_failed: u32,
    pub http_errors: u32,
    pub mismatches_detected: u64,
    pub would_fill: u64,
    pub auto_selected: bool,
    pub per_page: Option<Vec<PerPageDebugStat>>, // debug=true 일 때만 Some
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct PerPageDebugStat {
    pub page: u32,
    pub parsed_count: usize,
    pub updated_products: u64,
    pub updated_details: u64,
    pub mismatches: u64,
    pub would_fill: u64,
}

// -----------------------------------------------------------------------------
// Page coverage diagnostic: both_null 대상이 어느 (추정) 페이지 구간에 분포하는지 빠르게 계산
// products.created_at DESC 순서를 페이지 단위(12개)로 가정하여 approximate_page 계산.
// 사이트 페이지 사이즈(12)는 rehydrate 와 동일 가정.
// -----------------------------------------------------------------------------
#[derive(serde::Serialize, Clone)]
pub struct PageCoverageBucket { pub page: u32, pub both_null_count: u32 }
#[derive(serde::Serialize)]
pub struct PageCoverageDiagnosticResult {
    pub total_both_null: u32,
    pub distinct_urls: u32,
    pub buckets: Vec<PageCoverageBucket>,
    pub top_pages: Vec<PageCoverageBucket>,
    pub suggested_ranges: Vec<String>,
    pub note: String,
}

#[tauri::command]
pub async fn page_coverage_diagnostic(state: State<'_, _AppStateForRepair>, limit_pages: Option<u32>, top_n: Option<usize>) -> Result<PageCoverageDiagnosticResult, String> {
    let pool = state.get_database_pool().await.map_err(|e| e.to_string())?;
    // 1. 대상 URL 수집 (both_null)
    let rows = sqlx::query(r"SELECT p.url, p.created_at FROM products p JOIN product_details d ON d.url = p.url
        WHERE (p.page_id IS NULL OR p.index_in_page IS NULL)
          AND (d.page_id IS NULL OR d.index_in_page IS NULL)")
        .fetch_all(&pool).await.map_err(|e| e.to_string())?;
    let total_both_null = rows.len() as u32;
    if total_both_null == 0 {
        return Ok(PageCoverageDiagnosticResult { total_both_null: 0, distinct_urls: 0, buckets: vec![], top_pages: vec![], suggested_ranges: vec![], note: "no both_null targets".into() });
    }
    // 2. created_at 내림차순 rank 계산을 위해 전체 products ordering (필요 최소 칼럼) 가져오기
    //    메모리 비용을 줄이기 위해 최대 페이지 제한(limit_pages) 적용: limit_pages * 12 * 2 (여유) 정도만.
    let page_size = 12u32; // 사이트 canonical
    let scan_pages = limit_pages.unwrap_or(500).max(10); // 기본 500페이지 스캔(6000 row) 충분히 작을 것
    let scan_rows_cap = (scan_pages * page_size * 2) as i64; // 여유
    let product_rows = sqlx::query("SELECT url FROM products ORDER BY created_at DESC LIMIT ?")
        .bind(scan_rows_cap)
        .fetch_all(&pool).await.map_err(|e| e.to_string())?;
    // 3. URL -> approx_page 매핑
    use std::collections::HashMap;
    let mut approx: HashMap<String, u32> = HashMap::new();
    for (idx, r) in product_rows.iter().enumerate() { if let Ok(u) = r.try_get::<String, _>("url") { approx.insert(u, (idx as u32)/page_size + 1); } }
    // 4. buckets
    let mut bucket_map: HashMap<u32, u32> = HashMap::new();
    let mut unmatched = 0u32;
    for r in &rows { if let Ok(u) = r.try_get::<String,_>("url") { if let Some(p) = approx.get(&u) { *bucket_map.entry(*p).or_insert(0) += 1; } else { unmatched+=1; } } }
    let mut buckets: Vec<PageCoverageBucket> = bucket_map.iter().map(|(p,c)| PageCoverageBucket { page:*p, both_null_count:*c }).collect();
    buckets.sort_by_key(|b| b.page);
    // 5. top pages by descending count
    let mut top_pages = buckets.clone();
    top_pages.sort_by(|a,b| b.both_null_count.cmp(&a.both_null_count));
    let top_n_val = top_n.unwrap_or(10).min(50);
    top_pages.truncate(top_n_val);
    // 6. suggested ranges (그룹 연속 페이지 묶음)
    let mut suggested_ranges: Vec<String> = Vec::new();
    if !buckets.is_empty() {
        let mut start = buckets[0].page;
        let mut prev = start;
        for b in buckets.iter().skip(1) {
            if b.page == prev + 1 { prev = b.page; continue; }
            // flush
            if start == prev { suggested_ranges.push(format!("{}", start)); } else { suggested_ranges.push(format!("{}-{}", start, prev)); }
            start = b.page; prev = b.page;
        }
        if start == prev { suggested_ranges.push(format!("{}", start)); } else { suggested_ranges.push(format!("{}-{}", start, prev)); }
    }
    let note = if unmatched > 0 { format!("unmatched_urls={unmatched} (urls outside scanned range or very old)") } else { "ok".into() };
    Ok(PageCoverageDiagnosticResult { total_both_null, distinct_urls: total_both_null, buckets, top_pages, suggested_ranges, note })
}

fn parse_page_ranges(expr: &str) -> Vec<u32> {
    // 허용 패턴: 콤마 구분, 각 토큰은 단일 숫자 또는 start-end
    let mut out = Vec::new();
    for token in expr.split(',') {
        let t = token.trim();
        if t.is_empty() { continue; }
        if let Some(hy) = t.find('-') {
            let (a,b) = t.split_at(hy);
            let b = &b[1..];
            if let (Ok(sa), Ok(sb)) = (a.parse::<u32>(), b.parse::<u32>()) {
                if sa>0 && sb>=sa && (sb-sa) <= 50_000 { // sanity bound
                    for v in sa..=sb { out.push(v); }
                }
            }
        } else if let Ok(v) = t.parse::<u32>() { if v>0 { out.push(v); } }
    }
    out
}

#[tauri::command]
pub async fn rehydrate_list_pages(state: State<'_, _AppStateForRepair>, params: RehydrateListPagesParams) -> Result<RehydrateListPagesResult, String> {
    use std::time::Instant;
    use tracing::{info, warn, debug};
    use std::sync::Arc;
    let start = Instant::now();
    let pool = state.get_database_pool().await.map_err(|e| e.to_string())?;

    // Pre-metric: both NULL count
    let before: i64 = sqlx::query_scalar(r"SELECT COUNT(*) FROM products p JOIN product_details d ON d.url = p.url
        WHERE (p.page_id IS NULL OR p.index_in_page IS NULL)
          AND (d.page_id IS NULL OR d.index_in_page IS NULL)")
        .fetch_one(&pool).await.unwrap_or(0);

    // Pages selection: priority 1) explicit pages vector 2) page_ranges syntax 3) auto
    let mut targeted_pages: Vec<u32> = if let Some(ref vec_pages) = params.pages { vec_pages.clone() } else if let Some(ref ranges) = params.page_ranges { parse_page_ranges(ranges) } else { Vec::new() };
    targeted_pages.sort_unstable();
    targeted_pages.dedup();
    if let Some(lim) = params.limit { targeted_pages.truncate(lim as usize); }
    let dry_run = params.dry_run.unwrap_or(false);
    let debug_enabled = params.debug.unwrap_or(false);
    let mut auto_selected = false;
    if targeted_pages.is_empty() {
        auto_selected = true;
        // Temporary auto strategy: will refine later after we know distribution
        // We don't yet know total_pages; detect now (same logic reused below) so we perform a lightweight fetch first.
        // We'll postpone selection until after pagination meta detection (we need total_pages). For now store placeholder; we'll fill after meta.
    }

    // ---- Discover site pagination meta (total_pages & items_on_last_page) ----
    // Use lightweight HTTP client directly; fall back gracefully if detection fails.
    let http_client = match crate::infrastructure::HttpClient::create_from_global_config() { Ok(c)=>c, Err(e)=> return Err(format!("http_client_init_failed: {e}")) };
    let base_url = crate::infrastructure::config::utils::matter_products_page_url_simple(1); // page=1 URL (will adjust)
    // 1) Fetch page 1 to detect total pages via regex over pagination links.
    let first_html = match http_client.fetch_response(&base_url).await {
        Ok(resp) => match resp.text().await { Ok(t)=>t, Err(e)=> return Err(format!("failed_read_page1: {e}")) },
        Err(e) => return Err(format!("failed_fetch_page1: {e}"))
    };
    // NOTE: Previous regex was r"page/(\\d+)/" (incorrectly escaping \d) which matched literally "page/\d/" and always failed.
    // Correct pattern should capture digits: /page/{number}/. Provide a fallback pattern without trailing slash.
    let mut max_page_detected: u32 = 1;
    let mut links_found = 0u32;
    let primary_re = regex::Regex::new(r"/page/(\d+)/").map_err(|e| e.to_string())?;
    for cap in primary_re.captures_iter(&first_html) {
        if let Some(m) = cap.get(1) {
            if let Ok(v) = m.as_str().parse::<u32>() { links_found += 1; if v > max_page_detected { max_page_detected = v; } }
        }
    }
    if max_page_detected == 1 {
        // Fallback: some themes omit trailing slash in links
        let fallback_re = regex::Regex::new(r"/page/(\d+)\b").map_err(|e| e.to_string())?;
        for cap in fallback_re.captures_iter(&first_html) {
            if let Some(m) = cap.get(1) {
                if let Ok(v) = m.as_str().parse::<u32>() { links_found += 1; if v > max_page_detected { max_page_detected = v; } }
            }
        }
    }
    if max_page_detected == 1 { debug!(links_found, "pagination_detection: no additional pages detected; treating as single page (verify this is expected)"); }
    // 2) Fetch last page to count items (if >1)
    let last_page_html = if max_page_detected > 1 {
        let last_url = crate::infrastructure::config::utils::matter_products_page_url_simple(max_page_detected);
        match http_client.fetch_response(&last_url).await {
            Ok(resp) => match resp.text().await { Ok(t)=>Some(t), Err(e)=> { warn!(error=%e, page=max_page_detected, "failed_read_last_page_using_page1"); None } },
            Err(e) => { warn!(error=%e, page=max_page_detected, "failed_fetch_last_page_using_page1"); None }
        }
    } else { None };

    // Count items on last page: naive count of <article class..product> occurrences.
    let count_items = |html: &str| -> u32 {
        // very light heuristic; ProductListParser will refine later anyway
        let mut c=0; let pat = "<article"; let mut s=html; while let Some(idx)=s.find(pat){ c+=1; s=&s[idx+pat.len()..]; } c
    };
    let products_per_full_page = 12u32; // assumed canonical size (could derive from config)
    let items_on_last_page: u32 = if max_page_detected == 1 { count_items(&first_html) } else { last_page_html.as_ref().map(|h| count_items(h)).filter(|c| *c>0 && *c<=products_per_full_page).unwrap_or(products_per_full_page) };
    info!(target="rehydrate", total_pages=max_page_detected, items_on_last_page, "pagination_meta_detected");

    // If auto mode, now choose pages 1..=min(limit_or_default, total_pages)
    if auto_selected {
        let default_cap = params.limit.unwrap_or(40); // reduce default cap for faster feedback
        let cap = std::cmp::min(default_cap, max_page_detected);
        targeted_pages = (1..=cap).collect();
    }
    if targeted_pages.is_empty() {
    return Ok(RehydrateListPagesResult { pages_targeted: 0, pages_processed: 0, products_with_null_coords_before: before as u64, products_filled: 0, products_already_had_coords: 0, products_still_null_after: before as u64, elapsed_ms: start.elapsed().as_millis(), note: "no pages targeted".into(), pages_failed: 0, http_errors: 0, mismatches_detected: 0, would_fill: 0, auto_selected, per_page: None });
    }

    // Prepare canonical pagination context
    let pagination_context = crate::infrastructure::html_parser::PaginationContext { total_pages: max_page_detected, items_per_page: products_per_full_page, items_on_last_page, target_page_size: products_per_full_page };

    // Concurrency (batched) implementation using join_all
    use crate::infrastructure::parsing::{ProductListParser, ParseContext, ContextualParser};
    let parser = match ProductListParser::new() { Ok(p)=>Arc::new(p), Err(e)=> return Err(format!("parser_init_failed: {e}")) };
    let http_client = Arc::new(http_client);
    let pagination_context = Arc::new(pagination_context);

    #[derive(Debug)]
    struct PageOutcome { page: u32, products_filled: u64, products_already: u64, mismatches: u64, would_fill: u64, failed: bool, http_error: bool, parsed_count: usize, updated_details: u64 }

    // 동시성: 설정(AppConfig) 기반 list_page_max_concurrent 사용, 최소 1 보장, 상한 32 (안전)
    let cfg_concurrency = {
        let cfg = state.config.read().await.clone();
        cfg.user.crawling.workers.list_page_max_concurrent
    };
    let max_concurrency: usize = cfg_concurrency.clamp(1, 32);
    info!(target="rehydrate", max_concurrency, "using_config_concurrency");
    let mut pages_processed = 0u32;
    let mut products_filled = 0u64;
    let mut products_already = 0u64;
    let mut pages_failed = 0u32;
    let mut mismatches_detected = 0u64;
    let mut http_errors = 0u32;
    let mut would_fill = 0u64;
    let mut per_page_stats: Vec<PerPageDebugStat> = Vec::new();

    let mut tasks: Vec<tokio::task::JoinHandle<PageOutcome>> = Vec::new();
    for chunk in targeted_pages.chunks(max_concurrency) {
        tasks.clear();
        for p in chunk {
            let page_no = *p;
            let http = http_client.clone();
            let parser_cl = parser.clone();
            let pag_ctx = pagination_context.clone();
            let pool_local = pool.clone();
            let dry_run_local = dry_run;
            tasks.push(tokio::spawn(async move {
                let page_url = crate::infrastructure::config::utils::matter_products_page_url_simple(page_no);
                let html = match http.fetch_response(&page_url).await { Ok(r)=> match r.text().await { Ok(t)=>t, Err(_)=> return PageOutcome{page:page_no,products_filled:0,products_already:0,mismatches:0,would_fill:0,failed:true,http_error:true, parsed_count:0, updated_details:0} }, Err(_)=> return PageOutcome{page:page_no,products_filled:0,products_already:0,mismatches:0,would_fill:0,failed:true,http_error:true, parsed_count:0, updated_details:0} };
                let parse_ctx = ParseContext::new(page_no, page_url.clone());
                let parsed_products = match parser_cl.parse_with_context(&scraper::Html::parse_document(&html), &parse_ctx) { Ok(v)=> v, Err(_)=> return PageOutcome{page:page_no,products_filled:0,products_already:0,mismatches:0,would_fill:0,failed:true,http_error:false, parsed_count:0, updated_details:0} };
                let urls: Vec<String> = parsed_products.iter().map(|pp| pp.url.clone()).collect();
                let now = chrono::Utc::now();
                let mut local_products_filled=0u64; let mut local_products_already=0u64; let mut local_mismatches=0u64; let mut local_would_fill=0u64; let mut local_details_updated=0u64;
                let mut tx_opt: Option<sqlx::Transaction<'_, sqlx::Sqlite>> = if !dry_run_local { match pool_local.begin().await { Ok(t)=>Some(t), Err(_)=>None } } else { None };
                for (i,url) in urls.iter().enumerate() {
                    let (canon_page_id, canon_index) = pag_ctx.calculate_page_index_canonical(page_no, i as u32);
                    if let Ok(row_opt) = sqlx::query("SELECT page_id, index_in_page FROM products WHERE url = ?").bind(url).fetch_optional(&pool_local).await {
                        if let Some(row) = row_opt.as_ref() {
                            let ep: Option<i64> = row.get("page_id");
                            let ei: Option<i64> = row.get("index_in_page");
                            let need = ep.is_none() || ei.is_none();
                            if need { if !dry_run_local { if let Some(tx)=tx_opt.as_mut(){ if sqlx::query("UPDATE products SET page_id = ?, index_in_page = ?, updated_at = ? WHERE url = ?") .bind(canon_page_id).bind(canon_index).bind(now).bind(url).execute(&mut **tx).await.is_ok(){ local_products_filled+=1; } } } else { local_would_fill+=1; } }
                            else if ep.unwrap() as i32 != canon_page_id || ei.unwrap() as i32 != canon_index { local_mismatches+=1; local_products_already+=1; } else { local_products_already+=1; }
                        }
                    }
                    if let Ok(drow_opt) = sqlx::query("SELECT page_id, index_in_page FROM product_details WHERE url = ?").bind(url).fetch_optional(&pool_local).await {
                        if let Some(drow) = drow_opt.as_ref() {
                            let ep: Option<i64> = drow.get("page_id");
                            let ei: Option<i64> = drow.get("index_in_page");
                            let need = ep.is_none() || ei.is_none();
                            if need { if !dry_run_local { if let Some(tx)=tx_opt.as_mut(){ if sqlx::query("UPDATE product_details SET page_id = ?, index_in_page = ?, updated_at = ? WHERE url = ?") .bind(canon_page_id) .bind(canon_index) .bind(now) .bind(url) .execute(&mut **tx).await.is_ok(){ local_details_updated+=1; } } } else { local_would_fill+=1; } }
                            else if ep.unwrap() as i32 != canon_page_id || ei.unwrap() as i32 != canon_index { local_mismatches+=1; }
                        }
                    }
                }
                if let Some(tx)=tx_opt { let _ = tx.commit().await; }
                PageOutcome { page: page_no, products_filled: local_products_filled, products_already: local_products_already, mismatches: local_mismatches, would_fill: local_would_fill, failed:false, http_error:false, parsed_count: parsed_products.len(), updated_details: local_details_updated }
            }));
        }
        for outcome in futures::future::join_all(tasks.drain(..)).await {
            if let Ok(out) = outcome {
                pages_processed += 1;
                if out.failed { pages_failed += 1; if out.http_error { http_errors += 1; } }
                else { products_filled += out.products_filled; products_already += out.products_already; mismatches_detected += out.mismatches; would_fill += out.would_fill; }
                if debug_enabled { per_page_stats.push(PerPageDebugStat { page: out.page, parsed_count: out.parsed_count, updated_products: out.products_filled, updated_details: out.updated_details, mismatches: out.mismatches, would_fill: out.would_fill }); }
                info!(target="rehydrate", page=out.page, products_filled_page=out.products_filled, products_already=out.products_already, parsed_count=out.parsed_count, updated_details=out.updated_details, dry_run, failed=out.failed, http_err=out.http_error, "rehydrate_page_done");
            }
        }
    }


    // After metrics
    let after: i64 = sqlx::query_scalar(r"SELECT COUNT(*) FROM products p JOIN product_details d ON d.url = p.url
        WHERE (p.page_id IS NULL OR p.index_in_page IS NULL)
          AND (d.page_id IS NULL OR d.index_in_page IS NULL)")
        .fetch_one(&pool).await.unwrap_or(0);

    // Detect no-progress scenario (e.g., param mis-match or network blocked)
    let no_progress = !dry_run && products_filled == 0 && pages_processed > 0;
    let note = if dry_run {
        format!("dry_run=true pages_failed={pages_failed} mismatches_detected={mismatches_detected} http_errors={http_errors}")
    } else if no_progress {
        format!("rehydrate_complete BUT no_rows_filled pages_failed={pages_failed} mismatches_detected={mismatches_detected} http_errors={http_errors} (check: dry_run flag, network, URL pattern)")
    } else {
        format!("rehydrate_complete pages_failed={pages_failed} mismatches_detected={mismatches_detected} http_errors={http_errors}")
    };
    Ok(RehydrateListPagesResult { pages_targeted: targeted_pages.len() as u32, pages_processed, products_with_null_coords_before: before as u64, products_filled, products_already_had_coords: products_already, products_still_null_after: after as u64, elapsed_ms: start.elapsed().as_millis(), note, pages_failed, http_errors, mismatches_detected, would_fill, auto_selected, per_page: if debug_enabled { Some(per_page_stats) } else { None } })
}

// ============================================================================
// Filter-aware APIs for interactive filter experience
// ============================================================================

/// Available filter options based on current filter selections
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AvailableFilterOptions {
    pub categories: Vec<String>,
    pub device_types: Vec<String>,
    pub vendors: Vec<String>,
    pub transport_interfaces: Vec<String>,
}

/// Get available filter options based on current filter selections
/// This enables smart filtering where each filter shows only options that
/// will produce non-empty results given other active filters
#[tauri::command(rename_all = "snake_case")]
pub async fn get_available_filter_options(
    state: State<'_, DatabaseConnection>,
    current_filter: String,
) -> Result<AvailableFilterOptions, String> {
    // Log immediately at function entry to catch ALL invocations
    info!("🚨🚨🚨 get_available_filter_options CALLED! 🚨🚨🚨");
    info!("🔍 get_available_filter_options RAW parameter: '{}'", current_filter);
    
    let pool = state.pool();
    
    let filter_dsl = current_filter.trim();
    
    info!("🔍 get_available_filter_options filter_dsl after trim: '{}'", filter_dsl);
    
    // Parse the filter using the SAME logic as analytics_query
    // (DO NOT use parse_filter_dsl - it has a bug with whitespace splitting)
    let mut where_clauses: Vec<String> = Vec::new();
    let mut binds: Vec<String> = Vec::new();
    
    if !filter_dsl.is_empty() {
        // Enhanced tokenization: handle field:in:[...] specially (same as analytics_query)
        let mut tokens: Vec<String> = Vec::new();
        let mut current = String::new();
        let mut in_bracket = false;
        let mut paren_depth = 0;
        
        for ch in filter_dsl.chars() {
            match ch {
                '[' if !in_bracket => {
                    in_bracket = true;
                    current.push(ch);
                },
                ']' if in_bracket => {
                    in_bracket = false;
                    current.push(ch);
                },
                '(' => {
                    paren_depth += 1;
                    current.push(ch);
                },
                ')' => {
                    paren_depth -= 1;
                    current.push(ch);
                },
                ' ' | '\t' | '\n' if !in_bracket && paren_depth == 0 => {
                    if !current.is_empty() {
                        tokens.push(current.clone());
                        current.clear();
                    }
                },
                _ => {
                    current.push(ch);
                }
            }
        }
        if !current.is_empty() {
            tokens.push(current);
        }
        
        for t in &tokens {
            // Skip logical connector tokens
            let upper_tok = t.to_ascii_uppercase();
            if upper_tok == "AND" || upper_tok == "OR" {
                continue;
            }
            
            let t_str = t.as_str();
            
            let (field_part, op, value_part) = if let Some(pos) = t_str.find(">=") {
                (&t_str[..pos], Some(">="), &t_str[pos+2..])
            } else if let Some(pos) = t_str.find("<=") {
                (&t_str[..pos], Some("<="), &t_str[pos+2..])
            } else if let Some(pos) = t_str.find(":in:") {
                (&t_str[..pos], Some(":in:"), &t_str[pos+4..])
            } else if let Some(pos) = t_str.find('=') {
                (&t_str[..pos], Some("="), &t_str[pos+1..])
            } else if let Some(pos) = t_str.find('~') {
                (&t_str[..pos], Some("~"), &t_str[pos+1..])
            } else if let Some(pos) = t_str.find(':') {
                (&t_str[..pos], None, &t_str[pos+1..])
            } else {
                ("", None, t_str)
            };

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
            
            if value.is_empty() { continue; }
            
            // Map field
            let column = match field.as_str() {
                "vendor"|"v"|"ven" => "vendor_name",
                "vnum" => "vendor_number",
                "category"|"cat"|"device_category" => "device_category",
                "dtype"|"dname"|"dt"|"device_type_name" => "device_type_name",
                "transport"|"ti"|"transport_interface" => "transport_interface",
                "vendor_name" => "vendor_name",
                "model"|"m" => "model",
                "date" => "certification_date",
                "created"|"c" => "detail_created_at",
                _ => continue,
            };
            
            let op_used = op.unwrap_or(if matches!(field.as_str(), "date"|"created"|"c") { "=" } else { "~" });
            
            // Special handling for null values
            if value.eq_ignore_ascii_case("null") && (op_used == "=" || op_used == "~") {
                where_clauses.push(format!("{} IS NULL", column));
                continue;
            }
            
            match op_used {
                ":in:" => {
                    // Parse array: [value1,value2,...] or ["val1","val2",...]
                    if !value.starts_with('[') || !value.ends_with(']') {
                        continue;
                    }
                    let inner = &value[1..value.len()-1];
                    if inner.is_empty() {
                        continue;
                    }
                    
                    // Quote-aware CSV parsing
                    let mut items: Vec<String> = Vec::new();
                    let mut current_item = String::new();
                    let mut in_quotes = false;
                    let mut escape_next = false;
                    
                    for ch in inner.chars() {
                        if escape_next {
                            current_item.push(ch);
                            escape_next = false;
                        } else if ch == '\\' {
                            escape_next = true;
                        } else if ch == '"' {
                            in_quotes = !in_quotes;
                            current_item.push(ch);
                        } else if ch == ',' && !in_quotes {
                            let trimmed = current_item.trim();
                            if !trimmed.is_empty() {
                                let unquoted = if (trimmed.starts_with('"') && trimmed.ends_with('"')) || 
                                                  (trimmed.starts_with('\'') && trimmed.ends_with('\'')) {
                                    trimmed[1..trimmed.len()-1].to_string()
                                } else {
                                    trimmed.to_string()
                                };
                                items.push(unquoted);
                            }
                            current_item.clear();
                        } else {
                            current_item.push(ch);
                        }
                    }
                    
                    // Last item
                    let trimmed = current_item.trim();
                    if !trimmed.is_empty() {
                        let unquoted = if (trimmed.starts_with('"') && trimmed.ends_with('"')) || 
                                          (trimmed.starts_with('\'') && trimmed.ends_with('\'')) {
                            trimmed[1..trimmed.len()-1].to_string()
                        } else {
                            trimmed.to_string()
                        };
                        items.push(unquoted);
                    }
                    
                    if items.is_empty() {
                        continue;
                    }
                    
                    // Generate LIKE clauses for GROUP_CONCAT compatibility (same as analytics_query)
                    let like_clauses: Vec<String> = items.iter().map(|_| format!("{} LIKE ?", column)).collect();
                    where_clauses.push(format!("({})", like_clauses.join(" OR ")));
                    
                    for item in items {
                        binds.push(format!("%{}%", item));
                    }
                },
                "~" => {
                    where_clauses.push(format!("{} LIKE ?", column));
                    binds.push(format!("%{}%", value));
                },
                "=" => {
                    where_clauses.push(format!("{} = ?", column));
                    binds.push(value.to_string());
                },
                ">=" | "<=" => {
                    where_clauses.push(format!("{} {} ?", column, op_used));
                    binds.push(value.to_string());
                },
                _ => continue,
            }
        }
    }
    
    let where_clause = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };
    
    info!("🔍 Parsed WHERE clause: '{}', binds: {:?}", where_clause, binds);
    
    // Get available categories
    let categories_sql = if where_clause.is_empty() {
        "SELECT DISTINCT device_category FROM v_product_detail_analytics ORDER BY device_category".to_string()
    } else {
        format!(
            "SELECT DISTINCT device_category FROM v_product_detail_analytics {} ORDER BY device_category",
            where_clause  // where_clause already contains "WHERE ..."
        )
    };
    let mut categories_query = sqlx::query_scalar(&categories_sql);
    for val in &binds {
        categories_query = categories_query.bind(val);
    }
    let categories: Vec<String> = categories_query
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to fetch categories: {}", e))?;
    
    // Get available device types
    let device_types_sql = if where_clause.is_empty() {
        "SELECT DISTINCT device_type_name FROM v_product_detail_analytics WHERE device_type_name IS NOT NULL ORDER BY device_type_name".to_string()
    } else {
        format!(
            "SELECT DISTINCT device_type_name FROM v_product_detail_analytics {} AND device_type_name IS NOT NULL ORDER BY device_type_name",
            where_clause  // where_clause already contains "WHERE ..."
        )
    };
    let mut device_types_query = sqlx::query_scalar(&device_types_sql);
    for val in &binds {
        device_types_query = device_types_query.bind(val);
    }
    let device_types: Vec<String> = device_types_query
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to fetch device types: {}", e))?;
    
    // Get available vendors
    let vendors_sql = if where_clause.is_empty() {
        "SELECT DISTINCT vendor_name FROM v_product_detail_analytics WHERE vendor_name IS NOT NULL ORDER BY vendor_name".to_string()
    } else {
        format!(
            "SELECT DISTINCT vendor_name FROM v_product_detail_analytics {} AND vendor_name IS NOT NULL ORDER BY vendor_name",
            where_clause  // where_clause already contains "WHERE ..."
        )
    };
    let mut vendors_query = sqlx::query_scalar(&vendors_sql);
    for val in &binds {
        vendors_query = vendors_query.bind(val);
    }
    let vendors: Vec<String> = vendors_query
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to fetch vendors: {}", e))?;
    
    // Get available transport interfaces
    let transport_sql = if where_clause.is_empty() {
        "SELECT DISTINCT transport_interface FROM v_product_detail_analytics WHERE transport_interface IS NOT NULL ORDER BY transport_interface".to_string()
    } else {
        format!(
            "SELECT DISTINCT transport_interface FROM v_product_detail_analytics {} AND transport_interface IS NOT NULL ORDER BY transport_interface",
            where_clause  // where_clause already contains "WHERE ..."
        )
    };
    let mut transport_query = sqlx::query_scalar(&transport_sql);
    for val in &binds {
        transport_query = transport_query.bind(val);
    }
    let transport_interfaces: Vec<String> = transport_query
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to fetch transport interfaces: {}", e))?;
    
    Ok(AvailableFilterOptions {
        categories,
        device_types,
        vendors,
        transport_interfaces,
    })
}

/// Filtered analytics summary - returns summary and top rankings based on current filters
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FilteredAnalyticsSummary {
    pub total_products: i64,
    pub total_vendors: i64,
    pub total_categories: i64,
    pub total_device_types: i64,
    pub total_transport_interfaces: i64,
    pub top_vendors: Vec<(String, i64)>,
    pub top_categories: Vec<(String, i64)>,
    pub top_device_types: Vec<(String, i64)>,
    pub top_transport_interfaces: Vec<(String, i64)>,
    pub applied_filter: String,
}

/// Get filtered analytics summary based on current filter
#[tauri::command]
pub async fn get_filtered_analytics_summary(
    state: State<'_, DatabaseConnection>,
    filter: Option<String>,
) -> Result<FilteredAnalyticsSummary, String> {
    let pool = state.pool();
    let filter_dsl = filter.clone().unwrap_or_default();
    
    info!("📊 get_filtered_analytics_summary called with filter: {}", filter_dsl);
    
    // Use the SAME parsing logic as get_available_filter_options (smart tokenizer)
    // DO NOT use parse_filter_dsl - it's broken
    let mut where_clauses: Vec<String> = Vec::new();
    let mut binds: Vec<String> = Vec::new();
    
    if !filter_dsl.trim().is_empty() {
        // Smart tokenization (same as get_available_filter_options)
        let mut tokens: Vec<String> = Vec::new();
        let mut current = String::new();
        let mut in_bracket = false;
        let mut paren_depth = 0;
        
        for ch in filter_dsl.chars() {
            match ch {
                '[' if !in_bracket => {
                    in_bracket = true;
                    current.push(ch);
                },
                ']' if in_bracket => {
                    in_bracket = false;
                    current.push(ch);
                },
                '(' => {
                    paren_depth += 1;
                    current.push(ch);
                },
                ')' => {
                    paren_depth -= 1;
                    current.push(ch);
                },
                ' ' | '\t' | '\n' if !in_bracket && paren_depth == 0 => {
                    if !current.is_empty() {
                        tokens.push(current.clone());
                        current.clear();
                    }
                },
                _ => {
                    current.push(ch);
                }
            }
        }
        if !current.is_empty() {
            tokens.push(current);
        }
        
        for t in &tokens {
            let upper_tok = t.to_ascii_uppercase();
            if upper_tok == "AND" || upper_tok == "OR" {
                continue;
            }
            
            let t_str = t.as_str();
            let (field_part, op, value_part) = if let Some(pos) = t_str.find(">=") {
                (&t_str[..pos], Some(">="), &t_str[pos+2..])
            } else if let Some(pos) = t_str.find("<=") {
                (&t_str[..pos], Some("<="), &t_str[pos+2..])
            } else if let Some(pos) = t_str.find(":in:") {
                (&t_str[..pos], Some(":in:"), &t_str[pos+4..])
            } else if let Some(pos) = t_str.find('=') {
                (&t_str[..pos], Some("="), &t_str[pos+1..])
            } else if let Some(pos) = t_str.find('~') {
                (&t_str[..pos], Some("~"), &t_str[pos+1..])
            } else if let Some(pos) = t_str.find(':') {
                (&t_str[..pos], None, &t_str[pos+1..])
            } else {
                ("", None, t_str)
            };

            let field = field_part.to_lowercase();
            let value = value_part.trim();
            
            if field.is_empty() {
                if value.is_empty() { continue; }
                where_clauses.push("(model LIKE ? OR vendor_name LIKE ?)".into());
                let like = format!("%{}%", value);
                binds.push(like.clone());
                binds.push(like);
                continue;
            }
            
            if value.is_empty() { continue; }
            
            let column = match field.as_str() {
                "vendor"|"v"|"ven" => "vendor_name",
                "vnum" => "vendor_number",
                "category"|"cat"|"device_category" => "device_category",
                "dtype"|"dname"|"dt"|"device_type_name" => "device_type_name",
                "transport"|"ti"|"transport_interface" => "transport_interface",
                "vendor_name" => "vendor_name",
                "model"|"m" => "model",
                "date" => "certification_date",
                "created"|"c" => "detail_created_at",
                _ => continue,
            };
            
            let op_used = op.unwrap_or(if matches!(field.as_str(), "date"|"created"|"c") { "=" } else { "~" });
            
            if value.eq_ignore_ascii_case("null") && (op_used == "=" || op_used == "~") {
                where_clauses.push(format!("{} IS NULL", column));
                continue;
            }
            
            match op_used {
                ":in:" => {
                    if !value.starts_with('[') || !value.ends_with(']') {
                        continue;
                    }
                    let inner = &value[1..value.len()-1];
                    if inner.is_empty() {
                        continue;
                    }
                    
                    let mut items: Vec<String> = Vec::new();
                    let mut current_item = String::new();
                    let mut in_quotes = false;
                    let mut escape_next = false;
                    
                    for ch in inner.chars() {
                        if escape_next {
                            current_item.push(ch);
                            escape_next = false;
                        } else if ch == '\\' {
                            escape_next = true;
                        } else if ch == '"' {
                            in_quotes = !in_quotes;
                            current_item.push(ch);
                        } else if ch == ',' && !in_quotes {
                            let trimmed = current_item.trim();
                            if !trimmed.is_empty() {
                                let unquoted = if (trimmed.starts_with('"') && trimmed.ends_with('"')) || 
                                                  (trimmed.starts_with('\'') && trimmed.ends_with('\'')) {
                                    trimmed[1..trimmed.len()-1].to_string()
                                } else {
                                    trimmed.to_string()
                                };
                                items.push(unquoted);
                            }
                            current_item.clear();
                        } else {
                            current_item.push(ch);
                        }
                    }
                    
                    let trimmed = current_item.trim();
                    if !trimmed.is_empty() {
                        let unquoted = if (trimmed.starts_with('"') && trimmed.ends_with('"')) || 
                                          (trimmed.starts_with('\'') && trimmed.ends_with('\'')) {
                            trimmed[1..trimmed.len()-1].to_string()
                        } else {
                            trimmed.to_string()
                        };
                        items.push(unquoted);
                    }
                    
                    if items.is_empty() {
                        continue;
                    }
                    
                    let like_clauses: Vec<String> = items.iter().map(|_| format!("{} LIKE ?", column)).collect();
                    where_clauses.push(format!("({})", like_clauses.join(" OR ")));
                    
                    for item in items {
                        binds.push(format!("%{}%", item));
                    }
                },
                "~" => {
                    where_clauses.push(format!("{} LIKE ?", column));
                    binds.push(format!("%{}%", value));
                },
                "=" => {
                    where_clauses.push(format!("{} = ?", column));
                    binds.push(value.to_string());
                },
                ">=" | "<=" => {
                    where_clauses.push(format!("{} {} ?", column, op_used));
                    binds.push(value.to_string());
                },
                _ => continue,
            }
        }
    }
    
    let base_where = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };
    
    info!("📊 Generated WHERE clause: '{}' with {} binds", base_where, binds.len());
    for (i, bind) in binds.iter().enumerate() {
        info!("   Bind[{}]: '{}'", i, bind);
    }
    
    // Total products
    let total_products_sql = if base_where.is_empty() {
        "SELECT COUNT(*) FROM v_product_detail_analytics".to_string()
    } else {
        format!("SELECT COUNT(*) FROM v_product_detail_analytics {}", base_where)
    };
    info!("📊 Total products SQL: {}", total_products_sql);
    let mut total_products_query = sqlx::query_scalar::<_, i64>(&total_products_sql);
    for val in &binds {
        total_products_query = total_products_query.bind(val);
    }
    let total_products = total_products_query
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Failed to count products: {}", e))?;
    
    // Total unique vendors
    let total_vendors_sql = if base_where.is_empty() {
        "SELECT COUNT(DISTINCT vendor_name) FROM v_product_detail_analytics WHERE vendor_name IS NOT NULL".to_string()
    } else {
        format!("SELECT COUNT(DISTINCT vendor_name) FROM v_product_detail_analytics {} AND vendor_name IS NOT NULL", base_where)
    };
    let mut total_vendors_query = sqlx::query_scalar::<_, i64>(&total_vendors_sql);
    for val in &binds {
        total_vendors_query = total_vendors_query.bind(val);
    }
    let total_vendors = total_vendors_query
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Failed to count vendors: {}", e))?;
    
    // Total unique categories
    let total_categories_sql = if base_where.is_empty() {
        "SELECT COUNT(DISTINCT device_category) FROM v_product_detail_analytics WHERE device_category IS NOT NULL".to_string()
    } else {
        format!("SELECT COUNT(DISTINCT device_category) FROM v_product_detail_analytics {} AND device_category IS NOT NULL", base_where)
    };
    let mut total_categories_query = sqlx::query_scalar::<_, i64>(&total_categories_sql);
    for val in &binds {
        total_categories_query = total_categories_query.bind(val);
    }
    let total_categories = total_categories_query
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Failed to count categories: {}", e))?;
    
    // Total unique device types
    let total_device_types_sql = if base_where.is_empty() {
        "SELECT COUNT(DISTINCT device_type_name) FROM v_product_detail_analytics WHERE device_type_name IS NOT NULL".to_string()
    } else {
        format!("SELECT COUNT(DISTINCT device_type_name) FROM v_product_detail_analytics {} AND device_type_name IS NOT NULL", base_where)
    };
    let mut total_device_types_query = sqlx::query_scalar::<_, i64>(&total_device_types_sql);
    for val in &binds {
        total_device_types_query = total_device_types_query.bind(val);
    }
    let total_device_types = total_device_types_query
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Failed to count device types: {}", e))?;
    
    // Total unique transport interfaces
    let total_transport_interfaces_sql = if base_where.is_empty() {
        "SELECT COUNT(DISTINCT transport_interface) FROM v_product_detail_analytics WHERE transport_interface IS NOT NULL".to_string()
    } else {
        format!("SELECT COUNT(DISTINCT transport_interface) FROM v_product_detail_analytics {} AND transport_interface IS NOT NULL", base_where)
    };
    let mut total_transport_interfaces_query = sqlx::query_scalar::<_, i64>(&total_transport_interfaces_sql);
    for val in &binds {
        total_transport_interfaces_query = total_transport_interfaces_query.bind(val);
    }
    let total_transport_interfaces = total_transport_interfaces_query
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Failed to count transport interfaces: {}", e))?;
    
    // Top vendors
    let top_vendors_sql = if base_where.is_empty() {
        "SELECT vendor_name, COUNT(*) as cnt FROM v_product_detail_analytics WHERE vendor_name IS NOT NULL GROUP BY vendor_name ORDER BY cnt DESC LIMIT 10".to_string()
    } else {
        format!("SELECT vendor_name, COUNT(*) as cnt FROM v_product_detail_analytics {} AND vendor_name IS NOT NULL GROUP BY vendor_name ORDER BY cnt DESC LIMIT 10", base_where)
    };
    let mut top_vendors_query = sqlx::query(&top_vendors_sql);
    for val in &binds {
        top_vendors_query = top_vendors_query.bind(val);
    }
    let top_vendors: Vec<(String, i64)> = top_vendors_query
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to fetch top vendors: {}", e))?
        .into_iter()
        .filter_map(|r| {
            let name: Option<String> = r.get("vendor_name");
            let cnt: i64 = r.get("cnt");
            name.map(|n| (n, cnt))
        })
        .collect();
    
    // Top categories
    let top_categories_sql = if base_where.is_empty() {
        "SELECT device_category, COUNT(*) as cnt FROM v_product_detail_analytics WHERE device_category IS NOT NULL GROUP BY device_category ORDER BY cnt DESC LIMIT 10".to_string()
    } else {
        format!("SELECT device_category, COUNT(*) as cnt FROM v_product_detail_analytics {} AND device_category IS NOT NULL GROUP BY device_category ORDER BY cnt DESC LIMIT 10", base_where)
    };
    let mut top_categories_query = sqlx::query(&top_categories_sql);
    for val in &binds {
        top_categories_query = top_categories_query.bind(val);
    }
    let top_categories: Vec<(String, i64)> = top_categories_query
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to fetch top categories: {}", e))?
        .into_iter()
        .filter_map(|r| {
            let name: Option<String> = r.get("device_category");
            let cnt: i64 = r.get("cnt");
            name.map(|n| (n, cnt))
        })
        .collect();
    
    // Top device types
    let top_device_types_sql = if base_where.is_empty() {
        "SELECT device_type_name, COUNT(*) as cnt FROM v_product_detail_analytics WHERE device_type_name IS NOT NULL GROUP BY device_type_name ORDER BY cnt DESC LIMIT 10".to_string()
    } else {
        format!("SELECT device_type_name, COUNT(*) as cnt FROM v_product_detail_analytics {} AND device_type_name IS NOT NULL GROUP BY device_type_name ORDER BY cnt DESC LIMIT 10", base_where)
    };
    let mut top_device_types_query = sqlx::query(&top_device_types_sql);
    for val in &binds {
        top_device_types_query = top_device_types_query.bind(val);
    }
    let top_device_types: Vec<(String, i64)> = top_device_types_query
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to fetch top device types: {}", e))?
        .into_iter()
        .filter_map(|r| {
            let name: Option<String> = r.get("device_type_name");
            let cnt: i64 = r.get("cnt");
            name.map(|n| (n, cnt))
        })
        .collect();
    
    // Top transport interfaces
    let top_transport_interfaces_sql = if base_where.is_empty() {
        "SELECT transport_interface, COUNT(*) as cnt FROM v_product_detail_analytics WHERE transport_interface IS NOT NULL GROUP BY transport_interface ORDER BY cnt DESC LIMIT 10".to_string()
    } else {
        format!("SELECT transport_interface, COUNT(*) as cnt FROM v_product_detail_analytics {} AND transport_interface IS NOT NULL GROUP BY transport_interface ORDER BY cnt DESC LIMIT 10", base_where)
    };
    let mut top_transport_interfaces_query = sqlx::query(&top_transport_interfaces_sql);
    for val in &binds {
        top_transport_interfaces_query = top_transport_interfaces_query.bind(val);
    }
    let top_transport_interfaces: Vec<(String, i64)> = top_transport_interfaces_query
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to fetch top transport interfaces: {}", e))?
        .into_iter()
        .filter_map(|r| {
            let name: Option<String> = r.get("transport_interface");
            let cnt: i64 = r.get("cnt");
            name.map(|n| (n, cnt))
        })
        .collect();
    
    Ok(FilteredAnalyticsSummary {
        total_products,
        total_vendors,
        total_categories,
        total_device_types,
        total_transport_interfaces,
        top_vendors,
        top_categories,
        top_device_types,
        top_transport_interfaces,
        applied_filter: filter_dsl,
    })
}
