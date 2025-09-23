# Baseline Migration Plan (Post-025 Consolidated Schema)

Goal: Introduce a single `001_baseline.sql` for fresh installs representing the database state after migration 025 (including 022 cleanup removing the bridge) so historical incremental migrations (003-025) run only for legacy databases.

## Target Fresh Install Objects

Tables:
- products
- product_details (with `certification_date` DATE affinity and potential ISO-normalized values; omit temporary helper column)
- vendors
- crawling_results
- sync_sessions
- sync_observed
- device_types (final structure: id, code_hex, name, category, introduced_in, created_at, updated_at, type_id TEXT)

Views:
- v_product_detail_analytics (final JSON-based mapping with transport_if alias and introduced_in fallback logic from 023 + final JSON mapping approach from 022/023)

Triggers:
- products_updated_at
- product_details_updated_at
- vendors_updated_at
- device_types_updated_at

Indexes (retain naming for compatibility):
- idx_products_manufacturer
- idx_products_certificate_id
- idx_products_page_id
- idx_products_created_at
- idx_product_details_manufacturer
- idx_product_details_device_type
- idx_product_details_certificate_id
- idx_product_details_certification_date
- idx_product_details_vid
- idx_product_details_pid
- idx_product_details_specification_version
- idx_product_details_program_type
- idx_vendors_vendor_name
- idx_vendors_vendor_number
- idx_crawling_results_status
- idx_crawling_results_started_at
- idx_crawling_results_stage
- idx_sync_observed_session_page
- ux_device_types_name
- ux_device_types_code_hex
- ux_device_types_type_id (UNIQUE)

Removed (legacy, exclude from baseline):
- product_primary_device_types (bridge table)
- Any bridge triggers or backfill logic
- Transitional temp tables (device_types_new, product_details_new, certification_date_normalized column)
- Legacy hybrid analytics view variants (018, 019 forms)

## Compatibility & Upgrade Strategy

Detection logic for legacy DB (Rust):
1. Check for presence of table `product_primary_device_types` OR absence of column `type_id` in `device_types` OR `product_details.certification_date` TEXT affinity.
2. If any legacy markers present, run legacy migrations (existing 003+ chain) via splitter.
3. Else, run only baseline `001_baseline.sql` (idempotent on empty DB) + any new future migrations starting at 002.

## Baseline File Structure Outline (`src-tauri/migrations/001_baseline.sql`)

Order:
1. CREATE TABLE products
2. CREATE TABLE product_details (DATE affinity, no temp column)
3. CREATE TABLE vendors
4. CREATE TABLE crawling_results
5. CREATE TABLE sync_sessions
6. CREATE TABLE sync_observed + index
7. CREATE TABLE device_types (final schema) + indexes + trigger
8. Triggers for products/product_details/vendors
9. Indexes for tables (grouped by table for clarity)
10. View v_product_detail_analytics (from 023 with JSON direct mapping & fallback logic applied + introduced_in normalization step inline)
11. Seed minimal device_types canonical rows (subset from 022) OR defer seeding to runtime (decision TBD; current behavior seeds from JSON file) -> Recommendation: keep runtime JSON seeding; only ensure constraints.
12. (Optional) PRAGMA user_version = 1; (if using) or leave to Rust migration tracking table.

## Open Decisions
- Seeding strategy: Continue runtime JSON seed (preferred) to avoid duplication.
- user_version vs custom migrations table: Current system enumerates files; baseline can stay file-driven.
- Whether to bake normalization of introduced_in values inside view vs pre-update: For baseline, replicate 023 logic: pre-update device_types introduced_in empty -> 'Unknown'.

## Next Steps
1. Draft `001_baseline.sql` implementing above outline.
2. Add detection logic to `database_connection.rs` to choose between baseline path vs legacy chain.
3. Add documentation section to `matter-certis-v2-development-guide.md` referencing baseline strategy.
4. Remove now-superseded migrations (optional) or keep under `legacy/` folder.

-- End of Plan
