/// 독립적인 PageIdCalculator 테스트
/// 
/// 이 파일은 메인 프로젝트와 독립적으로 PageIdCalculator 로직을 테스트합니다.

const PRODUCTS_PER_PAGE: usize = 12;

#[derive(Debug, Clone)]
pub struct PageIdCalculation {
    pub page_id: i32,
    pub index_in_page: i32,
}

#[derive(Debug, Clone)]
pub struct PageIdCalculator {
    last_page_number: u32,
    products_in_last_page: usize,
}

impl PageIdCalculator {
    pub fn new(last_page_number: u32, products_in_last_page: usize) -> Self {
        Self {
            last_page_number,
            products_in_last_page,
        }
    }
    
    pub fn calculate(&self, actual_page_number: u32, product_index_in_actual_page: usize) -> PageIdCalculation {
        // 1. 총 제품 수 계산
        let total_products = (self.last_page_number as usize - 1) * PRODUCTS_PER_PAGE + self.products_in_last_page;
        
        // 2. 현재 제품의 최신순 글로벌 0-기반 인덱스 계산
        let current_product_global_index_from_newest = (actual_page_number as usize - 1) * PRODUCTS_PER_PAGE + product_index_in_actual_page;
        
        // 3. 현재 제품의 오래된순 글로벌 0-기반 인덱스 계산
        let from_oldest_product_number = (total_products - 1) - current_product_global_index_from_newest;
        
        // 4. page_id 계산 (정수 나눗셈)
        let page_id = (from_oldest_product_number / PRODUCTS_PER_PAGE) as i32;
        
        // 5. index_in_page 계산 (나머지 - 페이지 아래쪽부터 0)
        let index_in_page = (from_oldest_product_number % PRODUCTS_PER_PAGE) as i32;
        
        PageIdCalculation {
            page_id,
            index_in_page,
        }
    }
}

fn main() {
    println!("=== PageIdCalculator 테스트 ===");
    
    // 사용자 예시: 482페이지가 마지막 페이지이고 4개 제품이 있는 경우
    let calculator = PageIdCalculator::new(482, 4);
    
    println!("\n1. 482페이지 (마지막 페이지) 테스트:");
    println!("   - 총 제품 수: {}", (482 - 1) * 12 + 4);
    
    // 482페이지의 4개 제품 테스트
    for i in 0..4 {
        let result = calculator.calculate(482, i);
        println!("   - 482페이지 {}번 제품: page_id={}, index_in_page={}", i, result.page_id, result.index_in_page);
    }
    
    println!("\n2. 481페이지 테스트:");
    // 481페이지의 첫 4개 제품 (예상: pageId=1, indexInPage=3,2,1,0)
    for i in 0..4 {
        let result = calculator.calculate(481, i);
        println!("   - 481페이지 {}번 제품: page_id={}, index_in_page={}", i, result.page_id, result.index_in_page);
    }
    
    // 481페이지의 나머지 8개 제품 (예상: pageId=0, indexInPage=11,10,9,8,7,6,5,4)
    for i in 4..12 {
        let result = calculator.calculate(481, i);
        println!("   - 481페이지 {}번 제품: page_id={}, index_in_page={}", i, result.page_id, result.index_in_page);
    }
    
    println!("\n3. 예상 결과와 비교:");
    println!("   482페이지 4개 제품 => pageId=0, indexInPage=3,2,1,0");
    println!("   481페이지 앞 4개 제품 => pageId=1, indexInPage=3,2,1,0");
    println!("   481페이지 뒤 8개 제품 => pageId=0, indexInPage=11,10,9,8,7,6,5,4");
    
    println!("\n4. 몇 개 페이지 더 테스트:");
    for page in [480, 479, 478] {
        let result = calculator.calculate(page, 0); // 첫 번째 제품만
        println!("   - {}페이지 0번 제품: page_id={}, index_in_page={}", page, result.page_id, result.index_in_page);
    }
}
