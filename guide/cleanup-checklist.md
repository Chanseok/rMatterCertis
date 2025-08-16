# Cleanup & Legacy Isolation Checklist (Phase 1)

- DB Pool
  - [x] Initialize global Sqlite pool at startup (OnceLock)
  - [ ] Remove any ad-hoc SqlitePool::connect usages in active paths
  - [ ] Ensure repositories take cloned SqlitePool and do not create connections

- Commands surface
  - [x] Keep a single unified start command
  - [ ] Move legacy/experimental start commands under `_archive/` or behind cfg(feature = "legacy_ref")

- Logging & Events
  - [x] Structured KPIs for plan/session/batch
  - [ ] Add batch_completed KPI and extend verifier to assert batch boundaries
  - [x] Separate events.log from back_front.log

- Dead code & tests
  - [ ] Remove unused new() on HttpClient if unreferenced
  - [ ] Delete obsolete mocks/simulations; keep only a small reference under `_archive/`
  - [ ] Add minimal tests for SessionActor batch sequencing (sequential batches)

- Docs
  - [x] Add architecture diagram (Mermaid)
  - [ ] Update re-arch guides to reflect unified entrypoint and global pool
