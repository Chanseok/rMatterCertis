This directory is undergoing phased cleanup.
Phase 1 (done): Removed legacy commented ServiceBasedBatchCrawlingEngine block from actor_system_commands.rs.
Phase 2 (in progress): Legacy v3/v4/service_based command files quarantined (*.legacy) pending final removal (crawling_v4.rs, modern_crawling.rs, service_based_reference.rs).
Next: run cargo check to ensure no feature gates reference them; then remove after warning reduction pass.
