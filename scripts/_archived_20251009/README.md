# Archived Scripts (2025-10-09)

This directory contains obsolete scripts that are no longer used in the current codebase.
They are preserved for historical reference.

## Archived Files

### Diagnostic Scripts (Legacy)

- **check_csa_list_pages.mjs**: Old manual page verification script
  - Purpose: Fetch CSA-IoT Matter listing pages and extract URLs
  - Reason: Superseded by unified crawling engine with automated page discovery
  - Last Modified: 2024-08-18

- **diagnose_canonical_page.mjs**: Canonical page diagnosis tool
  - Purpose: Map URLs to (page_id, index_in_page) for specific pages
  - Reason: Integrated into DB diagnostics panel in UI (scanDbPaginationMismatches command)
  - Last Modified: 2024-09-08

### Empty Test Scripts

- **test_html_parsing.sh**: Empty test script
  - Reason: Never implemented, tests moved to Rust integration tests
  
- **test_site_structure.sh**: Empty test script
  - Reason: Never implemented, site structure verification in crawling engine

## Migration Notes

- **Page verification**: Use `CrawlingEngineTabSimple` > "사이트 상태 체크" button
- **DB diagnostics**: Use `DiagnosticsPanel` > "진단 실행" button
- **HTML parsing tests**: Run `cargo test` in `src-tauri/`
- **Site structure**: Automated in `StatusCheckerImpl::check_site_status()`

## Preservation

These scripts are kept in Git history and this archive directory for:
1. Understanding legacy diagnostic approaches
2. Reference for manual troubleshooting if needed
3. Historical context for architecture decisions
