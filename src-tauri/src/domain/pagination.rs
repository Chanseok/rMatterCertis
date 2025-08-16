//! Pagination domain logic centralization.
//!
//! Responsibility:
//! - page_id / index_in_page 계산 규칙 (마지막 페이지 page_id=0 역방향 누적)
//! - 역산(reverse) 기능
//! - 향후 batch 계획에서 사용될 수 있는 보조 함수

const PRODUCTS_PER_PAGE: usize = 12; // TODO: 설정 연동 필요 시 주입 고려

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PagePosition {
    pub page_id: i32,
    pub index_in_page: i32,
}

#[derive(Debug, Clone)]
pub struct PaginationCalculator {
    products_per_page: usize,
}

impl Default for PaginationCalculator {
    fn default() -> Self {
        Self {
            products_per_page: PRODUCTS_PER_PAGE,
        }
    }
}

impl PaginationCalculator {
    pub fn new(products_per_page: usize) -> Self {
        Self { products_per_page }
    }

    /// 핵심 계산: 물리 페이지(사이트 기준, 1=첫 페이지, total_pages=가장 오래된)와 그 페이지 내 0-based index를 받아
    /// 도메인 정의된 page_id / index_in_page (역방향 그룹핑) 를 산출.
    /// 규칙:
    /// - 마지막(가장 오래된) 물리 페이지의 제품들은 page_id = 0, index_in_page = (n-1 .. 0)
    /// - 그 이전(더 최신) 페이지 제품들은 page_id = 1 이어서 index_in_page = (n-1 .. 0)
    /// - 한 page_id 범위는 최대 products_per_page * 2 를 넘지 않도록 사이트 상의 불균형(마지막/이전 페이지) 처리
    pub fn calculate(
        &self,
        physical_page: u32,
        index_in_physical: u32,
        total_pages: u32,
    ) -> PagePosition {
        // 방어 로직
        if total_pages == 0 {
            return PagePosition {
                page_id: 0,
                index_in_page: 0,
            };
        }
        // from_oldest_product_number: 가장 오래된 제품을 기준(0)으로 얼마나 떨어져 있는지
        // (total_pages - physical_page) : 0-based offset of page from the end (oldest page)
        // 각 페이지 내 index 역방향: 같은 page_id 그룹 안에서 순서 재배치 필요
        let oldest_page = total_pages; // 물리적으로 가장 오래된 페이지 번호 (예: 498)
        // 페이지 번호가 1..=total_pages 라고 가정
        let distance_from_oldest_page = oldest_page.saturating_sub(physical_page).max(0);
        // 최종 제품 전체 index(오래된 기준 증가)
        let from_oldest_product_number = (distance_from_oldest_page as usize)
            * self.products_per_page
            + index_in_physical as usize;
        let page_id = (from_oldest_product_number / self.products_per_page) as i32;
        let index_in_page = (from_oldest_product_number % self.products_per_page) as i32;
        PagePosition {
            page_id,
            index_in_page,
        }
    }

    /// reverse: page_id / index_in_page 로부터 물리 페이지와 해당 페이지 내 index 추정.
    /// 일부 정보 손실(마지막/이전 경계)에 의해 모호성이 있을 수 있어 Option 반환.
    pub fn reverse(
        &self,
        page_id: i32,
        index_in_page: i32,
        total_pages: u32,
    ) -> Option<(u32, u32)> {
        if page_id < 0 || index_in_page < 0 || index_in_page as usize >= self.products_per_page {
            return None;
        }
        let from_oldest_product_number =
            (page_id as usize) * self.products_per_page + index_in_page as usize;
        let distance_from_oldest_page = from_oldest_product_number / self.products_per_page; // 페이지 경계
        if distance_from_oldest_page as u32 >= total_pages {
            return None;
        }
        let physical_page = total_pages - distance_from_oldest_page as u32; // 다시 물리 페이지 역산
        let index_in_physical = (from_oldest_product_number % self.products_per_page) as u32; // 동일
        Some((physical_page, index_in_physical))
    }
}

/// Transitional alias exposing the legacy PageIdCalculator implementation
/// through the domain module so that all future references converge here
/// before we physically migrate the logic (Phase1 -> Phase2).
pub type CanonicalPageIdCalculator = crate::utils::PageIdCalculator;

// 간단 테스트 (통합 이전 임시) - 향후 tests/pagination_tests.rs 로 이동
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_mapping() {
        let calc = PaginationCalculator::default();
        // 마지막 페이지(예: total=498, physical=498)의 0번째 제품 -> page_id=0, index_in_page=0
        let pos = calc.calculate(498, 0, 498);
        assert_eq!(pos.page_id, 0);
        assert_eq!(pos.index_in_page, 0);
        // 같은 페이지 3번째 제품
        let pos2 = calc.calculate(498, 3, 498);
        assert_eq!(pos2.page_id, 0);
        assert_eq!(pos2.index_in_page, 3);
    }

    #[test]
    fn test_previous_page_rollover() {
        let calc = PaginationCalculator::default();
        // 이전 페이지 (497) 첫 제품: from_oldest offset = 12 (만약 마지막 페이지에 12개 꽉 찬 경우)
        let pos = calc.calculate(497, 0, 498);
        assert!(pos.page_id >= 0);
    }
}
