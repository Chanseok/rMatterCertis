# DB Lock Fix: DATA_SAVING_RUN_GUARD Cleanup

## Issue Description

크롤링 탭에서 '진단 실행'을 누른 후에 수동 크롤링을 진행하면 DataSaving 단계에서 DB lock이 걸려 있어서 실패하는 문제가 발생했습니다.

## Root Cause Analysis

The issue was caused by the `DATA_SAVING_RUN_GUARD` mechanism in `src-tauri/src/crawl_engine/actors/stage_actor.rs`:

1. **Guard never cleared**: The `DATA_SAVING_RUN_GUARD` uses a global static `HashSet<String>` to prevent duplicate DataSaving operations, but guard keys were never removed after operations completed.

2. **Stale guard entries**: When diagnostics or other operations created guard entries, they remained in the HashSet indefinitely.

3. **Blocking subsequent operations**: Future DataSaving operations for the same session+batch combination were blocked by the stale guard entries.

## Solution Implemented

### 1. Guard Cleanup After Successful Operations

Added cleanup logic after successful DataSaving operations:

```rust
// Clean up DataSaving guard after successful completion
if matches!(stage_type, StageType::DataSaving) {
    let is_persist_target = matches!(lifecycle_item, StageItem::ProductDetails(_))
        || matches!(lifecycle_item, StageItem::ValidatedProducts(_));
    if is_persist_target {
        let guard_key = format!(
            "{}:{}:data_saving",
            session_id,
            batch_id.clone().unwrap_or_else(|| "none".into())
        );
        if let Ok(mut guard) = DATA_SAVING_RUN_GUARD.lock() {
            if guard.remove(&guard_key) {
                tracing::info!(target: "data_saving_diag", "[DataSaving] guard cleanup successful for key={guard_key}");
            }
        }
    }
}
```

### 2. Guard Cleanup After Failed Operations

Added cleanup logic in error paths to prevent stale guards from failed operations:

```rust
// Clean up DataSaving guard after failed completion
if matches!(stage_type, StageType::DataSaving) {
    // ... cleanup logic similar to success path
}
```

### 3. Session-Level Guard Cleanup Utility

Added utility method to clean all guards for a session:

```rust
pub fn cleanup_data_saving_guards_for_session(session_id: &str) {
    if let Ok(mut guard) = DATA_SAVING_RUN_GUARD.lock() {
        let keys_to_remove: Vec<String> = guard
            .iter()
            .filter(|key| key.starts_with(&format!("{}:", session_id)))
            .cloned()
            .collect();

        for key in keys_to_remove {
            guard.remove(&key);
            tracing::info!(target: "data_saving_diag", "[DataSaving] cleanup: removed stale guard key={key}");
        }
    }
}
```

### 4. Proactive Cleanup at Stage Start

Added guard cleanup at the start of DataSaving stage processing:

```rust
// Clean up any stale DataSaving guards for this session at the start of stage processing
if matches!(stage_type, StageType::DataSaving) {
    if let Some(ref session_id) = &context.session_id {
        Self::cleanup_data_saving_guards_for_session(session_id);
    }
}
```

## Testing

Added comprehensive unit tests to verify the guard cleanup functionality:

- `test_data_saving_guard_cleanup`: Tests basic session-level cleanup
- `test_data_saving_guard_cleanup_empty_session`: Tests cleanup on non-existent sessions
- `test_data_saving_guard_cleanup_partial_match`: Tests that only exact prefix matches are cleaned

## Impact

### Before Fix
- **Normal crawling**: Works fine initially
- **Post-diagnostics crawling**: Blocked by stale guard entries, fails at DataSaving stage

### After Fix
- **Normal crawling**: Continues to work as before
- **Post-diagnostics crawling**: Guard entries are properly cleaned up, DataSaving operations proceed normally
- **Error recovery**: Failed operations don't leave stale guard entries
- **Session isolation**: Guards for different sessions don't interfere with each other

## Files Modified

- `src-tauri/src/crawl_engine/actors/stage_actor.rs`: Added comprehensive guard cleanup mechanisms and tests

## Deployment Notes

This fix is backward compatible and doesn't require any database schema changes or configuration updates. The cleanup mechanisms are defensive and will not cause issues if called multiple times or on non-existent entries.