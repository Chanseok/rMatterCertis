-- Migration 008: Recompute primary_device_type_ids with corrected hex parser and refresh triggers
-- Safe to run multiple times. It will DROP/CREATE triggers idempotently and recompute values.

BEGIN TRANSACTION;

-- 1) Recompute for all rows that have legacy data
WITH RECURSIVE
split(url, rest, token) AS (
  SELECT url,
         TRIM(REPLACE(REPLACE(primary_device_type_id, ' ', ''), '0X', '0x')) || ',',
         NULL
  FROM product_details
  WHERE primary_device_type_id IS NOT NULL AND primary_device_type_id <> ''
  UNION ALL
  SELECT url,
         substr(rest, instr(rest, ',') + 1),
         substr(rest, 1, instr(rest, ',') - 1)
  FROM split
  WHERE rest <> '' AND instr(rest, ',') > 0
),
normalized(url, int_value) AS (
  SELECT url,
         (
           WITH RECURSIVE r(s, n) AS (
             SELECT lower(CASE WHEN token LIKE '0x%' THEN substr(token, 3) ELSE token END), 0
             UNION ALL
             SELECT substr(s, 2), n*16 + instr('0123456789abcdef', substr(s, 1, 1)) - 1
             FROM r
             WHERE s <> '' AND instr('0123456789abcdef', substr(s, 1, 1)) > 0
           )
           SELECT n FROM r WHERE s = ''
         )
  FROM split
),
dedup(url, int_value) AS (
  SELECT url, int_value FROM normalized WHERE int_value IS NOT NULL GROUP BY url, int_value
),
agg(url, json) AS (
  SELECT url,
         json_group_array(int_value) AS json
  FROM (
    SELECT url, int_value FROM dedup ORDER BY url, int_value
  )
  GROUP BY url
)
UPDATE product_details
SET primary_device_type_ids = (SELECT json FROM agg WHERE agg.url = product_details.url)
WHERE url IN (SELECT url FROM agg);

-- 2) Replace triggers with corrected logic
DROP TRIGGER IF EXISTS trg_pd_pdtid_after_insert;
DROP TRIGGER IF EXISTS trg_pd_pdtid_after_update;

CREATE TRIGGER IF NOT EXISTS trg_pd_pdtid_after_insert
AFTER INSERT ON product_details
FOR EACH ROW
BEGIN
  UPDATE product_details
  SET primary_device_type_ids = (
    WITH RECURSIVE
    split(rest, token) AS (
      SELECT TRIM(REPLACE(REPLACE(NEW.primary_device_type_id, ' ', ''), '0X', '0x')) || ',', NULL
      UNION ALL
      SELECT substr(rest, instr(rest, ',') + 1), substr(rest, 1, instr(rest, ',') - 1)
      FROM split
      WHERE rest <> '' AND instr(rest, ',') > 0
    ),
    normalized(int_value) AS (
      SELECT (
        WITH RECURSIVE r(s, n) AS (
          SELECT lower(CASE WHEN token LIKE '0x%' THEN substr(token, 3) ELSE token END), 0
          UNION ALL
          SELECT substr(s, 2), n*16 + instr('0123456789abcdef', substr(s, 1, 1)) - 1
          FROM r
          WHERE s <> '' AND instr('0123456789abcdef', substr(s, 1, 1)) > 0
        )
        SELECT n FROM r WHERE s = ''
      )
      FROM split
    ),
    dedup(int_value) AS (
      SELECT DISTINCT int_value FROM normalized WHERE int_value IS NOT NULL
    )
    SELECT json_group_array(int_value) FROM (SELECT int_value FROM dedup ORDER BY int_value)
  )
  WHERE url = NEW.url;
END;

CREATE TRIGGER IF NOT EXISTS trg_pd_pdtid_after_update
AFTER UPDATE OF primary_device_type_id ON product_details
FOR EACH ROW
BEGIN
  UPDATE product_details
  SET primary_device_type_ids = (
    WITH RECURSIVE
    split(rest, token) AS (
      SELECT TRIM(REPLACE(REPLACE(NEW.primary_device_type_id, ' ', ''), '0X', '0x')) || ',', NULL
      UNION ALL
      SELECT substr(rest, instr(rest, ',') + 1), substr(rest, 1, instr(rest, ',') - 1)
      FROM split
      WHERE rest <> '' AND instr(rest, ',') > 0
    ),
    normalized(int_value) AS (
      SELECT (
        WITH RECURSIVE r(s, n) AS (
          SELECT lower(CASE WHEN token LIKE '0x%' THEN substr(token, 3) ELSE token END), 0
          UNION ALL
          SELECT substr(s, 2), n*16 + instr('0123456789abcdef', substr(s, 1, 1)) - 1
          FROM r
          WHERE s <> '' AND instr('0123456789abcdef', substr(s, 1, 1)) > 0
        )
        SELECT n FROM r WHERE s = ''
      )
      FROM split
    ),
    dedup(int_value) AS (
      SELECT DISTINCT int_value FROM normalized WHERE int_value IS NOT NULL
    )
    SELECT json_group_array(int_value) FROM (SELECT int_value FROM dedup ORDER BY int_value)
  )
  WHERE url = NEW.url;
END;

COMMIT;
