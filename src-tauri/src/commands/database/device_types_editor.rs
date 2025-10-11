use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::{info, warn};
use sqlx::Row; // Row trait for try_get method
use std::path::PathBuf;

use crate::application::AppState;

// 번들된 Device Types JSON을 임베딩
// CARGO_MANIFEST_DIR = src-tauri, ../data = 프로젝트 루트의 data/
const BUNDLED_DEVICE_TYPES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../data/matter_device_types.json"));

/// Export 파일을 저장할 디렉토리 (DB와 같은 위치의 exports/)
fn exports_dir() -> PathBuf {
    let db_url = crate::infrastructure::get_main_database_url();
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
    p.join("exports")
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceTypesJsonPayload {
    pub json: String,
    pub path: Option<String>,
    pub count_in_file: usize,
    pub count_in_db: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveDeviceTypesResult {
    pub backup_path: Option<String>,
    pub written_path: Option<String>,
    pub parsed_count: usize,
    pub reseed: bool,
    pub inserted: u32,
    pub updated: u32,
    pub skipped: u32,
}

fn candidate_paths() -> Vec<&'static str> {
    vec![
        "data/matter_device_types.json",
        "./data/matter_device_types.json",
        "../data/matter_device_types.json",
    ]
}

async fn locate_device_types_file() -> Option<(String, String)> {
    for p in candidate_paths() {
        if let Ok(text) = tokio::fs::read_to_string(p).await {
            return Some((p.to_string(), text));
        }
    }
    // Try alongside executable
    if let Ok(exec) = std::env::current_exe() {
        if let Some(parent) = exec.parent() {
            let alt = parent.join("data/matter_device_types.json");
            if let Ok(text) = tokio::fs::read_to_string(&alt).await {
                return Some((alt.to_string_lossy().to_string(), text));
            }
        }
    }
    None
}

#[tauri::command]
pub async fn get_device_types_json(state: State<'_, AppState>) -> Result<DeviceTypesJsonPayload, String> {
    // 1. 로컬 파일 시도
    let (path_opt, json) = if let Some((p, txt)) = locate_device_types_file().await {
        (Some(p), txt)
    } else {
        // 2. 번들된 파일 사용
        (Some("[bundled]".to_string()), BUNDLED_DEVICE_TYPES.to_string())
    };
    
    // Count array length if possible
    let count_in_file = match serde_json::from_str::<serde_json::Value>(&json) { 
        Ok(serde_json::Value::Array(a)) => a.len(), 
        _ => 0 
    };
    let pool = state.get_database_pool().await?;
    let count_in_db = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM device_types")
        .fetch_one(&pool).await.unwrap_or(0);
    Ok(DeviceTypesJsonPayload { json, path: path_opt, count_in_file, count_in_db })
}

#[derive(Debug, Deserialize)]
pub struct SaveDeviceTypesOptions {
    pub reseed: Option<bool>,
}

#[tauri::command]
pub async fn save_device_types_json(
    state: State<'_, AppState>,
    new_json: String,
    options: Option<SaveDeviceTypesOptions>,
) -> Result<SaveDeviceTypesResult, String> {
    let reseed = options.as_ref().and_then(|o| o.reseed).unwrap_or(false);
    // Validate JSON structure
    let parsed: serde_json::Value = serde_json::from_str(&new_json)
        .map_err(|e| format!("Invalid JSON: {}", e))?;
    let arr = match parsed { serde_json::Value::Array(a) => a, _ => return Err("Root must be an array".into()) };
    // Basic schema validation
    for (idx, item) in arr.iter().enumerate() {
        if !item.get("id").and_then(|v| v.as_i64()).is_some() { return Err(format!("Item {} missing integer 'id'", idx)); }
        if !item.get("name").and_then(|v| v.as_str()).is_some() { return Err(format!("Item {} missing 'name'", idx)); }
    }
    
    // exports 디렉토리에 저장 (앱 재시작 방지)
    let export_dir = exports_dir();
    tokio::fs::create_dir_all(&export_dir)
        .await
        .map_err(|e| format!("Failed to create exports directory: {}", e))?;
    
    let ts = chrono::Utc::now().format("%Y%m%d%H%M%S");
    let filename = format!("matter_device_types_{}.json", ts);
    let target_path = export_dir.join(&filename);
    
    // 백업 파일 생성 (이전 버전이 있다면)
    let mut backup_path = None;
    if target_path.exists() {
        let backup_file = export_dir.join(format!("matter_device_types_{}.bak", ts));
        if let Ok(current) = tokio::fs::read(&target_path).await {
            if tokio::fs::write(&backup_file, current).await.is_ok() {
                backup_path = Some(backup_file.to_string_lossy().to_string());
            }
        }
    }
    
    // JSON 파일 저장
    tokio::fs::write(&target_path, &new_json)
        .await
        .map_err(|e| format!("Failed to write JSON file: {}", e))?;

    let mut inserted = 0u32;
    let mut updated = 0u32;
    let mut skipped = 0u32;
    if reseed {
        let pool = state.get_database_pool().await?;
        let has_type_id: bool = sqlx::query_scalar::<_, i64>("SELECT 1 FROM pragma_table_info('device_types') WHERE name='type_id' LIMIT 1;")
            .fetch_optional(&pool).await.ok().flatten().is_some();
        for item in &arr {
            let id = item.get("id").and_then(|v| v.as_i64()).unwrap();
            let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let code_hex = item.get("hex").and_then(|v| v.as_str()).unwrap_or("");
            let category = item.get("category").and_then(|v| v.as_str()).unwrap_or("");
            let introduced_in = item.get("introduced_in").and_then(|v| v.as_str()).unwrap_or("");
            let exists: Option<i64> = if has_type_id {
                sqlx::query_scalar("SELECT 1 FROM device_types WHERE type_id=?1 LIMIT 1").bind(id)
                    .fetch_optional(&pool).await.unwrap_or(None)
            } else {
                sqlx::query_scalar("SELECT 1 FROM device_types WHERE id=?1 LIMIT 1").bind(id)
                    .fetch_optional(&pool).await.unwrap_or(None)
            };
            let query = if has_type_id {
                r#"INSERT INTO device_types (type_id, code_hex, name, category, introduced_in) VALUES (?1, ?2, ?3, ?4, ?5)
                    ON CONFLICT(type_id) DO UPDATE SET code_hex=excluded.code_hex, name=excluded.name, category=excluded.category, introduced_in=excluded.introduced_in"#
            } else {
                r#"INSERT INTO device_types (id, code_hex, name, category, introduced_in) VALUES (?1, ?2, ?3, ?4, ?5)
                    ON CONFLICT(id) DO UPDATE SET code_hex=excluded.code_hex, name=excluded.name, category=excluded.category, introduced_in=excluded.introduced_in"#
            };
            if let Err(e) = sqlx::query(query)
                .bind(id)
                .bind(code_hex)
                .bind(name)
                .bind(category)
                .bind(introduced_in)
                .execute(&pool).await {
                warn!("Device type upsert failed id={} err={}", id, e);
                skipped += 1;
            } else if exists.is_some() { updated += 1; } else { inserted += 1; }
        }
        info!("Reseed complete: inserted={} updated={} skipped={} total_parsed={}", inserted, updated, skipped, arr.len());
    }

    Ok(SaveDeviceTypesResult {
        backup_path,
        written_path: Some(target_path.to_string_lossy().to_string()),
        parsed_count: arr.len(),
        reseed,
        inserted,
        updated,
        skipped,
    })
}

#[derive(Debug, Serialize)]
pub struct ExportDeviceTypesResult {
    pub path: String,
    pub count: usize,
}

/// DB에서 device types를 조회하여 JSON 파일로 export
#[tauri::command]
pub async fn export_device_types_from_db(state: State<'_, AppState>) -> Result<ExportDeviceTypesResult, String> {
    let pool = state.get_database_pool().await?;
    
    // type_id 기준으로 조회 (Matter 스펙 ID)
    let rows = sqlx::query(
        "SELECT type_id, code_hex, name, category, introduced_in FROM device_types WHERE type_id IS NOT NULL ORDER BY CAST(type_id AS INTEGER)"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| format!("DB query failed: {}", e))?;

    // JSON 배열로 변환 (matter_device_types.json 포맷)
    let mut json_array: Vec<serde_json::Value> = Vec::new();
    
    for row in rows {
        let type_id: String = row.try_get("type_id").map_err(|e| format!("Failed to get type_id: {}", e))?;
        let code_hex: String = row.try_get("code_hex").unwrap_or_default();
        let name: String = row.try_get("name").map_err(|e| format!("Failed to get name: {}", e))?;
        let category: String = row.try_get("category").unwrap_or_default();
        let introduced_in: Option<String> = row.try_get("introduced_in").ok();
        
        // type_id를 정수로 파싱 (JSON에서 숫자로 표현)
        let id_num = type_id.parse::<i64>().unwrap_or(0);
        
        json_array.push(serde_json::json!({
            "id": id_num,
            "hex": code_hex,
            "name": name,
            "category": category,
            "introduced_in": introduced_in
        }));
    }

    let json_str = serde_json::to_string_pretty(&json_array)
        .map_err(|e| format!("JSON serialization failed: {}", e))?;

    // exports 디렉토리에 저장 (앱 재시작 방지)
    let export_dir = exports_dir();
    tokio::fs::create_dir_all(&export_dir)
        .await
        .map_err(|e| format!("Failed to create exports directory: {}", e))?;
    
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("matter_device_types_export_{}.json", timestamp);
    let export_path = export_dir.join(&filename);

    tokio::fs::write(&export_path, &json_str)
        .await
        .map_err(|e| format!("File write failed: {}", e))?;

    let export_path_str = export_path.to_string_lossy().to_string();
    info!("Exported {} device types to {}", json_array.len(), export_path_str);

    Ok(ExportDeviceTypesResult {
        path: export_path_str,
        count: json_array.len(),
    })
}

#[derive(Debug, Deserialize)]
pub struct ImportDeviceTypesOptions {
    pub file_path: String,
    pub replace_all: bool,
}

/// JSON 파일에서 device types를 읽어 DB에 import
#[tauri::command]
pub async fn import_device_types_to_db(
    state: State<'_, AppState>,
    options: ImportDeviceTypesOptions,
) -> Result<SaveDeviceTypesResult, String> {
    // JSON 파일 읽기
    let json_content = tokio::fs::read_to_string(&options.file_path)
        .await
        .map_err(|e| format!("Failed to read file: {}", e))?;

    // JSON 파싱 및 유효성 검사
    let parsed: serde_json::Value = serde_json::from_str(&json_content)
        .map_err(|e| format!("Invalid JSON: {}", e))?;
    
    let arr = match parsed {
        serde_json::Value::Array(a) => a,
        _ => return Err("Root must be an array".into()),
    };

    // 스키마 검증
    for (idx, item) in arr.iter().enumerate() {
        if !item.get("id").and_then(|v| v.as_i64()).is_some() {
            return Err(format!("Item {} missing integer 'id'", idx));
        }
        if !item.get("name").and_then(|v| v.as_str()).is_some() {
            return Err(format!("Item {} missing 'name'", idx));
        }
    }

    let pool = state.get_database_pool().await?;
    
    // replace_all 옵션: 기존 데이터 전체 삭제
    if options.replace_all {
        sqlx::query("DELETE FROM device_types")
            .execute(&pool)
            .await
            .map_err(|e| format!("Failed to clear existing data: {}", e))?;
        info!("Cleared all existing device types for import");
    }

    let mut inserted = 0u32;
    let mut updated = 0u32;
    let mut skipped = 0u32;

    // 각 레코드 upsert (code_hex 기반 중복 체크)
    for item in &arr {
        let id = item.get("id").and_then(|v| v.as_i64()).unwrap();
        let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let code_hex = item.get("hex").and_then(|v| v.as_str()).unwrap_or("");
        let category = item.get("category").and_then(|v| v.as_str()).unwrap_or("");
        let introduced_in = item.get("introduced_in").and_then(|v| v.as_str()).unwrap_or("");

        // code_hex로 중복 체크 (더 정확함)
        let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM device_types WHERE code_hex=?1 LIMIT 1")
            .bind(code_hex)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None);

        // 중복이면 UPDATE, 아니면 INSERT (type_id도 함께 저장)
        let query = if exists.is_some() {
            "UPDATE device_types SET name=?1, category=?2, introduced_in=?3, type_id=CAST(?4 AS TEXT) WHERE code_hex=?5"
        } else {
            "INSERT INTO device_types (name, category, introduced_in, type_id, code_hex) VALUES (?1, ?2, ?3, CAST(?4 AS TEXT), ?5)"
        };

        if let Err(e) = sqlx::query(query)
            .bind(name)
            .bind(category)
            .bind(introduced_in)
            .bind(id) // type_id로 저장
            .bind(code_hex)
            .execute(&pool)
            .await
        {
            warn!("Device type import failed code_hex={} err={}", code_hex, e);
            skipped += 1;
        } else if exists.is_some() {
            updated += 1;
        } else {
            inserted += 1;
        }
    }

    info!(
        "Import complete: inserted={} updated={} skipped={} total_parsed={}",
        inserted, updated, skipped, arr.len()
    );

    Ok(SaveDeviceTypesResult {
        backup_path: None,
        written_path: Some(options.file_path.clone()),
        parsed_count: arr.len(),
        reseed: true,
        inserted,
        updated,
        skipped,
    })
}

