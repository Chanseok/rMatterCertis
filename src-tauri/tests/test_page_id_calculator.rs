use matter_certis_v2_lib::domain::pagination::CanonicalPageIdCalculator as PageIdCalculator;

/// `PageIdCalculator` smoke test via integration tests
#[test]
fn test_page_id_calculator_with_correct_values() {
    let total_pages = 100; // 총 100페이지
    let products_on_last_page = 8; // 마지막 페이지에 8개 제품

    let calculator = PageIdCalculator::new(total_pages, products_on_last_page);

    for (p, i) in &[(1, 0), (1, 11), (2, 0), (100, 0), (100, 7)] {
        let r = calculator.calculate(*p, *i);
        assert!(r.index_in_page >= 0 && r.index_in_page <= 11);
        assert!(r.page_id >= 0 && u32::try_from(r.page_id).unwrap_or(u32::MAX) <= total_pages);
    }
}
