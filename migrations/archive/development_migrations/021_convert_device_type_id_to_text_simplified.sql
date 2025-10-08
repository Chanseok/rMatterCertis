-- Archived simplified variant of migration 021. Superseded by consolidated baseline.

-- Step 1: Create a new table with TEXT type_id
-- (content removed)

-- Step 2: Copy existing data, converting INTEGER type_id to TEXT
-- (content removed)

-- Step 3: Drop the old table and rename the new one
-- (content removed)

-- Step 4: Recreate any indexes that might have existed
-- (content removed)

-- Step 5: Add any missing device types for unmatched JSON values
-- (content removed)

-- Step 6: Populate the bridge table with proper TEXT matching
-- (content removed)

-- Step 7: Update the analytics view to use TEXT type_id
-- (content removed)

-- Step 8: Show results
SELECT 'Migration 021 completed successfully!' as status;
