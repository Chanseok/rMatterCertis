use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::domain::product::ProductDetail;
use std::fmt;
use tracing::{info, warn, error};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataQualityReport {
    pub total_products: usize,
    pub complete_products: usize,
    pub incomplete_products: usize,
    pub missing_fields: HashMap<String, usize>,
    pub quality_score: f32,
    pub issues: Vec<QualityIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityIssue {
    pub severity: IssueSeverity,
    pub field_name: String,
    pub issue_type: IssueType,
    pub product_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IssueSeverity {
    Critical,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IssueType {
    MissingRequired,
    InvalidFormat,
    EmptyValue,
    Duplicate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageRecommendation {
    HighlyRecommended,
    ConditionallyRecommended,
    ReviewRequired,
    NotRecommended,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageAssessment {
    pub total_products: usize,
    pub quality_score: f32,
    pub critical_issues: usize,
    pub warning_issues: usize,
    pub recommendation: StorageRecommendation,
    pub summary: String,
}

pub struct DataQualityAnalyzer;

impl DataQualityAnalyzer {
    pub fn new() -> Self {
        Self
    }

    pub async fn analyze_product_quality(&self, products: &[ProductDetail]) -> Result<DataQualityReport, String> {
        let mut missing_fields = HashMap::new();
        let mut issues = Vec::new();
        let mut complete_count = 0;

        for product in products {
            let completeness = self.analyze_product_completeness(product);
            
            if completeness.is_complete {
                complete_count += 1;
            }

            // Track missing fields
            for field in &completeness.missing_fields {
                *missing_fields.entry(field.clone()).or_insert(0) += 1;
            }

            // Generate issues
            for issue in completeness.issues {
                issues.push(issue);
            }
        }

        let total_products = products.len();
        let quality_score = if total_products > 0 {
            (complete_count as f32 / total_products as f32) * 100.0
        } else {
            0.0
        };

        Ok(DataQualityReport {
            total_products,
            complete_products: complete_count,
            incomplete_products: total_products - complete_count,
            missing_fields,
            quality_score,
            issues,
        })
    }

    fn analyze_product_completeness(&self, product: &ProductDetail) -> ProductCompleteness {
        let mut missing_fields = Vec::new();
        let mut issues = Vec::new();

        // Check critical fields based on actual ProductDetail struct
        if product.manufacturer.as_ref().map_or(true, |s| s.trim().is_empty()) {
            missing_fields.push("manufacturer".to_string());
            issues.push(QualityIssue {
                severity: IssueSeverity::Critical,
                field_name: "manufacturer".to_string(),
                issue_type: IssueType::MissingRequired,
                product_url: product.url.clone(),
            });
        }

        if product.model.as_ref().map_or(true, |s| s.trim().is_empty()) {
            missing_fields.push("model".to_string());
            issues.push(QualityIssue {
                severity: IssueSeverity::Critical,
                field_name: "model".to_string(),
                issue_type: IssueType::MissingRequired,
                product_url: product.url.clone(),
            });
        }

        if product.device_type.as_ref().map_or(true, |s| s.trim().is_empty()) {
            missing_fields.push("device_type".to_string());
            issues.push(QualityIssue {
                severity: IssueSeverity::Warning,
                field_name: "device_type".to_string(),
                issue_type: IssueType::MissingRequired,
                product_url: product.url.clone(),
            });
        }

        if product.certificate_id.as_ref().map_or(true, |s| s.trim().is_empty()) {
            missing_fields.push("certificate_id".to_string());
            issues.push(QualityIssue {
                severity: IssueSeverity::Warning,
                field_name: "certificate_id".to_string(),
                issue_type: IssueType::MissingRequired,
                product_url: product.url.clone(),
            });
        }

        // Check VID/PID for Matter devices
        if product.vid.is_none() {
            missing_fields.push("vid".to_string());
            issues.push(QualityIssue {
                severity: IssueSeverity::Warning,
                field_name: "vid".to_string(),
                issue_type: IssueType::MissingRequired,
                product_url: product.url.clone(),
            });
        }

        if product.pid.is_none() {
            missing_fields.push("pid".to_string());
            issues.push(QualityIssue {
                severity: IssueSeverity::Warning,
                field_name: "pid".to_string(),
                issue_type: IssueType::MissingRequired,
                product_url: product.url.clone(),
            });
        }

        let is_complete = missing_fields.is_empty();

        ProductCompleteness {
            is_complete,
            missing_fields,
            issues,
        }
    }

    pub async fn validate_before_storage(&self, products: &[ProductDetail]) -> Result<Vec<ProductDetail>, String> {
        let report = self.analyze_product_quality(products).await?;
        
        // 🚨 CRITICAL: 데이터 수집 실패 진단
        if report.total_products == 0 {
            error!("🚨 CRITICAL DATA COLLECTION FAILURE:");
            error!("  📊 Total products analyzed: {}", report.total_products);
            error!("  💥 ROOT CAUSE: No product data was extracted from the pipeline");
            error!("  🔍 LIKELY ISSUES:");
            error!("    1. 🔗 Data pipeline break: Stage 3 (ProductDetailCrawling) → Stage 4 (DataValidation)");
            error!("    2. 📦 Wrong item type received: Expected ProductUrls, got Page type");
            error!("    3. 🌐 Network/parsing issues during product detail extraction");
            error!("    4. 🏗️ Stage transformation logic not working properly");
            error!("  🛠️  IMMEDIATE ACTIONS NEEDED:");
            error!("    - Check if Stage 3 successfully collected ProductDetails");
            error!("    - Verify data flow between stages");
            error!("    - Fix stage item type transformation (Page → ProductUrls)");
            error!("  💾 Storage Recommendation: 🔴 NOT RECOMMENDED - No data to store");
            return Ok(Vec::new());
        }

        // Log quality report for actual data
        info!("🔍 Data Quality Analysis Report:");
        info!("  📊 Total products analyzed: {}", report.total_products);
        info!("  ✅ Complete products: {} ({:.1}%)", 
                 report.complete_products, 
                 if report.total_products > 0 { (report.complete_products as f32 / report.total_products as f32) * 100.0 } else { 0.0 });
        info!("  ⚠️  Incomplete products: {}", report.incomplete_products);
        info!("  📈 Overall quality score: {:.2}%", report.quality_score);
        
        // 🔍 상세 필드 분석 및 로컬 DB 비교
        if !report.missing_fields.is_empty() {
            info!("  📋 Missing field frequency (compared to ideal product structure):");
            for (field, count) in &report.missing_fields {
                let percentage = (*count as f32 / report.total_products as f32) * 100.0;
                let impact = if matches!(field.as_str(), "manufacturer" | "model") {
                    "🔴 CRITICAL (blocks identification)"
                } else if matches!(field.as_str(), "certificate_id" | "device_type") {
                    "🟡 HIGH (affects searchability)"
                } else if matches!(field.as_str(), "vid" | "pid") {
                    "🟠 MEDIUM (Matter standard compliance)"
                } else {
                    "🟢 LOW (optional metadata)"
                };
                info!("    - {}: {} products ({:.1}%) {}", 
                         field, count, percentage, impact);
            }
        }
        
        // 🔍 특정 제품 사례 상세 분석
        if !products.is_empty() {
            info!("  🔬 DETAILED EXAMPLE ANALYSIS:");
            let sample_product = &products[0];
            info!("    📄 Sample Product: {}", sample_product.url.split('/').last().unwrap_or("Unknown"));
            info!("    🏷️  Field Completeness Analysis:");
            
            // Critical fields analysis
            info!("      🔴 CRITICAL FIELDS:");
            self.analyze_field("manufacturer", &sample_product.manufacturer);
            self.analyze_field("model", &sample_product.model);
            
            // Important fields analysis  
            info!("      🟡 IMPORTANT FIELDS:");
            self.analyze_field("device_type", &sample_product.device_type);
            self.analyze_field("certificate_id", &sample_product.certificate_id);
            
            // Matter-specific fields
            info!("      🟠 MATTER COMPLIANCE FIELDS:");
            info!("        - vid: {} {}", 
                if sample_product.vid.is_some() { "✅ Present" } else { "❌ Missing" },
                sample_product.vid.map_or("(required for Matter device identification)".to_string(), |v| format!("(value: {})", v))
            );
            info!("        - pid: {} {}", 
                if sample_product.pid.is_some() { "✅ Present" } else { "❌ Missing" },
                sample_product.pid.map_or("(required for Matter device identification)".to_string(), |v| format!("(value: {})", v))
            );
            
            // Additional metadata
            info!("      🟢 OPTIONAL METADATA:");
            self.analyze_field("specification_version", &sample_product.specification_version);
            self.analyze_field("transport_interface", &sample_product.transport_interface);
            self.analyze_field("certification_date", &sample_product.certification_date);
        }
        
        if !report.issues.is_empty() {
            warn!("  🚨 Quality issues by severity:");
            let mut critical_count = 0;
            let mut warning_count = 0;
            let mut info_count = 0;
            
            for issue in &report.issues {
                match issue.severity {
                    IssueSeverity::Critical => critical_count += 1,
                    IssueSeverity::Warning => warning_count += 1,
                    IssueSeverity::Info => info_count += 1,
                }
            }
            
            if critical_count > 0 {
                error!("    🔴 Critical issues: {} (blocks database storage)", critical_count);
            }
            if warning_count > 0 {
                warn!("    🟡 Warning issues: {} (reduces data usability)", warning_count);
            }
            if info_count > 0 {
                info!("    🔵 Info issues: {} (minor concerns)", info_count);
            }
            
            // Show first few issues as examples
            info!("  📝 Sample issues (first 5):");
            for (i, issue) in report.issues.iter().take(5).enumerate() {
                warn!("    {}. {} {} in '{}' for product: {}", 
                    i + 1,
                    match issue.severity {
                        IssueSeverity::Critical => "🔴 CRITICAL",
                        IssueSeverity::Warning => "🟡 WARNING", 
                        IssueSeverity::Info => "🔵 INFO",
                    },
                    match issue.issue_type {
                        IssueType::MissingRequired => "Missing Required Field",
                        IssueType::InvalidFormat => "Invalid Format",
                        IssueType::EmptyValue => "Empty Value",
                        IssueType::Duplicate => "Duplicate",
                    },
                    issue.field_name,
                    issue.product_url.split('/').last().unwrap_or(&issue.product_url)
                );
            }
            if report.issues.len() > 5 {
                info!("    ... and {} more issues", report.issues.len() - 5);
            }
        }
        
        // Storage recommendation with detailed rationale
        let storage_recommendation = if report.quality_score >= 80.0 {
            "🟢 RECOMMENDED - High quality data, safe to store"
        } else if report.quality_score >= 60.0 {
            "🟡 CONDITIONAL - Moderate quality, review critical issues before storage"
        } else {
            "🔴 NOT RECOMMENDED - Low quality data, fix critical issues first"
        };
        
        info!("  💾 Storage Recommendation: {}", storage_recommendation);
        
        // Filter out products with critical issues if needed
        // For now, return all products but log the assessment
        Ok(products.to_vec())
    }

    // Helper method for field analysis
    fn analyze_field(&self, field_name: &str, field_value: &Option<String>) {
        match field_value {
            Some(value) if !value.trim().is_empty() => {
                info!("        - {}: ✅ Present ('{}')", field_name, 
                    if value.len() > 50 { 
                        format!("{}...", &value[..47]) 
                    } else { 
                        value.clone() 
                    }
                );
            }
            Some(value) if value.trim().is_empty() => {
                warn!("        - {}: ⚠️  Empty (present but no content)", field_name);
            }
            Some(value) => {
                info!("        - {}: ✅ Present ('{}')", field_name, 
                    if value.len() > 50 { 
                        format!("{}...", &value[..47]) 
                    } else { 
                        value.clone() 
                    }
                );
            }
            None => {
                warn!("        - {}: ❌ Missing (critical for product identification)", field_name);
            }
        }
    }

    /// Analyze collected product data and provide storage recommendation
    pub async fn assess_for_storage(&self, products: &[ProductDetail]) -> Result<StorageAssessment, String> {
        let report = self.analyze_product_quality(products).await?;
        
        let critical_issues = report.issues.iter().filter(|i| matches!(i.severity, IssueSeverity::Critical)).count();
        let warning_issues = report.issues.iter().filter(|i| matches!(i.severity, IssueSeverity::Warning)).count();
        
        let recommendation = if report.quality_score >= 80.0 && critical_issues == 0 {
            StorageRecommendation::HighlyRecommended
        } else if report.quality_score >= 60.0 && critical_issues <= 2 {
            StorageRecommendation::ConditionallyRecommended
        } else if critical_issues <= 5 {
            StorageRecommendation::ReviewRequired
        } else {
            StorageRecommendation::NotRecommended
        };
        
        Ok(StorageAssessment {
            total_products: report.total_products,
            quality_score: report.quality_score,
            critical_issues,
            warning_issues,
            recommendation,
            summary: self.generate_summary(&report),
        })
    }
    
    fn generate_summary(&self, report: &DataQualityReport) -> String {
        format!(
            "Analyzed {} products: {:.1}% complete, {} critical issues, {} warnings",
            report.total_products,
            report.quality_score,
            report.issues.iter().filter(|i| matches!(i.severity, IssueSeverity::Critical)).count(),
            report.issues.iter().filter(|i| matches!(i.severity, IssueSeverity::Warning)).count()
        )
    }
}

#[derive(Debug)]
struct ProductCompleteness {
    is_complete: bool,
    missing_fields: Vec<String>,
    issues: Vec<QualityIssue>,
}

impl fmt::Display for IssueSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IssueSeverity::Critical => write!(f, "Critical"),
            IssueSeverity::Warning => write!(f, "Warning"),
            IssueSeverity::Info => write!(f, "Info"),
        }
    }
}

impl fmt::Display for IssueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IssueType::MissingRequired => write!(f, "Missing Required Field"),
            IssueType::InvalidFormat => write!(f, "Invalid Format"),
            IssueType::EmptyValue => write!(f, "Empty Value"),
            IssueType::Duplicate => write!(f, "Duplicate"),
        }
    }
}
