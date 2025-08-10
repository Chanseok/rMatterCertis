# Status JSON & Resume Token v2 Documentation (P0)

## Session Status JSON (get_session_status)

Top-level fields:
- session_id: string
- status: Running | Paused | Completed | Failed | ShuttingDown
- started_at / completed_at: RFC3339 timestamps
- contract_version: u32 (current actor contract indicator)

Pages object:
- processed: u64
- total: u64
- percent: f64 (processed/total * 100)
- failed: usize (length of failed_pages)
- failed_rate: f64 (failed / processed; 0 if processed == 0)
- retrying: usize (length of retrying_pages)
- failure_threshold: u32 (page_failure_threshold)

Batches object:
- completed / total / percent

Errors object:
- last: Option<String>
- count: u32
- rate: f64 (error_count / processed_pages)

Resume / Progress:
- resume_token: Option<String> (latest generated v2 token if session completed or failed with token emission)
- remaining_pages: Option<Vec<u32>> (physical pages not yet processed)

Retry Policy:
- product_list_max_retries: u32 (configured max retries per page task)

Metrics:
- elapsed_ms: u64 (since session started)
- throughput_pages_per_min: f64
- eta_ms: f64 (estimated remaining; 0 if throughput not yet established)

Params:
- batch_size: u32
- concurrency_limit: u32 (initial page phase concurrency)

Retries object:
- total_attempts: sum(retries_per_page.values())
- per_page_sample (in command version only): first 20 (page, count) pairs

Details object (ProductDetails phase):
- total / completed / failed: u64 counters
- failed_ids_sample: up to 20 detail IDs that failed
- remaining_ids: Option<Vec<String>> (remaining detail tasks) — omitted when None
- retries_total: u64 (aggregate detail retries)
- retry_histogram: Vec<{retries, count}> distribution
- retry_counts_sample: first 20 (id, count)
- failure_threshold: u32 (detail_failure_threshold)
- downshifted: bool (whether dynamic concurrency downshift triggered)
- downshift_meta: null OR object:
  - timestamp: RFC3339 timestamp (first trigger)
  - old_limit: u32
  - new_limit: u32
  - trigger: String (e.g. "fail_rate>0.31")

## Resume Token v2 Schema
```
{
  "version": 2,                // optional (may be absent for backward compatibility)
  "plan_hash": "<string>",
  "remaining_pages": [u32, ...],
  "remaining_detail_ids": ["detail_id", ...],            // optional (v2)
  "detail_retry_counts": [["detail_id", retry_u32], ...], // optional (v2)
  "detail_retries_total": u64,                            // optional (v2)
  "generated_at": "RFC3339",                              // emission timestamp
  "processed_pages": u64,                                  // total pages processed when token emitted
  "total_pages": u64,                                      // original planned pages
  "batch_size": u32,
  "concurrency_limit": u32,
  "retrying_pages": [u32,...],                             // pages currently pending retry
  "failed_pages": [u32,...],                               // pages permanently failed
  "retries_per_page": [[page_u32, retry_count_u32], ...],  // per-page retry counts
  "detail_retry_histogram": [[retries_u32, count_u32], ...] // optional (future extension)
}
```

Backward Compatibility:
- A token missing any detail_* fields is interpreted as v1; code treats absent arrays as empty.
- version field currently optional; absence implies legacy/v1; presence of detail fields upgrades interpretation.

Parsing Logic Highlights:
- remaining_pages must be non-empty or resume is rejected.
- detail_* fields default to empty/hashmaps when missing.
- plan_hash is trusted now; future phase will recompute for integrity check.

Downshift Metadata Persistence:
- When detail failure rate first exceeds 0.30: record timestamp, old_limit, new_limit, trigger string; set detail_downshifted=true.
- Status JSON exposes these via details.downshift_meta.

## Emission Timing
- Status endpoint can be polled at any time; metrics and counters update live.
- Resume token is generated lazily on completion if absent, or earlier when needed (future enhancement: periodic snapshot tokens).

## Future Extensions (documented placeholders)
- contract_version bump criteria (new breaking fields, semantics changes)
- detail dynamic re-upshift (not implemented yet)
- partial page slot integrity hash validations

## Minimal Client Consumption Strategy
1. Call get_session_status(session_id).
2. Read pages.percent & details.completed/total to drive progress bars.
3. Show warning badge if details.downshifted == true (tooltip from details.downshift_meta.trigger).
4. Offer resume button only if status is Failed or Paused and remaining_pages not empty.

## Example Downshift Meta
```
"downshift_meta": {
  "timestamp": "2025-08-10T02:11:34.123Z",
  "old_limit": 8,
  "new_limit": 4,
  "trigger": "fail_rate>0.31"
}
```

---
Generated as part of P0 stabilization deliverables (downshift metadata + resume token v2 documentation).
