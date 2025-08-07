use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};

use crate::domain::integrated_product::ProductDetail;
use crate::infrastructure::IntegratedProductRepository;

/// 데이터 품질 분석 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataQualityReport {
    pub total_crawled: u32,
    pub total_existing: u32,
    pub new_products: u32,
    pub updated_products: u32,
    pub duplicate_products: u32,
    pub incomplete_products: u32,
    pub quality_score: f64, // 0.0-1.0
    pub field_completeness: FieldCompletenessReport,
    pub recommendations: Vec<String>,
}

/// 필드별 완정성 보고서
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldCompletenessReport {
    pub product_name: f64,
    pub manufacturer: f64,
    pub model_name: f64,
    pub certification_id: f64,
    pub matter_version: f64,
    pub device_type: f64,
    pub protocol_support: f64,
    pub overall_completeness: f64,
}

/// 데이터 품질 분석기
pub struct DataQualityAnalyzer {
    repository: std::sync::Arc<IntegratedProductRepository>,
}

impl DataQualityAnalyzer {
    pub fn new(repository: std::sync::Arc<IntegratedProductRepository>) -> Self {
        Self { repository }
    }

    /// 크롤링된 데이터 vs 기존 데이터 품질 비교 분석
    pub async fn analyze_crawled_data(
        &self,
        crawled_products: &[ProductDetail],
        pages_range: &str,
    ) -> Result<DataQualityReport, Box<dyn std::error::Error + Send + Sync>> {
        info!("🔍 Starting data quality analysis for {} products from pages {}", 
              crawled_products.len(), pages_range);

        // 1. 기존 데이터베이스에서 동일 범위 데이터 조회
        let existing_products = self.get_existing_products_for_comparison().await?;
        
        // 2. 중복 및 새로운 제품 분석
        let (new_products, updated_products, duplicates) = 
            self.categorize_products(crawled_products, &existing_products).await;

        // 3. 필드 완정성 분석
        let field_completeness = self.analyze_field_completeness(crawled_products);

        // 4. 데이터 품질 점수 계산
        let quality_score = self.calculate_quality_score(&field_completeness, crawled_products);

        // 5. 불완전한 제품 식별
        let incomplete_products = self.identify_incomplete_products(crawled_products);

        // 6. 개선 권장사항 생성
        let recommendations = self.generate_recommendations(
            &field_completeness, 
            quality_score,
            incomplete_products.len() as u32,
        );

        let report = DataQualityReport {
            total_crawled: crawled_products.len() as u32,
            total_existing: existing_products.len() as u32,
            new_products: new_products.len() as u32,
            updated_products: updated_products.len() as u32,
            duplicate_products: duplicates.len() as u32,
            incomplete_products: incomplete_products.len() as u32,
            quality_score,
            field_completeness,
            recommendations,
        };

        info!("✅ Data quality analysis completed: quality_score={:.2}, new_products={}, updated_products={}", 
              report.quality_score, report.new_products, report.updated_products);

        Ok(report)
    }

    /// 기존 데이터베이스에서 비교용 제품 데이터 조회
    async fn get_existing_products_for_comparison(&self) -> Result<Vec<ProductDetail>, Box<dyn std::error::Error + Send + Sync>> {
        // 최근 100개 제품을 샘플로 조회 (성능 최적화)
        info!("📊 Fetching existing products for comparison...");
        
        // 실제 구현에서는 repository의 메소드를 호출
        // 여기서는 임시로 빈 벡터 반환
        Ok(vec![])
    }

    /// 제품을 신규/업데이트/중복으로 분류
    async fn categorize_products(
        &self,
        crawled: &[ProductDetail],
        existing: &[ProductDetail],
    ) -> (Vec<&ProductDetail>, Vec<&ProductDetail>, Vec<&ProductDetail>) {
        let mut new_products = Vec::new();
        let mut updated_products = Vec::new();
        let mut duplicates = Vec::new();

        let existing_map: HashMap<String, &ProductDetail> = existing
            .iter()
            .map(|p| (p.certification_id.clone().unwrap_or_default(), p))
            .collect();

        for product in crawled {
            let cert_id = product.certification_id.clone().unwrap_or_default();
            
            if let Some(existing_product) = existing_map.get(&cert_id) {
                // 데이터 변경 여부 확인
                if self.has_significant_changes(product, existing_product) {
                    updated_products.push(product);
                } else {
                    duplicates.push(product);
                }
            } else {
                new_products.push(product);
            }
        }

        info!("📊 Product categorization: new={}, updated={}, duplicates={}", 
              new_products.len(), updated_products.len(), duplicates.len());

        (new_products, updated_products, duplicates)
    }

    /// 제품 간 의미있는 변경사항이 있는지 확인
    fn has_significant_changes(&self, new: &ProductDetail, existing: &ProductDetail) -> bool {
        // 주요 필드들의 변경사항 확인
        new.product_name != existing.product_name ||
        new.manufacturer != existing.manufacturer ||
        new.model_name != existing.model_name ||
        new.matter_version != existing.matter_version ||
        new.device_type != existing.device_type
    }

