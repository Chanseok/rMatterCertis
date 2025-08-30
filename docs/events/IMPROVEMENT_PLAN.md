# Event System Improvement Plan (events-improve)

This plan reflects the analysis captured in `.local/prompts7` and our latest code state. It keeps the UI-focused fine-grained events while removing legacy duplication and clarifying roles and lifecycles.

## Goals
- Keep fine-grained, realtime UI signals for dynamic visualization (page/product item tasks).
- Remove legacy `domain::events::CrawlingEvent` usage from the active pipeline; use a single source: `AppEvent`.
- Clarify and simplify hierarchy and naming; reduce ambiguous or duplicate variants.
- Preserve KPI logs separately from UI-events; avoid coupling.

## Current State Summary
- Two event systems co-exist:
  - `AppEvent` (active actor system) under `crawl_engine/actors/types.rs`.
  - Legacy `domain::events::CrawlingEvent` still referenced (e.g., `infrastructure/system_broadcaster.rs`) and in generated TS types.
- Overlapping granular variants exist: `PageTaskStarted` vs `PageLifecycle`.
- "Phase*" events are used to denote top-level sequential orchestration steps; their role overlaps with session/batch/stage.

## Decisions
1. Single event source of truth: `AppEvent` remains primary for runtime and UI.
2. Keep fine-grained task-level emissions. Prefer `PageLifecycle` over `PageTask*` for richer status.
3. Retain KPI logs (`kpi.*`) as separate diagnostic channel.
4. Phase events: either (A) constrain and document as orchestration milestones, or (B) deprecate in favor of Session/Batch/Stage.

## Milestones

### M1: Inventory and guardrails (No behavior change)
- Document event taxonomy and mapping:
  - Session, Batch, Stage, Task(Page/Product) lifecycles
  - KPI vs UI event channels
- Add dev docs (this file) and a quick reference map (events-matrix.md) with producers/consumers.
- Add crate-level comments on `AppEvent` describing intended consumers and guidelines.

### M2: De-duplicate overlapping variants (Small changes)
- Prefer `AppEvent::PageLifecycle { status: fetch_started|fetched|parse_started|parsed|upserted|failed, ... }`.
- Mark `PageTaskStarted/Completed/Failed` as deprecated in code comments; keep emitting for a short transition if still used.
- Update `actor_event_bridge` to not generate combined page-lifecycle from PageTask when StageActor already emits native `PageLifecycle` (keep existing guard, verify paths).

### M3: Retire CrawlingEvent usage (Incremental)
- Remove conversions to `domain::events::CrawlingEvent` from active pipeline.
- Replace references in `infrastructure/system_broadcaster.rs` with `AppEvent`-centric publishing.
- Regenerate or remove generated TS types that depend on `CrawlingEvent`.
- Keep archival code in `_archive/` only.

### M4: Phase events decision
- Option A (retain): Rename to `OrchestrationPhase*` and limit to coarse milestones; ensure UI does not rely on them for per-item visuals.
- Option B (deprecate): Replace with well-defined Session/Stage boundaries + PlanReady.
- Whichever chosen, document with concrete producer functions and sample payloads.

### M5: Plan signals (pre-run estimates)
- Add/confirm `PlanReady { total_pages, total_products, total_batches, estimated_time_secs }`.
- Emitted after site+DB+config analysis, before starting session; consumed by UI to prime progress UI.

### M6: Quality gates and telemetry
- Add unit tests for JSON shape stability of `AppEvent` critical variants.
- Add a sample event trace test for one batch with a couple of tasks (no external IO).
- Ensure logging policy: UI events go to frontend; KPI logged to file; both can be toggled via config.

## Scope of changes
- Rust: actor_event_bridge.rs, actors/types.rs (docs), system_broadcaster.rs, possibly commands/actor_system_commands.rs where Phase emitted.
- TS: remove legacy generated types that import CrawlingEvent; ensure frontend listens to unified AppEvent names.

## Rollout
- Branch: `events-improve`.
- Phased PRs by milestones to keep diffs small, with build green after each step.

## Risks
- Frontend TypeScript types relying on CrawlingEvent need regeneration or removal.
- Any hidden consumer of `CrawlingEvent` may break; search and test thoroughly.

## Success Criteria
- No runtime use of `domain::events::CrawlingEvent` outside `_archive/`.
- Page/product item visualizations powered solely by `AppEvent` lifecycles.
- Documented matrix of events with clear roles; fewer ambiguous variants.
