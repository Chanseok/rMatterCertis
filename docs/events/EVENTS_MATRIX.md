# Events Matrix (Current → Target)

This is a living map of event variants, their producers, and intended consumers, to guide the events-improve work.

## Current key variants (AppEvent)
- SessionStarted / SessionCompleted / SessionFailed
  - Producers: Session orchestration (commands/actor_system_commands.rs)
  - Consumers: Frontend UI, logs
- PhaseStarted / PhaseCompleted
  - Producers: commands/actor_system_commands.rs
  - Consumers: Logs, sometimes UI (ambiguous)
- BatchStarted / BatchCompleted
  - Producers: BatchActor/Orchestrator
  - Consumers: UI batch progress, logs
- StageStarted / StageCompleted
  - Producers: StageActor
  - Consumers: UI per-stage status and timing, logs
- PageTaskStarted / PageTaskCompleted / PageTaskFailed
  - Producers: StageActor and callers (older path)
  - Consumers: UI item-level visualization (legacy path), logs
- PageLifecycle { status: fetch_started|fetched|parse_started|parsed|upserted|failed, ... }
  - Producers: StageActor (new path)
  - Consumers: UI item-level visualization (preferred), logs

Other channels
- KPI logs `kpi.*` (plan, batch, perf)
  - Producers: various services/commands
  - Consumers: File logger (events.log), not UI

## Duplications/ambiguities
- PageTask* overlaps PageLifecycle. Prefer PageLifecycle (richer states). Keep PageTask* temporarily for transition.
- Phase* overlaps coarse session/stage milestones; decide to retain with constrained role or deprecate.

## Target intents
- Single source: AppEvent for UI; no runtime use of domain::events::CrawlingEvent.
- UI consumes Session/Batch/Stage and PageLifecycle for rich item-level progress.
- KPI remains separate diagnostic stream.

## Action checklist (by milestone)
- M1 Docs: finalize taxonomy and this matrix; add comments to `actors/types.rs` (no behavior change).
- M2 De-dup: deprecate PageTask* in comments, prefer PageLifecycle everywhere; ensure bridge does not double-emit.
- M3 Retire CrawlingEvent: remove conversions and replace infra broadcaster usage with AppEvent; clean generated TS types.
- M4 Phase decision: retain and rename as OrchestrationPhase* with documented scope, or deprecate.
- M5 PlanReady: ensure pre-run plan emission with totals and estimate.
- M6 Tests/telemetry: JSON shape tests for critical variants; sample trace test.
