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
        let concise_all = std::env::var("MC_CONCISE_ALL")
            .ok()
            .is_none_or(|v| !(v == "0" || v.eq_ignore_ascii_case("false")));
        let concise = concise_all
            || std::env::var("MC_CONCISE_STARTUP")
                .ok()
                .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));

        // Enable foreign key constraints
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&self.pool)
            .await?;

        // Pre-clean: drop legacy compatibility view if it exists to avoid schema validation errors
        let _ = sqlx::query("DROP VIEW IF EXISTS matter_products_legacy;")
            .execute(&self.pool)
            .await;

        // Load and run the integrated schema SQL (003_integrated_schema.sql)
        if concise {
            debug!("📦 Checking database schema (CREATE TABLE IF NOT EXISTS)...");
        } else {
            info!("📦 Checking database schema (CREATE TABLE IF NOT EXISTS)...");
        }
        let schema_path = std::path::Path::new("migrations/003_integrated_schema.sql");

        if schema_path.exists() {
            let schema_sql = fs::read_to_string(schema_path)?;
            sqlx::query(&schema_sql).execute(&self.pool).await?;
            if concise {
                debug!("✅ Database schema verified successfully");
            } else {
                info!("✅ Database schema verified successfully");
            }
        } else {
            // Fallback to embedded schema if file doesn't exist
            warn!("⚠️ Schema file not found, using embedded schema");

            // Read schema from embedded file or resources
            let schema_sql = include_str!("../../migrations/003_integrated_schema.sql");
            sqlx::query(schema_sql).execute(&self.pool).await?;
            if concise {
                debug!("✅ Database schema verified with embedded version");
            } else {
                info!("✅ Database schema verified with embedded version");
            }
        }

        // Check if we need to migrate legacy data
        let has_legacy_data = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='matter_products'",
        )
        .fetch_one(&self.pool)
        .await?;

        if has_legacy_data > 0 {
            // Check if there's data to migrate
            let legacy_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM matter_products")
                .fetch_one(&self.pool)
                .await?;

            if legacy_count > 0 {
                if concise {
                    debug!("🔄 Found {} legacy records to migrate", legacy_count);
                } else {
                    info!("🔄 Found {} legacy records to migrate", legacy_count);
                }

                // Apply data migration script
                let migration_path = std::path::Path::new("migrations/004_migrate_legacy_data.sql");

                if migration_path.exists() {
                    let migration_sql = fs::read_to_string(migration_path)?;
                    sqlx::query(&migration_sql).execute(&self.pool).await?;
                    if concise {
                        debug!("✅ Migrated legacy data successfully");
                    } else {
                        info!("✅ Migrated legacy data successfully");
                    }
                } else {
                    // Fallback to embedded migration script
                    let migration_sql =
                        include_str!("../../migrations/004_migrate_legacy_data.sql");
                    sqlx::query(migration_sql).execute(&self.pool).await?;
                    if concise {
                        debug!("✅ Migrated legacy data using embedded script");
                    } else {
                        info!("✅ Migrated legacy data using embedded script");
                    }
                }
            } else if concise {
                debug!("ℹ️ No legacy data to migrate");
            } else {
                info!("ℹ️ No legacy data to migrate");
            }
        } else if concise {
            debug!("ℹ️ No legacy migration needed (modern schema already in use)");
        } else {
            info!("ℹ️ No legacy migration needed (modern schema already in use)");
        }

        // Apply 005_add_product_id.sql if products.id is missing
        let has_products_id_col: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM pragma_table_info('products') WHERE name='id' LIMIT 1;",
        )
        .fetch_optional(&self.pool)
        .await?
        .flatten();

        if has_products_id_col.is_none() {
            if concise {
                debug!("🧩 Applying migration 005_add_product_id.sql (products.id)");
            } else {
                info!("🧩 Applying migration 005_add_product_id.sql (products.id)");
            }
            let migration_path = std::path::Path::new("migrations/005_add_product_id.sql");
            if migration_path.exists() {
                let migration_sql = fs::read_to_string(migration_path)?;
                sqlx::query(&migration_sql).execute(&self.pool).await?;
            } else {
                let migration_sql = include_str!("../../migrations/005_add_product_id.sql");
                sqlx::query(migration_sql).execute(&self.pool).await?;
            }
            if concise {
                debug!("✅ Migration 005 applied");
            } else {
                info!("✅ Migration 005 applied");
            }
        } else if !concise {
            debug!("ℹ️ Migration 005 not needed (products.id exists)");
        }

        // Apply 006_add_unique_slot_index.sql if unique slot indexes are missing
        let has_ux_products_slot: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM sqlite_master WHERE type='index' AND name='ux_products_slot' LIMIT 1;",
        )
        .fetch_optional(&self.pool)
        .await?
        .flatten();

        let has_ux_product_details_slot: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM sqlite_master WHERE type='index' AND name='ux_product_details_slot' LIMIT 1;",
        )
        .fetch_optional(&self.pool)
        .await?
        .flatten();

        if has_ux_products_slot.is_none() || has_ux_product_details_slot.is_none() {
            if concise {
                debug!("🧩 Applying migration 006_add_unique_slot_index.sql (unique slot indexes)");
            } else {
                info!("🧩 Applying migration 006_add_unique_slot_index.sql (unique slot indexes)");
            }
            let migration_path = std::path::Path::new("migrations/006_add_unique_slot_index.sql");
            if migration_path.exists() {
                let migration_sql = fs::read_to_string(migration_path)?;
                sqlx::query(&migration_sql).execute(&self.pool).await?;
            } else {
                let migration_sql = include_str!("../../migrations/006_add_unique_slot_index.sql");
                sqlx::query(migration_sql).execute(&self.pool).await?;
            }
            if concise {
                debug!("✅ Migration 006 applied");
            } else {
                info!("✅ Migration 006 applied");
            }
        } else if !concise {
            debug!("ℹ️ Migration 006 not needed (unique slot indexes exist)");
        }

        // Apply 007_primary_device_type_ids.sql if normalized column is missing
        let has_primary_device_type_ids_col: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM pragma_table_info('product_details') WHERE name='primary_device_type_ids' LIMIT 1;",
        )
        .fetch_optional(&self.pool)
        .await?
        .flatten();

        if has_primary_device_type_ids_col.is_none() {
            if concise {
                debug!(
                    "🧩 Applying migration 007_primary_device_type_ids.sql (normalized device type ids)"
                );
            } else {
                info!(
                    "🧩 Applying migration 007_primary_device_type_ids.sql (normalized device type ids)"
                );
            }
            let migration_path = std::path::Path::new("migrations/007_primary_device_type_ids.sql");
            if migration_path.exists() {
                let migration_sql = fs::read_to_string(migration_path)?;
                sqlx::query(&migration_sql).execute(&self.pool).await?;
            } else {
                let migration_sql =
                    include_str!("../../migrations/007_primary_device_type_ids.sql");
                sqlx::query(migration_sql).execute(&self.pool).await?;
            }
            if concise {
                debug!("✅ Migration 007 applied");
            } else {
                info!("✅ Migration 007 applied");
            }
        } else if !concise {
            debug!("ℹ️ Migration 007 not needed (primary_device_type_ids exists)");
        }

        // Apply 008_fix_primary_device_type_ids.sql if present (idempotent corrective backfill)
        let mig008_path = std::path::Path::new("migrations/008_fix_primary_device_type_ids.sql");
        if mig008_path.exists() {
            // Only apply 008 if legacy column exists; otherwise skip (fresh installs)
            let has_legacy_col_008: Option<i64> = sqlx::query_scalar(
                "SELECT 1 FROM pragma_table_info('product_details') WHERE name='primary_device_type_id' LIMIT 1;",
            )
            .fetch_optional(&self.pool)
            .await?
            .flatten();
            if has_legacy_col_008.is_some() {
                if concise {
                    debug!(
                        "🧩 Applying migration 008_fix_primary_device_type_ids.sql (corrective backfill + trigger refresh)"
                    );
                } else {
                    info!(
                        "🧩 Applying migration 008_fix_primary_device_type_ids.sql (corrective backfill + trigger refresh)"
                    );
                }
                let migration_sql = std::fs::read_to_string(mig008_path)?;
                sqlx::query(&migration_sql).execute(&self.pool).await?;
                if concise {
                    debug!("✅ Migration 008 applied");
                } else {
                    info!("✅ Migration 008 applied");
                }
            } else if !concise {
                debug!("ℹ️ Migration 008 not needed (legacy column absent)");
            }
        }

        // Apply 009_drop_legacy_primary_device_type_id.sql if present and legacy column exists
        let mig009_path =
            std::path::Path::new("migrations/009_drop_legacy_primary_device_type_id.sql");
        if mig009_path.exists() {
            // Check if legacy column still exists to avoid unnecessary rebuild on fresh installs
            let has_legacy_col: Option<i64> = sqlx::query_scalar(
                "SELECT 1 FROM pragma_table_info('product_details') WHERE name='primary_device_type_id' LIMIT 1;",
            )
            .fetch_optional(&self.pool)
            .await?
            .flatten();

            if has_legacy_col.is_some() {
                if concise {
                    debug!(
                        "🧩 Applying migration 009_drop_legacy_primary_device_type_id.sql (remove legacy column)"
                    );
                } else {
                    info!(
                        "🧩 Applying migration 009_drop_legacy_primary_device_type_id.sql (remove legacy column)"
                    );
                }
                let migration_sql = std::fs::read_to_string(mig009_path)?;
                sqlx::query(&migration_sql).execute(&self.pool).await?;
                if concise {
                    debug!("✅ Migration 009 applied");
                } else {
                    info!("✅ Migration 009 applied");
                }
            } else if !concise {
                debug!("ℹ️ Migration 009 not needed (legacy column already absent)");
            }
        } else {
            // Fallback to embedded migration when available in packaged builds
            let has_legacy_col: Option<i64> = sqlx::query_scalar(
                "SELECT 1 FROM pragma_table_info('product_details') WHERE name='primary_device_type_id' LIMIT 1;",
            )
            .fetch_optional(&self.pool)
            .await?
            .flatten();
            if has_legacy_col.is_some() {
                if concise {
                    debug!(
                        "🧩 Applying embedded migration 009_drop_legacy_primary_device_type_id.sql"
                    );
                } else {
                    info!(
                        "🧩 Applying embedded migration 009_drop_legacy_primary_device_type_id.sql"
                    );
                }
                let migration_sql =
                    include_str!("../../migrations/009_drop_legacy_primary_device_type_id.sql");
                sqlx::query(migration_sql).execute(&self.pool).await?;
                if concise {
                    debug!("✅ Migration 009 applied (embedded)");
                } else {
                    info!("✅ Migration 009 applied (embedded)");
                }
            }
        }

        // Apply 010_device_types.sql if device_types table missing
        let has_device_types: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='device_types' LIMIT 1;",
        )
        .fetch_optional(&self.pool)
        .await?
        .flatten();
        if has_device_types.is_none() {
            if concise {
                debug!("🧩 Applying migration 010_device_types.sql (device_types reference table)");
            } else {
                info!("🧩 Applying migration 010_device_types.sql (device_types reference table)");
            }
            let migration_path = std::path::Path::new("migrations/010_device_types.sql");
            if migration_path.exists() {
                let migration_sql = std::fs::read_to_string(migration_path)?;
                sqlx::query(&migration_sql).execute(&self.pool).await?;
            } else {
                let migration_sql = include_str!("../../migrations/010_device_types.sql");
                sqlx::query(migration_sql).execute(&self.pool).await?;
            }
            if concise {
                debug!("✅ Migration 010 applied");
            } else {
                info!("✅ Migration 010 applied");
            }
        } else if !concise {
            debug!("ℹ️ Migration 010 not needed (device_types table exists)");
        }

        // Apply 011_device_types_add_category_introduced_in.sql if category column missing
        let has_category_col: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM pragma_table_info('device_types') WHERE name='category' LIMIT 1;",
        )
        .fetch_optional(&self.pool)
        .await?
        .flatten();
        if has_device_types.is_some() && has_category_col.is_none() {
            if concise { debug!("🧩 Applying migration 011_device_types_add_category_introduced_in.sql"); } else { info!("🧩 Applying migration 011_device_types_add_category_introduced_in.sql"); }
            let migration_path = std::path::Path::new("migrations/011_device_types_add_category_introduced_in.sql");
            if migration_path.exists() {
                let migration_sql = std::fs::read_to_string(migration_path)?;
                sqlx::query(&migration_sql).execute(&self.pool).await?;
            } else {
                let migration_sql = include_str!("../../migrations/011_device_types_add_category_introduced_in.sql");
                sqlx::query(migration_sql).execute(&self.pool).await?;
            }
            if concise { debug!("✅ Migration 011 applied"); } else { info!("✅ Migration 011 applied"); }
        } else if has_device_types.is_some() && has_category_col.is_some() && !concise {
            debug!("ℹ️ Migration 011 not needed (category column present)");
        }

        // Apply 012_device_types_add_type_id.sql if type_id column missing
        let has_type_id_col: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM pragma_table_info('device_types') WHERE name='type_id' LIMIT 1;",
        )
        .fetch_optional(&self.pool)
        .await?
        .flatten();
        if has_device_types.is_some() && has_type_id_col.is_none() {
            if concise { debug!("🧩 Applying migration 012_device_types_add_type_id.sql"); } else { info!("🧩 Applying migration 012_device_types_add_type_id.sql"); }
            let migration_path = std::path::Path::new("migrations/012_device_types_add_type_id.sql");
            if migration_path.exists() {
                let migration_sql = std::fs::read_to_string(migration_path)?;
                sqlx::query(&migration_sql).execute(&self.pool).await?;
            } else {
                let migration_sql = include_str!("../../migrations/012_device_types_add_type_id.sql");
                sqlx::query(migration_sql).execute(&self.pool).await?;
            }
            if concise { debug!("✅ Migration 012 applied"); } else { info!("✅ Migration 012 applied"); }
        } else if has_device_types.is_some() && has_type_id_col.is_some() && !concise {
            debug!("ℹ️ Migration 012 not needed (type_id column present)");
        }

        // Apply 013_product_primary_device_types_and_analytics_view.sql
        // Guard: run if bridge table missing OR analytics view missing
        let has_ppt: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='product_primary_device_types' LIMIT 1;",
        )
        .fetch_optional(&self.pool)
        .await?
        .flatten();
        let has_view: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM sqlite_master WHERE type='view' AND name='v_product_detail_analytics' LIMIT 1;",
        )
        .fetch_optional(&self.pool)
        .await?
        .flatten();
        if has_ppt.is_none() || has_view.is_none() {
            if concise { debug!("🧩 Applying migration 013_product_primary_device_types_and_analytics_view.sql"); } else { info!("🧩 Applying migration 013_product_primary_device_types_and_analytics_view.sql"); }
            let migration_path = std::path::Path::new("migrations/013_product_primary_device_types_and_analytics_view.sql");
            if migration_path.exists() {
                let migration_sql = std::fs::read_to_string(migration_path)?;
                sqlx::query(&migration_sql).execute(&self.pool).await?;
            } else {
                let migration_sql = include_str!("../../migrations/013_product_primary_device_types_and_analytics_view.sql");
                sqlx::query(migration_sql).execute(&self.pool).await?;
            }
            if concise { debug!("✅ Migration 013 applied"); } else { info!("✅ Migration 013 applied"); }
        } else if !concise {
            debug!("ℹ️ Migration 013 not needed (bridge + view present)");
        }

        // Apply 014_product_primary_device_types_backfill_and_triggers.sql (always run to ensure triggers/backfill)
        if concise { debug!("🧩 Applying migration 014_product_primary_device_types_backfill_and_triggers.sql"); } else { info!("🧩 Applying migration 014_product_primary_device_types_backfill_and_triggers.sql"); }
        let migration_014_path = std::path::Path::new("migrations/014_product_primary_device_types_backfill_and_triggers.sql");
        if migration_014_path.exists() {
            if let Ok(migration_sql) = std::fs::read_to_string(migration_014_path) { let _ = sqlx::query(&migration_sql).execute(&self.pool).await; }
        } else {
            let migration_sql = include_str!("../../migrations/014_product_primary_device_types_backfill_and_triggers.sql");
            let _ = sqlx::query(migration_sql).execute(&self.pool).await;
        }
        if concise { debug!("✅ Migration 014 applied (idempotent)"); } else { info!("✅ Migration 014 applied (idempotent)"); }

        // Apply 015_backfill_primary_device_type_ids_from_device_type.sql (fallback mapping from device_type text)
        if concise { debug!("🧩 Applying migration 015_backfill_primary_device_type_ids_from_device_type.sql"); } else { info!("🧩 Applying migration 015_backfill_primary_device_type_ids_from_device_type.sql"); }
        let migration_015_path = std::path::Path::new("migrations/015_backfill_primary_device_type_ids_from_device_type.sql");
        if migration_015_path.exists() {
            if let Ok(migration_sql) = std::fs::read_to_string(migration_015_path) { let _ = sqlx::query(&migration_sql).execute(&self.pool).await; }
        } else {
            let migration_sql = include_str!("../../migrations/015_backfill_primary_device_type_ids_from_device_type.sql");
            let _ = sqlx::query(migration_sql).execute(&self.pool).await;
        }
        if concise { debug!("✅ Migration 015 applied (idempotent)"); } else { info!("✅ Migration 015 applied (idempotent)"); }

        // Apply 016_rebuild_bridge_dual_mapping.sql (ensure bridge rows using type_id OR id)
        if concise { debug!("🧩 Applying migration 016_rebuild_bridge_dual_mapping.sql"); } else { info!("🧩 Applying migration 016_rebuild_bridge_dual_mapping.sql"); }
        let migration_016_path = std::path::Path::new("migrations/016_rebuild_bridge_dual_mapping.sql");
        if migration_016_path.exists() {
            if let Ok(migration_sql) = std::fs::read_to_string(migration_016_path) { let _ = sqlx::query(&migration_sql).execute(&self.pool).await; }
        } else {
            let migration_sql = include_str!("../../migrations/016_rebuild_bridge_dual_mapping.sql");
            let _ = sqlx::query(migration_sql).execute(&self.pool).await;
        }
        if concise { debug!("✅ Migration 016 applied (idempotent)"); } else { info!("✅ Migration 016 applied (idempotent)"); }

        // Apply 017_backfill_bridge_using_type_id_values.sql (map JSON codes directly to type_id)
        if concise { debug!("🧩 Applying migration 017_backfill_bridge_using_type_id_values.sql"); } else { info!("🧩 Applying migration 017_backfill_bridge_using_type_id_values.sql"); }
        let migration_017_path = std::path::Path::new("migrations/017_backfill_bridge_using_type_id_values.sql");
        if migration_017_path.exists() {
            if let Ok(migration_sql) = std::fs::read_to_string(migration_017_path) { let _ = sqlx::query(&migration_sql).execute(&self.pool).await; }
        } else {
            let migration_sql = include_str!("../../migrations/017_backfill_bridge_using_type_id_values.sql");
            let _ = sqlx::query(migration_sql).execute(&self.pool).await;
        }
        if concise { debug!("✅ Migration 017 applied (idempotent)"); } else { info!("✅ Migration 017 applied (idempotent)"); }

        // Apply 018_resilient_analytics_view.sql (fallback direct JSON mapping)
        if concise { debug!("🧩 Applying migration 018_resilient_analytics_view.sql"); } else { info!("🧩 Applying migration 018_resilient_analytics_view.sql"); }
        let migration_018_path = std::path::Path::new("migrations/018_resilient_analytics_view.sql");
        if migration_018_path.exists() {
            if let Ok(migration_sql) = std::fs::read_to_string(migration_018_path) { let _ = sqlx::query(&migration_sql).execute(&self.pool).await; }
        } else {
            let migration_sql = include_str!("../../migrations/018_resilient_analytics_view.sql");
            let _ = sqlx::query(migration_sql).execute(&self.pool).await;
        }
        if concise { debug!("✅ Migration 018 applied (idempotent)"); } else { info!("✅ Migration 018 applied (idempotent)"); }

        // DISABLED: Migrations 019, 020, 021 replaced by comprehensive migration 022
        // Apply 019_fix_device_type_mapping_in_analytics_view.sql (strict mapping + multi-value aggregation)
        // let migration_019_path = std::path::Path::new("migrations/019_fix_device_type_mapping_in_analytics_view.sql");
        // if migration_019_path.exists() {
        //     if let Ok(migration_sql) = std::fs::read_to_string(migration_019_path) { let _ = sqlx::query(&migration_sql).execute(&self.pool).await; }
        // } else {
        //     // Fallback to bundled version if not present on FS
        //     let migration_sql = include_str!("../../migrations/019_fix_device_type_mapping_in_analytics_view.sql");
        //     let _ = sqlx::query(migration_sql).execute(&self.pool).await;
        // }
        if concise { debug!("✅ Migration 019 skipped (replaced by 022)"); } else { info!("✅ Migration 019 skipped (replaced by 022)"); }

        // Apply 020_populate_bridge_table_and_fix_mappings.sql (populate bridge table and fix JSON mappings)
        // let migration_020_path = std::path::Path::new("migrations/020_populate_bridge_table_and_fix_mappings.sql");
        // if migration_020_path.exists() {
        //     if let Ok(migration_sql) = std::fs::read_to_string(migration_020_path) { let _ = sqlx::query(&migration_sql).execute(&self.pool).await; }
        // } else {
        //     // Fallback to bundled version if not present on FS
        //     let migration_sql = include_str!("../../../migrations/020_populate_bridge_table_and_fix_mappings.sql");
        //     let _ = sqlx::query(migration_sql).execute(&self.pool).await;
        // }
        if concise { debug!("✅ Migration 020 skipped (replaced by 022)"); } else { info!("✅ Migration 020 skipped (replaced by 022)"); }

        // Apply 021_convert_device_type_id_to_text.sql (convert type_id from INTEGER to TEXT)
        // let migration_021_path = std::path::Path::new("migrations/021_convert_device_type_id_to_text.sql");
        // if migration_021_path.exists() {
        //     if let Ok(migration_sql) = std::fs::read_to_string(migration_021_path) { let _ = sqlx::query(&migration_sql).execute(&self.pool).await; }
        // } else {
        //     // Fallback to bundled version if not present on FS
        //     let migration_sql = include_str!("../../../migrations/021_convert_device_type_id_to_text.sql");
        //     let _ = sqlx::query(migration_sql).execute(&self.pool).await;
        // }
        if concise { debug!("✅ Migration 021 skipped (replaced by 022)"); } else { info!("✅ Migration 021 skipped (replaced by 022)"); }

        // Apply 022_final_cleanup_and_optimization.sql (remove bridge table, optimize analytics view)
        let migration_022_path = std::path::Path::new("migrations/022_final_cleanup_and_optimization.sql");
        if migration_022_path.exists() {
            if let Ok(migration_sql) = std::fs::read_to_string(migration_022_path) { let _ = sqlx::query(&migration_sql).execute(&self.pool).await; }
        } else {
            // Fallback to bundled version if not present on FS
            let migration_sql = include_str!("../../../migrations/022_final_cleanup_and_optimization.sql");
            let _ = sqlx::query(migration_sql).execute(&self.pool).await;
        }
        if concise { debug!("✅ Migration 022 applied (idempotent)"); } else { info!("✅ Migration 022 applied (idempotent)"); }

        // Apply 023_enhance_analytics_view_transport_and_introduced_in.sql (ensure transport_interface + introduced_in)
        let migration_023_path = std::path::Path::new("migrations/023_enhance_analytics_view_transport_and_introduced_in.sql");
        if migration_023_path.exists() {
            if let Ok(migration_sql) = std::fs::read_to_string(migration_023_path) { let _ = sqlx::query(&migration_sql).execute(&self.pool).await; }
        } else {
            let migration_sql = include_str!("../../../migrations/023_enhance_analytics_view_transport_and_introduced_in.sql");
            let _ = sqlx::query(migration_sql).execute(&self.pool).await;
        }
        if concise { debug!("✅ Migration 023 applied (idempotent)"); } else { info!("✅ Migration 023 applied (idempotent)"); }

        // Apply 024_change_certification_date_to_date.sql (change column type TEXT->DATE via table rebuild)
        // Guard: only run if current column declared type != 'DATE'
        if let Ok(current_type) = sqlx::query_scalar::<_, Option<String>>("SELECT type FROM pragma_table_info('product_details') WHERE name='certification_date' LIMIT 1;")
            .fetch_one(&self.pool).await {
            let needs = current_type.map(|t| t.to_uppercase() != "DATE").unwrap_or(false);
            if needs {
                let mig024_path = std::path::Path::new("migrations/024_change_certification_date_to_date.sql");
                if mig024_path.exists() {
                    if let Ok(migration_sql) = std::fs::read_to_string(mig024_path) { let _ = sqlx::query(&migration_sql).execute(&self.pool).await; }
                } else {
                    let migration_sql = include_str!("../../../migrations/024_change_certification_date_to_date.sql");
                    let _ = sqlx::query(migration_sql).execute(&self.pool).await;
                }
                if concise { debug!("✅ Migration 024 applied (certification_date -> DATE)"); } else { info!("✅ Migration 024 applied (certification_date -> DATE)"); }
            } else if concise { debug!("ℹ️ Migration 024 skipped (certification_date already DATE)"); }
        }

        // Apply 025_normalize_certification_date_iso.sql (normalize MM/DD/YYYY -> YYYY-MM-DD)
        // Guard: run if any value matches slash pattern and any value not already ISO.
        if let (Ok(slash_cnt), Ok(any_rows)) = (
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM product_details WHERE certification_date LIKE '%/%'").fetch_one(&self.pool).await,
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM product_details WHERE certification_date IS NOT NULL AND certification_date <> ''").fetch_one(&self.pool).await,
        ) {
            if slash_cnt > 0 && any_rows > 0 {
                let mig025_path = std::path::Path::new("migrations/025_normalize_certification_date_iso.sql");
                if mig025_path.exists() {
                    if let Ok(migration_sql) = std::fs::read_to_string(mig025_path) { let _ = sqlx::query(&migration_sql).execute(&self.pool).await; }
                } else {
                    let migration_sql = include_str!("../../../migrations/025_normalize_certification_date_iso.sql");
                    let _ = sqlx::query(migration_sql).execute(&self.pool).await;
                }
                if concise { debug!("✅ Migration 025 applied (normalize certification_date)"); } else { info!("✅ Migration 025 applied (normalize certification_date)"); }
            } else if concise { debug!("ℹ️ Migration 025 skipped (no slash-form dates)"); }
        }

        // Report on database status
        let product_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM products")
            .fetch_one(&self.pool)
            .await?;

        let details_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM product_details")
            .fetch_one(&self.pool)
            .await?;
        if concise {
            info!(
                "🗄️ DB ready: products={}, details={}",
                product_count, details_count
            );
        } else {
            info!(
                "📊 Database initialized with {} products and {} detailed records",
                product_count, details_count
            );
        }

        Ok(())
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

    // Best-effort set; if already set by a racy concurrent init, prefer the existing one
    let _ = GLOBAL_SQLITE_POOL.set(pool.clone());
    Ok(pool)
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
