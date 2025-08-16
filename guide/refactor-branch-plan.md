# Refactor Branching & Workflow (Phase 1 Cleanup)

- Current branch: `cleanup/phase-1`
- Recommendation:
  1) Merge stable changes to `main` (ensure build + runtime verifier green)
  2) Create `refactor/global-cleanup-phase-1` for dead-code removal and command unification
  3) Use PR with checklist from `guide/cleanup-checklist.md` as acceptance criteria

- Definition of Done (DoD):
  - Build + tests green; runtime verifier passes
  - Single start command exposed (unified entrypoint)
  - Global Sqlite pool reused everywhere; no ad-hoc `SqlitePool::connect` in active paths
  - events.log separated; batch_started/batch_completed KPIs present
  - Legacy code isolated under `_archive/` or guarded by feature flags
