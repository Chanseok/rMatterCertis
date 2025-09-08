# Vendor Sync Plan (CSA DCL -> Local DB)

Goal: Fetch vendorInfo from https://on.dcl.csa-iot.org/dcl/vendorinfo/vendors with pagination, and upsert entries into the local SQLite `vendors` table until `pagination.next_key` is null.

## Contract

- Source API
  - Endpoint: `GET /dcl/vendorinfo/vendors` (optional `?pagination.key=<next_key>`)
  - Response shape:
    - `vendorInfo: Array<{ vendorID: number, vendorName: string, companyLegalName: string, companyPreferredName?: string, vendorLandingPageURL?: string, creator?: string, schemaVersion?: number }>`
    - `pagination: { next_key: string | null, total: string }` (total is string)
- Local DB schema (existing)
  - Table: `vendors(vendor_id INTEGER PK AUTOINCREMENT, vendor_number INTEGER UNIQUE, vendor_name TEXT NOT NULL, company_legal_name TEXT, created_at, updated_at)`
  - UNIQUE on `vendor_number`

## Mapping

- API `vendorID` -> DB `vendor_number`
- API `vendorName` -> DB `vendor_name`
- API `companyLegalName` -> DB `company_legal_name`
- Ignore for now: `companyPreferredName`, `vendorLandingPageURL`, `creator`, `schemaVersion` (can extend schema later)

## Update Logic

1. Fetch first page (no key). Parse `pagination.total` (string -> u32).
2. Compare with local count: `SELECT COUNT(*) FROM vendors`.
   - If `api_total <= local_count`, optionally no-op (still safe to continue to catch renames; see notes).
3. Loop pages until `next_key` is null:
   - For each `vendorInfo` item, UPSERT by `vendor_number`:
     - INSERT `(vendor_number, vendor_name, company_legal_name)`
     - ON CONFLICT(vendor_number) DO UPDATE SET `vendor_name`, `company_legal_name`, `updated_at = CURRENT_TIMESTAMP` if values differ.
   - Follow `pagination.next_key` via `?pagination.key=...`.
4. Return summary: `{ inserted, updated, skipped, api_total, final_count }`.

## Rust Implementation (files and responsibilities)

- infrastructure/csa_vendor_client.rs
  - Types: `CsaVendorsPage { vendorInfo: Vec<CsaVendor>, pagination: { next_key: Option<String>, total: String } }`
  - `fetch_page(next_key: Option<&str>) -> anyhow::Result<CsaVendorsPage>` using `reqwest` with timeout and retry (simple backoff 3x).

- infrastructure/integrated_product_repository.rs
  - Add:
    - `pub async fn count_vendors(&self) -> Result<i64>`
    - `pub async fn upsert_vendor_by_number(&self, number: i32, name: &str, company_legal_name: Option<&str>) -> Result<UpsertOutcome>`
      - Use SQLite UPSERT: `INSERT ... ON CONFLICT(vendor_number) DO UPDATE SET vendor_name = excluded.vendor_name, company_legal_name = excluded.company_legal_name, updated_at = CURRENT_TIMESTAMP`.

- application/vendor_sync.rs (or application/services.rs if preferred)
  - `pub async fn sync_vendors(repo: &IntegratedProductRepository) -> Result<VendorSyncResult>`
  - Orchestrates: first page fetch, compare counts, iterate pages, call upsert, accumulate metrics.

- commands/database/vendor_sync.rs
  - Tauri command: `#[tauri::command] pub async fn update_vendors_from_csa(state: State<'_, AppState>) -> Result<VendorSyncResult, String>`
  - Wires to `application::vendor_sync::sync_vendors`.
  - Register in `src-tauri/src/lib.rs` (or `main.rs` via `tauri::Builder::invoke_handler`).

## Error Handling & Retries

- Network errors: retry 3x with exponential backoff (e.g., 300ms, 900ms, 2.7s).
- Malformed page: log and skip page, continue; surface partial success in result.
- Pagination safety: track seen `next_key`s to prevent infinite loops.

## Edge Cases

- `pagination.total` decreases (API fixes/removals): do not delete local rows automatically.
- Vendor name/legal name changes: UPSERT updates names.
- Duplicate vendorIDs within a page: dedupe in-memory by `vendorID` before DB writes.
- Large totals: throttle requests (sleep 100–250ms between pages) to be polite.

## Verification

- Unit test: map `CsaVendor` -> upsert calls (mock repository), ensure counts.
- Integration (optional): mock HTTP via `wiremock` and run full sync over 2–3 pages.
- Post-check: `SELECT COUNT(*)` equals or exceeds previous count; sample a few known IDs.

## Optional Schema Extension (later)

- Add columns to `vendors`: `company_preferred_name TEXT`, `vendor_landing_page_url TEXT`.
- Migrate: `ALTER TABLE vendors ADD COLUMN ...` (SQLite-compatible), backfill via next sync.

## Minimal CLI/Tauri usage

- Tauri devtools: invoke `update_vendors_from_csa` from devtools panel, or via frontend if needed.
- CLI test (optional): add a Rust `tests/vendor_sync_smoke.rs` that runs `sync_vendors` with live network behind a `#[ignore]` flag.

## Done Criteria

- Command returns a summary with non-zero `inserted | updated` when API `total` > local count.
- Re-running is idempotent (subsequent runs mostly `skipped`).
