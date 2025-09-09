use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::{info, warn};

use crate::application::AppState;

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
    let (path_opt, json) = if let Some((p, txt)) = locate_device_types_file().await { (Some(p), txt) } else { (None, String::new()) };
    // Count array length if possible
    let count_in_file = match serde_json::from_str::<serde_json::Value>(&json) { Ok(serde_json::Value::Array(a)) => a.len(), _ => 0 };
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
    // Locate existing file (may not exist)
    let existing = locate_device_types_file().await;
    let mut backup_path = None;
    let target_path: Option<String>;
    if let Some((path, current_contents)) = existing {
        // Write backup
        let ts = chrono::Utc::now().format("%Y%m%d%H%M%S");
        let backup = format!("{}.bak_{}", path, ts);
        if tokio::fs::write(&backup, current_contents).await.is_ok() {
            backup_path = Some(backup.clone());
        }
        target_path = Some(path.clone());
        // Atomic write via temp file
        let tmp = format!("{}.tmp", path);
        tokio::fs::write(&tmp, &new_json).await.map_err(|e| format!("Write temp failed: {}", e))?;
        tokio::fs::rename(&tmp, &path).await.map_err(|e| format!("Atomic rename failed: {}", e))?;
    } else {
        // Default to first candidate path directory
        let path = "data/matter_device_types.json";
        if let Some(parent) = std::path::Path::new(path).parent() { let _ = tokio::fs::create_dir_all(parent).await; }
        tokio::fs::write(path, &new_json).await.map_err(|e| format!("Write failed: {}", e))?;
    target_path = Some(path.to_string());
    }

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
    written_path: target_path.clone(),
        parsed_count: arr.len(),
        reseed,
        inserted,
        updated,
        skipped,
    })
}
