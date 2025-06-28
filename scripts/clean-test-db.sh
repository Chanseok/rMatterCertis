#!/bin/bash

# Clean up test database files and ensure fresh state
# This script removes any test-related database files to prevent stale state issues

echo "🧹 Cleaning up test database files..."

# Remove any test database files
find . -name "*.db" -path "*/data/*" -name "*test*" -delete 2>/dev/null || true
find . -name "*.db-shm" -path "*/data/*" -delete 2>/dev/null || true  
find . -name "*.db-wal" -path "*/data/*" -delete 2>/dev/null || true

# Remove any temporary test files
find . -name "test_*.db" -delete 2>/dev/null || true
find . -name "*_test.db" -delete 2>/dev/null || true

# Clean up any lock files
find . -name "*.lock" -path "*/target/*" -delete 2>/dev/null || true

echo "✅ Test database cleanup complete"
echo ""
echo "💡 Note: All tests now use in-memory databases (sqlite::memory:)"
echo "   This script is mainly for cleaning up any legacy test files."
echo ""
echo "🚀 Ready to run tests with clean state!"
