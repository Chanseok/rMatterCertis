This directory is undergoing phased cleanup.
Phase 1 (done): Removed legacy commented ServiceBasedBatchCrawlingEngine block from actor_system_commands.rs.
Phase 2 (near completion): Legacy v3/v4/service_based command files quarantined (crawling_v4.rs, modern_crawling.rs, service_based_reference.rs) – stubs ready for deletion.

2025-08-09 Baseline warnings before Strategy A: 18 (struct fields, unused methods, parameters)
Applied Strategy A steps:
- Prefixed unused simple params (_executor, _throughput_rps, etc.)
- Added #[allow(dead_code)] to legacy/transition structs & impls
- Marked Phase3 removal candidates with REMOVE_CANDIDATE comments
- Will delete stub files after final reference grep verification

Next (Phase2 finalization):
1. Physically delete stubs (modern_crawling.rs, crawling_v4.rs, service_based_reference.rs)
2. cargo check & record post-deletion warning count
3. Declare Phase2 complete; define Phase3 aggressive pruning criteria (48h stability window, metrics captured)
Next: run cargo check to ensure no feature gates reference them; then remove after warning reduction pass.
