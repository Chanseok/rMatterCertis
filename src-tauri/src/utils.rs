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
    /// 새로운 계산기 생성
    /// 
    /// # Arguments
    /// * `last_page_number` - 사이트의 마지막 페이지 번호 (가장 오래된 페이지)
    /// * `products_in_last_page` - 마지막 페이지의 제품 수
    pub fn new(last_page_number: u32, products_in_last_page: usize) -> Self {
        Self {
            last_page_number,
            products_in_last_page,
        }
    }
    
    /// 실제 페이지 번호와 페이지 내 제품 인덱스를 기반으로 pageId와 indexInPage 계산
    /// 
    /// prompts5에서 제시된 간결한 공식 사용:
    /// 1. total_products = (last_page_number - 1) * PRODUCTS_PER_PAGE + products_in_last_page
    /// 2. current_product_global_index_from_newest = (actual_page_number - 1) * PRODUCTS_PER_PAGE + product_index_in_actual_page
    /// 3. from_oldest_product_number = (total_products - 1) - current_product_global_index_from_newest
    /// 4. page_id = from_oldest_product_number / PRODUCTS_PER_PAGE
    /// 5. index_in_page = from_oldest_product_number % PRODUCTS_PER_PAGE
    /// 
    /// # Arguments
    /// * `actual_page_number` - 사이트의 실제 페이지 번호 (1부터 시작)
    /// * `product_index_in_actual_page` - 해당 페이지에서 제품의 인덱스 (0부터 시작, 위에서부터)
    /// 
    /// # Returns
    /// PageIdCalculation 구조체
    pub fn calculate(&self, actual_page_number: u32, product_index_in_actual_page: usize) -> PageIdCalculation {
        // 경계값 보정 및 방어적 계산으로 언더플로우/오버플로우 방지
        // 0 페이지가 들어오면 1로 보정, 마지막 페이지보다 큰 값이 들어오면 경고 후 마지막 페이지로 보정
        let mut page = if actual_page_number == 0 { 1 } else { actual_page_number };
        if page > self.last_page_number {
            tracing::warn!(
                "⚠️ PageIdCalculator: actual_page_number ({}) > last_page_number ({}). Clamping to last_page_number.",
                page, self.last_page_number
            );
            page = self.last_page_number;
        }

        // 페이지 내 인덱스는 0..12-1 범위로 보정 (과대 입력 시 상한 클램프)
        let bounded_index = if product_index_in_actual_page >= PRODUCTS_PER_PAGE {
            tracing::warn!(
                "⚠️ PageIdCalculator: product_index_in_actual_page ({}) >= {}. Clamping to {}.",
                product_index_in_actual_page, PRODUCTS_PER_PAGE, PRODUCTS_PER_PAGE - 1
            );
            PRODUCTS_PER_PAGE - 1
        } else {
            product_index_in_actual_page
        };

        // 1. 총 제품 수 계산 (saturating 사용)
        let pages_except_last = self.last_page_number.saturating_sub(1) as usize;
        let total_products = pages_except_last
            .saturating_mul(PRODUCTS_PER_PAGE)
            .saturating_add(self.products_in_last_page.min(PRODUCTS_PER_PAGE));
        
        // 2. 현재 제품의 최신순 글로벌 0-기반 인덱스 계산 (saturating 사용)
        let current_product_global_index_from_newest = (page.saturating_sub(1) as usize)
            .saturating_mul(PRODUCTS_PER_PAGE)
            .saturating_add(bounded_index);
        
        // 3. 현재 제품의 오래된순 글로벌 0-기반 인덱스 계산 (saturating 사용)
        let from_oldest_product_number = total_products
            .saturating_sub(1)
            .saturating_sub(current_product_global_index_from_newest);
        
        // 4. page_id 계산 (정수 나눗셈)
        let page_id = (from_oldest_product_number / PRODUCTS_PER_PAGE) as i32;
        
        // 5. index_in_page 계산 (나머지 - 페이지 아래쪽부터 0)
        let index_in_page = (from_oldest_product_number % PRODUCTS_PER_PAGE) as i32;
        
        PageIdCalculation { page_id, index_in_page }
    }
    
    /// 특정 pageId와 indexInPage로부터 실제 페이지 번호와 인덱스 역계산
    /// 
    /// # Arguments
    /// * `page_id` - 계산된 페이지 ID
    /// * `index_in_page` - 계산된 페이지 내 인덱스
    /// 
    /// # Returns
    /// (actual_page_number, product_index_in_actual_page) 튜플
    pub fn reverse_calculate(&self, page_id: i32, index_in_page: i32) -> Option<(u32, usize)> {
        if page_id < 0 || index_in_page < 0 || index_in_page >= PRODUCTS_PER_PAGE as i32 {
            return None;
        }
        
        // 1. from_oldest_product_number 역계산
        let from_oldest_product_number = (page_id as usize)
            .saturating_mul(PRODUCTS_PER_PAGE)
            .saturating_add(index_in_page as usize);
        
        // 2. 총 제품 수 계산
        let pages_except_last = self.last_page_number.saturating_sub(1) as usize;
        let total_products = pages_except_last
            .saturating_mul(PRODUCTS_PER_PAGE)
            .saturating_add(self.products_in_last_page.min(PRODUCTS_PER_PAGE));
        
        // 3. current_product_global_index_from_newest 역계산
        let current_product_global_index_from_newest = total_products
            .saturating_sub(1)
            .saturating_sub(from_oldest_product_number);
        
        // 4. actual_page_number 및 product_index_in_actual_page 역계산
        let actual_page_number = (current_product_global_index_from_newest / PRODUCTS_PER_PAGE) + 1;
        let product_index_in_actual_page = current_product_global_index_from_newest % PRODUCTS_PER_PAGE;
        
        Some((actual_page_number as u32, product_index_in_actual_page))
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
