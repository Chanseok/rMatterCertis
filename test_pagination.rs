// 페이지네이션 계산 테스트 스크립트
use std::io::{self, Write};

#[derive(Debug, Clone)]
pub struct PaginationContext {
    pub total_pages: u32,
    pub items_per_page: u32,
    pub items_on_last_page: u32,
    pub target_page_size: u32,
}

impl PaginationContext {
    pub fn calculate_page_index(&self, current_page: u32, index_on_page: u32) -> (i32, i32) {
        // Step 1: Calculate total products
        let total_products = (self.total_pages - 1) * self.items_per_page + self.items_on_last_page;
        
        // Step 2: Calculate index from newest (0-based from page 1 item 1)
        let index_from_newest = (current_page - 1) * self.items_per_page + index_on_page;
        
        // Step 3: Calculate total index (0-based from oldest product)
        let total_index = total_products - 1 - index_from_newest;
        
        // Step 4: Calculate final pageId and indexInPage
        let page_id = total_index / self.target_page_size;
        let index_in_page = total_index % self.target_page_size;
        
        (page_id as i32, index_in_page as i32)
    }
}

fn test_pagination_examples() {
    // prompts6에서 제시한 시나리오 테스트
    let context = PaginationContext {
        total_pages: 480,
        items_per_page: 12,
        items_on_last_page: 7,
        target_page_size: 12,
    };
    
    println!("=== 페이지네이션 계산 테스트 ===");
    println!("시나리오: 총 480페이지, 마지막 페이지에 7개 제품");
    println!("전체 제품 수: {}", (context.total_pages - 1) * context.items_per_page + context.items_on_last_page);
    println!();
    
    // Test case 1: 원본 480페이지의 7번째 제품 (가장 오래된 제품)
    let (page_id, index) = context.calculate_page_index(480, 6);
    println!("1. 원본 480페이지, 7번째 제품 (index=6) -> pageId={}, indexInPage={}", page_id, index);
    assert_eq!((page_id, index), (0, 0), "가장 오래된 제품은 pageId=0, indexInPage=0이어야 함");
    
    // Test case 2: 원본 480페이지의 1번째 제품
    let (page_id, index) = context.calculate_page_index(480, 0);
    println!("2. 원본 480페이지, 1번째 제품 (index=0) -> pageId={}, indexInPage={}", page_id, index);
    assert_eq!((page_id, index), (0, 6), "480페이지 1번째 제품은 pageId=0, indexInPage=6이어야 함");
    
    // Test case 3: 원본 479페이지의 12번째 제품
    let (page_id, index) = context.calculate_page_index(479, 11);
    println!("3. 원본 479페이지, 12번째 제품 (index=11) -> pageId={}, indexInPage={}", page_id, index);
    assert_eq!((page_id, index), (0, 7), "479페이지 12번째 제품은 pageId=0, indexInPage=7이어야 함");
    
    // Test case 4: 원본 479페이지의 1번째 제품
    let (page_id, index) = context.calculate_page_index(479, 0);
    println!("4. 원본 479페이지, 1번째 제품 (index=0) -> pageId={}, indexInPage={}", page_id, index);
    assert_eq!((page_id, index), (1, 6), "479페이지 1번째 제품은 pageId=1, indexInPage=6이어야 함");
    
    // Test case 5: 원본 1페이지의 1번째 제품 (가장 최신 제품)
    let (page_id, index) = context.calculate_page_index(1, 0);
    println!("5. 원본 1페이지, 1번째 제품 (index=0) -> pageId={}, indexInPage={}", page_id, index);
    
    println!("\n✅ 모든 테스트 케이스가 통과했습니다!");
}

fn main() {
    test_pagination_examples();
}
