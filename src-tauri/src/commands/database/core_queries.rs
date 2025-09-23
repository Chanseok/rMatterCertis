use crate::infrastructure::integrated_product_repository::IntegratedProductRepository;
use sqlx::SqlitePool;
use crate::domain::product::Product;
use tracing::{error};
use regex::Regex;
use sqlx::Row;

/// Result row for core analytics (mirrors AnalyticsRow in data_queries)
#[derive(Debug, Clone)]
pub struct CoreAnalyticsRow {
    pub product_detail_url: Option<String>,
    pub model: Option<String>,
    pub vendor_name: Option<String>,
    pub device_type_name: Option<String>,
    pub device_category: Option<String>,
    pub certification_date: Option<String>,
    pub detail_created_at: Option<String>,
    pub transport_interface: Option<String>,
}

pub struct CoreAnalyticsPage {
    pub rows: Vec<CoreAnalyticsRow>,
    pub total: i64,
    pub offset: i64,
    pub limit: i64,
}

#[derive(Debug, Clone)]
pub struct CoreDiagnosticsAnalytics {
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
    pub mapping_coverage_pct: f64,
}

/// Core diagnostics (subset) without samples/backfill side effects.
pub async fn core_diagnostics_analytics_mapping(pool: &SqlitePool) -> CoreDiagnosticsAnalytics {
    let product_details_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_details").fetch_one(pool).await.unwrap_or(0);
    let product_details_with_ids: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_details WHERE primary_device_type_ids IS NOT NULL AND primary_device_type_ids <> ''").fetch_one(pool).await.unwrap_or(0);
    let bridge_exists: bool = sqlx::query_scalar::<_, i64>("SELECT 1 FROM sqlite_master WHERE type='table' AND name='product_primary_device_types' LIMIT 1")
        .fetch_optional(pool).await.unwrap_or(None).is_some();
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
    let mapping_coverage_pct = if analytics_distinct_products_total > 0 { (analytics_distinct_products_mapped as f64 * 100.0) / analytics_distinct_products_total as f64 } else { 0.0 };
    CoreDiagnosticsAnalytics { product_details_with_ids, product_details_total, bridge_rows, distinct_bridge_products, device_types_total, device_types_with_type_id, device_types_type_id_null, analytics_rows_total, analytics_with_device_type, analytics_distinct_products_total, analytics_distinct_products_mapped, mapping_coverage_pct }
}

