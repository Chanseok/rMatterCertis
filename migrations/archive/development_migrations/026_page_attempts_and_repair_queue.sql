-- 026_page_attempts_and_repair_queue.sql
-- Adds instrumentation tables for crawl attempt logging and repair workflow.
-- Idempotent: CREATE TABLE IF NOT EXISTS used for safety when developing.

PRAGMA foreign_keys=ON;

-- Each fetch (attempt) for a logical page gets a row.
-- logical_page_id: domain-level page identifier (e.g., numeric page index or composed key)
-- attempt_no: monotonically increasing per logical_page_id (assigned in application logic)
-- fetched_at: timestamp
-- product_count: number of parsed products in this attempt
-- distinct_indices: number of distinct indices observed (contiguity may differ)
-- contiguous_ok: 1 if indices form 0..(PRODUCTS_PER_PAGE-1) when product_count == PRODUCTS_PER_PAGE
-- count_mismatch: 1 if product_count != PRODUCTS_PER_PAGE for a non-terminal page expectation
-- index_mismatch: 1 if gap/duplicate detected
-- is_terminal_guess: 1 if crawler believed this might be the last page (partial allowed)
-- success_final: 1 if this attempt satisfied success predicate
-- error_code: short stable code for classified failure (NULL on success)
-- error_detail: optional extra context (trimmed)
-- duration_ms: total time cost of the attempt
-- retry_scheduled: 1 if after this attempt a retry was queued
CREATE TABLE IF NOT EXISTS page_fetch_attempts (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  logical_page_id INTEGER NOT NULL,
  attempt_no INTEGER NOT NULL,
  fetched_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  product_count INTEGER NOT NULL,
  distinct_indices INTEGER NOT NULL,
  contiguous_ok INTEGER NOT NULL,
  count_mismatch INTEGER NOT NULL,
  index_mismatch INTEGER NOT NULL,
  is_terminal_guess INTEGER NOT NULL,
  success_final INTEGER NOT NULL,
  error_code TEXT,
  error_detail TEXT,
  duration_ms INTEGER,
  retry_scheduled INTEGER NOT NULL DEFAULT 0,
  CONSTRAINT uq_attempt UNIQUE(logical_page_id, attempt_no)
);

CREATE INDEX IF NOT EXISTS idx_page_fetch_attempts_page ON page_fetch_attempts(logical_page_id);
CREATE INDEX IF NOT EXISTS idx_page_fetch_attempts_success ON page_fetch_attempts(success_final);
CREATE INDEX IF NOT EXISTS idx_page_fetch_attempts_error ON page_fetch_attempts(error_code);
CREATE INDEX IF NOT EXISTS idx_page_fetch_attempts_mismatch ON page_fetch_attempts(count_mismatch, index_mismatch);

-- Queue of pages requiring targeted repair / re-crawl.
-- inserted_at: when the page was enqueued
-- next_action_at: earliest time we should attempt repair (backoff scheduling)
-- priority: lower number = higher priority
-- state: pending|in_progress|done|failed (string; keep simple for now)
-- last_error_code: if failed, last classification
CREATE TABLE IF NOT EXISTS page_repair_queue (
  logical_page_id INTEGER PRIMARY KEY,
  inserted_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  next_action_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  priority INTEGER NOT NULL DEFAULT 100,
  state TEXT NOT NULL DEFAULT 'pending',
  attempt_failures INTEGER NOT NULL DEFAULT 0,
  last_error_code TEXT
);

CREATE INDEX IF NOT EXISTS idx_page_repair_queue_state ON page_repair_queue(state, priority, next_action_at);

-- (Optional future) Materialized summary view for quick completeness gap scan.
-- For now just define a view that inspects latest attempt per logical_page_id.
CREATE VIEW IF NOT EXISTS v_page_latest_attempt AS
WITH latest AS (
  SELECT logical_page_id, MAX(attempt_no) AS max_attempt
  FROM page_fetch_attempts
  GROUP BY logical_page_id
)
SELECT a.*
FROM page_fetch_attempts a
JOIN latest l ON a.logical_page_id = l.logical_page_id AND a.attempt_no = l.max_attempt;

-- Helper view focusing on problematic pages (non-success most recent attempt)
CREATE VIEW IF NOT EXISTS v_page_latest_problem AS
SELECT * FROM v_page_latest_attempt WHERE success_final = 0;