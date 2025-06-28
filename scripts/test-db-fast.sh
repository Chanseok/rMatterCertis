#!/bin/bash
# Fast database tests with minimal recompilation

echo "🚀 Running database tests with optimizations..."

# Set environment variables for faster compilation
export CARGO_INCREMENTAL=1
export RUST_BACKTRACE=1

# Use specific test with correct syntax for faster feedback
cd src-tauri
cargo test database_connection --lib -- --nocapture

echo "✅ Database tests completed!"