/// Core analytics query without Tauri State dependency.
pub async fn core_analytics_query(
    pool: &SqlitePool,
    offset: i64,
    limit: i64,
    raw_filter: Option<&str>,
    sort: Option<&[String]>,
) -> Result<CoreAnalyticsPage, String> {
    let mut limit = limit;
    if limit <= 0 { limit = 50; }
    if limit > 200 { limit = 200; }
    let offset = offset.max(0);
    let raw_filter = raw_filter.map(|s| s.trim().to_string()).unwrap_or_default();
    let mut where_clauses: Vec<String> = Vec::new();
    let mut binds: Vec<String> = Vec::new();
    if !raw_filter.is_empty() {
        let tokens: Vec<&str> = raw_filter.split_whitespace().collect();
        let _field_pattern = Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*").unwrap();
        let mut filter_error: Option<String> = None;
        for t in &tokens {
            if t.contains('"') || t.contains('\'') { filter_error = Some("quote not supported".into()); break; }
            let (field_part, op, value_part) = if let Some(pos) = t.find(">=") { (&t[..pos], Some(">="), &t[pos+2..]) } else if let Some(pos) = t.find("<=") { (&t[..pos], Some("<="), &t[pos+2..]) } else if let Some(pos) = t.find('=') { (&t[..pos], Some("="), &t[pos+1..]) } else if let Some(pos) = t.find('~') { (&t[..pos], Some("~"), &t[pos+1..]) } else if let Some(pos) = t.find(':') { (&t[..pos], None, &t[pos+1..]) } else { ("", None, *t) };
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
            if value.is_empty() { filter_error = Some("empty value".into()); break; }
            let column = match field.as_str() {
                "vendor"|"v"|"ven" => "vendor_name",
                "vnum" => "vendor_number",
                "category"|"cat" => "device_category",
                "dtype"|"dname"|"dt" => "device_type_name",
                "model"|"m" => "model",
                "date" => "certification_date",
                "created"|"c" => "detail_created_at",
                _ => { filter_error = Some("unknown field".into()); break; }
            };
            let op_used = op.unwrap_or(if matches!(field.as_str(), "date"|"created"|"c") { "=" } else { "~" });
            match op_used {
                "~" => { where_clauses.push(format!("{} LIKE ?", column)); binds.push(format!("%{}%", value)); },
                "=" | ">=" | "<=" => { where_clauses.push(format!("{} {} ?", column, op_used)); binds.push(value.to_string()); },
                _ => { filter_error = Some("bad op".into()); break; }
            }
        }
        if filter_error.is_some() { where_clauses.clear(); binds.clear(); }
    }
    let where_sql = if !where_clauses.is_empty() { format!("WHERE {}", where_clauses.join(" AND ")) } else { String::new() };
    let count_sql = format!("SELECT COUNT(*) FROM v_product_detail_analytics {}", where_sql);
    let mut cq = sqlx::query_scalar::<_, i64>(&count_sql);
    for b in &binds { cq = cq.bind(b); }
    let total = cq.fetch_one(pool).await.unwrap_or(0);
    // Sorting
    let mut order_by_parts: Vec<String> = Vec::new();
    if let Some(sort_vec) = sort { for s in sort_vec { let mut split = s.split(':'); let field = split.next().unwrap_or(""); let dir_raw = split.next().unwrap_or(""); let dir = match dir_raw.to_lowercase().as_str() { "asc" => "ASC", "desc" => "DESC", _ => continue }; let col = match field { "device_category"=>Some("device_category"), "device_type_name"=>Some("device_type_name"), "model"=>Some("model"), "vendor_name"=>Some("vendor_name"), "certification_date"=>Some("certification_date"), "detail_created_at"=>Some("detail_created_at"), "transport_interface"|"transport_if"=>Some("transport_interface"), _=>None }; if let Some(c) = col { order_by_parts.push(format!("{} {}", c, dir)); }} }
    if order_by_parts.is_empty() { order_by_parts.push("detail_created_at DESC".into()); }
    let order_by_sql = format!("ORDER BY {}", order_by_parts.join(", "));
    let select_sql = format!("SELECT product_detail_url, model, vendor_name, device_type_name, device_category, certification_date, detail_created_at, transport_interface FROM v_product_detail_analytics {} {} LIMIT ? OFFSET ?", where_sql, order_by_sql);
    let mut q = sqlx::query(&select_sql);
    for b in &binds { q = q.bind(b); }
    q = q.bind(limit).bind(offset);
    let rows = match q.fetch_all(pool).await { Ok(rs) => rs.into_iter().map(|r| CoreAnalyticsRow { product_detail_url: r.get("product_detail_url"), model: r.get("model"), vendor_name: r.get("vendor_name"), device_type_name: r.get("device_type_name"), device_category: r.get("device_category"), certification_date: r.get("certification_date"), detail_created_at: r.get("detail_created_at"), transport_interface: r.get("transport_interface") }).collect(), Err(_) => Vec::new() };
    Ok(CoreAnalyticsPage { rows, total, offset, limit })
}

/// Core (side-effect free) pagination query.
/// Returns (products, total_count) or error string.
pub async fn core_fetch_products_page(
    pool: &SqlitePool,
    page: u32,
    size: u32,
) -> Result<(Vec<Product>, u32), String> {
    let repo = IntegratedProductRepository::new(pool.clone());
    let page_i32 = i32::try_from(page).unwrap_or(i32::MAX);
    let size_i32 = i32::try_from(size).unwrap_or(i32::MAX);

    let products = repo.get_products_paginated(page_i32, size_i32).await.map_err(|e| format!("paginate error: {e}"))?;

    let total_count = match repo.count_products().await {
        Ok(count) => u32::try_from(count).unwrap_or(u32::MAX),
        Err(e) => {
            error!(?e, "count_products failed");
            0
        }
    };
    Ok((products, total_count))
}
