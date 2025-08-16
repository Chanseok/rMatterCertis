#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

echo "[precommit] rust fmt"
cargo fmt --manifest-path "$ROOT_DIR/src-tauri/Cargo.toml" --all

echo "[precommit] clippy -D warnings"
cargo clippy --manifest-path "$ROOT_DIR/src-tauri/Cargo.toml" --all-targets -- -D warnings

echo "[precommit] cargo test"
cargo test --manifest-path "$ROOT_DIR/src-tauri/Cargo.toml" --all

echo "[precommit] TS type-check"
(cd "$ROOT_DIR" && npm run -s type-check)

echo "[precommit] verify_runtime (best-effort)"
bash "$ROOT_DIR/scripts/verify_runtime_plan.sh" "$ROOT_DIR/src-tauri/target/debug/logs/back_front.log" || echo "[precommit] verify_runtime skipped/failed (non-blocking)"

echo "[precommit] done"
