# Persist Metrics Normalization (Phase 1 FE)

## Goal
Unify semantics of Stage 5 (Persist) counts to remove ambiguity and prevent double counting when both `product-lifecycle-group` (phase=persist) and `product-lifecycle` + `persist_result` metrics appear. This is an interim FE-only normalization pending backend emission redesign.

## Raw Event Types
1. Group Event: `actor-product-lifecycle-group` with `phase = "persist"`
   - Provides: `group_size` (attempted), `succeeded`, `failed` (mixed: duplicates + unchanged + true failures), `duplicates`, `duration_ms`.
   - Does NOT distinguish inserted vs updated.
2. Result Event: `actor-product-lifecycle` with `status` in `persist_*` and `metrics.persist_result` string
   - Provides authoritative: `attempted, inserted, updated, duplicates, unchanged` (+ status classification, duration_ms)
   - Derived: `succeeded = inserted + updated`.
3. Edge Event: `actor-product-lifecycle` `status = persist_empty` – no items.
4. Fallback Batch Event: `actor-batch-completed` may expose coarse `products_inserted`, `products_updated` when persist chain missing.

## Normalized Fields
| Field | Meaning | Source Priority |
|-------|---------|-----------------|
| attempted | Total items processed (rows considered for persistence) | group + result summed (result authoritative once any result event seen) |
| inserted | Rows newly created | result only (or batch fallback) |
| updated | Rows changed (non-new) | result only (or batch fallback) |
| succeeded | inserted + updated | derived |
| duplicates | Rows skipped due to duplication | group (pre-result phase) + result (authoritative once seen) |
| unchanged | Rows determined identical (no change) | group (pre-result) + result authoritative once seen |
| failedTrue | attempted - (succeeded + duplicates + unchanged) | derived, clamp >= 0 |
| failed | attempted - succeeded (includes duplicates + unchanged + failedTrue) | derived |
| durationMs | Sum of durations from events | both |
| statusCounts.* | Tally of result status categories | result only |
| mode | Accumulator mode: `group-only` | `mixed` (after first result) | derived |

## Phase 1 Strategy (No Unique Group IDs Yet)
- Maintain two internal buckets: group bucket (only increments until first persist_result) and result bucket (increments after parsing each persist_result string).
- After the first persist_result is observed (`seenPersistResult = true`):
  - Group events no longer contribute counts (except duration) to avoid duplicate counting.
  - This may undercount duplicates/unchanged for groups that *only* ever emit group events after this point; acceptable interim trade-off.
- `persist_empty` resets accumulator.
- Batch fallback (`actor-batch-completed`) populates inserted/updated only if no result events have occurred (keeps `mode = group-only`).

## Mode Semantics
- group-only: Only group events (and maybe batch fallback) observed; inserted/updated may be 0 or from fallback.
- mixed: At least one persist_result observed; all authoritative counts come from result bucket thereafter; group bucket remains frozen.

## Future Backend Enhancements (Phase 2)
- Emit a unique `group_id` for both event types enabling exact reconciliation.
- Remove need for mode logic; merge on group_id keys.
- Provide explicit `unchanged` and `duplicates` in result event (already present) and omit them from group summary once result variant confirmed.

## Derived Rates (Optional for UI)
- successRate = succeeded / attempted
- duplicateRate = duplicates / attempted
- unchangedRate = unchanged / attempted
- failedTrueRate = failedTrue / attempted

## Reset Conditions
- New session start (engine start) – call `reset()`.
- `persist_empty` – sets all counters to zero and mode to group-only.

## Data Integrity Guards
- Clamp negative derived values to 0.
- Ignore placeholder group events with `group_size = 0` and `succeeded = 0` and `failed = 0`.

## Implementation Notes
- FE accumulator exposed via `getNormalized()` returning current snapshot.
- UI keeps existing field names to avoid broad refactor; adds `mode` & derived rate calculation inside normalization helper.

