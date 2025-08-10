# Event Catalog (Backend -> Frontend)

> 자동화된 1차 추출 기반 초안. 필드/시맨틱은 실제 payload 코드와 로그 재검증 후 보강 예정.

## Session Lifecycle
- actor-session-started
- actor-session-paused
- actor-session-resumed
- actor-session-completed
- actor-session-failed
- actor-session-timeout

## Phase / Stage / Progress
- actor-phase-started / actor-phase-completed / actor-phase-aborted
- actor-stage-started / actor-stage-completed / actor-stage-failed
- actor-progress (generic percentage & message)
- crawling-progress (legacy / service-based)

## Batch
- actor-batch-started
- actor-batch-completed
- actor-batch-failed
- actor-batch-report

## Page Tasks
- actor-page-task-started
- actor-page-task-completed
- actor-page-task-failed

## Detail Tasks
- actor-detail-task-started
- actor-detail-task-completed
- actor-detail-task-failed
- actor-detail-concurrency-downshifted

## Metrics & Reports
- actor-performance-metrics
- actor-session-report (crawl summary)

## Shutdown
- actor-shutdown-requested
- actor-shutdown-completed

---
## Pending Additions / Validation TODO
| Event | Must Fields | Nice-to-have | Notes |
|-------|-------------|--------------|-------|
| actor-detail-task-* | session_id, detail_id, timestamp | page(optional) | For animation: need cumulative counts per session (merge logic) |
| actor-detail-concurrency-downshifted | session_id, old_limit, new_limit, trigger, timestamp | failure_rate snapshot | Drives DOWN pulse UI |
| actor-batch-started | session_id, batch_id, pages_count, timestamp | planned_pages[] | Needed for batch progress segmentation |
| actor-progress | session_id, percentage, current_step, total_steps | message | Mixes multiple granularities; may split later |
| actor-page-task-completed | session_id, page, duration_ms | batch_id | Needed for page throughput charts |
| actor-session-report | session_id, batches_processed, total_pages, total_success, total_failed, duration_ms | retries | Source of final KPIs |

## Gaps / Recommendations
1. Add monotonic `seq` to every AppEvent before bridging.
2. Include `backend_ts` (RFC3339) for latency measurement.
3. Ensure batch_started always precedes first page-task-started of that batch.
4. Consider delta-specific events (e.g. detail-progress-delta) to reduce payload size.
5. Provide a lightweight heartbeat when idle (>5s gap) to keep UI Live indicator.

## Verification Steps (Planned)
1. Instrument frontend hook with debug flag to log receipt order & latency.
2. Run small session (1 batch) then multi-batch session; export logs.
3. Compare against expected sequence diagrams (to be added).

---
(Generated initial draft)
