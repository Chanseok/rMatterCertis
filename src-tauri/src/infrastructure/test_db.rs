use sqlx::{sqlite::SqlitePoolOptions, SqlitePool, Executor};
use anyhow::Result;
use tracing::{info};

/// Build an in-memory SQLite pool with a minimal schema for DB query tests.
/// The schema is supplied as raw SQL (multiple statements allowed).
pub async fn build_memory_pool(schema_sql: &str) -> Result<SqlitePool> {
    // `:memory:` is connection-local; to share across connections use shared cache URI.
    // We need multiple connections for sqlx query validations, so use file: with shared cache.
    let url = "sqlite::memory:?cache=shared"; // persistent for process lifetime
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(url)
        .await?;
    pool.execute("PRAGMA foreign_keys = ON").await?;
    pool.execute(schema_sql).await?;
    info!("memory_db=initialized");
    Ok(pool)
}

/// Apply seed SQL (INSERT statements) after schema.
pub async fn apply_seed_sql(pool: &SqlitePool, seed_sql: &str) -> Result<()> {
    if seed_sql.trim().is_empty() { return Ok(()); }
    pool.execute(seed_sql).await?;
    Ok(())
}

/// Convenience: build schema + seed in one call.
pub async fn build_memory_db(schema_sql: &str, seed_sql: &str) -> Result<SqlitePool> {
    let pool = build_memory_pool(schema_sql).await?;
    apply_seed_sql(&pool, seed_sql).await?;
    Ok(pool)
}
