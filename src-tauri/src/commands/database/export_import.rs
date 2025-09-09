use std::path::PathBuf;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tauri::State;
use tracing::{error, info, warn};

use crate::infrastructure::DatabaseConnection;

/// Supported datasets for Phase 2 export/import.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dataset {
    Vendors,
    DeviceTypes,
    Analytics, // export-only (view)
}

impl Dataset {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "vendors" => Some(Self::Vendors),
            "device_types" | "device-types" | "devicetypes" => Some(Self::DeviceTypes),
            "analytics" => Some(Self::Analytics),
            _ => None,
        }
    }
    fn file_stem(&self) -> &'static str { match self { Self::Vendors => "vendors", Self::DeviceTypes => "device_types", Self::Analytics => "analytics" } }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportResult {
    pub dataset: String,
    pub processed: u32,
    pub inserted: u32,
    pub updated: u32,
    pub errors: Vec<String>,
    pub backup_file: Option<String>,
}

fn exports_dir() -> PathBuf {
    // Co-locate with main database file directory for portability.
    let db_url = crate::infrastructure::get_main_database_url();
    // Expect format: sqlite://<path>
    let path_part = db_url.strip_prefix("sqlite://").unwrap_or(&db_url);
    let mut p = PathBuf::from(path_part);
    if let Some(parent) = p.parent() { p = parent.to_path_buf(); }
    p.join("exports")
}

fn timestamp() -> String { Utc::now().format("%Y%m%d%H%M%S").to_string() }

/// Export a dataset to CSV. Returns the absolute file path.
#[tauri::command]
pub async fn export_data(state: State<'_, DatabaseConnection>, dataset: String) -> Result<String, String> {
    let Some(ds) = Dataset::from_str(&dataset) else { return Err(format!("Unknown dataset: {}", dataset)); };
    let pool = state.pool();
    let dir = exports_dir();
    if tokio::fs::create_dir_all(&dir).await.is_err() { warn!("Failed to create exports directory: {:?}", dir); }
    let filename = format!("{}_{}.csv", ds.file_stem(), timestamp());
    let path = dir.join(filename);
    let mut wtr = csv::Writer::from_writer(Vec::<u8>::new());

    match ds {
        Dataset::Vendors => {
            wtr.write_record(["vendor_number", "name"]).map_err(|e| e.to_string())?;
            let rows = sqlx::query("SELECT vendor_number, name FROM vendors ORDER BY vendor_number")
                .fetch_all(pool).await.map_err(|e| e.to_string())?;
            for r in rows { let vnum: i64 = r.get("vendor_number"); let name: String = r.get("name"); wtr.write_record([vnum.to_string(), name]).map_err(|e| e.to_string())?; }
        }
        Dataset::DeviceTypes => {
            wtr.write_record(["type_id", "code_hex", "name", "category", "introduced_in"]).map_err(|e| e.to_string())?;
            let rows = sqlx::query("SELECT type_id, code_hex, name, category, introduced_in FROM device_types ORDER BY type_id")
                .fetch_all(pool).await.map_err(|e| e.to_string())?;
            for r in rows { 
                let type_id: Option<i64> = r.get("type_id");
                let code_hex: Option<String> = r.get("code_hex");
                let name: String = r.get("name");
                let category: Option<String> = r.get("category");
                let introduced_in: Option<String> = r.get("introduced_in");
                wtr.write_record([
                    type_id.map(|v| v.to_string()).unwrap_or_default(),
                    code_hex.unwrap_or_default(),
                    name,
                    category.unwrap_or_default(),
                    introduced_in.unwrap_or_default()
                ]).map_err(|e| e.to_string())?;
            }
        }
        Dataset::Analytics => {
            wtr.write_record(["product_detail_url","model","vendor_name","device_type_name","device_category","certification_date","detail_created_at"]).map_err(|e| e.to_string())?;
            let rows = sqlx::query("SELECT product_detail_url, model, vendor_name, device_type_name, device_category, certification_date, detail_created_at FROM v_product_detail_analytics ORDER BY detail_created_at DESC")
                .fetch_all(pool).await.map_err(|e| e.to_string())?;
            for r in rows {
                let get = |col: &str| -> String { r.try_get::<Option<String>, _>(col).ok().flatten().unwrap_or_default() };
                wtr.write_record([
                    get("product_detail_url"),
                    get("model"),
                    get("vendor_name"),
                    get("device_type_name"),
                    get("device_category"),
                    get("certification_date"),
                    get("detail_created_at"),
                ]).map_err(|e| e.to_string())?;
            }
        }
    }

    let bytes = wtr.into_inner().map_err(|e| e.to_string())?;
    if let Err(e) = tokio::fs::write(&path, &bytes).await { return Err(format!("Failed to write export file: {}", e)); }
    info!("✅ Exported dataset '{}' to {:?}", dataset, path);
    Ok(path.to_string_lossy().to_string())
}

