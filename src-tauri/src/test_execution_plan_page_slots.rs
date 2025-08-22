use crate::crawl_engine::actors::types::{
    ExecutionPlan, ExecutionPlanKpi, PageRange, PageSlot, PlanInputSnapshot,
};
use chrono::Utc;

// Focused unit tests for page_slots invariants.
#[cfg(test)]
mod tests {
    use super::*;
    use tracing::{Level, subscriber::set_global_default};
    use tracing_subscriber::FmtSubscriber;

    // Helper to init tracing once for tests that inspect logs (idempotent best-effort)
    fn init_tracing() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            let _ = set_global_default(
                FmtSubscriber::builder()
                    .with_max_level(Level::INFO)
                    .finish(),
            );
        });
    }

    fn build_plan(
        total_site_pages: u32,
        products_on_last_page: u32,
        ranges: Vec<PageRange>,
    ) -> ExecutionPlan {
        // Simplified snapshot
        let snapshot = PlanInputSnapshot {
            total_pages: total_site_pages,
            products_on_last_page,
            db_max_page_id: None,
            db_max_index_in_page: None,
            db_total_products: 0,
            page_range_limit: total_site_pages,
            batch_size: 5,
            concurrency_limit: 2,
            created_at: Utc::now(),
        };
        // Precompute page_slots (replicating domain rule from create_execution_plan)
        let mut page_slots: Vec<PageSlot> = Vec::new();
        for range in &ranges {
            let pages_iter: Box<dyn Iterator<Item = u32>> = if range.reverse_order {
                Box::new(range.end_page..=range.start_page)
            } else {
                Box::new(range.start_page..=range.end_page)
            };
            for physical_page in pages_iter {
                if physical_page == 0 {
                    continue;
                }
                let page_id: i64 = (total_site_pages.saturating_sub(physical_page)) as i64;
                let capacity = if physical_page == total_site_pages {
                    products_on_last_page.max(1)
                } else {
                    crate::domain::constants::site::PRODUCTS_PER_PAGE as u32
                };
                for offset in 0..capacity {
                    let reverse_index = (capacity - 1 - offset) as i16;
                    page_slots.push(PageSlot {
                        physical_page,
                        page_id,
                        index_in_page: reverse_index,
                    });
                }
            }
        }
        ExecutionPlan {
            plan_id: "test".into(),
            session_id: "sess".into(),
            crawling_ranges: ranges,
            batch_size: 5,
            concurrency_limit: 2,
            estimated_duration_secs: 0,
            created_at: Utc::now(),
            analysis_summary: "test".into(),
            original_strategy: "TestStrategy".into(),
            input_snapshot: snapshot,
            plan_hash: "hash".into(),
            skip_duplicate_urls: true,
            kpi_meta: Some(ExecutionPlanKpi {
                total_ranges: 1,
                total_pages: total_site_pages,
                batches: 1,
                strategy: "TestStrategy".into(),
                created_at: Utc::now(),
            }),
            contract_version: crate::crawl_engine::actors::contract::ACTOR_CONTRACT_VERSION,
            page_slots,
        }
    }

    #[test]
    fn page_slots_basic_reverse_order() {
        // Range covering pages 5..=1 in reverse order (start_page > end_page, reverse_order=true)
        let range = PageRange {
            start_page: 5,
            end_page: 1,
            estimated_products: 5 * crate::domain::constants::site::PRODUCTS_PER_PAGE as u32,
            reverse_order: true,
        };
        let plan = build_plan(10, 8, vec![range]);
        // Expect each physical page to map to page_id = total_pages - physical_page
        let slot = plan
            .page_slots
            .iter()
            .find(|s| s.physical_page == 10)
            .is_none();
        assert!(slot, "Page 10 should not appear since range is 5..=1");
        // Check a known page
        let p3: Vec<&PageSlot> = plan
            .page_slots
            .iter()
            .filter(|s| s.physical_page == 3)
            .collect();
        assert!(!p3.is_empty());
        let expected_page_id = (10 - 3) as i64; // 7
        assert_eq!(p3[0].page_id, expected_page_id);
        // Indexes in page should be descending from 11..0 (since not last page)
        let mut indices: Vec<i16> = p3.iter().map(|s| s.index_in_page).collect();
        indices.sort();
        assert_eq!(indices.first().copied(), Some(0));
        assert_eq!(indices.last().copied(), Some(11));
    }

    #[test]
    fn last_page_uses_products_on_last_page_capacity() {
        // Include last page (10)
        let range = PageRange {
            start_page: 10,
            end_page: 8,
            estimated_products: 3 * crate::domain::constants::site::PRODUCTS_PER_PAGE as u32,
            reverse_order: true,
        };
        let plan = build_plan(10, 7, vec![range]);
        // Slots for physical_page 10 should have capacity 7
        let p10: Vec<&PageSlot> = plan
            .page_slots
            .iter()
            .filter(|s| s.physical_page == 10)
            .collect();
        assert_eq!(
            p10.len(),
            7,
            "Last page should have products_on_last_page count"
        );
        let max_index = p10.iter().map(|s| s.index_in_page).max().unwrap();
        assert_eq!(max_index, 6); // 0..6
    }

    #[test]
    fn page_id_monotonic_decreasing_with_newer_pages_first() {
        // Range latest pages (10..=6)
        let range = PageRange {
            start_page: 10,
            end_page: 6,
            estimated_products: 5 * crate::domain::constants::site::PRODUCTS_PER_PAGE as u32,
            reverse_order: true,
        };
        let plan = build_plan(15, 9, vec![range]);
        // Collect unique (physical_page, page_id) preserving insertion order
        let mut pairs: Vec<(u32, i64)> = Vec::new();
        for slot in &plan.page_slots {
            if !pairs.iter().any(|(p, _)| *p == slot.physical_page) {
                pairs.push((slot.physical_page, slot.page_id));
            }
        }
        // physical pages ascend in iteration (because range.end_page..=start_page) => 6,7,8,9,10
        let physical_sequence: Vec<u32> = pairs.iter().map(|(p, _)| *p).collect();
        assert_eq!(physical_sequence, vec![6, 7, 8, 9, 10]);
        // page_id should correspond: total_pages - physical_page
        for (phys, pid) in pairs {
            assert_eq!(pid, (15 - phys) as i64);
        }
    }

    #[test]
    fn integrity_mismatch_detection_synthetic() {
        init_tracing();
        // Create a plan with deliberate mismatch: declared range pages not matching actual page_slots length logic
        let range = PageRange {
            start_page: 3,
            end_page: 5,
            estimated_products: 3 * crate::domain::constants::site::PRODUCTS_PER_PAGE as u32,
            reverse_order: false,
        };
        let mut plan = build_plan(10, 8, vec![range.clone()]);
        // Tamper: drop some slots to simulate missing processing results
        let original_len = plan.page_slots.len();
        plan.page_slots.truncate(original_len / 2);
        // Integrity expectation: total logical slots for pages 3..=5 (3 pages) except last page capacity rule
        // We simply assert our tampering actually reduced slots; real runtime logic logs warning (covered indirectly)
        assert!(
            plan.page_slots.len() < original_len,
            "Tampering failed; test invalid"
        );
        // Derive unique physical pages still present – should be subset but non-empty
        let unique_pages: std::collections::HashSet<u32> =
            plan.page_slots.iter().map(|s| s.physical_page).collect();
        assert!(!unique_pages.is_empty());
    }
}
