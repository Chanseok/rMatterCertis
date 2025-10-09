# unwrap()/expect() Audit Report
**Date**: 2025-10-09  
**Total Count**: 220 instances (excluding tests/)

## Top Files

| File | Count | Priority | Notes |
|------|-------|----------|-------|
| infrastructure/logging.rs | 29 | Low | Initialization code, panic acceptable |
| infrastructure/html_parser.rs | 28 | Medium | Mostly compile-time regexes, some runtime data |
| infrastructure/integrated_product_repository.rs | 16 | High | Database operations |
| crawl_engine/actor_event_bridge.rs | 15 | High | Actor system critical path |
| crawl_engine/actors/types.rs | 13 | Medium | Mostly test code |
| commands/crawling/actor_system.rs | 12 | High | Command handlers |
| metrics.rs | 11 | Medium | Metrics collection |
| infrastructure/simple_http_client.rs | 10 | Medium | HTTP client wrapper |
| crawl_engine/stages/strategies/default.rs | 10 | Low | Test setup code |
| domain/session_manager.rs | 7 | High | Session lifecycle |

## Analysis

### Low Priority (Acceptable Panic)
- **logging.rs**: Initialization code - if logging setup fails, app can't start
- **database_paths.rs**: `global()` method - documented panic, by design
- **html_parser.rs**: Regex compilation - compile-time constants
- **Test code**: All `expect()` in test functions are acceptable

### Medium Priority (Should Fix Eventually)
- **html_parser.rs**: Line 756 - runtime data extraction (`captures.get(1).unwrap()`)
- **metrics.rs**: Metrics collection should be resilient
- **simple_http_client.rs**: HTTP operations should return Result

### High Priority (Fix Now)
- **integrated_product_repository.rs**: Database operations must handle errors
- **actor_event_bridge.rs**: Actor system critical path
- **commands/crawling/actor_system.rs**: Command handlers need proper error propagation
- **domain/session_manager.rs**: Session lifecycle errors

## Recommendations

1. **Phase 2.1a** (Current): Audit and categorize all unwrap()/expect()
2. **Phase 2.1b** (Week 2): Fix HIGH priority files (4 files, ~50 instances)
3. **Phase 2.1c** (Week 3): Fix MEDIUM priority files (~50 instances)
4. **Phase 2.1d** (Week 4): Add lint rule to prevent new unwrap() in production code

## Current Status

- ✅ Audit complete (220 instances found)
- ⏳ Categorization in progress
- ⏸️ Remediation pending

## Next Steps

1. Focus on HIGH priority files first
2. Use `anyhow::Context` for error propagation
3. Add `.context("descriptive error message")` to all `?` operators
4. Update REFACTORING_MASTER_PLAN.md with realistic timeline
