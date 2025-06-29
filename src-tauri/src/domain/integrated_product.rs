//! 통합 제품 도메인 모델
//! 
//! 새로운 통합 스키마를 기반으로 한 제품 관련 도메인 엔티티와 로직을 포함합니다.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use chrono::{DateTime, Utc};

// Re-export types from the product module for compatibility
pub use super::product::{Product, ProductDetail, ProductWithDetails, Vendor, ProductSearchCriteria, ProductSearchResult};
pub use super::session_manager::CrawlingResult;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct IntegratedProduct {
    pub id: i64,
    pub external_id: String,
    pub name: String,
    pub category: String,
    pub subcategory: Option<String>,
    pub brand: Option<String>,
    pub model_number: Option<String>,
    pub description: Option<String>,
    pub specifications: Option<String>, // JSON 형태로 저장
    pub price_current: Option<f64>,
    pub price_original: Option<f64>,
    pub price_discount_rate: Option<f64>,
    pub currency: String,
    pub availability_status: String,
    pub stock_quantity: Option<i32>,
    pub rating_average: Option<f64>,
    pub rating_count: Option<i32>,
    pub review_count: Option<i32>,
    pub image_urls: Option<String>, // JSON 배열 형태로 저장
    pub product_url: String,
    pub vendor: String,
    pub vendor_product_code: Option<String>,
    pub tags: Option<String>, // JSON 배열 형태로 저장
    pub weight: Option<f64>,
    pub dimensions: Option<String>, // JSON 형태로 저장
    pub color: Option<String>,
    pub size: Option<String>,
    pub material: Option<String>,
    pub warranty_period: Option<String>,
    pub shipping_info: Option<String>, // JSON 형태로 저장
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub crawled_at: DateTime<Utc>,
    pub data_quality_score: Option<f64>,
    pub is_active: bool,
}

/// 통합 제품 검색 조건
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegratedProductSearchCriteria {
    pub query: Option<String>,
    pub category: Option<String>,
    pub vendor: Option<String>,
    pub price_min: Option<f64>,
    pub price_max: Option<f64>,
    pub rating_min: Option<f64>,
    pub availability_status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl Default for IntegratedProductSearchCriteria {
    fn default() -> Self {
        Self {
            query: None,
            category: None,
            vendor: None,
            price_min: None,
            price_max: None,
            rating_min: None,
            availability_status: None,
            limit: Some(50),
            offset: Some(0),
        }
    }
}

/// 데이터베이스 통계 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStatistics {
    pub total_products: i64,
    pub active_products: i64,
    pub unique_vendors: i64,
    pub unique_categories: i64,
    pub avg_rating: Option<f64>,
    pub total_reviews: i64,
    pub last_crawled: Option<DateTime<Utc>>,
    // Legacy fields for compatibility
    pub total_details: i64,
    pub unique_manufacturers: i64,
    pub unique_device_types: i64,
    pub latest_crawl_date: Option<DateTime<Utc>>,
    pub matter_products_count: i64,
    pub completion_rate: f64,
}

