# Runtime Mode

The project runs a single, unified runtime: SessionActor + StageActor (with StageBatcher).
The former BatchActor path has been retired and is no longer available behind a feature flag.

## Running tests
- From `src-tauri/`:
  - `cargo test -q`

## CI overview
- Rust tests: `cargo test --all -q`
- TS type-check runs after generating Rust -> TS bindings.

Notes:
- Parity scripts and docs for comparing legacy vs. stage have been removed.
- If you still see mentions of MC_USE_STAGE_DIRECT or legacy-batch in older notes, treat them as historical context.