    /// 필드별 완정성 분석
    fn analyze_field_completeness(&self, products: &[ProductDetail]) -> FieldCompletenessReport {
        let total = products.len() as f64;
        if total == 0.0 {
            return FieldCompletenessReport {
                product_name: 0.0,
                manufacturer: 0.0,
                model_name: 0.0,
                certification_id: 0.0,
                matter_version: 0.0,
                device_type: 0.0,
                protocol_support: 0.0,
                overall_completeness: 0.0,
            };
        }

        let product_name_complete = products.iter()
            .filter(|p| p.product_name.as_ref().map_or(false, |s| !s.is_empty()))
            .count() as f64 / total;

        let manufacturer_complete = products.iter()
            .filter(|p| p.manufacturer.as_ref().map_or(false, |s| !s.is_empty()))
            .count() as f64 / total;

        let model_name_complete = products.iter()
            .filter(|p| p.model_name.as_ref().map_or(false, |s| !s.is_empty()))
            .count() as f64 / total;

        let certification_id_complete = products.iter()
            .filter(|p| p.certification_id.as_ref().map_or(false, |s| !s.is_empty()))
            .count() as f64 / total;

        let matter_version_complete = products.iter()
            .filter(|p| p.matter_version.as_ref().map_or(false, |s| !s.is_empty()))
            .count() as f64 / total;

        let device_type_complete = products.iter()
            .filter(|p| p.device_type.as_ref().map_or(false, |s| !s.is_empty()))
            .count() as f64 / total;

        let protocol_support_complete = products.iter()
            .filter(|p| p.protocol_support.as_ref().map_or(false, |s| !s.is_empty()))
            .count() as f64 / total;

        let overall_completeness = (
            product_name_complete + manufacturer_complete + model_name_complete + 
            certification_id_complete + matter_version_complete + device_type_complete + 
            protocol_support_complete
        ) / 7.0;

        FieldCompletenessReport {
            product_name: product_name_complete,
            manufacturer: manufacturer_complete,
            model_name: model_name_complete,
            certification_id: certification_id_complete,
            matter_version: matter_version_complete,
            device_type: device_type_complete,
            protocol_support: protocol_support_complete,
            overall_completeness,
        }
    }

    /// 전체 품질 점수 계산
    fn calculate_quality_score(&self, completeness: &FieldCompletenessReport, products: &[ProductDetail]) -> f64 {
        // 가중치 적용된 품질 점수 계산
        let completeness_weight = 0.7;
        let uniqueness_weight = 0.2;
        let consistency_weight = 0.1;

        let completeness_score = completeness.overall_completeness;
        let uniqueness_score = self.calculate_uniqueness_score(products);
        let consistency_score = self.calculate_consistency_score(products);

        (completeness_score * completeness_weight) +
        (uniqueness_score * uniqueness_weight) +
        (consistency_score * consistency_weight)
    }

    /// 고유성 점수 계산 (중복도 평가)
    fn calculate_uniqueness_score(&self, products: &[ProductDetail]) -> f64 {
        if products.is_empty() {
            return 1.0;
        }

        let total = products.len();
        let unique_cert_ids: std::collections::HashSet<_> = products
            .iter()
            .filter_map(|p| p.certification_id.as_ref())
            .collect();

        unique_cert_ids.len() as f64 / total as f64
    }

    /// 일관성 점수 계산 (데이터 형식 일관성)
    fn calculate_consistency_score(&self, _products: &[ProductDetail]) -> f64 {
        // 실제 구현에서는 데이터 형식 일관성을 확인
        // 예: URL 형식, 날짜 형식, 버전 형식 등
        0.9 // 임시 고정값
    }

    /// 불완전한 제품들 식별
    fn identify_incomplete_products(&self, products: &[ProductDetail]) -> Vec<&ProductDetail> {
        products
            .iter()
            .filter(|product| {
                // 핵심 필드가 누락된 경우를 불완전으로 간주
                product.product_name.as_ref().map_or(true, |s| s.is_empty()) ||
                product.manufacturer.as_ref().map_or(true, |s| s.is_empty()) ||
                product.certification_id.as_ref().map_or(true, |s| s.is_empty())
            })
            .collect()
    }

    /// 개선 권장사항 생성
    fn generate_recommendations(
        &self,
        completeness: &FieldCompletenessReport,
        quality_score: f64,
        incomplete_count: u32,
    ) -> Vec<String> {
        let mut recommendations = Vec::new();

        if quality_score < 0.7 {
            recommendations.push("전체 데이터 품질이 낮습니다. 크롤링 로직을 점검해주세요.".to_string());
        }

        if completeness.product_name < 0.8 {
            recommendations.push("제품명 누락률이 높습니다. HTML 파싱 로직을 확인해주세요.".to_string());
        }

        if completeness.manufacturer < 0.8 {
            recommendations.push("제조사 정보 누락률이 높습니다. 제조사 추출 로직을 개선해주세요.".to_string());
        }

        if completeness.certification_id < 0.9 {
            recommendations.push("인증 ID 누락률이 높습니다. 이는 중복 제거에 영향을 줄 수 있습니다.".to_string());
        }

        if incomplete_count > 0 {
            recommendations.push(format!("{}개의 불완전한 제품 데이터가 발견되었습니다.", incomplete_count));
        }

        if recommendations.is_empty() {
            recommendations.push("데이터 품질이 우수합니다. 데이터베이스 저장을 진행해도 좋습니다.".to_string());
        }

        recommendations
    }
}
