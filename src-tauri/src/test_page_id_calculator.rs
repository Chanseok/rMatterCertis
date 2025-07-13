use crate::utils::PageIdCalculator;

/// PageIdCalculator 테스트
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_id_calculator_with_correct_values() {
        // 실제 사이트 분석 결과를 시뮬레이션
        let total_pages = 100;  // 총 100페이지
        let products_on_last_page = 8;  // 마지막 페이지에 8개 제품
        
        let calculator = PageIdCalculator::new(total_pages, products_on_last_page);
        
        // 페이지 1, 인덱스 0 (첫 번째 제품)
        let result1 = calculator.calculate(1, 0);
        println!("Page 1, Index 0: page_id={}, index_in_page={}", result1.page_id, result1.index_in_page);
        
        // 페이지 1, 인덱스 11 (12번째 제품)
        let result2 = calculator.calculate(1, 11);
        println!("Page 1, Index 11: page_id={}, index_in_page={}", result2.page_id, result2.index_in_page);
        
        // 페이지 2, 인덱스 0 (두 번째 페이지 첫 번째 제품)
        let result3 = calculator.calculate(2, 0);
        println!("Page 2, Index 0: page_id={}, index_in_page={}", result3.page_id, result3.index_in_page);
        
        // 페이지 100, 인덱스 0 (마지막 페이지 첫 번째 제품)
        let result4 = calculator.calculate(100, 0);
        println!("Page 100, Index 0: page_id={}, index_in_page={}", result4.page_id, result4.index_in_page);
        
        // 페이지 100, 인덱스 7 (마지막 페이지 마지막 제품)
        let result5 = calculator.calculate(100, 7);
        println!("Page 100, Index 7: page_id={}, index_in_page={}", result5.page_id, result5.index_in_page);
        
        // 모든 값이 0이 아닌지 확인
        assert_ne!(result1.page_id, 0);
        assert_ne!(result2.page_id, 0);
        assert_ne!(result3.page_id, 0);
        assert_ne!(result4.page_id, 0);
        assert_ne!(result5.page_id, 0);
    }
    
    #[test]
    fn test_page_id_calculator_old_vs_new() {
        // 이전 잘못된 방식 (start_page = 1, products_in_last_page = 4)
        let old_calculator = PageIdCalculator::new(1, 4);
        let old_result = old_calculator.calculate(1, 0);
        println!("Old method: page_id={}, index_in_page={}", old_result.page_id, old_result.index_in_page);
        
        // 새로운 올바른 방식 (total_pages = 100, products_on_last_page = 8)
        let new_calculator = PageIdCalculator::new(100, 8);
        let new_result = new_calculator.calculate(1, 0);
        println!("New method: page_id={}, index_in_page={}", new_result.page_id, new_result.index_in_page);
        
        // 새로운 방식이 0이 아닌 의미 있는 값을 생성하는지 확인
        assert_ne!(new_result.page_id, 0);
        assert_ne!(new_result.index_in_page, 0);
    }
}

/// 통합 테스트를 위한 public 함수
pub fn test_page_id_calculator() {
    println!("=== PageIdCalculator 테스트 시작 ===");
    
    // 실제 사이트 분석 결과를 시뮬레이션
    let total_pages = 100;
    let products_on_last_page = 8;
    
    let calculator = PageIdCalculator::new(total_pages, products_on_last_page);
    
    println!("총 페이지 수: {}, 마지막 페이지 제품 수: {}", total_pages, products_on_last_page);
    
    // 여러 페이지와 인덱스 조합 테스트
    let test_cases = vec![
        (1, 0),   // 첫 번째 페이지, 첫 번째 제품
        (1, 11),  // 첫 번째 페이지, 마지막 제품
        (2, 0),   // 두 번째 페이지, 첫 번째 제품
        (50, 5),  // 중간 페이지, 중간 제품
        (100, 0), // 마지막 페이지, 첫 번째 제품
        (100, 7), // 마지막 페이지, 마지막 제품
    ];
    
    for (page, index) in test_cases {
        let result = calculator.calculate(page, index);
        println!("페이지 {}, 인덱스 {} -> page_id: {}, index_in_page: {}", 
                 page, index, result.page_id, result.index_in_page);
        
        // 모든 값이 0이 아닌지 확인
        if result.page_id == 0 || result.index_in_page == 0 {
            println!("❌ 경고: page_id 또는 index_in_page가 0입니다!");
        } else {
            println!("✅ 정상적인 값이 계산되었습니다.");
        }
    }
    
    println!("=== PageIdCalculator 테스트 완료 ===");
}
