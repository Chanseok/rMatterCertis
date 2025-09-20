# Event Schema Change Log

Format: `YYYY-MM-DD` - Category - Description

## 2025-09-20
- Added initial structured `CrawlEvent` schema (v1) with variants: CrawlSessionStarted, CrawlSessionCompleted, CrawlSessionFailed, StageStarted, StageProgress (planned), StageItemStarted, StageItemCompleted, StageItemFailed, StageItemRetrying, OverallProgressUpdate.
- Introduced dual emission (legacy `actor-event` + `crawl_updates`).
- Added throttle (500ms) for OverallProgressUpdate and StageProgress.
- Added mapping layer `map_app_event` (partial coverage).
- Added `error_category` field to `CrawlSessionFailed` and `StageItemFailed` variants (taxonomy phase 1: SessionFailure, StageFailure).

## Pending
- Add distinct StageFailed variant instead of reusing StageItemFailed with `item_id="__stage__"`.
- Expand error taxonomy (granular categories: Network, Parse, Validation, Storage, ExternalRateLimit, Internal, Unknown).
- Implement codegen pipeline Rust -> TS for CrawlEvent definitions (replace manual TS file).
