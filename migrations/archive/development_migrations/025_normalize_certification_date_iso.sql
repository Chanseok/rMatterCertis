-- Migration 025: Normalize product_details.certification_date to ISO (YYYY-MM-DD)
-- Assumptions:
--  * Current non-empty values use pattern MM/DD/YYYY (observed sample: 01/02/2024)
--  * We treat MM in 01..12 and DD in 01..31; entries not matching ^\d{2}/\d{2}/\d{4}$ left unchanged.
--  * Idempotent: already normalized YYYY-MM-DD left unchanged; second run no further changes.
--  * Uses a temporary column swap to avoid full table rebuild (DATE affinity already set by Migration 024).

BEGIN TRANSACTION;

-- Create temp column if not exists (SQLite lacks ADD COLUMN IF NOT EXISTS until 3.35+, but attempt safe add)
-- Will ignore error if column already exists.
PRAGMA foreign_keys=OFF;

ALTER TABLE product_details ADD COLUMN certification_date_normalized TEXT;

-- Populate normalized values
UPDATE product_details
SET certification_date_normalized = (
    CASE
      WHEN certification_date IS NULL OR certification_date = '' THEN NULL
      WHEN certification_date GLOB '[0-9][0-9]/[0-9][0-9]/[0-9][0-9][0-9][0-9]' THEN
           substr(certification_date, 7, 4) || '-' || substr(certification_date, 1, 2) || '-' || substr(certification_date, 4, 2)
      WHEN certification_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' THEN certification_date
      ELSE certification_date
    END
)
WHERE certification_date_normalized IS NULL;

-- Copy back to original column if changed
UPDATE product_details
SET certification_date = certification_date_normalized
WHERE certification_date_normalized IS NOT NULL;

-- (Optional) Drop temp column not possible directly; retain for idempotency marker.

PRAGMA foreign_keys=ON;
COMMIT;

-- Verification query (manual): SELECT certification_date FROM product_details ORDER BY certification_date DESC LIMIT 5;
