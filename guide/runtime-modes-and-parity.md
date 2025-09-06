# Runtime Modes and Parity

This project now defaults to the Stage path (SessionActor + StageActor with StageBatcher). The legacy BatchActor path is gated behind a Cargo feature and used only when explicitly enabled.

## Defaults
- Default build: legacy-batch feature OFF
- Default runtime: StageActor direct path
- Env toggle honored by some tests/binaries:
  - MC_USE_STAGE_DIRECT=1 forces Stage path
  - MC_USE_STAGE_DIRECT=0 prefers legacy path (requires legacy-batch feature at compile time)

## Running tests
- Default (Stage path):
  - From `src-tauri/`:
    - `cargo test -q`
- Legacy available (feature on):
  - From `src-tauri/`:
    - `cargo test -q --features legacy-batch`

## Parity compare locally
Runs the ignored smoke test twice (stage vs legacy) and compares summaries.

- From repo root:
  - Non-strict: `bash scripts/ci_parity_compare.sh`
  - Strict: `bash scripts/ci_parity_compare.sh --strict`

Artifacts are written to `parity/`.

## CI overview
- Rust tests matrix:
  - default: `cargo test --all -q`
  - legacy: `cargo test --all -q --features legacy-batch`
- TS type-check runs after generating Rust -> TS bindings.
- Parity compare runs strict by default; summaries uploaded as artifacts.
