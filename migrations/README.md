# Database Migrations

## Current Schema Version: 1004

## Migration Files

### Active Migrations

- **001_baseline_consolidated.sql** - Complete baseline schema for fresh installations
  - Version: 1.0 (2025-10-08)
  - Includes all production-ready schema changes up to version 1004
  - Creates all tables, indexes, views, and seeds initial data
  - Idempotent: safe to run multiple times

### Archived Migrations

All development-phase migrations have been archived in `archive/development_migrations/`:
- Migration files 020-027: Development phase migrations
- Files 1002-1004: Incremental updates now consolidated into baseline

## Migration Policy

### For Fresh Installations

1. Run only `001_baseline_consolidated.sql`
2. This creates the complete schema at version 1004
3. No need to run any archived migrations

### For Existing Databases

Existing databases should already be at version 1004 or later.
If updating from an older version, apply migrations sequentially from the archive.

### Future Migrations

New schema changes should:
1. Create a new migration file with the next version number (1005, 1006, etc.)
2. Be idempotent (safe to run multiple times)
3. Update PRAGMA user_version at the end
4. Document the changes in this README

## Schema Overview

### Core Tables

- **vendors** - Vendor/manufacturer information
- **products** - Product catalog (URL-based, coordinates are reference only)
- **product_details** - Detailed product information
- **device_types** - Matter device type definitions

### Tracking Tables

- **page_fetch_attempts** - Crawling attempt instrumentation
- **sync_sessions** - Vendor sync session tracking
- **sync_observed** - Products observed during sync
- **crawling_results** - Crawling session results

### Key Design Decisions

1. **URL as Primary Key**: Both `products` and `product_details` use `url` as the primary key
2. **No Coordinate Constraints**: `(page_id, index_in_page)` are reference metadata only, not unique
3. **Dynamic Repositioning**: Allows products to move positions on the source website
4. **Idempotent Operations**: All migrations and seed data use `IF NOT EXISTS` or `INSERT OR IGNORE`

## Testing Fresh Installation

To test the app as if it were freshly installed:

1. Backup your current database:
   ```bash
   mv certis_cache.db certis_cache.db.backup
   ```

2. Run the app - it will create a fresh database using `001_baseline_consolidated.sql`

3. To restore your data:
   ```bash
   mv certis_cache.db.backup certis_cache.db
   ```

## Archive Structure

```
migrations/
  001_baseline_consolidated.sql  (Active baseline)
  README.md                       (This file)
  archive/
    development_migrations/       (All development-phase migrations)
      001_baseline_consolidated.sql (Original baseline)
      020_*.sql                    (Development migrations)
      021_*.sql
      ...
      1004_*.sql                   (Latest incremental)
```
