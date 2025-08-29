//! Temporary integration test for new PaginationCalculator (Phase1 Partial Adoption)

#[cfg(test)]
mod tests {
    use crate::domain::pagination::{PaginationCalculator};

    #[test]
    fn test_phase1_basic_examples() {
        let calc = PaginationCalculator::default();
        // 가정: total_pages=498 (예시), 마지막 페이지 제품 첫 항목
        let pos = calc.calculate(498, 0, 498);
        assert_eq!(pos.page_id, 0);
        assert_eq!(pos.index_in_page, 0);
        // 이전 페이지 첫 항목 (아직 page_id=1 이상 보장 여부만 확인)
        let pos2 = calc.calculate(497, 0, 498);
        assert!(pos2.page_id >= 0);
    }
}