#[derive(Debug, Serialize)]
pub struct DeviceTypeRecord {
    pub id: i64,
    pub hex: String,
    pub name: String,
    pub category: String,
    pub introduced_in: Option<String>,
}

#[tauri::command]
pub async fn get_all_device_types_from_db(state: State<'_, AppState>) -> Result<Vec<DeviceTypeRecord>, String> {
    let pool = state.get_database_pool().await?;
    
    #[derive(sqlx::FromRow)]
    struct DbRow {
        type_id: Option<String>,
        code_hex: Option<String>,
        name: String,
        category: Option<String>,
        introduced_in: Option<String>,
    }

    // type_id를 id로 반환 (Matter 스펙 ID)
    let rows: Vec<DbRow> = sqlx::query_as::<_, DbRow>(
        "SELECT type_id, code_hex, name, category, introduced_in FROM device_types WHERE type_id IS NOT NULL ORDER BY CAST(type_id AS INTEGER)"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| format!("DB query failed: {}", e))?;

    Ok(rows
        .into_iter()
        .map(|r| DeviceTypeRecord {
            id: r.type_id.as_ref().and_then(|t| t.parse::<i64>().ok()).unwrap_or(0),
            hex: r.code_hex.unwrap_or_else(|| "0x0000".to_string()),
            name: r.name,
            category: r.category.unwrap_or_else(|| "Other".to_string()),
            introduced_in: r.introduced_in,
        })
        .collect())
}

