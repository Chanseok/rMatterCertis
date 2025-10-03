-- Mirror of root migrations/001_baseline_consolidated.sql for embedded fallback include.
-- KEEP IN SYNC WITH ROOT VERSION.

PRAGMA foreign_keys = ON;
BEGIN;
-- ...existing baseline content is sourced from root file at runtime if available.
-- This embedded copy kept minimal to avoid duplication drift; prefer root file.
-- For safety in offline builds, we redundantly embed the full SQL below.

-- CORE TABLES
CREATE TABLE IF NOT EXISTS vendors (
  vendor_number INTEGER PRIMARY KEY,
  vendor_name TEXT,
  company_legal_name TEXT,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS products (
  url TEXT PRIMARY KEY,
  page_id INTEGER,
  index_in_page INTEGER,
  id TEXT,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS product_details (
  url TEXT PRIMARY KEY,
  page_id INTEGER,
  index_in_page INTEGER,
  id TEXT,
  manufacturer TEXT,
  model TEXT,
  device_type TEXT,
  certificate_id TEXT,
  certification_date DATE,
  software_version TEXT,
  hardware_version TEXT,
  firmware_version TEXT,
  specification_version TEXT,
  vid INTEGER,
  pid INTEGER,
  family_sku TEXT,
  family_variant_sku TEXT,
  family_id TEXT,
  tis_trp_tested TEXT,
  transport_interface TEXT,
  primary_device_type_ids TEXT,
  application_categories TEXT,
  description TEXT,
  compliance_document_url TEXT,
  program_type TEXT DEFAULT 'Matter',
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (url) REFERENCES products (url) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS device_types (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  code_hex TEXT,
  name TEXT NOT NULL,
  category TEXT,
  introduced_in TEXT,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
  type_id TEXT
);
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
CREATE TABLE IF NOT EXISTS page_repair_queue (
  logical_page_id INTEGER PRIMARY KEY,
  inserted_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  next_action_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  priority INTEGER NOT NULL DEFAULT 100,
  state TEXT NOT NULL DEFAULT 'pending',
  attempt_failures INTEGER NOT NULL DEFAULT 0,
  last_error_code TEXT
);
-- INDEXES
CREATE INDEX IF NOT EXISTS idx_product_details_manufacturer ON product_details (manufacturer);
CREATE INDEX IF NOT EXISTS idx_product_details_device_type ON product_details (device_type);
CREATE INDEX IF NOT EXISTS idx_product_details_certificate_id ON product_details (certificate_id);
CREATE INDEX IF NOT EXISTS idx_product_details_certification_date ON product_details (certification_date);
CREATE INDEX IF NOT EXISTS idx_product_details_vid ON product_details (vid);
CREATE INDEX IF NOT EXISTS idx_product_details_pid ON product_details (pid);
CREATE INDEX IF NOT EXISTS idx_product_details_specification_version ON product_details (specification_version);
CREATE INDEX IF NOT EXISTS idx_product_details_program_type ON product_details (program_type);
CREATE INDEX IF NOT EXISTS idx_device_types_type_id ON device_types (type_id);
CREATE INDEX IF NOT EXISTS idx_device_types_category ON device_types (category);
CREATE INDEX IF NOT EXISTS idx_page_fetch_attempts_page ON page_fetch_attempts (logical_page_id);
CREATE INDEX IF NOT EXISTS idx_page_fetch_attempts_success ON page_fetch_attempts (success_final);
CREATE INDEX IF NOT EXISTS idx_page_fetch_attempts_error ON page_fetch_attempts (error_code);
CREATE INDEX IF NOT EXISTS idx_page_fetch_attempts_mismatch ON page_fetch_attempts (count_mismatch, index_mismatch);
CREATE INDEX IF NOT EXISTS idx_page_repair_queue_state ON page_repair_queue (state, priority, next_action_at);
COMMIT;
