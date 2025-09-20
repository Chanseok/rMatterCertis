# DB Tests Refactor Plan

## 1. Background & Problems (Pre-Refactor)
Legacy inline tests inside `data_queries.rs` and other DB-heavy modules were:
- Coupled to Tauri state & side effects (logging, runtime backfill, migrations)
- Intermittently failing due to schema drift & runtime-created views
- Hard to evolve (adding a field required editing large integration-like inline code)
- Slow (real file-backed DB, no focused dataset control)

## 2. Goals
| Goal | Description | Acceptance |
|------|-------------|------------|
| Deterministic | In-memory / temp DB with explicit schema snapshot | Test unaffected by host state |
| Fast iteration | Sub‑second core query tests | Typical suite < 2s (core) |
| Isolation | Each test controls its own dataset | No cross-test leakage |
| Layered coverage | Core query pure functions + command layer smoke | Core logic testable w/o Tauri State |
| Minimal surface | Test only canonical queries & diagnostics KPIs | Extraneous legacy paths removed |

## 3. Refactor Stages Summary
| Stage | Action | Status | Artifact |
|-------|--------|--------|----------|
| 1 | Remove broken inline DB tests | Done | Inline test block removed |
| 2 | Extract core functions (pagination/products) | Done | `core_fetch_products_page` |
| 3 | Add in-memory harness + minimal schema & seed | Done | `test_db.rs`, `min_schema.sql`, `seed_basic.sql` |
| 4 | Rebuild paging + analytics tests (core) | Done | `test_pagination_and_analytics.rs` |
| 5 | Diagnostics coverage (core subset) | Done | `core_diagnostics_analytics_mapping` |
| 6 | Documentation & future enhancements plan | Done | This file |

## 4. Current Core Test Harness
Components:
- `infrastructure/test_db.rs`: `build_memory_db(schema, seed)` → shared-cache memory SQLite
- `tests/db/min_schema.sql`: Minimal deterministic schema (subset tables + simplified view)
- `tests/db/seed_basic.sql`: Small polymorphic dataset (single vendor with multi device types)

Design Choices:
- Shared-cache `sqlite::memory:?cache=shared` so multiple sqlx connections see same schema
- Simplified analytics view: direct LEFT JOIN composition; robust against migration variations
- Bridge relationship pre-populated via seed to avoid runtime backfill complexity in core tests

## 5. Core Functions Under Test
| Function | Responsibility | Side Effects | Tested Scenarios |
|----------|----------------|--------------|------------------|
| `core_fetch_products_page` | Paginated product listing (count + slice) | None | Page 0/1 size boundaries |
| `core_analytics_query` | Filter / sort / paginate analytics view | None | Bareword filter, default sort |
| `core_diagnostics_analytics_mapping` | Coverage metrics (counts + % mapped) | None | Coverage computed > 0 |

## 6. What Was Deliberately Excluded
| Concern | Reason for Exclusion | Future Path |
|---------|----------------------|-------------|
| Full runtime backfill logic | Coupled to side effects & transactions | Separate integration test (optional) |
| Large vendor sync flows | Slow & I/O heavy | Use synthetic targeted unit tests later |
| Migration chain validation | Belongs to migration smoke stage, not query tests | Add CI job invoking `sqlx migrate run` |
| Error path exhaustive tests | Low ROI, underlying sqlx error typed already | Add single failure injection harness if needed |

## 7. Mapping to Metrics & Observability
Diagnostics test surfaces the same coverage metric that production will export (mapping coverage %). This ensures:
- Early drift detection (schema or view break → metric collapse to 0)
- Confidence that simplified view still produces meaningful mapping counts

## 8. Future Enhancements (Optional Roadmap)
| Enhancement | Benefit | Complexity |
|------------|---------|------------|
| Failure Injection (BEGIN fail / deadlock simulation) | Robust retry semantics test | Medium |
| Temporal Fixtures (controlled timestamps) | Deterministic date range analytics | Low |
| DSL Parser Unit Tests (token-level) | Independent evolution of filter grammar | Low |
| Property Tests (quickcheck) for filter -> SQL mapping | Fuzz invalid sequences safely | Medium |
| Snapshot of analytics rows JSON (stable schema) | Change detection on view adjustments | Low |
| Coverage gating (fail if coverage_pct < threshold in seed) | Guard accidental seed regressions | Very Low |

## 9. Guidelines for Adding New DB Tests
1. Decide layer: core function vs command wrapper
2. Prefer: extend minimal schema instead of using production migrations for speed
3. Add seed rows explicitly; keep seed < 30 rows for readability
4. No sleeping / timing assumptions (use deterministic data)
5. If adding a new optional column: default it Null in minimal schema, assert fallback logic

## 10. Risk & Mitigation
| Risk | Mitigation |
|------|-----------|
| Minimal schema diverges too far from production | Periodic sync review: diff core tables vs latest migration |
| Hidden dependency on field absent in minimal schema | Failing test prompts explicit addition (fail-fast) |
| Overgrowth of seed complexity | Enforce < 30 rows, consolidate patterns |
| Silent SQL change breaks analytics view | Core test failing early reveals issue |

## 11. Acceptance (Refactor Complete)
- All planned stages completed (1–6)
- No inline DB tests remain in production source files
- Core tests pass with `cargo test --tests` (target subset) in < 2s expected (empirical check pending)
- Documentation (this file) committed

## 12. Maintenance Cadence
- Review minimal schema quarterly or after major migration batches
- Add new core function tests upon introducing non-trivial SQL (JOIN / aggregation / filtering) logic

(End of Document)
