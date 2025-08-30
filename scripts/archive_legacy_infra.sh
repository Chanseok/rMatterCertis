#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SRC_DIR="$ROOT_DIR/src-tauri/src"
ARCHIVE_DIR="$SRC_DIR/_archive/infrastructure"

mkdir -p "$ARCHIVE_DIR"

move_if_exists() {
  local src="$1"
  if [ -f "$src" ]; then
    echo "[archive] moving $(basename "$src")"
    git mv -f "$src" "$ARCHIVE_DIR/"
  else
    echo "[skip] not found: $src"
  fi
}

move_if_exists "$SRC_DIR/infrastructure/advanced_crawling_engine.rs"
move_if_exists "$SRC_DIR/infrastructure/crawling_engine.rs"
move_if_exists "$SRC_DIR/infrastructure/service_based_crawling_engine.rs"
move_if_exists "$SRC_DIR/infrastructure/work_queue_engine.rs"
move_if_exists "$SRC_DIR/infrastructure/matter_crawler.rs"
move_if_exists "$SRC_DIR/infrastructure/crawling_result_repository.rs"
move_if_exists "$SRC_DIR/infrastructure/http.rs"
move_if_exists "$SRC_DIR/infrastructure/repositories_adapter.rs"
move_if_exists "$SRC_DIR/infrastructure/crawling.rs"
# parsing.rs는 최신 구조와 얽혀 있을 수 있어 이동하지 않음
move_if_exists "$SRC_DIR/infrastructure/repositories.rs"

echo "[archive] done. Review git status and run cargo check."
