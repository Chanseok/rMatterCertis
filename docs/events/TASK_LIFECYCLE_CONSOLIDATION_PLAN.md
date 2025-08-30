# TaskLifecycle Consolidation Plan (Draft)

Objective
- Move towards a single task-level lifecycle: TaskLifecycle { task_kind: Page | Product, ... }
- Reduce duplicate streams: prefer TaskLifecycle over PageLifecycle/ProductLifecycle when feasible.
- Keep additive-only policy; deprecate older variants after FE migration completes.

Why
- One lifecycle simplifies FE reducers and metrics.
- Shared fields (timestamp, duration_ms, retry, metrics) reduce schema sprawl.

Contract (v1 draft)
Inputs
- Emitted by Stage actors and item actors at key milestones.
Outputs
- Unified FE event name: actor-task-lifecycle
Error modes
- Missing optional fields (batch_id, page_number, duration_ms) must be allowed; no hard dependency in FE rendering.
Success criteria
- FE visualizations and KPIs derived solely from actor-task-lifecycle + StageStarted/Completed.

Mapping (interim)
- PageLifecycle.fetch_started|fetched|parse_started|parsed|upserted|failed → TaskLifecycle { task_kind: Page, status: same }
- ProductLifecycle.started|parsed|upserted|failed → TaskLifecycle { task_kind: Product, status: same }
- ProductLifecycleGroup → summary log only (retain for diagnostics), optional aggregation into TaskLifecycle as metrics.delta.

Migration steps
1. Backend
   - Keep emitting PageLifecycle/ProductLifecycle for now.
   - Ensure native TaskLifecycle emission covers the same milestones.
   - Add minimal metrics parity (urls parsed, scheduled_details, error when present) to TaskLifecycle.metrics.
2. Frontend
   - Prefer actor-task-lifecycle in stores/hooks; keep PageLifecycle/ProductLifecycle as fallback.
   - Add a metrics adapter for old events until removed.
3. Decommission
   - After FE stabilizes, mark PageLifecycle/ProductLifecycle as deprecated in docs.
   - Remove synthetic derivations and update logs to use TaskLifecycle summaries.

Open questions
- Do we need separate ProductDetailFailed granular variants, or is TaskLifecycle.status sufficient? (Leaning: sufficient.)
- For batch-level summaries, leverage BatchReport and SessionReport instead.

Verification
- Unit tests for JSON shapes (ts-rs outlines + serde roundtrip).
- Trace test: simulate one batch with 2 pages and 3 products; assert FE receives expected sequence via actor-task-lifecycle.

Timeline
- Stage 1 (now): FE already consuming actor-task-lifecycle; backend native emission done. Keep dual for one sprint.
- Stage 2: Remove PageLifecycle/ProductLifecycle listeners in FE; keep only actor-task-lifecycle.
- Stage 3: Remove PageLifecycle/ProductLifecycle emissions (backend), leaving only TaskLifecycle.