#[derive(Serialize)]
pub struct DeleteDeviceTypesResult {
    pub deleted_count: usize,
}

/// DB에서 직접 Device Types 레코드 삭제 (type_id 기준)
#[tauri::command]
pub async fn delete_device_types_from_db(
    state: State<'_, AppState>,
    ids: Vec<i64>
) -> Result<DeleteDeviceTypesResult, String> {
    let pool = state.get_database_pool().await?;
    
    let mut deleted_count = 0;
    for id in ids {
        let result = sqlx::query("DELETE FROM device_types WHERE type_id = CAST(? AS TEXT)")
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| format!("Failed to delete type_id {}: {}", id, e))?;
        
        deleted_count += result.rows_affected() as usize;
    }
    
    Ok(DeleteDeviceTypesResult { deleted_count })
}

#[derive(Deserialize)]
pub struct AddDeviceTypeInput {
    pub id: i64,
    pub hex: String,
    pub name: String,
    pub category: String,
    pub introduced_in: Option<String>,
}

#[derive(Serialize)]
pub struct AddDeviceTypeResult {
    pub id: i64,
}

/// DB에 직접 Device Type 레코드 추가
#[tauri::command]
pub async fn add_device_type_to_db(
    state: State<'_, AppState>,
    device_type: AddDeviceTypeInput
) -> Result<AddDeviceTypeResult, String> {
    let pool = state.get_database_pool().await?;
    
    // code_hex 중복 체크 (더 정확한 중복 검사)
    let existing: Option<i64> = sqlx::query_scalar("SELECT 1 FROM device_types WHERE code_hex = ?")
        .bind(&device_type.hex)
        .fetch_optional(&pool)
        .await
        .map_err(|e| format!("Failed to check duplicate: {}", e))?;
    
    if existing.is_some() {
        return Err(format!("Device type with code_hex {} already exists", device_type.hex));
    }
    
    // 레코드 추가 (type_id에 id 값 저장)
    sqlx::query(
        "INSERT INTO device_types (code_hex, name, category, introduced_in, type_id) VALUES (?, ?, ?, ?, CAST(? AS TEXT))"
    )
    .bind(&device_type.hex)
    .bind(&device_type.name)
    .bind(&device_type.category)
    .bind(&device_type.introduced_in)
    .bind(device_type.id) // type_id로 Matter 스펙 ID 저장
    .execute(&pool)
    .await
    .map_err(|e| format!("Failed to insert: {}", e))?;
    
    Ok(AddDeviceTypeResult { id: device_type.id })
}

#[derive(Deserialize)]
pub struct UpdateDeviceTypeInput {
    pub id: i64,
    pub hex: String,
    pub name: String,
    pub category: String,
    pub introduced_in: Option<String>,
}

#[derive(Serialize)]
pub struct UpdateDeviceTypeResult {
    pub updated: bool,
}

/// DB에서 직접 Device Type 레코드 업데이트 (type_id 기준)
#[tauri::command]
pub async fn update_device_type_in_db(
    state: State<'_, AppState>,
    device_type: UpdateDeviceTypeInput
) -> Result<UpdateDeviceTypeResult, String> {
    let pool = state.get_database_pool().await?;
    
    let result = sqlx::query(
        "UPDATE device_types SET code_hex = ?, name = ?, category = ?, introduced_in = ? WHERE type_id = CAST(? AS TEXT)"
    )
    .bind(&device_type.hex)
    .bind(&device_type.name)
    .bind(&device_type.category)
    .bind(&device_type.introduced_in)
    .bind(device_type.id) // type_id로 검색
    .execute(&pool)
    .await
    .map_err(|e| format!("Failed to update: {}", e))?;
    
    Ok(UpdateDeviceTypeResult { 
        updated: result.rows_affected() > 0 
    })
}
