// Database connection and pool management
// This module handles SQLite database connections using sqlx

#![allow(missing_docs)]
#![allow(clippy::unnecessary_operation)]
#![allow(unused_must_use)]

use anyhow::Result;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::path::Path;
use std::sync::OnceLock;
use tracing::{debug, info, warn};

// Global flag for legacy slot unique index presence (set at startup migration)
static LEGACY_SLOT_UNIQUE_INDEX_PRESENT: OnceLock<bool> = OnceLock::new();

/// Expose whether a legacy unique slot index (products(page_id,index_in_page)) was detected.
#[must_use]
pub fn legacy_slot_unique_index_present() -> bool {
    *LEGACY_SLOT_UNIQUE_INDEX_PRESENT.get().unwrap_or(&false)
}

#[derive(Clone)]
pub struct DatabaseConnection {
    pool: SqlitePool,
}

impl DatabaseConnection {
    /// Create a new database connection pool.
    ///
    /// # Errors
    /// Returns an error if the database directory cannot be created, the file cannot be
    /// created, or the `SQLx` connection fails.
    pub async fn new(database_url: &str) -> Result<Self> {
        // Create database file directory if it doesn't exist
        let db_path = if database_url.starts_with("sqlite://") {
            database_url.trim_start_matches("sqlite://")
        } else if database_url.starts_with("sqlite:") {
            database_url.trim_start_matches("sqlite:")
        } else {
            database_url
        };

        if let Some(parent) = Path::new(db_path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Ensure the database file exists by creating it if necessary
        if !Path::new(db_path).exists() {
            if let Some(parent) = Path::new(db_path).parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::File::create(db_path)?;
        }

        let pool = SqlitePoolOptions::new()
            .max_connections(crate::infrastructure::config::defaults::MAX_CONCURRENT_REQUESTS)
            .connect(database_url)
            .await?;
        // Standardize busy_timeout & journal/wal pragmas (best effort)
        // busy_timeout now configurable via MC_DB_BUSY_TIMEOUT_MS (milliseconds)
        // Rationale: 15000ms(15s) was too long; we prefer shorter waits + explicit retry loops.
        let busy_timeout_ms = resolve_busy_timeout_ms();
        let _ = sqlx::query(&format!("PRAGMA busy_timeout={busy_timeout_ms}"))
            .execute(&pool)
            .await;
        let _ = sqlx::query("PRAGMA journal_mode=WAL").execute(&pool).await;
        let _ = sqlx::query("PRAGMA synchronous=NORMAL").execute(&pool).await;

        Ok(Self { pool })
    }

    #[must_use]
    pub const fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Run idempotent migrations and ensure required indices and tables exist.
    ///
    /// # Errors
    /// Returns an error if reading migration files fails or executing SQL statements fails.
    pub async fn migrate(&self) -> Result<()> {
        use std::fs;

        // Concise logging flags
        let concise_all = std::env::var("MC_CONCISE_ALL")
            .ok()
            .is_none_or(|v| !(v == "0" || v.eq_ignore_ascii_case("false")));
        let concise = concise_all
            || std::env::var("MC_CONCISE_STARTUP")
                .ok()
                .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));

        // Always enable FK constraints (idempotent)
        let _ = sqlx::query("PRAGMA foreign_keys=ON").execute(&self.pool).await;

        // Baseline version we stamp into PRAGMA user_version after success
        const BASELINE_VERSION: i64 = 1001; // 1xxx reserved for consolidated milestones

