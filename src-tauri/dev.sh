#!/bin/bash
# Shim to centralized script
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if [ -x "$REPO_ROOT/scripts/dev.sh" ]; then
    exec "$REPO_ROOT/scripts/dev.sh" "$@"
else
    echo "scripts/dev.sh not found or not executable" >&2
    exit 1
fi
