# Product Detail Event Schema (Implemented v1 via `AppEvent::ProductDetailKeyed`)

This document specifies the keyed per-product event stream now implemented through the new `AppEvent::ProductDetailKeyed` variant. Frontend tracker (`detail-tracker.ts`) consumes these to provide authoritative Stage 2 counts.

## Goals
- Eliminate ambiguity between page/batch/product scopes.
- Avoid frontend delta reconstruction and heuristic guards.
- Provide precise retry & failure tracking per product.

## Event: ProductDetailKeyed (AppEvent variant)
```jsonc
{
  "variant": "ProductDetailKeyed",          // flattened by bridge as event_name=actor-product-detail-keyed
  "session_id": "actor_session_...",
  "batch_id": "actor_session_...-pre-7",
  "product_key": "csa-iot.org/csa_product/wiz-led-lamp-tunable-white-24", // canonical or hash
  "product_url": "https://csa-iot.org/...-24/",
  "phase": "fetch",                          // fetch | parse | persist
  "status": "started",                       // started | succeeded | failed
  "attempt": 1,
  "duration_ms": 1234,                        // optional (mainly on succeeded/failed terminal)
  "html_size": 48765,                         // optional (parse/persist stages)
  "extracted_fields": 38,                     // optional (persist success)
  "error": "parse_failed",                   // optional (failed)
  "timestamp": "2025-09-17T02:33:41.512Z"
}
```

### Phase / Status Mapping (Bridge → Tracker)

Backend emits coarse `(phase, status)`; frontend maps to fine-grained tracker phases:

| Backend phase | Backend status | Tracker phase        |
|---------------|----------------|----------------------|
| fetch         | started        | fetch_started        |
| fetch         | succeeded      | fetch_succeeded      |
| fetch         | failed         | fetch_failed         |
| parse         | started        | parse_started        |
| parse         | succeeded      | parse_succeeded      |
| parse         | failed         | parse_failed         |
| persist       | started        | persist_started      |
| persist       | succeeded      | persist_succeeded    |
| persist       | failed         | persist_failed       |

Attempts currently default to 1 (sequential strategy). Future concurrency & retry logic will increment `attempt` on additional fetch attempts.

## Batch Summary (Optional)
```jsonc
{
  "type": "ProductDetailBatchSummary",
  "batch_id": "actor_session_...-pre-8",
  "phase": "fetch", // or persist
  "delta": { "attempts": 36, "succeeded": 36, "failed": 0 },
  "cumulative": { "attempts": 84, "succeeded": 80, "failed": 4 },
  "pages": [ { "page": 515, "count": 36 } ],
  "dur_ms": 20123
}
```
Frontend uses only `delta` to increment fast counters; `cumulative` is for reconciliation / telemetry.

## Backend Rust Sketch (Implemented Variant)
```rust
AppEvent::ProductDetailKeyed {
  session_id: String,
  batch_id: Option<String>,
  product_key: String,
  product_url: String,
  phase: String,      // fetch | parse | persist
  status: String,     // started | succeeded | failed
  attempt: Option<u32>,
  duration_ms: Option<u64>,
  html_size: Option<u32>,
  extracted_fields: Option<u32>,
  error: Option<String>,
  timestamp: DateTime<Utc>,
}
```

## Canonical Key Strategy
1. Canonicalize URL: lowercase host, remove trailing slash, decode percent encodings, stable ordering of query params (if any).
2. Hash with xxhash64 → send both full canonical URL (attempt=1 only) + 64-bit hex key for later attempts.
3. Frontend maintains map keyed by `key`.

## Migration Plan (Updated)
1. Variant added (shadow alongside legacy `ProductLifecycleGroup`).
2. Frontend activates tracker on first `actor-product-detail-keyed` event.
3. Validate parity (started == succeeded + failed) for multiple sessions.
4. Remove mapping + top-up heuristic code; retire fetch-phase `ProductLifecycleGroup` emission.
5. (Optional) Introduce hashed `product_key` + canonical URL emission-once optimization.

## Reconciliation
- Frontend compares: `tracker.fetch.succeeded + tracker.fetch.failed` vs latest batch cumulative (if present); log warning on drift > 1.

## Benefits
- Deterministic counts for started/completed/failed/retried.
- Simpler UI code (no delta math, no page-based guessing).
- Clear path to advanced metrics (latency per phase, retry reasons, quality gating, etc.).

## Next Steps
- Add canonical URL normalization + xxhash64 (emit both for attempt=1 then hash only).
- Add parse & persist intermediate (started) events (currently only fetch_started + parse_started emitted; persist_started optional).
- Implement retry-aware attempt increment (parallel/concurrent version).
- Remove legacy Stage 2 top-up once stability confirmed.
