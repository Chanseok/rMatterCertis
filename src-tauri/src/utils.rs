//! 공통 유틸리티 함수들
//! 
//! Modern Rust 2024 & Slippy 개발 권고에 따라 단일 파일로 구성
//! 이 모듈은 애플리케이션 전반에서 사용되는 공통 유틸리티를 제공합니다.

/// 페이지 ID 및 인덱스 계산 유틸리티
/// 
/// 사용자 요구사항:
/// - 가장 오래된 페이지(사이트의 마지막 페이지)의 pageId는 0
/// - 한 페이지에 12개의 제품이 있음
/// - indexInPage는 페이지 아래쪽부터 0, 1, 2, ..., 11 순서 (가장 아래쪽 제품이 0)
/// - 482페이지가 마지막 페이지이고 4개 제품이 있다면:
///   - 4개 제품은 pageId=0, indexInPage=3,2,1,0 (아래쪽부터)
/// - 481페이지는:
///   - 앞의 4개 제품: pageId=1, indexInPage=3,2,1,0
///   - 뒤의 8개 제품: pageId=0, indexInPage=11,10,9,8,7,6,5,4

const PRODUCTS_PER_PAGE: usize = 12;

/// 페이지 ID와 인덱스 계산 결과
#[derive(Debug, Clone)]
pub struct PageIdCalculation {
    pub page_id: i32,
    pub index_in_page: i32,
}

/// 페이지 ID와 인덱스를 계산하는 유틸리티
#[derive(Debug, Clone)]
pub struct PageIdCalculator {
    last_page_number: u32,
    products_in_last_page: usize,
}

impl PageIdCalculator {
    pub fn new(last_page_number: u32, products_in_last_page: usize) -> Self { Self { last_page_number, products_in_last_page } }
    pub fn calculate(&self, actual_page_number: u32, product_index_in_actual_page: usize) -> PageIdCalculation {
        // 위임: domain::pagination::PaginationCalculator 와 동일한 역방향 공식 사용.
        // NOTE: products_in_last_page 는 현재 위임 경로에서 직접 활용되지 않지만 호환성 유지.
        // 개선: 필요한 경우 last_page_number == actual_page_number 일 때 남은 제품 수 보정 로직 추가.
        let calc = crate::domain::pagination::PaginationCalculator::default();
        // PaginationCalculator 는 (physical_page, index_in_physical-from-top, total_pages) 서명을 사용 →
        // 여기서 total_pages = last_page_number, index_in_physical 는 top 기준 그대로 전달.
        let pos = calc.calculate(actual_page_number, product_index_in_actual_page as u32, self.last_page_number);
        PageIdCalculation { page_id: pos.page_id, index_in_page: pos.index_in_page }
    }
    pub fn reverse_calculate(&self, page_id: i32, index_in_page: i32) -> Option<(u32, usize)> {
        let calc = crate::domain::pagination::PaginationCalculator::default();
        calc.reverse(page_id, index_in_page, self.last_page_number).map(|(phys, idx)| (phys, idx as usize))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_page_id_calculation_example() {
        // 사용자 예시: 482페이지가 마지막 페이지이고 4개 제품이 있는 경우
        let calculator = PageIdCalculator::new(482, 4);
        
        // 482페이지의 4개 제품 테스트
        let result1 = calculator.calculate(482, 0); // 첫 번째 제품
        assert_eq!(result1.page_id, 0);
        assert_eq!(result1.index_in_page, 3);
        
        let result2 = calculator.calculate(482, 1); // 두 번째 제품
        assert_eq!(result2.page_id, 0);
        assert_eq!(result2.index_in_page, 2);
        
        let result3 = calculator.calculate(482, 2); // 세 번째 제품
        assert_eq!(result3.page_id, 0);
        assert_eq!(result3.index_in_page, 1);
        
        let result4 = calculator.calculate(482, 3); // 네 번째 제품
        assert_eq!(result4.page_id, 0);
        assert_eq!(result4.index_in_page, 0);
    }
    
    #[test]
    fn test_page_id_calculation_481_page() {
        // 481페이지 테스트
        let calculator = PageIdCalculator::new(482, 4);
        
        // 481페이지의 첫 4개 제품 (pageId=1, indexInPage=3,2,1,0)
        let result1 = calculator.calculate(481, 0);
        assert_eq!(result1.page_id, 1);
        assert_eq!(result1.index_in_page, 3);
        
        let result2 = calculator.calculate(481, 1);
        assert_eq!(result2.page_id, 1);
        assert_eq!(result2.index_in_page, 2);
        
        let result3 = calculator.calculate(481, 2);
        assert_eq!(result3.page_id, 1);
        assert_eq!(result3.index_in_page, 1);
        
        let result4 = calculator.calculate(481, 3);
        assert_eq!(result4.page_id, 1);
        assert_eq!(result4.index_in_page, 0);
        
        // 481페이지의 나머지 8개 제품 (pageId=0, indexInPage=11,10,9,8,7,6,5,4)
        let result5 = calculator.calculate(481, 4);
        assert_eq!(result5.page_id, 0);
        assert_eq!(result5.index_in_page, 11);
        
        let result6 = calculator.calculate(481, 5);
        assert_eq!(result6.page_id, 0);
        assert_eq!(result6.index_in_page, 10);
        
        let result7 = calculator.calculate(481, 11); // 마지막 제품
        assert_eq!(result7.page_id, 0);
        assert_eq!(result7.index_in_page, 4);
    }
    
    #[test]
    fn test_reverse_calculation() {
        let calculator = PageIdCalculator::new(482, 4);
        
        // 정방향 계산 후 역방향 계산으로 검증
        let result = calculator.calculate(482, 0);
        let reverse = calculator.reverse_calculate(result.page_id, result.index_in_page);
        assert_eq!(reverse, Some((482, 0)));
        
        let result = calculator.calculate(481, 4);
        let reverse = calculator.reverse_calculate(result.page_id, result.index_in_page);
        assert_eq!(reverse, Some((481, 4)));
    }
}
