# Test Best Practices for rMatterCertis

This guide outlines best practices for testing in the rMatterCertis project, particularly focusing on database testing and avoiding common pitfalls with file-based state.

## Core Testing Principles

### 1. Always Use In-Memory Databases for Tests

**❌ Don't do this:**
```rust
// This creates file-based state that can cause test interference
let db = DatabaseConnection::new("sqlite:./data/test.db").await?;
```

**✅ Do this instead:**
```rust
// This creates isolated in-memory database for each test
let db = DatabaseConnection::new("sqlite::memory:").await?;
```

### 2. Use Test Utilities for Consistency

The project provides `TestContext` and `TestDatabase` utilities for consistent test setup:

```rust
use matter_certis_v2_lib::test_utils::TestContext;

#[tokio::test]
async fn test_vendor_creation() {
    let ctx = TestContext::new().await.unwrap();
    
    // All repositories and use cases are pre-configured
    let vendor = ctx.vendor_use_cases.create_vendor(CreateVendorDto {
        vendor_number: 123,
        vendor_name: "Test Vendor".to_string(),
        company_legal_name: "Test Corp".to_string(),
    }).await.unwrap();
    
    assert_eq!(vendor.vendor_name, "Test Vendor");
}
```

### 3. Test Isolation Guarantees

Each test should be completely isolated:

- **✅ Fresh database state**: Every test gets a new in-memory database
- **✅ No shared state**: Tests cannot interfere with each other
- **✅ No cleanup needed**: In-memory databases are automatically cleaned up
- **✅ Fast execution**: No file I/O overhead

## Available Test Binaries

### Production-Like Testing
- `test_db.rs` - Full integration test with all components
- `test_db_new.rs` - Complete integration test (updated version)

### Fast Development Testing  
- `test_minimal.rs` - Minimal dependencies for quick testing
- `test_db_light.rs` - Lightweight test for core functionality
- `test_db_fast.rs` - Ultra-fast test execution
- `test_with_utils.rs` - Example using new test utilities

### Session Management Testing
- `test_session_management.rs` - Tests in-memory session management and result persistence

## Running Tests

### Quick Tests (Recommended for Development)
```bash
# Minimal test with fastest compilation
cargo run --bin test_minimal

# Light integration test
cargo run --bin test_db_light

# Test utilities example
cargo run --bin test_with_utils
```

### Full Integration Tests
```bash
# Complete integration test
cargo run --bin test_db_new

# Session management test
cargo run --bin test_session_management
```

### Unit Tests
```bash
# Run all unit tests
cargo test

# Run tests with test utilities feature
cargo test --features test-utils
```

## Database Testing Architecture

### In-Memory Session Management
- All session state is kept in memory using `SessionManager`
- No database persistence of session state
- Fast and reliable session operations

### Persistent Result Storage
- Only final crawling results are saved to database
- Uses `CrawlingResultRepository` for persistence
- Clean separation between session state and results

### Test Database Setup
```rust
// Automatic setup with TestContext
let ctx = TestContext::new().await?;

// Manual setup for specific needs
let db = TestDatabase::new().await?;
let pool = db.pool();
let repo = SqliteVendorRepository::new(pool);
```

## Common Testing Patterns

### Testing Repository Operations
```rust
#[tokio::test]
async fn test_vendor_repository() {
    let ctx = TestContext::new().await.unwrap();
    
    let vendor = Vendor {
        id: 0,
        vendor_number: 123,
        vendor_name: "Test".to_string(),
        company_legal_name: "Test Corp".to_string(),
        created_at: Utc::now().naive_utc(),
        updated_at: Utc::now().naive_utc(),
    };
    
    let saved = ctx.vendor_repo.create(vendor).await.unwrap();
    assert!(saved.id > 0);
}
```

### Testing Use Cases
```rust
#[tokio::test]
async fn test_vendor_use_case() {
    let ctx = TestContext::new().await.unwrap();
    
    let result = ctx.vendor_use_cases.create_vendor(CreateVendorDto {
        vendor_number: 123,
        vendor_name: "Test".to_string(),
        company_legal_name: "Test Corp".to_string(),
    }).await;
    
    assert!(result.is_ok());
}
```

### Testing Session Management
```rust
#[tokio::test]
async fn test_session_lifecycle() {
    let ctx = TestContext::new().await.unwrap();
    
    // Start session
    ctx.session_manager.start_session(
        "test-123", 
        "https://example.com", 
        vec!["example.com".to_string()]
    ).await.unwrap();
    
    // Update progress
    ctx.session_manager.update_progress(
        "test-123", 
        50, 
        "Processing...".to_string()
    ).await.unwrap();
    
    // Complete session
    ctx.session_manager.complete_session("test-123").await.unwrap();
    
    // Verify session is completed
    let status = ctx.session_manager.get_session_status("test-123").await.unwrap();
    assert!(status.is_some());
}
```

## Troubleshooting

### Database Lock Issues
If you encounter database lock issues:
1. **Check for file-based databases**: All tests should use `sqlite::memory:`
2. **Run cleanup script**: `./scripts/clean-test-db.sh`
3. **Restart development**: Sometimes file handles need to be released

### Test Interference
If tests are interfering with each other:
1. **Verify isolation**: Each test should create its own `TestContext`
2. **Check for shared state**: Avoid global variables or static state
3. **Use in-memory databases**: Never use file-based databases in tests

### Slow Test Execution
If tests are running slowly:
1. **Use minimal test binaries**: `test_minimal.rs`, `test_db_light.rs`
2. **Enable fast compilation**: Project already configured for fast dev builds
3. **Run specific tests**: Use `cargo run --bin test_minimal` instead of full test suite

## Cleanup and Maintenance

### Automatic Cleanup
- In-memory databases are automatically cleaned up when tests finish
- No manual cleanup required for normal testing

### Manual Cleanup (If Needed)
```bash
# Clean up any legacy test files
./scripts/clean-test-db.sh

# Clean cargo cache if needed
cargo clean
```

### File Structure
```
src-tauri/
├── src/
│   ├── test_utils.rs           # Test utilities and helpers
│   └── bin/
│       ├── test_minimal.rs     # Minimal fast test
│       ├── test_with_utils.rs  # Test utilities example
│       └── test_*.rs           # Other test binaries
├── scripts/
│   └── clean-test-db.sh        # Cleanup script
└── Cargo.toml                  # Test features and profiles
```

This architecture ensures that tests are fast, reliable, and isolated, preventing the common issues associated with file-based database testing.
