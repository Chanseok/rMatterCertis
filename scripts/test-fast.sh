#!/bin/bash

# Fast Rust test runner script
# Usage: ./scripts/test-fast.sh [test_name]

set -e

cd "$(dirname "$0")/.."

# Change to src-tauri directory
cd src-tauri

# Set environment variables for faster, stable builds
export CARGO_INCREMENTAL=1
export RUST_LOG=warn

# Avoid global overrides that break tests (sccache/lld, etc.)
unset RUSTC_WRAPPER
unset SCCACHE_CACHE_SIZE
unset SCCACHE_DIR
unset SCCACHE_ERROR_LOG
# Clear custom flags that may inject unsupported link args
export RUSTFLAGS=""

echo "🚀 Running fast Rust tests..."

# Run tests with optimizations
if [ -n "$1" ]; then
    echo "🔍 Running specific test: $1"
    time cargo test "$1" --lib --bins
else
    echo "🧪 Running all tests"
    time cargo test --lib --bins
fi

echo "✅ Tests completed!"
