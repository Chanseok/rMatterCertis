# Fresh Installation Testing Guide

## Purpose

This guide helps you test the application as if it were freshly installed, simulating a first-time user experience.

## Prerequisites

- The app is not currently running
- You have access to the terminal

## Testing Steps

### 1. Backup Current Database

Before starting, backup your current database:

```bash
cd /Users/chanseok/Codes/rMatterCertis
mv certis_cache.db certis_cache.db.backup
```

### 2. Run the Application

Start the application normally:

```bash
npm run tauri dev
```

The app will:
1. Detect that no database exists
2. Create a fresh `certis_cache.db`
3. Run `migrations/001_baseline_consolidated.sql`
4. Initialize the database with the complete schema (version 1004)

### 3. Verify Fresh Installation

After the app starts, check:

1. **Database was created**:
   ```bash
   ls -lh certis_cache.db
   ```
   Should show a newly created file

2. **Schema version is correct**:
   ```bash
   sqlite3 certis_cache.db "PRAGMA user_version;"
   ```
   Should output: `1004`

3. **Tables were created**:
   ```bash
   sqlite3 certis_cache.db "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name;"
   ```
   Should show: vendors, products, product_details, device_types, page_fetch_attempts, sync_sessions, sync_observed, crawling_results

4. **Device types were seeded**:
   ```bash
   sqlite3 certis_cache.db "SELECT COUNT(*) FROM device_types;"
   ```
   Should show: `35` or more

5. **App functionality works**:
   - Try crawling a few pages
   - Check that products are saved
   - Verify vendor sync works
   - Test export functionality

### 4. Restore Production Database

After testing, restore your production database:

```bash
cd /Users/chanseok/Codes/rMatterCertis
rm certis_cache.db  # Remove test database
mv certis_cache.db.backup certis_cache.db  # Restore production
```

## What to Test

### First Launch Experience
- [ ] App starts without errors
- [ ] No migration errors in console
- [ ] Database is created successfully
- [ ] All tables are present
- [ ] Seed data is loaded

### Basic Functionality
- [ ] Can perform a crawl
- [ ] Products are saved correctly
- [ ] Vendor information is stored
- [ ] Device types are available
- [ ] Search/filter works
- [ ] Export functions work

### Data Integrity
- [ ] No duplicate records
- [ ] Foreign key constraints work
- [ ] Indexes are created
- [ ] Views return correct data

## Troubleshooting

### Database already exists error
If you get an error about the database already existing:
```bash
rm certis_cache.db
```
Then restart the app.

### Migration errors
Check the Tauri console output for specific migration errors. The migration should be idempotent, so you can run it multiple times.

### Missing tables
If tables are missing, check:
1. The migration file is in the correct location: `migrations/001_baseline_consolidated.sql`
2. The app has read permissions for the migrations directory
3. Console logs for any SQL errors

## Production Deployment Testing

Before releasing a new version:

1. Test fresh installation on all supported platforms:
   - macOS (arm64 and x86_64)
   - Windows
   - Linux

2. Test upgrade path:
   - Install previous version
   - Create some test data
   - Upgrade to new version
   - Verify data is preserved

3. Test database migration:
   - Backup production database
   - Apply any new migrations
   - Verify schema version
   - Test all features

## Automation

To automate fresh install testing:

```bash
#!/bin/bash
# test_fresh_install.sh

# Backup
mv certis_cache.db certis_cache.db.backup

# Run app in background
npm run tauri dev &
APP_PID=$!

# Wait for app to initialize
sleep 10

# Verify
sqlite3 certis_cache.db "PRAGMA user_version;" | grep -q "1004" && echo "✓ Schema version correct"
sqlite3 certis_cache.db "SELECT COUNT(*) FROM device_types;" | grep -q "[0-9]\+" && echo "✓ Device types seeded"

# Cleanup
kill $APP_PID
rm certis_cache.db
mv certis_cache.db.backup certis_cache.db
```
