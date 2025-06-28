#!/bin/bash

# Fast Rust test runner script
# Usage: ./scripts/test-fast.sh [test_name]

set -e

cd "$(dirname "$0")/.."

# Change to src-tauri directory
cd src-tauri

# Set environment variables for faster builds
export CARGO_INCREMENTAL=1
export RUST_LOG=warn

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
