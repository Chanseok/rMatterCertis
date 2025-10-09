# Development Phase Migrations Archive

This directory contains the incremental migrations used during the development phase.
These files are preserved for historical reference but are not used in production.

## Timeline

- **001_baseline.sql** - Initial baseline schema
- **003-019_*.sql** - Development phase incremental migrations

## Production Schema

The production schema is consolidated into a single baseline file:
- `/migrations/002_baseline_cleaned.sql` - Current production baseline (v2.0)

## Archived Migrations Summary

These migrations were consolidated and cleaned in v2.0:

1. **Removed Tables**:
   - `sync_sessions` - 타입 불일치로 실제 동작 안 함
   - `sync_observed` - 외래 키 제약으로 실패
   - `crawling_results` - 사용처 없음
   - `page_fetch_attempts` - 선택적 디버깅 기능 (기본 비활성화)

2. **Removed Columns**:
   - `product_details.application_categories` - 파싱 로직 없음, 항상 NULL

3. **Retained Features**:
   - Core tables: vendors, products, product_details, device_types
   - Indexes for query optimization
   - Analytics view: v_product_detail_analytics
   - Device type seed data

## Notes

- Do not modify files in this archive
- For schema changes, edit `/migrations/002_baseline_cleaned.sql`
- These files are git-tracked for historical reference only