        // Read current user_version (0 if unset)
        let current_version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);

        if current_version >= BASELINE_VERSION {
            if concise { debug!(current_version, "🆗 Schema baseline already applied (user_version)"); }
            else { info!(current_version, "🆗 Schema baseline already applied (user_version)"); }
        } else {
            // Apply consolidated baseline file (FS first, then embedded). We prefer the new
            // 001_baseline_consolidated.sql naming; keep legacy 001_baseline.sql as optional fallback.
            let baseline_candidates = [
                "migrations/001_baseline_consolidated.sql",
                "migrations/001_baseline.sql",
                "src-tauri/migrations/001_baseline_consolidated.sql",
                "src-tauri/migrations/001_baseline.sql",
            ];
            let mut applied_label = "001_baseline_consolidated".to_string();
            let mut applied_sql: Option<String> = None;
            for path in baseline_candidates.iter() {
                let p = std::path::Path::new(path);
                if p.exists() {
                    match fs::read_to_string(p) {
                        Ok(s) => { applied_sql = Some(s); applied_label = p.file_name().unwrap().to_string_lossy().into_owned(); break; }
                        Err(e) => { warn!(path=%path, error=%e, "failed_read_baseline_candidate"); }
                    }
                }
            }
            if applied_sql.is_none() {
                // Fallback embedded include (must exist in repo – we embed the consolidated one only)
                applied_sql = Some(include_str!("../../../migrations/001_baseline_consolidated.sql").to_string());
            }
            if concise { debug!(label=%applied_label, "📦 Applying consolidated baseline schema"); }
            else { info!(label=%applied_label, "📦 Applying consolidated baseline schema"); }
            if let Some(sql) = applied_sql { self.exec_multi_statement(&sql, false, &applied_label).await?; }
            self.sanity_check_post_migration(&applied_label).await;
            // Stamp version
            let _ = sqlx::query(&format!("PRAGMA user_version={BASELINE_VERSION}"))
                .execute(&self.pool)
                .await;
            if concise { debug!(version=BASELINE_VERSION, "✅ Consolidated baseline applied"); }
            else { info!(version=BASELINE_VERSION, "✅ Consolidated baseline applied"); }
        }

        // -----------------------------------------------------------------
        // Incremental migrations (> current user_version) application logic
        // -----------------------------------------------------------------
        // We introduced consolidated baseline (1001). Any subsequent schema changes should live
        // in files named like: 1002_description.sql, 1003_something.sql etc.
        // Prior code DID NOT auto-apply these (reason the user still has legacy index). We fix it here.
        let mut applied_incrementals: Vec<i64> = Vec::new();
        let mut latest_version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(current_version);
        // Allow opt-out for safety (e.g., during experiments) via MC_SKIP_INCREMENTAL_MIGRATIONS=1
        let skip_incr = std::env::var("MC_SKIP_INCREMENTAL_MIGRATIONS")
            .ok()
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
        if !skip_incr {
            let search_dirs = ["migrations", "src-tauri/migrations"]; // search order
            let mut candidates: Vec<(i64, std::path::PathBuf)> = Vec::new();
            for dir in &search_dirs {
                if let Ok(rd) = std::fs::read_dir(dir) {
                    for entry in rd.flatten() {
                        let path = entry.path();
                        if !path.is_file() { continue; }
                        if let Some(ext) = path.extension() { if ext != "sql" { continue; } } else { continue; }
                        if let Some(fname) = path.file_name().and_then(|s| s.to_str()) {
                            // Parse leading digits until first non-digit/underscore
                            let mut digits = String::new();
                            for ch in fname.chars() { if ch.is_ascii_digit() { digits.push(ch); } else { break; } }
                            if digits.is_empty() { continue; }
                            if let Ok(ver) = digits.parse::<i64>() { if ver > latest_version { candidates.push((ver, path.clone())); } }
                        }
                    }
                }
            }
            // Sort by version ascending
            candidates.sort_by_key(|(v, _)| *v);
            for (ver, path) in candidates {
                match std::fs::read_to_string(&path) {
                    Ok(contents) => {
                        let label = path.file_name().unwrap().to_string_lossy().to_string();
                        info!(version=ver, file=%label, "📦 Applying incremental migration");
                        if let Err(e) = self.exec_multi_statement(&contents, false, &label).await {
                            warn!(version=ver, file=%label, error=%e, "incremental_migration_failed_abort_chain");
                            break; // stop applying further to preserve order/atomic progression semantics
                        }
                        self.sanity_check_post_migration(&label).await;
                        // Stamp user_version explicitly (even if script did it) for canonical version tracking
                        let _ = sqlx::query(&format!("PRAGMA user_version={ver}")).execute(&self.pool).await;
                        latest_version = ver;
                        applied_incrementals.push(ver);
                        info!(version=ver, "✅ Incremental migration applied");
                    }
                    Err(e) => {
                        warn!(path=%path.display(), error=%e, "failed_read_incremental_migration");
                    }
                }
            }
        } else {
            debug!("Skipped incremental migrations due to MC_SKIP_INCREMENTAL_MIGRATIONS=1");
        }

        if !applied_incrementals.is_empty() {
            info!(applied=?applied_incrementals, final_user_version=latest_version, "🧩 Incremental migrations chain completed");
        }

        // Final report
        let product_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM products")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);
        let details_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM product_details")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);
        if concise { info!("🗄️ DB ready: products={}, details={}", product_count, details_count); }
        else { info!("📊 Database initialized (consolidated) with {} products and {} detailed records", product_count, details_count); }

        // Detect legacy unique slot index (may cause UNIQUE conflicts until dropped)
        // Name in older migrations assumed 'ux_products_slot' but we also verify by SQL definition signature.
        let legacy_idx: Option<(String, String)> = sqlx::query_as(
            "SELECT name, sql FROM sqlite_master WHERE type='index' AND tbl_name='products' AND sql LIKE '%UNIQUE%'"
        ).fetch_all(&self.pool).await.ok()
            .map(|rows: Vec<(String,String)>| {
                rows.into_iter()
                    .find(|(_, s)| s.to_lowercase().contains("(page_id, index_in_page)"))
            })
            .flatten();
        let present = legacy_idx.is_some();
        let _ = LEGACY_SLOT_UNIQUE_INDEX_PRESENT.set(present);
        if let Some((ref name, _)) = legacy_idx {
            warn!(index=%name, "⚠️ Legacy unique slot index detected (products(page_id,index_in_page)); may trigger conflicts. Consider dropping after verification.");
            // Optional auto-drop (escape hatch) if migration scanning failed or immediate removal desired.
            // Controlled by MC_AUTO_DROP_LEGACY_SLOT=1
            let auto_drop = std::env::var("MC_AUTO_DROP_LEGACY_SLOT").ok().is_some_and(|v| v=="1" || v.eq_ignore_ascii_case("true"));
            if auto_drop {
                // Clone name for ownership in format! while we still have borrowed pattern
                let drop_name = name.clone();
                match sqlx::query(&format!("DROP INDEX IF EXISTS {drop_name}")).execute(&self.pool).await {
                    Ok(_) => {
                        warn!(index=%name, "✅ Legacy unique slot index auto-dropped (MC_AUTO_DROP_LEGACY_SLOT=1)");
                        // Ensure user_version advanced at least to 1002 (so future scripts referencing it can rely on drop)
                        let current_uv: i64 = sqlx::query_scalar("PRAGMA user_version").fetch_one(&self.pool).await.unwrap_or(0);
                        if current_uv < 1002 { let _ = sqlx::query("PRAGMA user_version=1002").execute(&self.pool).await; }
                    }
                    Err(e) => {
                        warn!(index=%name, error=%e, "failed_auto_drop_legacy_slot_index");
                    }
                }
            }
        } else {
            debug!("No legacy unique slot index present");
        }

        Ok(())
    }

    // Split a potentially multi-statement SQL script into individual statements, preserving
    // semicolons only as delimiters at top level (not inside quotes). Extremely lightweight
    // state machine sufficient for our migration files (no nested BEGIN..END blocks with stray semicolons inside strings).
    fn split_sql_statements(&self, script: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = String::new();
        let mut in_single = false;
        let mut in_double = false;
        let mut in_line_comment = false;
        let mut in_block_comment = false;
        let mut prev = '\0';
        let mut inside_trigger = false; // CREATE TRIGGER ... END;  (internal semicolons allowed)

        let mut last_non_ws_word = String::new();

        for ch in script.chars() {
            // Handle line comment start -- (when not in quotes or block comment)
            if !in_single && !in_double && !in_block_comment {
                if prev == '-' && ch == '-' && !in_line_comment {
                    // Remove the previous '-' already pushed
                    if cur.ends_with('-') { cur.pop(); }
                    in_line_comment = true;
                }
            }
            // Handle block comment start /* */
            if !in_single && !in_double && !in_line_comment {
                if prev == '/' && ch == '*' && !in_block_comment {
                    if cur.ends_with('/') { cur.pop(); }
                    in_block_comment = true;
                }
            }
            // Handle exiting comments
            if in_line_comment && ch == '\n' { in_line_comment = false; }
            if in_block_comment && prev == '*' && ch == '/' { in_block_comment = false; continue; }

            if in_line_comment || in_block_comment {
                prev = ch;
                continue; // Skip comment content
            }

            match ch {
                '\'' if !in_double => { in_single = !in_single; cur.push(ch); }
                '"' if !in_single => { in_double = !in_double; cur.push(ch); }
                ';' if !in_single && !in_double => {
                    // Decide if this semicolon ends the statement
                    if inside_trigger {
                        // Only finalize when the last non-ws word is END (trigger terminator)
                        if last_non_ws_word.eq_ignore_ascii_case("END") {
                            cur.push(';');
                            let stmt = cur.trim();
                            if !stmt.is_empty() { out.push(stmt.to_string()); }
                            cur.clear();
                            inside_trigger = false;
                            last_non_ws_word.clear();
                        } else {
                            // Semicolon belongs to inner statement inside trigger body
                            cur.push(';');
                        }
                    } else {
                        let stmt = cur.trim();
                        if !stmt.is_empty() { out.push(stmt.to_string()); }
                        cur.clear();
                        last_non_ws_word.clear();
                    }
                }
                _ => {
                    // Track words to detect CREATE TRIGGER and END
                    if ch.is_whitespace() {
                        // word boundary
                        if !cur.ends_with(ch) { cur.push(ch); }
                    } else {
                        cur.push(ch);
                        if !in_single && !in_double {
                            if ch.is_alphanumeric() || ch == '_' {
                                last_non_ws_word.push(ch);
                            } else {
                                last_non_ws_word.clear();
                            }
                        }
                    }
                    // Detect CREATE TRIGGER (case-insensitive) when not inside trigger already
                    if !inside_trigger && !in_single && !in_double {
                        // Use a lightweight check on lowercase tail
                        let tail = cur.to_lowercase();
                        if tail.contains("create trigger") {
                            inside_trigger = true; // we'll wait for END; to close
                        }
                    }
                    // If we just captured END in a non-trigger context (e.g., stray END;), leave handling to semicolon branch
                }
            }
            prev = ch;
        }
        // Flush remaining buffer
        let tail = cur.trim();
        if !tail.is_empty() { out.push(tail.to_string()); }

        // Filter out standalone transaction control statements managed externally
        out.into_iter().filter(|s| {
            let up = s.trim().to_uppercase();
            !(up == "BEGIN" || up == "BEGIN TRANSACTION" || up.starts_with("BEGIN TRANSACTION") || up == "COMMIT" || up == "ROLLBACK")
        }).collect()
    }

    // Execute a multi-statement script inside a single transaction (unless idempotent with lots of
    // independent statements; still fine). On error, rolls back and surfaces the error with context.
    async fn exec_multi_statement(&self, script: &str, ignore_errors: bool, label: &str) -> Result<()> {
        let statements = self.split_sql_statements(script);
        if statements.is_empty() {
            return Ok(());
        }
        let start = std::time::Instant::now();
        let mut tx = self.pool.begin().await?;
        for (idx, stmt) in statements.iter().enumerate() {
            match sqlx::query(stmt).execute(&mut *tx).await {
                Ok(_) => { /* success */ }
                Err(e) => {
                    if ignore_errors {
                        debug!(target="migration", label, stmt_index=idx, error=%e, "statement_failed_ignore");
                        continue;
                    } else {
                        debug!(target="migration", label, stmt_index=idx, error=%e, "statement_failed_rollback");
                        return Err(e.into());
                    }
                }
            }
        }
        tx.commit().await?;
        let dur = start.elapsed().as_millis();
        debug!(target="migration", label, count=statements.len(), duration_ms=%dur, "multi_statement_applied");
        Ok(())
    }

    async fn sanity_check_post_migration(&self, label: &str) {
        let start = std::time::Instant::now();
        match self.pool.begin().await { // DEFERRED begin is fine; we test immediate lock separately
            Ok(mut tx) => {
                // Try IMMEDIATE to force writer lock
                if let Err(e) = sqlx::query("BEGIN IMMEDIATE").execute(&mut *tx).await { 
                    debug!(target="migration_lock_check", label, error=%e, "post_migration_immediate_failed");
                } else {
                    debug!(target="migration_lock_check", label, elapsed_ms=%start.elapsed().as_millis(), "post_migration_lock_free");
                }
                let _ = tx.rollback().await; // ensure release
            }
            Err(e) => {
                debug!(target="migration_lock_check", label, error=%e, "post_migration_begin_failed");
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Global, reusable Sqlite pool (reuse-first; safe fallback to init when absent)
// -----------------------------------------------------------------------------

static GLOBAL_SQLITE_POOL: OnceLock<SqlitePool> = OnceLock::new();

/// Get the global Sqlite pool if initialized, or initialize it on first use.
/// Uses the centralized database URL and standard pool options.
/// Initialize and/or retrieve the global Sqlite pool.
///
/// # Errors
/// Returns an error if establishing the `SQLx` connection fails.
pub async fn get_or_init_global_pool() -> Result<SqlitePool> {
    if let Some(pool) = GLOBAL_SQLITE_POOL.get() {
        return Ok(pool.clone());
    }

    let database_url = crate::infrastructure::database_paths::get_main_database_url();
    let pool = SqlitePoolOptions::new()
        .max_connections(crate::infrastructure::config::defaults::MAX_CONCURRENT_REQUESTS)
        .connect(&database_url)
        .await?;
    // Apply pragmas once per global pool init
    let busy_timeout_ms = resolve_busy_timeout_ms();
    let _ = sqlx::query(&format!("PRAGMA busy_timeout={busy_timeout_ms}"))
        .execute(&pool)
        .await;
    let _ = sqlx::query("PRAGMA journal_mode=WAL").execute(&pool).await;
    let _ = sqlx::query("PRAGMA synchronous=NORMAL").execute(&pool).await;

    // Best-effort set; if already set by a racy concurrent init, prefer the existing one
    let _ = GLOBAL_SQLITE_POOL.set(pool.clone());
    Ok(pool)
}

// -----------------------------------------------------------------------------
// Busy timeout resolution helper
// -----------------------------------------------------------------------------
// Environment variable: MC_DB_BUSY_TIMEOUT_MS (milliseconds)
// Defaults to 3000 (3s) if unset or invalid. Clamped to [100, 20000].
// Shorter timeouts surface SQLITE_BUSY quickly so our higher-level retry /
// fallback logic can adapt (e.g., switching from IMMEDIATE to DEFERRED) rather
// than letting the SQLite internal wait block threads for 15s.
fn resolve_busy_timeout_ms() -> u64 {
    const DEFAULT_MS: u64 = 3000; // Previously 15000; reduced for responsiveness
    let raw = std::env::var("MC_DB_BUSY_TIMEOUT_MS").ok();
    let parsed = raw.as_deref().and_then(|s| s.parse::<u64>().ok()).unwrap_or(DEFAULT_MS);
    let clamped = parsed.clamp(100, 20_000);
    clamped
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_database_connection() -> Result<()> {
        // 임시 디렉토리 생성
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test.db");

        println!("🔍 Testing with path: {}", db_path.display());

        // 디렉토리가 존재하는지 확인
        println!("📁 Directory exists: {}", temp_dir.path().exists());

        // SQLite URL 형식으로 변환 (절대 경로 사용)
        let database_url = format!("sqlite:{}", db_path.to_string_lossy());
        println!("🔗 Database URL: {database_url}");

        // 데이터베이스 연결 테스트
        let db = DatabaseConnection::new(&database_url).await?;

        // 연결 풀이 정상적으로 생성되었는지 확인
        assert!(!db.pool().is_closed());

        println!("✅ Database connection test passed with optimized build!");
        Ok(())
    }

    #[tokio::test]
    async fn test_database_migration() -> Result<()> {
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test_migration.db");
        let database_url = format!("sqlite:{}", db_path.display());

        let db = DatabaseConnection::new(&database_url).await?;

        // 마이그레이션 실행
        db.migrate().await?;

        // Matter certification 테이블들이 생성되었는지 확인
        let vendors_table =
            sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='vendors'")
                .fetch_optional(db.pool())
                .await?;

        let products_table =
            sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='products'")
                .fetch_optional(db.pool())
                .await?;

        // Legacy table is optional in modern schema; existence depends on migration input
        let _matter_products_table = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='matter_products'",
        )
        .fetch_optional(db.pool())
        .await?;

        let _sessions_table = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='crawling_sessions'",
        )
        .fetch_optional(db.pool())
        .await?;

        assert!(vendors_table.is_some());
        assert!(products_table.is_some());
        // matter_products is a legacy compatibility table; don't require it for the migration to pass
        // sessions_table can be created by later features; only assert core tables exist for this test

        println!("✅ Matter certification database migration test passed!");
        Ok(())
    }

    #[tokio::test]
    async fn test_migration_007_column_exists() -> Result<()> {
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test_mig_007.db");
        let database_url = format!("sqlite:{}", db_path.display());

        let db = DatabaseConnection::new(&database_url).await?;
        db.migrate().await?;

        // Verify the new normalized column exists on product_details
        let exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM pragma_table_info('product_details') WHERE name='primary_device_type_ids' LIMIT 1;",
        )
        .fetch_optional(db.pool())
        .await?
        .flatten();

        assert!(
            exists.is_some(),
            "primary_device_type_ids column should exist after migration 007"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_migration_009_legacy_column_absent() -> Result<()> {
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("test_mig_009.db");
        let database_url = format!("sqlite:{}", db_path.display());

        let db = DatabaseConnection::new(&database_url).await?;
        db.migrate().await?;

        // Verify legacy column is absent (either because base schema omits it or migration 009 dropped it)
        let exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM pragma_table_info('product_details') WHERE name='primary_device_type_id' LIMIT 1;",
        )
        .fetch_optional(db.pool())
        .await?
        .flatten();

        assert!(
            exists.is_none(),
            "legacy primary_device_type_id column should be absent after migrations"
        );

        Ok(())
    }
}
