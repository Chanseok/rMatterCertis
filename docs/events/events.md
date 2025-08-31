# Unified Event Model (AppEvent)

This document describes the single, unified event stream emitted by the backend and consumed by the frontend. Legacy CrawlingEvent/Phase*/PageTask* are removed from runtime and docs.

## Principles
- Single source: AppEvent only, bridged to FE via unified "actor-event" channel.
- Hierarchy: Session > Batch > Stage > Task(Page/Product).
- Additive schema: new fields/variants are additive; removals go through deprecation + archive.
- Low-latency: emit as milestones happen; include timestamps for duration calculations.

## Core Variants
- SessionStarted | SessionCompleted | SessionFailed
- BatchStarted | BatchCompleted | BatchReport
- StageStarted | StageCompleted
- TaskLifecycle { task_kind: "Page" | "Product", status, batch_id?, page_number?, product_ref?, retry?, timestamp, duration_ms?, metrics? }
- PageLifecycle / ProductLifecycle (transitional; prefer TaskLifecycle)
- ProductLifecycleGroup (summary stats per page; diagnostic)
- ValidationReport
- DatabaseStats
- DetailConcurrencyDownshifted { old_limit, new_limit, reason, timestamp }
- SessionReport (final summary)

## Required fields by type
- All events: session_id, event_name, variant, seq, backend_ts (RFC3339)
- Stage/Batch/Task events: include batch_id; TaskLifecycle includes task_kind and status

## Emission order guarantees
- BatchStarted precedes first TaskLifecycle in that batch
- StageStarted/Completed wrap task sequences
- Heartbeat or low-frequency progress is allowed if idle >5s

## Frontend consumption
- Subscribe to 'actor-event'; dispatch by payload.event_name and variant
- Prefer TaskLifecycle for item-level visualization (pages/products)
- Use BatchReport/SessionReport for aggregate KPIs

## Deprecated/Removed
- Phase* variants: removed
- PageTask* variants: removed; use TaskLifecycle or PageLifecycle
- domain::events::CrawlingEvent: removed from runtime; only referenced in archives if needed

## Testing and stability
- JSON-shape snapshot tests for critical variants
- Trace test (1 batch, 2 pages, 3 products) asserting receipt order
