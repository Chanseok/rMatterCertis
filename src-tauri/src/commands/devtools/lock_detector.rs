use chrono::{Utc, DateTime};
use serde::Serialize;
use tracing::info;
use crate::infrastructure::database_connection::get_or_init_global_pool;

#[derive(Debug, Serialize, ts_rs::TS)]
#[ts(export, export_to = "../../../../generated-types/")]
pub struct LockDetectorResult {
    pub timestamp_utc: String,
    pub immediate_write_ok: bool,
    pub error: Option<String>,
    pub attempt_ms: u64,
}

#[tauri::command(async)]
pub async fn debug_active_writer_lock() -> Result<LockDetectorResult, String> {
    let pool = get_or_init_global_pool().await.map_err(|e| e.to_string())?;
    let start = std::time::Instant::now();
    let mut immediate_write_ok = false;
    let mut error: Option<String> = None;
    match pool.acquire().await {
        Ok(mut conn) => {
            let _ = sqlx::query("PRAGMA busy_timeout=1").execute(&mut *conn).await;
            match sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await {
                Ok(_) => { let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await; immediate_write_ok = true; },
                Err(e) => { error = Some(e.to_string()); let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await; }
            }
        }
        Err(e) => error = Some(format!("acquire failed: {e}")),
    }
    let attempt_ms = start.elapsed().as_millis() as u64;
    info!(target="lock_detector", immediate_write_ok, error = error.as_deref().unwrap_or(""), attempt_ms, "debug_active_writer_lock executed");
    Ok(LockDetectorResult { timestamp_utc: DateTime::<Utc>::from(Utc::now()).to_rfc3339(), immediate_write_ok, error, attempt_ms })
}