/// Import CSV (base64 encoded) for vendors or device_types. Returns structured result.
#[tauri::command]
pub async fn import_data(state: State<'_, DatabaseConnection>, dataset: String, base64_csv: String) -> Result<ImportResult, String> {
    let Some(ds) = Dataset::from_str(&dataset) else { return Err(format!("Unknown dataset: {}", dataset)); };
    if matches!(ds, Dataset::Analytics) { return Err("Analytics dataset is export-only".into()); }
    let pool = state.pool();
    // Create backup first (inline export to avoid double borrow of state)
    let backup_file = match ds {
        Dataset::Vendors | Dataset::DeviceTypes => {
            let dir = exports_dir();
            if tokio::fs::create_dir_all(&dir).await.is_err() { warn!("Failed to create exports directory for backup: {:?}", dir); }
            let filename = format!("backup_{}_{}.csv", ds.file_stem(), timestamp());
            let path = dir.join(&filename);
            let mut wtr = csv::Writer::from_writer(Vec::<u8>::new());
            let backup_res: Result<_, String> = async {
                match ds {
                    Dataset::Vendors => {
                        wtr.write_record(["vendor_number","name"]).map_err(|e| e.to_string())?;
                        let rows = sqlx::query("SELECT vendor_number,name FROM vendors ORDER BY vendor_number").fetch_all(pool).await.map_err(|e| e.to_string())?;
                        for r in rows { let v: i64 = r.get("vendor_number"); let n: String = r.get("name"); wtr.write_record([v.to_string(), n]).map_err(|e| e.to_string())?; }
                    }
                    Dataset::DeviceTypes => {
                        wtr.write_record(["type_id","code_hex","name","category","introduced_in"]).map_err(|e| e.to_string())?;
                        let rows = sqlx::query("SELECT type_id,code_hex,name,category,introduced_in FROM device_types ORDER BY type_id").fetch_all(pool).await.map_err(|e| e.to_string())?;
                        for r in rows { let tid: Option<i64> = r.get("type_id"); let ch: Option<String> = r.get("code_hex"); let n: String = r.get("name"); let cat: Option<String> = r.get("category"); let intro: Option<String> = r.get("introduced_in"); wtr.write_record([tid.map(|v| v.to_string()).unwrap_or_default(), ch.unwrap_or_default(), n, cat.unwrap_or_default(), intro.unwrap_or_default()]).map_err(|e| e.to_string())?; }
                    }
                    Dataset::Analytics => unreachable!(),
                }
                let bytes = wtr.into_inner().map_err(|e| e.to_string())?;
                tokio::fs::write(&path, &bytes).await.map_err(|e| e.to_string())?;
                Ok::<_, String>(path.to_string_lossy().to_string())
            }.await;
            match backup_res { Ok(p) => Some(p), Err(e) => { warn!("Failed to create pre-import backup: {}", e); None } }
        }
        Dataset::Analytics => None,
    };

    let decoded = BASE64_STANDARD.decode(base64_csv.as_bytes()).map_err(|e| format!("Base64 decode failed: {}", e))?;
    let mut rdr = csv::Reader::from_reader(decoded.as_slice());
    let headers = rdr.headers().map_err(|e| e.to_string())?.clone();

    let mut result = ImportResult { dataset: dataset.clone(), processed: 0, inserted: 0, updated: 0, errors: Vec::new(), backup_file };

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    while let Some(record_res) = rdr.records().next() {
        match record_res {
            Ok(rec) => {
                result.processed += 1;
                match ds {
                    Dataset::Vendors => {
                        // Expect vendor_number,name
                        let idx_vnum = headers.iter().position(|h| h.eq_ignore_ascii_case("vendor_number"));
                        let idx_name = headers.iter().position(|h| h.eq_ignore_ascii_case("name"));
                        if let (Some(i_vnum), Some(i_name)) = (idx_vnum, idx_name) {
                            let vnum_str = rec.get(i_vnum).unwrap_or("").trim();
                            let name = rec.get(i_name).unwrap_or("").trim();
                            if vnum_str.is_empty() || name.is_empty() { result.errors.push(format!("Row {} missing vendor_number or name", result.processed)); continue; }
                            let Ok(vnum) = vnum_str.parse::<i64>() else { result.errors.push(format!("Invalid vendor_number '{}' at row {}", vnum_str, result.processed)); continue; };
                            // Determine insert vs update
                            let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM vendors WHERE vendor_number = ?1 LIMIT 1").bind(vnum).fetch_optional(&mut *tx).await.map_err(|e| e.to_string())?;
                            sqlx::query("INSERT INTO vendors (vendor_number, name) VALUES (?1, ?2) ON CONFLICT(vendor_number) DO UPDATE SET name=excluded.name")
                                .bind(vnum).bind(name).execute(&mut *tx).await.map_err(|e| e.to_string())?;
                            if exists.is_some() { result.updated += 1; } else { result.inserted += 1; }
                        } else { result.errors.push(format!("Row {} missing required headers", result.processed)); }
                    }
                    Dataset::DeviceTypes => {
                        // Expect type_id,code_hex,name,category,introduced_in
                        let get_idx = |n: &str| headers.iter().position(|h| h.eq_ignore_ascii_case(n));
                        let idx_type = get_idx("type_id");
                        let idx_name = get_idx("name");
                        let idx_hex = get_idx("code_hex");
                        if let (Some(i_type), Some(i_name), Some(i_hex)) = (idx_type, idx_name, idx_hex) {
                            let type_id_str = rec.get(i_type).unwrap_or("").trim();
                            let name = rec.get(i_name).unwrap_or("").trim();
                            let code_hex = rec.get(i_hex).unwrap_or("").trim();
                            if type_id_str.is_empty() || name.is_empty() { result.errors.push(format!("Row {} missing type_id or name", result.processed)); continue; }
                            let Ok(type_id) = type_id_str.parse::<i64>() else { result.errors.push(format!("Invalid type_id '{}' at row {}", type_id_str, result.processed)); continue; };
                            let category = get_idx("category").and_then(|i| rec.get(i)).unwrap_or("");
                            let introduced_in = get_idx("introduced_in").and_then(|i| rec.get(i)).unwrap_or("");
                            let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM device_types WHERE type_id = ?1 LIMIT 1").bind(type_id).fetch_optional(&mut *tx).await.map_err(|e| e.to_string())?;
                            sqlx::query("INSERT INTO device_types (type_id, code_hex, name, category, introduced_in) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(type_id) DO UPDATE SET code_hex=excluded.code_hex, name=excluded.name, category=excluded.category, introduced_in=excluded.introduced_in")
                                .bind(type_id).bind(code_hex).bind(name).bind(category).bind(introduced_in).execute(&mut *tx).await.map_err(|e| e.to_string())?;
                            if exists.is_some() { result.updated += 1; } else { result.inserted += 1; }
                        } else { result.errors.push(format!("Row {} missing required headers", result.processed)); }
                    }
                    Dataset::Analytics => unreachable!(),
                }
            }
            Err(e) => result.errors.push(format!("CSV parse error row {}: {}", result.processed + 1, e)),
        }
    }
    if let Err(e) = tx.commit().await { error!("Import transaction failed: {}", e); return Err(format!("Import transaction failed: {}", e)); }
    info!("✅ Import complete dataset='{}' processed={} inserted={} updated={} errors={}", dataset, result.processed, result.inserted, result.updated, result.errors.len());
    Ok(result)
}

