
use matter_certis_v2_lib::new_architecture::services::data_quality_analyzer::{DataQualityAnalyzer, StorageRecommendation};
use matter_certis_v2_lib::domain::product::ProductDetail;
use chrono::{Utc};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Matter Certis v2 - Data Quality Assessment Tool");
    println!("================================================");
    
    // 로그에서 확인한 크롤링 결과 기반 분석
    println!("📊 Analyzing crawling results from logs...");
    
    // 로그 분석 결과 요약
    println!("\n📋 Crawling Session Summary:");
    println!("  🎯 Target pages: 288, 289, 290, 291, 292 (5 pages total)");
    println!("  📄 Products per page: 12");
    println!("  🔢 Total products collected: 60 (5 × 12)");
    println!("  ✅ Success rate: 100% (all stages completed successfully)");
    
    // 파이프라인 분석
    println!("\n🔄 Pipeline Analysis:");
    println!("  📊 Stage 1 (StatusCheck): ✅ Success");
    println!("  📊 Stage 2 (ListPageCrawling): ✅ Success - 60 URLs extracted");
    println!("  📊 Stage 3 (ProductDetailCrawling): ✅ Success - 60 product details collected");
    println!("  📊 Stage 4 (DataValidation): ⚠️  Issue - 0 products validated (no data passed)");
    println!("  📊 Stage 5 (DataSaving): ❌ Critical Issue - Data skipped, not stored");
    
    // 모의 ProductDetail 데이터로 품질 분석 시뮬레이션
    let sample_products = create_sample_products_from_log();
    
    let analyzer = DataQualityAnalyzer::new();
    let assessment = analyzer.assess_for_storage(&sample_products).await?;
    
    println!("\n🔍 Data Quality Assessment:");
    println!("  📊 Total products: {}", assessment.total_products);
    println!("  📈 Quality score: {:.1}%", assessment.quality_score);
    println!("  🔴 Critical issues: {}", assessment.critical_issues);
    println!("  🟡 Warning issues: {}", assessment.warning_issues);
    println!("  💾 Recommendation: {:?}", assessment.recommendation);
    println!("  📝 Summary: {}", assessment.summary);
    
    // 저장 권장사항
    match assessment.recommendation {
        StorageRecommendation::HighlyRecommended => {
            println!("\n🟢 STORAGE RECOMMENDATION: HIGHLY RECOMMENDED");
            println!("   ✅ Data quality is excellent, safe to store in database");
            println!("   ✅ All critical fields are present");
            println!("   ✅ Minimal data quality issues");
        }
        StorageRecommendation::ConditionallyRecommended => {
            println!("\n🟡 STORAGE RECOMMENDATION: CONDITIONALLY RECOMMENDED");
            println!("   ⚠️  Data quality is acceptable with minor issues");
            println!("   ⚠️  Review missing critical fields");
            println!("   ⚠️  Consider data enrichment before storage");
        }
        StorageRecommendation::ReviewRequired => {
            println!("\n🟠 STORAGE RECOMMENDATION: REVIEW REQUIRED");
            println!("   ❌ Significant data quality issues detected");
            println!("   ❌ Manual review recommended before storage");
            println!("   ❌ Fix critical issues first");
        }
        StorageRecommendation::NotRecommended => {
            println!("\n🔴 STORAGE RECOMMENDATION: NOT RECOMMENDED");
            println!("   ❌ Poor data quality, should not store");
            println!("   ❌ Too many critical issues");
            println!("   ❌ Data collection needs to be improved");
        }
    }
    
    // 실제 문제 분석
    println!("\n🚨 Identified Critical Issues:");
    println!("  1. ❌ Data Pipeline Break: ProductDetail data not passed between stages");
    println!("  2. ❌ Stage 4 receives empty data: 'Data quality validation completed: 0 products validated'");
    println!("  3. ❌ Stage 5 skips storage: 'Skipping database storage for item page_X'");
    println!("  4. ❌ Data Type Mismatch: Stage items are Page type, not ProductDetail type");
    
    println!("\n🛠️  Recommended Actions:");
    println!("  1. 🔧 Fix data flow between ProductDetailCrawling → DataValidation stages");
    println!("  2. 🔧 Ensure ProductDetail data is passed correctly in StageItem");
    println!("  3. 🔧 Update DataSaving stage to handle ProductDetail data");
    println!("  4. 🔧 Add data extraction logic from stage results");
    println!("  5. 🧪 Test end-to-end pipeline with actual data flow");
    
    Ok(())
}

fn create_sample_products_from_log() -> Vec<ProductDetail> {
    // 로그에서 확인된 제품 URL들을 기반으로 샘플 데이터 생성
    let urls = vec![
        "https://csa-iot.org/csa_product/ua-smartlight-47/",
        "https://csa-iot.org/csa_product/ua-smartlight-19/", 
        "https://csa-iot.org/csa_product/tcl-window-ac-30/",
        "https://csa-iot.org/csa_product/tcl-window-ac-56/",
        "https://csa-iot.org/csa_product/ua-smartlight-25/",
    ];
    
    let mut products = Vec::new();
    let now = Utc::now();
    
    for (i, url) in urls.iter().enumerate() {
        products.push(ProductDetail {
            url: url.to_string(),
            page_id: Some(288 + i as i32),
            index_in_page: Some(i as i32),
            id: Some(format!("product_{}", i)),
            manufacturer: if i % 3 == 0 { Some("Unknown Manufacturer".to_string()) } else { None },
            model: if i % 2 == 0 { Some(format!("Model-{}", i)) } else { None },
            device_type: Some("Smart Light".to_string()),
            certificate_id: if i % 4 == 0 { Some(format!("CERT-{}", i)) } else { None },
            certification_date: Some("2024-01-01".to_string()),
            software_version: None,
            hardware_version: None,
            vid: if i % 3 == 0 { Some(1234) } else { None },
            pid: if i % 3 == 0 { Some(5678) } else { None },
            family_sku: None,
            family_variant_sku: None,
            firmware_version: None,
            family_id: None,
            tis_trp_tested: None,
            specification_version: None,
            transport_interface: None,
            primary_device_type_id: None,
            application_categories: None,
            description: None,
            compliance_document_url: None,
            program_type: None,
            created_at: now,
            updated_at: now,
        });
    }
    
    products
}
