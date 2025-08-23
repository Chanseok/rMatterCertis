#!/usr/bin/env bash
set -euo pipefail

echo "[install] Installing Rust dependency visualization tools"
echo "[install] Tools: cargo-depgraph (crate graph), cargo-modules (module graph)"

cargo install cargo-depgraph || true
cargo install cargo-modules || true

echo "[install] Optional: Graphviz for rendering DOT -> SVG/PNG (macOS)"
echo "  brew install graphviz"