// ---------------------------------------------------------------------------
// Phase 3: Delete Range (product_details & products) - preview + execute
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteRangePreview {
    pub from_page: u32,
    pub to_page: u32,
    pub product_details_count: i64,
    pub products_count: i64,
    pub product_details_with_primary_types: i64,
}

#[tauri::command]
pub async fn preview_delete_range(
    state: State<'_, DatabaseConnection>,
    from_page: u32,
    to_page: u32,
) -> Result<DeleteRangePreview, String> {
    if to_page < from_page { return Err("to_page must be >= from_page".into()); }
    // Guard insane ranges
    if to_page - from_page > 50 { return Err("Range too large (max 50 pages per operation)".into()); }
    let pool = state.pool();
    let sql_pd = r#"SELECT COUNT(*) FROM product_details WHERE page_id BETWEEN ?1 AND ?2"#;
    let sql_p = r#"SELECT COUNT(*) FROM products WHERE page_id BETWEEN ?1 AND ?2"#;
    let sql_bridge = r#"SELECT COUNT(DISTINCT ppt.product_detail_id) FROM product_primary_device_types ppt JOIN product_details pd ON pd.url = ppt.product_detail_id WHERE pd.page_id BETWEEN ?1 AND ?2"#;
    let product_details_count = sqlx::query_scalar::<_, i64>(sql_pd).bind(from_page).bind(to_page).fetch_one(pool).await.unwrap_or(0);
    let products_count = sqlx::query_scalar::<_, i64>(sql_p).bind(from_page).bind(to_page).fetch_one(pool).await.unwrap_or(0);
    let product_details_with_primary_types = sqlx::query_scalar::<_, i64>(sql_bridge).bind(from_page).bind(to_page).fetch_one(pool).await.unwrap_or(0);
    Ok(DeleteRangePreview { from_page, to_page, product_details_count, products_count, product_details_with_primary_types })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteRangeResult {
    pub from_page: u32,
    pub to_page: u32,
    pub deleted_product_details: u64,
    pub deleted_products: u64,
    pub deleted_bridge_rows: u64,
}

#[tauri::command]
pub async fn delete_range(
    state: State<'_, DatabaseConnection>,
    from_page: u32,
    to_page: u32,
) -> Result<DeleteRangeResult, String> {
    if to_page < from_page { return Err("to_page must be >= from_page".into()); }
    if to_page - from_page > 50 { return Err("Range too large (max 50 pages per operation)".into()); }
    let pool = state.pool();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    // Delete bridge rows first (FK safety)
    let bridge_del = sqlx::query("DELETE FROM product_primary_device_types WHERE product_detail_id IN (SELECT url FROM product_details WHERE page_id BETWEEN ?1 AND ?2)")
        .bind(from_page).bind(to_page).execute(&mut *tx).await.map_err(|e| e.to_string())?.rows_affected();
    let pd_del = sqlx::query("DELETE FROM product_details WHERE page_id BETWEEN ?1 AND ?2")
        .bind(from_page).bind(to_page).execute(&mut *tx).await.map_err(|e| e.to_string())?.rows_affected();
    let p_del = sqlx::query("DELETE FROM products WHERE page_id BETWEEN ?1 AND ?2")
        .bind(from_page).bind(to_page).execute(&mut *tx).await.map_err(|e| e.to_string())?.rows_affected();
    tx.commit().await.map_err(|e| e.to_string())?;
    info!("🗑️ Deleted pages range {}-{} (products={}, product_details={}, bridge={})", from_page, to_page, p_del, pd_del, bridge_del);
    Ok(DeleteRangeResult { from_page, to_page, deleted_product_details: pd_del, deleted_products: p_del, deleted_bridge_rows: bridge_del })
}

