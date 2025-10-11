use std::path::PathBuf;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tauri::State;
use tracing::{error, info, warn};
use rust_xlsxwriter::{Workbook, Format, Color};
use calamine::{Reader, open_workbook, Xlsx, DataType};
use ts_rs::TS;

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
    // Expect format: sqlite://<path> or sqlite:<path>
    // Remove "sqlite://" or "sqlite:" prefix
    let path_str = if let Some(p) = db_url.strip_prefix("sqlite://") {
        p
    } else if let Some(p) = db_url.strip_prefix("sqlite:") {
        p
    } else {
        &db_url
    };
    
    let mut p = PathBuf::from(path_str);
    if let Some(parent) = p.parent() { 
        p = parent.to_path_buf(); 
    }
    let exports = p.join("exports");
    info!("📂 DB URL: {}, Exports directory: {:?}", db_url, exports);
    exports
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
            wtr.write_record(["vendor_number", "vendor_name"]).map_err(|e| e.to_string())?;
            let rows = sqlx::query("SELECT vendor_number, vendor_name FROM vendors ORDER BY vendor_number")
                .fetch_all(pool).await.map_err(|e| e.to_string())?;
            for r in rows { let vnum: i64 = r.get("vendor_number"); let name: String = r.get("vendor_name"); wtr.write_record([vnum.to_string(), name]).map_err(|e| e.to_string())?; }
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
                        wtr.write_record(["vendor_number","vendor_name"]).map_err(|e| e.to_string())?;
                        let rows = sqlx::query("SELECT vendor_number,vendor_name FROM vendors ORDER BY vendor_number").fetch_all(pool).await.map_err(|e| e.to_string())?;
                        for r in rows { let v: i64 = r.get("vendor_number"); let n: String = r.get("vendor_name"); wtr.write_record([v.to_string(), n]).map_err(|e| e.to_string())?; }
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
                        // Expect vendor_number,vendor_name
                        let idx_vnum = headers.iter().position(|h| h.eq_ignore_ascii_case("vendor_number"));
                        let idx_name = headers.iter().position(|h| h.eq_ignore_ascii_case("vendor_name") || h.eq_ignore_ascii_case("name"));
                        if let (Some(i_vnum), Some(i_name)) = (idx_vnum, idx_name) {
                            let vnum_str = rec.get(i_vnum).unwrap_or("").trim();
                            let name = rec.get(i_name).unwrap_or("").trim();
                            if vnum_str.is_empty() || name.is_empty() { result.errors.push(format!("Row {} missing vendor_number or vendor_name", result.processed)); continue; }
                            let Ok(vnum) = vnum_str.parse::<i64>() else { result.errors.push(format!("Invalid vendor_number '{}' at row {}", vnum_str, result.processed)); continue; };
                            // Determine insert vs update
                            let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM vendors WHERE vendor_number = ?1 LIMIT 1").bind(vnum).fetch_optional(&mut *tx).await.map_err(|e| e.to_string())?;
                            sqlx::query("INSERT INTO vendors (vendor_number, vendor_name) VALUES (?1, ?2) ON CONFLICT(vendor_number) DO UPDATE SET vendor_name=excluded.vendor_name")
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

/// Get the exports directory path (used as default path for import dialogs)
#[tauri::command]
pub fn get_exports_directory() -> String {
    exports_dir().to_string_lossy().to_string()
}

/// Get the maximum page_id from products table
#[tauri::command]
pub async fn get_max_page_id(
    state: State<'_, DatabaseConnection>,
) -> Result<u32, String> {
    let pool = state.pool();
    let max_page_id = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT MAX(page_id) FROM products"
    )
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?
    .unwrap_or(0);
    
    Ok(max_page_id as u32)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteRangePreview {
    pub from_page: u32,
    pub to_page: u32,
    pub product_details_count: i64,
    pub products_count: i64,
}

#[tauri::command]
pub async fn preview_delete_range(
    state: State<'_, DatabaseConnection>,
    from_page: u32,
    to_page: u32,
) -> Result<DeleteRangePreview, String> {
    info!("🔍 preview_delete_range called: from_page={}, to_page={}", from_page, to_page);
    if to_page < from_page { return Err("to_page must be >= from_page".into()); }
    // Guard insane ranges
    if to_page - from_page > 100 { return Err("Range too large (max 100 pages per operation)".into()); }
    let pool = state.pool();
    let sql_pd = r#"SELECT COUNT(*) FROM product_details WHERE page_id BETWEEN ?1 AND ?2"#;
    let sql_p = r#"SELECT COUNT(*) FROM products WHERE page_id BETWEEN ?1 AND ?2"#;
    let product_details_count = sqlx::query_scalar::<_, i64>(sql_pd).bind(from_page).bind(to_page).fetch_one(pool).await.unwrap_or(0);
    let products_count = sqlx::query_scalar::<_, i64>(sql_p).bind(from_page).bind(to_page).fetch_one(pool).await.unwrap_or(0);
    info!("✅ preview_delete_range result: products={}, product_details={}", products_count, product_details_count);
    Ok(DeleteRangePreview { from_page, to_page, product_details_count, products_count })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteRangeResult {
    pub from_page: u32,
    pub to_page: u32,
    pub deleted_product_details: u64,
    pub deleted_products: u64,
}

#[tauri::command]
pub async fn delete_range(
    state: State<'_, DatabaseConnection>,
    from_page: u32,
    to_page: u32,
) -> Result<DeleteRangeResult, String> {
    if to_page < from_page { return Err("to_page must be >= from_page".into()); }
    if to_page - from_page > 100 { return Err("Range too large (max 100 pages per operation)".into()); }
    let pool = state.pool();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    
    // Delete product_details first (child table)
    let pd_del = sqlx::query("DELETE FROM product_details WHERE page_id BETWEEN ?1 AND ?2")
        .bind(from_page).bind(to_page).execute(&mut *tx).await.map_err(|e| e.to_string())?.rows_affected();
    
    // Delete products (parent table)
    let p_del = sqlx::query("DELETE FROM products WHERE page_id BETWEEN ?1 AND ?2")
        .bind(from_page).bind(to_page).execute(&mut *tx).await.map_err(|e| e.to_string())?.rows_affected();
    
    tx.commit().await.map_err(|e| e.to_string())?;
    info!("🗑️ Deleted pages range {}-{} (products={}, product_details={})", from_page, to_page, p_del, pd_del);
    Ok(DeleteRangeResult { from_page, to_page, deleted_product_details: pd_del, deleted_products: p_del })
}

// ---------------------------------------------------------------------------
// Excel Export/Import for Full Database Backup/Restore
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ExcelExportResult {
    pub file_path: String,
    pub products_count: i64,
    pub product_details_count: i64,
    pub device_types_count: i64,
    pub vendors_count: i64,
}

/// Export full database (products + product_details + device_types + vendors) to Excel (.xlsx)
#[tauri::command]
pub async fn export_full_database_excel(
    state: State<'_, DatabaseConnection>,
) -> Result<ExcelExportResult, String> {
    info!("🚀 Starting full database Excel export...");
    
    let pool = state.pool();
    let dir = exports_dir();
    
    // Create exports directory
    if let Err(e) = tokio::fs::create_dir_all(&dir).await {
        let err_msg = format!("Failed to create exports directory {:?}: {}", dir, e);
        error!("{}", err_msg);
        return Err(err_msg);
    }
    
    let filename = format!("full_database_{}.xlsx", timestamp());
    let path = dir.join(&filename);
    info!("📁 Export path: {:?}", path);
    
    // Create workbook
    let mut workbook = Workbook::new();
    
    // Header format
    let header_format = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0x4472C4))
        .set_font_color(Color::White);
    
    // ===== Products Sheet =====
    info!("📊 Creating products sheet...");
    let products_sheet = workbook.add_worksheet();
    if let Err(e) = products_sheet.set_name("products") {
        let err_msg = format!("Failed to set products sheet name: {}", e);
        error!("{}", err_msg);
        return Err(err_msg);
    }
    
    // Products headers
    let product_headers = vec![
        "url", "manufacturer", "model", "certificate_id", "page_id", 
        "index_in_page", "id", "created_at", "updated_at"
    ];
    
    for (col, header) in product_headers.iter().enumerate() {
        products_sheet
            .write_string_with_format(0, col as u16, &**header, &header_format)
            .map_err(|e| format!("Failed to write product header '{}': {}", header, e))?;
    }
    
    // Query products
    info!("🔍 Querying products...");
    let products = sqlx::query(
        "SELECT url, manufacturer, model, certificate_id, page_id, index_in_page, id, created_at, updated_at FROM products ORDER BY page_id, index_in_page"
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch products: {}", e))?;
    
    let products_count = products.len() as i64;
    info!("✅ Found {} products", products_count);
    
    for (row_idx, product) in products.iter().enumerate() {
        let row = (row_idx + 1) as u32;
        
        if row_idx > 0 && row_idx % 1000 == 0 {
            info!("  📝 Writing product row {}/{}", row_idx, products_count);
        }
        
        let url: Option<String> = product.try_get("url").ok().flatten();
        let manufacturer: Option<String> = product.try_get("manufacturer").ok().flatten();
        let model: Option<String> = product.try_get("model").ok().flatten();
        let certificate_id: Option<String> = product.try_get("certificate_id").ok().flatten();
        let page_id: Option<i64> = product.try_get("page_id").ok().flatten();
        let index_in_page: Option<i64> = product.try_get("index_in_page").ok().flatten();
        let id: Option<String> = product.try_get("id").ok().flatten();
        let created_at: Option<String> = product.try_get("created_at").ok().flatten();
        let updated_at: Option<String> = product.try_get("updated_at").ok().flatten();
        
        products_sheet.write_string(row, 0, &url.unwrap_or_default())
            .map_err(|e| format!("Failed to write product URL at row {}: {}", row, e))?;
        products_sheet.write_string(row, 1, &manufacturer.unwrap_or_default())
            .map_err(|e| format!("Failed to write manufacturer at row {}: {}", row, e))?;
        products_sheet.write_string(row, 2, &model.unwrap_or_default())
            .map_err(|e| format!("Failed to write model at row {}: {}", row, e))?;
        products_sheet.write_string(row, 3, &certificate_id.unwrap_or_default())
            .map_err(|e| format!("Failed to write certificate_id at row {}: {}", row, e))?;
        if let Some(pid) = page_id {
            products_sheet.write_number(row, 4, pid as f64)
                .map_err(|e| format!("Failed to write page_id at row {}: {}", row, e))?;
        }
        if let Some(idx) = index_in_page {
            products_sheet.write_number(row, 5, idx as f64)
                .map_err(|e| format!("Failed to write index_in_page at row {}: {}", row, e))?;
        }
        products_sheet.write_string(row, 6, &id.unwrap_or_default())
            .map_err(|e| format!("Failed to write id at row {}: {}", row, e))?;
        products_sheet.write_string(row, 7, &created_at.unwrap_or_default())
            .map_err(|e| format!("Failed to write created_at at row {}: {}", row, e))?;
        products_sheet.write_string(row, 8, &updated_at.unwrap_or_default())
            .map_err(|e| format!("Failed to write updated_at at row {}: {}", row, e))?;
    }
    
    // ===== Product Details Sheet =====
    info!("📊 Creating product_details sheet...");
    let details_sheet = workbook.add_worksheet();
    if let Err(e) = details_sheet.set_name("product_details") {
        let err_msg = format!("Failed to set product_details sheet name: {}", e);
        error!("{}", err_msg);
        return Err(err_msg);
    }
    
    // Product details headers (all columns from schema)
    let detail_headers = vec![
        "url", "page_id", "index_in_page", "id", "manufacturer", "model", "device_type",
        "certificate_id", "certification_date", "hardware_version",
        "firmware_version", "specification_version", "vid", "pid", "family_sku",
        "family_variant_sku", "family_id", "transport_interface",
        "primary_device_type_ids", "created_at", "updated_at"
    ];
    
    for (col, header) in detail_headers.iter().enumerate() {
        details_sheet
            .write_string_with_format(0, col as u16, &**header, &header_format)
            .map_err(|e| format!("Failed to write detail header '{}': {}", header, e))?;
    }
    
    // Query product_details
    info!("🔍 Querying product_details...");
    let details = sqlx::query(
        r#"SELECT url, page_id, index_in_page, id, manufacturer, model, device_type, certificate_id,
           certification_date, hardware_version, firmware_version, specification_version,
           vid, pid, family_sku, family_variant_sku, family_id, transport_interface,
           primary_device_type_ids, created_at, updated_at 
           FROM product_details ORDER BY page_id, index_in_page"#
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch product_details: {}", e))?;
    
    let details_count = details.len() as i64;
    info!("✅ Found {} product_details", details_count);
    
    for (row_idx, detail) in details.iter().enumerate() {
        let row = (row_idx + 1) as u32;
        
        if row_idx > 0 && row_idx % 1000 == 0 {
            info!("  📝 Writing detail row {}/{}", row_idx, details_count);
        }
        
        macro_rules! write_str {
            ($col:expr, $field:expr) => {
                if let Ok(Some(val)) = detail.try_get::<Option<String>, _>($field) {
                    details_sheet.write_string(row, $col, &val)
                        .map_err(|e| format!("Failed to write {} at row {}: {}", $field, row, e))?;
                }
            };
        }
        
        macro_rules! write_num {
            ($col:expr, $field:expr) => {
                if let Ok(Some(val)) = detail.try_get::<Option<i64>, _>($field) {
                    details_sheet.write_number(row, $col, val as f64)
                        .map_err(|e| format!("Failed to write {} at row {}: {}", $field, row, e))?;
                }
            };
        }
        
        write_str!(0, "url");
        write_num!(1, "page_id");
        write_num!(2, "index_in_page");
        write_str!(3, "id");
        write_str!(4, "manufacturer");
        write_str!(5, "model");
        write_str!(6, "device_type");
        write_str!(7, "certificate_id");
        write_str!(8, "certification_date");
        write_str!(9, "hardware_version");
        write_str!(10, "firmware_version");
        write_str!(11, "specification_version");
        write_num!(12, "vid");
        write_num!(13, "pid");
        write_str!(14, "family_sku");
        write_str!(15, "family_variant_sku");
        write_str!(16, "family_id");
        write_str!(17, "transport_interface");
        write_str!(18, "primary_device_type_ids");
        write_str!(19, "created_at");
        write_str!(20, "updated_at");
    }
    
    // ===== Device Types Sheet =====
    info!("📊 Creating device_types sheet...");
    let device_types_sheet = workbook.add_worksheet();
    if let Err(e) = device_types_sheet.set_name("device_types") {
        let err_msg = format!("Failed to set device_types sheet name: {}", e);
        error!("{}", err_msg);
        return Err(err_msg);
    }
    
    // Device types headers
    let device_type_headers = vec![
        "type_id", "code_hex", "name", "category", "introduced_in", "created_at", "updated_at"
    ];
    
    for (col, header) in device_type_headers.iter().enumerate() {
        device_types_sheet
            .write_string_with_format(0, col as u16, &**header, &header_format)
            .map_err(|e| format!("Failed to write device_type header '{}': {}", header, e))?;
    }
    
    // Query device_types
    info!("🔍 Querying device_types...");
    let device_types = sqlx::query(
        "SELECT type_id, code_hex, name, category, introduced_in, created_at, updated_at FROM device_types ORDER BY type_id"
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch device_types: {}", e))?;
    
    let device_types_count = device_types.len() as i64;
    info!("✅ Found {} device_types", device_types_count);
    
    for (row_idx, dt) in device_types.iter().enumerate() {
        let row = (row_idx + 1) as u32;
        
        if row_idx > 0 && row_idx % 100 == 0 {
            info!("  📝 Writing device_type row {}/{}", row_idx, device_types_count);
        }
        
        if let Ok(Some(type_id)) = dt.try_get::<Option<i64>, _>("type_id") {
            device_types_sheet.write_number(row, 0, type_id as f64)
                .map_err(|e| format!("Failed to write type_id at row {}: {}", row, e))?;
        }
        if let Ok(Some(val)) = dt.try_get::<Option<String>, _>("code_hex") {
            device_types_sheet.write_string(row, 1, &val)
                .map_err(|e| format!("Failed to write code_hex at row {}: {}", row, e))?;
        }
        if let Ok(Some(val)) = dt.try_get::<Option<String>, _>("name") {
            device_types_sheet.write_string(row, 2, &val)
                .map_err(|e| format!("Failed to write name at row {}: {}", row, e))?;
        }
        if let Ok(Some(val)) = dt.try_get::<Option<String>, _>("category") {
            device_types_sheet.write_string(row, 3, &val)
                .map_err(|e| format!("Failed to write category at row {}: {}", row, e))?;
        }
        if let Ok(Some(val)) = dt.try_get::<Option<String>, _>("introduced_in") {
            device_types_sheet.write_string(row, 4, &val)
                .map_err(|e| format!("Failed to write introduced_in at row {}: {}", row, e))?;
        }
        if let Ok(Some(val)) = dt.try_get::<Option<String>, _>("created_at") {
            device_types_sheet.write_string(row, 5, &val)
                .map_err(|e| format!("Failed to write created_at at row {}: {}", row, e))?;
        }
        if let Ok(Some(val)) = dt.try_get::<Option<String>, _>("updated_at") {
            device_types_sheet.write_string(row, 6, &val)
                .map_err(|e| format!("Failed to write updated_at at row {}: {}", row, e))?;
        }
    }
    
    // ===== Vendors Sheet =====
    info!("📊 Creating vendors sheet...");
    let vendors_sheet = workbook.add_worksheet();
    if let Err(e) = vendors_sheet.set_name("vendors") {
        let err_msg = format!("Failed to set vendors sheet name: {}", e);
        error!("{}", err_msg);
        return Err(err_msg);
    }
    
    // Vendors headers
    let vendor_headers = vec!["vendor_number", "vendor_name", "company_legal_name", "created_at", "updated_at"];
    
    for (col, header) in vendor_headers.iter().enumerate() {
        vendors_sheet
            .write_string_with_format(0, col as u16, &**header, &header_format)
            .map_err(|e| format!("Failed to write vendor header '{}': {}", header, e))?;
    }
    
    // Query vendors
    info!("� Querying vendors...");
    let vendors = sqlx::query(
        "SELECT vendor_number, vendor_name, company_legal_name, created_at, updated_at FROM vendors ORDER BY vendor_number"
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch vendors: {}", e))?;
    
    let vendors_count = vendors.len() as i64;
    info!("✅ Found {} vendors", vendors_count);
    
    for (row_idx, vendor) in vendors.iter().enumerate() {
        let row = (row_idx + 1) as u32;
        
        if row_idx > 0 && row_idx % 100 == 0 {
            info!("  📝 Writing vendor row {}/{}", row_idx, vendors_count);
        }
        
        if let Ok(vendor_number) = vendor.try_get::<i64, _>("vendor_number") {
            vendors_sheet.write_number(row, 0, vendor_number as f64)
                .map_err(|e| format!("Failed to write vendor_number at row {}: {}", row, e))?;
        }
        if let Ok(Some(val)) = vendor.try_get::<Option<String>, _>("vendor_name") {
            vendors_sheet.write_string(row, 1, &val)
                .map_err(|e| format!("Failed to write vendor_name at row {}: {}", row, e))?;
        }
        if let Ok(Some(val)) = vendor.try_get::<Option<String>, _>("company_legal_name") {
            vendors_sheet.write_string(row, 2, &val)
                .map_err(|e| format!("Failed to write company_legal_name at row {}: {}", row, e))?;
        }
        if let Ok(Some(val)) = vendor.try_get::<Option<String>, _>("created_at") {
            vendors_sheet.write_string(row, 3, &val)
                .map_err(|e| format!("Failed to write created_at at row {}: {}", row, e))?;
        }
        if let Ok(Some(val)) = vendor.try_get::<Option<String>, _>("updated_at") {
            vendors_sheet.write_string(row, 4, &val)
                .map_err(|e| format!("Failed to write updated_at at row {}: {}", row, e))?;
        }
    }
    
    // Save workbook
    info!("�💾 Saving workbook to {:?}...", path);
    if let Err(e) = workbook.save(&path) {
        let err_msg = format!("Failed to save workbook to {:?}: {}", path, e);
        error!("{}", err_msg);
        return Err(err_msg);
    }
    
    info!("✅ Exported full database to Excel: {:?} (products={}, details={}, device_types={}, vendors={})", 
        path, products_count, details_count, device_types_count, vendors_count);
    
    Ok(ExcelExportResult {
        file_path: path.to_string_lossy().to_string(),
        products_count,
        product_details_count: details_count,
        device_types_count,
        vendors_count,
    })
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ExcelImportResult {
    pub products_imported: u32,
    pub products_updated: u32,
    pub details_imported: u32,
    pub details_updated: u32,
    pub device_types_imported: u32,
    pub device_types_updated: u32,
    pub vendors_imported: u32,
    pub vendors_updated: u32,
    pub errors: Vec<String>,
    pub backup_file: Option<String>,
}

/// Import full database from Excel (.xlsx) backup
#[tauri::command]
pub async fn import_full_database_excel(
    state: State<'_, DatabaseConnection>,
    file_path: String,
) -> Result<ExcelImportResult, String> {
    let pool = state.pool();
    
    // Create backup first
    let backup_result = export_full_database_excel(state.clone()).await;
    let backup_file = backup_result.ok().map(|r| r.file_path);
    
    let mut result = ExcelImportResult {
        products_imported: 0,
        products_updated: 0,
        details_imported: 0,
        details_updated: 0,
        device_types_imported: 0,
        device_types_updated: 0,
        vendors_imported: 0,
        vendors_updated: 0,
        errors: Vec::new(),
        backup_file,
    };
    
    // Open Excel file
    let mut workbook: Xlsx<_> = open_workbook(&file_path)
        .map_err(|e| format!("Failed to open Excel file: {}", e))?;
    
    // Start transaction
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    
    // ===== Import Products =====
    if let Ok(range) = workbook.worksheet_range("products") {
        let mut rows = range.rows();
        let _headers = rows.next(); // Skip header row
        
        for (row_num, row) in rows.enumerate() {
            if row.len() < 9 {
                result.errors.push(format!("Products row {} has insufficient columns", row_num + 2));
                continue;
            }
            
            let url = row[0].get_string().unwrap_or("").to_string();
            if url.is_empty() {
                result.errors.push(format!("Products row {} missing URL", row_num + 2));
                continue;
            }
            
            let manufacturer = row[1].get_string().map(|s| s.to_string());
            let model = row[2].get_string().map(|s| s.to_string());
            let certificate_id = row[3].get_string().map(|s| s.to_string());
            let page_id = row[4].get_float().map(|f| f as i64);
            let index_in_page = row[5].get_float().map(|f| f as i64);
            let id = row[6].get_string().map(|s| s.to_string());
            let created_at = row[7].get_string().map(|s| s.to_string()).unwrap_or_else(|| Utc::now().to_rfc3339());
            let updated_at = row[8].get_string().map(|s| s.to_string()).unwrap_or_else(|| Utc::now().to_rfc3339());
            
            // Check if exists
            let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM products WHERE url = ?1 LIMIT 1")
                .bind(&url)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
            
            // Insert or update
            sqlx::query(
                r#"INSERT INTO products (url, manufacturer, model, certificate_id, page_id, index_in_page, id, created_at, updated_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                   ON CONFLICT(url) DO UPDATE SET
                   manufacturer=excluded.manufacturer, model=excluded.model, certificate_id=excluded.certificate_id,
                   page_id=excluded.page_id, index_in_page=excluded.index_in_page, id=excluded.id,
                   updated_at=excluded.updated_at"#
            )
            .bind(&url)
            .bind(&manufacturer)
            .bind(&model)
            .bind(&certificate_id)
            .bind(&page_id)
            .bind(&index_in_page)
            .bind(&id)
            .bind(&created_at)
            .bind(&updated_at)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
            
            if exists.is_some() {
                result.products_updated += 1;
            } else {
                result.products_imported += 1;
            }
        }
    } else {
        result.errors.push("Products sheet not found in Excel file".to_string());
    }
    
    // ===== Import Product Details =====
    if let Ok(range) = workbook.worksheet_range("product_details") {
        let mut rows = range.rows();
        let _headers = rows.next(); // Skip header row
        
        for (row_num, row) in rows.enumerate() {
            if row.len() < 21 {
                result.errors.push(format!("Product_details row {} has insufficient columns (expected 21, got {})", row_num + 2, row.len()));
                continue;
            }
            
            let url = row[0].get_string().unwrap_or("").to_string();
            if url.is_empty() {
                result.errors.push(format!("Product_details row {} missing URL", row_num + 2));
                continue;
            }
            
            // Check if exists
            let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM product_details WHERE url = ?1 LIMIT 1")
                .bind(&url)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
            
            macro_rules! get_str {
                ($idx:expr) => {
                    row.get($idx).and_then(|d| d.get_string()).map(|s| s.to_string())
                };
            }
            
            macro_rules! get_num {
                ($idx:expr) => {
                    row.get($idx).and_then(|d| d.get_float()).map(|f| f as i64)
                };
            }
            
            // Get timestamps or use current time as default
            let created_at = get_str!(19).unwrap_or_else(|| Utc::now().to_rfc3339());
            let updated_at = get_str!(20).unwrap_or_else(|| Utc::now().to_rfc3339());
            
            // Insert or update
            sqlx::query(
                r#"INSERT INTO product_details (
                    url, page_id, index_in_page, id, manufacturer, model, device_type, certificate_id,
                    certification_date, hardware_version, firmware_version, specification_version,
                    vid, pid, family_sku, family_variant_sku, family_id, transport_interface,
                    primary_device_type_ids, created_at, updated_at
                   ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)
                   ON CONFLICT(url) DO UPDATE SET
                   page_id=excluded.page_id, index_in_page=excluded.index_in_page, id=excluded.id,
                   manufacturer=excluded.manufacturer, model=excluded.model, device_type=excluded.device_type,
                   certificate_id=excluded.certificate_id, certification_date=excluded.certification_date,
                   hardware_version=excluded.hardware_version,
                   firmware_version=excluded.firmware_version, specification_version=excluded.specification_version,
                   vid=excluded.vid, pid=excluded.pid, family_sku=excluded.family_sku,
                   family_variant_sku=excluded.family_variant_sku, family_id=excluded.family_id,
                   transport_interface=excluded.transport_interface,
                   primary_device_type_ids=excluded.primary_device_type_ids,
                   updated_at=excluded.updated_at"#
            )
            .bind(&url)
            .bind(&get_num!(1))
            .bind(&get_num!(2))
            .bind(&get_str!(3))
            .bind(&get_str!(4))
            .bind(&get_str!(5))
            .bind(&get_str!(6))
            .bind(&get_str!(7))
            .bind(&get_str!(8))
            .bind(&get_str!(9))
            .bind(&get_str!(10))
            .bind(&get_str!(11))
            .bind(&get_num!(12))
            .bind(&get_num!(13))
            .bind(&get_str!(14))
            .bind(&get_str!(15))
            .bind(&get_str!(16))
            .bind(&get_str!(17))
            .bind(&get_str!(18))
            .bind(&created_at)
            .bind(&updated_at)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
            
            if exists.is_some() {
                result.details_updated += 1;
            } else {
                result.details_imported += 1;
            }
        }
    } else {
        result.errors.push("Product_details sheet not found in Excel file".to_string());
    }
    
    // ===== Import Device Types =====
    if let Ok(range) = workbook.worksheet_range("device_types") {
        let mut rows = range.rows();
        let _headers = rows.next(); // Skip header row
        
        for (row_num, row) in rows.enumerate() {
            if row.len() < 7 {
                result.errors.push(format!("Device_types row {} has insufficient columns", row_num + 2));
                continue;
            }
            
            let type_id = match row[0].get_float() {
                Some(f) => f as i64,
                None => {
                    result.errors.push(format!("Device_types row {} missing type_id", row_num + 2));
                    continue;
                }
            };
            
            macro_rules! get_str {
                ($idx:expr) => {
                    row.get($idx).and_then(|d| d.get_string()).map(|s| s.to_string())
                };
            }
            
            // Check if exists
            let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM device_types WHERE type_id = ?1 LIMIT 1")
                .bind(type_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
            
            // Insert or update
            sqlx::query(
                r#"INSERT INTO device_types (type_id, code_hex, name, category, introduced_in, created_at, updated_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                   ON CONFLICT(type_id) DO UPDATE SET
                   code_hex=excluded.code_hex, name=excluded.name, category=excluded.category,
                   introduced_in=excluded.introduced_in, updated_at=excluded.updated_at"#
            )
            .bind(type_id)
            .bind(&get_str!(1))
            .bind(&get_str!(2))
            .bind(&get_str!(3))
            .bind(&get_str!(4))
            .bind(&get_str!(5))
            .bind(&get_str!(6))
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
            
            if exists.is_some() {
                result.device_types_updated += 1;
            } else {
                result.device_types_imported += 1;
            }
        }
    } else {
        result.errors.push("Device_types sheet not found in Excel file".to_string());
    }
    
    // ===== Import Vendors =====
    if let Ok(range) = workbook.worksheet_range("vendors") {
        let mut rows = range.rows();
        let _headers = rows.next(); // Skip header row
        
        for (row_num, row) in rows.enumerate() {
            if row.len() < 4 {
                result.errors.push(format!("Vendors row {} has insufficient columns", row_num + 2));
                continue;
            }
            
            let vendor_number = match row[0].get_float() {
                Some(f) => f as i64,
                None => {
                    result.errors.push(format!("Vendors row {} missing vendor_number", row_num + 2));
                    continue;
                }
            };
            
            macro_rules! get_str {
                ($idx:expr) => {
                    row.get($idx).and_then(|d| d.get_string()).map(|s| s.to_string())
                };
            }
            
            // Check if exists
            let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM vendors WHERE vendor_number = ?1 LIMIT 1")
                .bind(vendor_number)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
            
            // Insert or update
            sqlx::query(
                r#"INSERT INTO vendors (vendor_number, vendor_name, company_legal_name, created_at, updated_at)
                   VALUES (?1, ?2, ?3, ?4, ?5)
                   ON CONFLICT(vendor_number) DO UPDATE SET
                   vendor_name=excluded.vendor_name, company_legal_name=excluded.company_legal_name, updated_at=excluded.updated_at"#
            )
            .bind(vendor_number)
            .bind(&get_str!(1))
            .bind(&get_str!(2))
            .bind(&get_str!(3))
            .bind(&get_str!(4))
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
            
            if exists.is_some() {
                result.vendors_updated += 1;
            } else {
                result.vendors_imported += 1;
            }
        }
    } else {
        result.errors.push("Vendors sheet not found in Excel file".to_string());
    }
    
    // Commit transaction
    if let Err(e) = tx.commit().await {
        error!("Import transaction failed: {}", e);
        return Err(format!("Import transaction failed: {}", e));
    }
    
    info!("✅ Imported from Excel: products(new={}, updated={}), details(new={}, updated={}), device_types(new={}, updated={}), vendors(new={}, updated={}), errors={}",
        result.products_imported, result.products_updated,
        result.details_imported, result.details_updated,
        result.device_types_imported, result.device_types_updated,
        result.vendors_imported, result.vendors_updated,
        result.errors.len());
    
    Ok(result)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteAllResult {
    pub deleted_products: u64,
    pub deleted_product_details: u64,
    pub backup_file: Option<String>,
}

/// Delete all records from products and product_details tables (with confirmation token)
#[tauri::command]
pub async fn delete_all_records(
    state: State<'_, DatabaseConnection>,
    confirmation_token: String,
) -> Result<DeleteAllResult, String> {
    // Safety check: require exact confirmation
    if confirmation_token != "DELETE_ALL_CONFIRMED" {
        return Err("Invalid confirmation token. Operation cancelled for safety.".to_string());
    }
    
    let pool = state.pool();
    
    // Create backup first
    let backup_result = export_full_database_excel(state.clone()).await;
    let backup_file = backup_result.ok().map(|r| r.file_path);
    
    // Start transaction
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    
    // Delete product_details (child table)
    let details_del = sqlx::query("DELETE FROM product_details")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .rows_affected();
    
    // Delete products (parent table)
    let products_del = sqlx::query("DELETE FROM products")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .rows_affected();
    
    // Commit transaction
    tx.commit().await.map_err(|e| e.to_string())?;
    
    // VACUUM to reclaim space
    let _ = sqlx::query("VACUUM").execute(pool).await;
    
    info!("🗑️ Deleted all records: products={}, product_details={}", 
        products_del, details_del);
    
    Ok(DeleteAllResult {
        deleted_products: products_del,
        deleted_product_details: details_del,
        backup_file,
    })
}

