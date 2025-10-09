# Database Migrations

## Current Schema Version: 2000 (v2.0 Cleaned)

## Migration Files

### Active Migrations

- **002_baseline_cleaned.sql** - Cleaned baseline schema for fresh installations ⭐ **CURRENT**
  - Version: 2.0 (2025-10-10)
  - **Removed**: sync_sessions, sync_observed, crawling_results, page_fetch_attempts (실제 동작 안 함)
  - **Removed**: application_categories 컬럼 (파싱 로직 없음, 항상 NULL)
  - Idempotent: safe to run multiple times

- **001_baseline_consolidated.sql** - Previous baseline (v1.0)
  - Version: 1.0 (2025-10-08)
  - Includes all production-ready schema changes up to version 1004
  - **Deprecated**: Use 002_baseline_cleaned.sql instead

### Archived Migrations

All development-phase migrations have been archived in `archive/development_migrations/`:
- Migration files 020-027: Development phase migrations
- Files 1002-1004: Incremental updates now consolidated into baseline

## Migration Policy

### For Fresh Installations

1. Run only `002_baseline_cleaned.sql` ⭐
2. This creates the complete schema at version 2000
3. No need to run any archived migrations

### For Existing Databases (v1.0 → v2.0)

To upgrade from v1.0 to v2.0, run:
```sql
-- Remove deprecated tables
DROP TABLE IF EXISTS sync_sessions;
DROP TABLE IF EXISTS sync_observed;
DROP TABLE IF EXISTS crawling_results;
DROP TABLE IF EXISTS page_fetch_attempts;
DROP VIEW IF EXISTS v_page_latest_attempt;
DROP VIEW IF EXISTS v_page_latest_problem;

-- Remove deprecated column (SQLite doesn't support DROP COLUMN directly)
-- application_categories will be ignored by new code

-- Update schema version
PRAGMA user_version = 2000;
```

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

### Views

- **v_product_detail_analytics** - Analytics view joining products, details, vendors, and device types

### Removed in v2.0 (실제 동작하지 않았던 테이블)

- ~~**page_fetch_attempts**~~ - 선택적 디버깅 기능 (MC_ATTEMPT_LOG_SQLITE 환경 변수 필요, 일반 사용 안 함)
- ~~**sync_sessions**~~ - 타입 불일치로 실제 동작 안 함
- ~~**sync_observed**~~ - 외래 키 제약으로 실패
- ~~**crawling_results**~~ - 사용처 없음
- ~~**product_details.application_categories**~~ - 파싱 로직 없음, 항상 NULL

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
