# Data Release Management

This document explains how to manage and distribute data files (JSON, Excel exports) independently from application releases.

## 📦 Release Structure

### Application Releases
- **Tag format**: `v0.8.1`, `v0.9.0`, etc.
- **Contents**: Application binaries (DMG, MSI)
- **URL**: `https://github.com/Chanseok/rMatterCertis/releases/tag/v0.8.1`

### Data Releases
- **Tag format**: `data-v1.0.0`, `data-v1.1.0`, etc.
- **Contents**: Matter device types JSON, database exports
- **URL**: `https://github.com/Chanseok/rMatterCertis/releases/tag/data-v1.0.0`

## 🚀 How to Create a Data Release

### Method 1: Automatic (Recommended)

The `data-release.yml` workflow automatically creates a release when:
1. `data/matter_device_types.json` is updated and pushed to `main`
2. Manual trigger via GitHub Actions UI

**To trigger manually:**
1. Go to **Actions** → **Data Release**
2. Click **Run workflow**
3. Enter version number (e.g., `1.0.0`)
4. Click **Run workflow**

### Method 2: Manual

1. **Prepare files:**
   ```bash
   # Export database to Excel (run in app)
   # Or use existing exports in exports/ directory
   ```

2. **Create a release:**
   ```bash
   gh release create data-v1.0.0 \
     --title "Matter Device Types Data v1.0.0" \
     --notes "75 device types included" \
     data/matter_device_types.json \
     exports/full_database_export_20251011.xlsx
   ```

3. **Or via GitHub web UI:**
   - Go to **Releases** → **Draft a new release**
   - Tag: `data-v1.0.0`
   - Title: `Matter Device Types Data v1.0.0`
   - Attach files: `matter_device_types.json`, Excel files
   - Publish

## 📥 Download URLs

### Latest Data Release
```
https://github.com/Chanseok/rMatterCertis/releases/latest/download/matter_device_types.json
```

### Specific Version
```
https://github.com/Chanseok/rMatterCertis/releases/download/data-v1.0.0/matter_device_types.json
```

### All Releases
```
https://github.com/Chanseok/rMatterCertis/releases
```

## 🔄 Update Workflow

1. **Update `data/matter_device_types.json`**
   ```bash
   # Edit the file
   nano data/matter_device_types.json
   
   # Commit and push
   git add data/matter_device_types.json
   git commit -m "data: add 5 new device types (80 total)"
   git push origin main
   ```

2. **Automatic release is created** with tag `data-vYYYYMMDD-HHMMSS`

3. **Users download new data:**
   ```bash
   curl -L -o matter_device_types.json \
     https://github.com/Chanseok/rMatterCertis/releases/download/data-latest/matter_device_types.json
   ```

## 📊 Version History

| Data Version | Device Types | Release Date | Notes |
|-------------|--------------|--------------|-------|
| data-v1.0.0 | 75           | 2025-10-11   | Initial release |
| data-v1.1.0 | 80           | TBD          | Added 5 new types |

## 🎯 Use Cases

### For End Users
- Download latest device types without reinstalling app
- Import Excel exports to restore database

### For Developers
- Use in CI/CD testing
- Reference data for validation

### For Apps (Future)
- Auto-update device types from GitHub
- Check for data updates via API

## 🔗 Integration Example

### In App (Future Feature)
```rust
// Check for updates
async fn check_data_update() -> Result<String> {
    let url = "https://api.github.com/repos/Chanseok/rMatterCertis/releases/tags/data-latest";
    // Fetch and compare version
}

// Download new data
async fn download_device_types() -> Result<()> {
    let url = "https://github.com/Chanseok/rMatterCertis/releases/latest/download/matter_device_types.json";
    // Download and import
}
```

## 📝 Notes

- Data releases are independent from app releases
- App version `v0.8.1` can use any data version (`data-v1.0.0`, `data-v1.1.0`, etc.)
- Old data versions remain available for download
- `data-latest` tag always points to the newest data release
