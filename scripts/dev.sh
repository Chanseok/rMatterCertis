#!/bin/bash

# Fast Development Script for rMatterCertis
# 빠른 개발/테스트를 위한 스크립트

set -e

echo "🚀 rMatterCertis Fast Development Tools"
echo "======================================="

case "${1:-help}" in
    "quick" | "q")
        echo "⚡ Quick test (minimal dependencies)"
        time cargo run --bin test_minimal --no-default-features
        ;;
    
    "check" | "c")
        echo "🔍 Quick syntax check"
        time cargo check --bin test_minimal --no-default-features
        ;;
    
    "full")
        echo "🧪 Full integration test"
        time cargo run --bin test_db
        ;;
    
    "fast")
        echo "⚡ Fast integration test"
        time cargo run --bin test_db_fast
        ;;
    
    "light")
        echo "💡 Light integration test"
        time cargo run --bin test_db_light
        ;;
    
    "clean")
        echo "🧹 Clean build cache"
        cargo clean
        echo "✅ Cache cleaned"
        ;;
    
    "watch")
        echo "👀 Watch mode for quick tests"
        cargo watch -x "run --bin test_minimal --no-default-features"
        ;;
    
    "bench")
        echo "⏱️  Benchmark all test types"
        echo ""
        echo "1. Minimal test:"
        time cargo run --bin test_minimal --no-default-features
        echo ""
        echo "2. Light test:"  
        time cargo run --bin test_db_light
        echo ""
        echo "3. Full test:"
        time cargo run --bin test_db
        ;;
    
    "help" | *)
        echo "Usage: scripts/dev.sh [command]"
        echo ""
        echo "Commands:"
        echo "  quick, q    - Quick test (minimal deps, ~0.5s)"
        echo "  check, c    - Syntax check only (~0.9s)"
        echo "  fast        - Fast integration test (~2-5s)"
        echo "  light       - Light integration test (~5-10s)"
        echo "  full        - Full integration test (~15-20s)"
        echo "  clean       - Clean build cache"
        echo "  watch       - Watch mode for development"
        echo "  bench       - Benchmark all test types"
        echo "  help        - Show this help"
        echo ""
        echo "💡 Pro tip: Use 'quick' during development for fastest feedback!"
        ;;
    esac