impl IntegratedProduct {
    /// 새로운 통합 제품 인스턴스 생성
    pub fn new(
        external_id: String,
        name: String,
        category: String,
        product_url: String,
        vendor: String,
        currency: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: 0, // 데이터베이스에서 자동 할당
            external_id,
            name,
            category,
            subcategory: None,
            brand: None,
            model_number: None,
            description: None,
            specifications: None,
            price_current: None,
            price_original: None,
            price_discount_rate: None,
            currency,
            availability_status: "unknown".to_string(),
            stock_quantity: None,
            rating_average: None,
            rating_count: None,
            review_count: None,
            image_urls: None,
            product_url,
            vendor,
            vendor_product_code: None,
            tags: None,
            weight: None,
            dimensions: None,
            color: None,
            size: None,
            material: None,
            warranty_period: None,
            shipping_info: None,
            created_at: now,
            updated_at: now,
            crawled_at: now,
            data_quality_score: None,
            is_active: true,
        }
    }

    /// 제품의 할인율 계산
    pub fn calculate_discount_rate(&self) -> Option<f64> {
        match (self.price_current, self.price_original) {
            (Some(current), Some(original)) if original > 0.0 && current < original => {
                Some((original - current) / original * 100.0)
            }
            _ => None,
        }
    }

    /// 제품이 할인 중인지 확인
    pub fn is_on_sale(&self) -> bool {
        self.calculate_discount_rate().map(|rate| rate > 0.0).unwrap_or(false)
    }

    /// 제품의 데이터 품질 점수 계산
    pub fn calculate_data_quality_score(&self) -> f64 {
        let mut score = 0.0;
        let mut max_score = 0.0;

        // 필수 필드 (각 10점)
        max_score += 30.0;
        if !self.name.is_empty() { score += 10.0; }
        if !self.category.is_empty() { score += 10.0; }
        if !self.product_url.is_empty() { score += 10.0; }

        // 중요 필드 (각 8점)
        max_score += 32.0;
        if self.price_current.is_some() { score += 8.0; }
        if self.description.is_some() && !self.description.as_ref().unwrap().is_empty() { score += 8.0; }
        if self.image_urls.is_some() { score += 8.0; }
        if self.availability_status != "unknown" { score += 8.0; }

        // 추가 정보 필드 (각 5점)
        max_score += 30.0;
        if self.brand.is_some() { score += 5.0; }
        if self.rating_average.is_some() { score += 5.0; }
        if self.specifications.is_some() { score += 5.0; }
        if self.vendor_product_code.is_some() { score += 5.0; }
        if self.weight.is_some() { score += 5.0; }
        if self.dimensions.is_some() { score += 5.0; }

        // 메타데이터 (각 2점)
        max_score += 8.0;
        if self.tags.is_some() { score += 2.0; }
        if self.color.is_some() { score += 2.0; }
        if self.size.is_some() { score += 2.0; }
        if self.material.is_some() { score += 2.0; }

        ((score / max_score * 100.0) as f64).round()
    }

    /// 제품 정보 업데이트
    pub fn update_from_crawl_data(&mut self, other: &IntegratedProduct) {
        self.name = other.name.clone();
        self.category = other.category.clone();
        self.subcategory = other.subcategory.clone();
        self.brand = other.brand.clone();
        self.description = other.description.clone();
        self.specifications = other.specifications.clone();
        self.price_current = other.price_current;
        self.price_original = other.price_original;
        self.price_discount_rate = other.calculate_discount_rate();
        self.availability_status = other.availability_status.clone();
        self.stock_quantity = other.stock_quantity;
        self.rating_average = other.rating_average;
        self.rating_count = other.rating_count;
        self.review_count = other.review_count;
        self.image_urls = other.image_urls.clone();
        self.tags = other.tags.clone();
        self.weight = other.weight;
        self.dimensions = other.dimensions.clone();
        self.color = other.color.clone();
        self.size = other.size.clone();
        self.material = other.material.clone();
        self.warranty_period = other.warranty_period.clone();
        self.shipping_info = other.shipping_info.clone();
        self.updated_at = Utc::now();
        self.crawled_at = Utc::now();
        self.data_quality_score = Some(self.calculate_data_quality_score());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_integrated_product() {
        let product = IntegratedProduct::new(
            "test123".to_string(),
            "Test Product".to_string(),
            "Electronics".to_string(),
            "https://example.com/product/123".to_string(),
            "TestVendor".to_string(),
            "USD".to_string(),
        );

        assert_eq!(product.external_id, "test123");
        assert_eq!(product.name, "Test Product");
        assert_eq!(product.category, "Electronics");
        assert_eq!(product.vendor, "TestVendor");
        assert_eq!(product.currency, "USD");
        assert_eq!(product.availability_status, "unknown");
        assert!(product.is_active);
    }

    #[test]
    fn test_calculate_discount_rate() {
        let mut product = IntegratedProduct::new(
            "test".to_string(),
            "Test".to_string(),
            "Test".to_string(),
            "https://test.com".to_string(),
            "Test".to_string(),
            "USD".to_string(),
        );

        // 할인 없음
        product.price_current = Some(100.0);
        product.price_original = Some(100.0);
        assert_eq!(product.calculate_discount_rate(), None);

        // 25% 할인
        product.price_current = Some(75.0);
        product.price_original = Some(100.0);
        assert_eq!(product.calculate_discount_rate(), Some(25.0));

        // 가격 정보 없음
        product.price_current = None;
        product.price_original = None;
        assert_eq!(product.calculate_discount_rate(), None);
    }

    #[test]
    fn test_is_on_sale() {
        let mut product = IntegratedProduct::new(
            "test".to_string(),
            "Test".to_string(),
            "Test".to_string(),
            "https://test.com".to_string(),
            "Test".to_string(),
            "USD".to_string(),
        );

        product.price_current = Some(75.0);
        product.price_original = Some(100.0);
        assert!(product.is_on_sale());

        product.price_current = Some(100.0);
        product.price_original = Some(100.0);
        assert!(!product.is_on_sale());
    }

    #[test]
    fn test_calculate_data_quality_score() {
        let mut product = IntegratedProduct::new(
            "test".to_string(),
            "Test Product".to_string(),
            "Electronics".to_string(),
            "https://test.com".to_string(),
            "Test".to_string(),
            "USD".to_string(),
        );

        // 기본 점수 (필수 필드만)
        let basic_score = product.calculate_data_quality_score();
        assert!(basic_score > 0.0);

        // 추가 정보로 점수 향상
        product.price_current = Some(100.0);
        product.description = Some("Great product".to_string());
        product.brand = Some("TestBrand".to_string());
        product.rating_average = Some(4.5);
        
        let improved_score = product.calculate_data_quality_score();
        assert!(improved_score > basic_score);
    }

    #[test]
    fn test_update_from_crawl_data() {
        let mut original = IntegratedProduct::new(
            "test".to_string(),
            "Old Name".to_string(),
            "Old Category".to_string(),
            "https://test.com".to_string(),
            "Test".to_string(),
            "USD".to_string(),
        );

        let mut updated = IntegratedProduct::new(
            "test".to_string(),
            "New Name".to_string(),
            "New Category".to_string(),
            "https://test.com".to_string(),
            "Test".to_string(),
            "USD".to_string(),
        );
        updated.price_current = Some(99.99);
        updated.description = Some("Updated description".to_string());

        let old_updated_at = original.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(1));
        
        original.update_from_crawl_data(&updated);

        assert_eq!(original.name, "New Name");
        assert_eq!(original.category, "New Category");
        assert_eq!(original.price_current, Some(99.99));
        assert_eq!(original.description, Some("Updated description".to_string()));
        assert!(original.updated_at > old_updated_at);
        assert!(original.data_quality_score.is_some());
    }

    #[test]
    fn test_search_criteria_default() {
        let criteria = IntegratedProductSearchCriteria::default();
        assert_eq!(criteria.limit, Some(50));
        assert_eq!(criteria.offset, Some(0));
        assert!(criteria.query.is_none());
    }
}
