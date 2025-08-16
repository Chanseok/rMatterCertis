#!/usr/bin/env bash
set -euo pipefail

# Strict on fmt, soft on clippy
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
# Run clippy but don't fail build yet (collect warnings)
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets || true
