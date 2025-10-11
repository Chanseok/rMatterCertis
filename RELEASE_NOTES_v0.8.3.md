# Release Notes - v0.8.3

**Release Date:** 2025-01-12

## 🎯 Major Improvements: Device Types Management

This release focuses on comprehensive improvements to the Device Types management feature, fixing critical bugs and establishing a proper data architecture.

### ✨ New Features

#### Device Types Management UI
- **📋 Complete Table Interface**: Browse, search, filter, and manage 75+ Matter device types
- **➕ Add/Edit/Delete Operations**: Direct DB manipulation for all CRUD operations
- **📤 Export JSON**: Export device types to exports/ directory
- **📥 Import JSON**: File picker dialog for easy JSON imports
- **🔍 Search & Filter**: Real-time search by name, hex, category
- **🎨 Clean UI**: Removed confusing ID column (internal AUTOINCREMENT)

### 🐛 Bug Fixes

#### Critical Architecture Fixes
- **Fixed Device Types → Product Details JOIN**: 
  - Properly map JSON `id` ↔ DB `type_id` (Matter spec ID)
  - Analytics view now correctly joins on `type_id`
  - Device Types and Category filters working again
  
- **Fixed Duplicate Import Issue**:
  - Changed from ID-based to `code_hex`-based duplicate checking
  - Prevents duplicate records on repeated imports
  - Proper UPDATE vs INSERT logic

- **Fixed App Restart on Save**:
  - Moved all write operations to `exports/` directory
  - Stopped writing to bundled `data/` directory
  - Export JSON and Save operations no longer crash app

- **Fixed UI Not Loading Data**:
  - Removed incorrect `getAllDeviceTypesFromDb()` dependency
  - Changed `initDeviceTypes()` to use DB directly
  - Proper `type_id` parsing for display

#### Excel Export/Import
- **Removed `application_categories` Column**:
  - Column doesn't exist in migration schema
  - Adjusted column count validation (22 → 21)
  - Fixed index references for `created_at`/`updated_at`

### 🏗️ Architecture Changes

#### Data Flow Redesign
**Before (Broken):**
```
UI → JSON file → save → DB (maybe)
     ↓
   Source of Truth unclear
```

**After (Fixed):**
```
UI → DB (direct operations) → UI refresh
     ↑
  Single Source of Truth
```

#### Proper ID Mapping
- **JSON `id`**: Matter specification Device Type ID (256, 257, ...)
- **DB `type_id`**: TEXT column storing Matter spec ID
- **DB `id`**: AUTOINCREMENT internal ID (hidden from UI)

### 📊 Technical Details

#### Modified Backend Commands
- `export_device_types_from_db()`: Uses `type_id` instead of `id`
- `import_device_types_to_db()`: Maps JSON `id` → DB `type_id`
- `get_all_device_types_from_db()`: Returns `type_id` as `id`
- `add_device_type_to_db()`: Stores to `type_id`, checks `code_hex` duplicates
- `update_device_type_in_db()`: Updates by `type_id`
- `delete_device_types_from_db()`: Deletes by `type_id`

#### UI Improvements
- Portal-based dialogs for better rendering
- File picker integration with Tauri dialog API
- Removed confusing auto-increment ID from table
- Proper error messages and validation

### 📦 Files Changed
- `src-tauri/src/commands/database/device_types_editor.rs`
- `src-tauri/src/commands/database/export_import.rs`
- `src/components/DeviceTypeTableEditor.tsx`
- `src/stores/localDbDashboardStore.ts`
- `src/services/tauri-api.ts`
- `package.json`, `Cargo.toml`, `tauri.conf.json` (version bump)

### 🔄 Migration Guide

**For Users with Existing Data:**

1. **Backup your database** before upgrading
2. **Delete existing device_types records** (they have wrong mapping)
3. **Import JSON** from bundled `data/matter_device_types.json` (75 types)
4. **Verify**: Device Types and Category filters should now work

**SQL to clean old data:**
```sql
DELETE FROM device_types;
```

Then use "Import JSON" in the Device Types tab.

### 🎉 Result

Device Types management is now fully functional:
- ✅ 75 Matter device types properly loaded
- ✅ Analytics filters working (Device Types, Categories)
- ✅ No more app crashes on save/export
- ✅ Proper duplicate prevention
- ✅ Clean, intuitive UI

---

## Previous Releases
- [v0.8.1](RELEASE_NOTES_v0.8.1.md) - Initial Device Types feature
