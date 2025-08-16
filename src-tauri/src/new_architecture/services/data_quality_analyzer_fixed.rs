use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::domain::product::ProductDetail;
use std::fmt;

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

        if product.certification_id.as_ref().map_or(true, |s| s.trim().is_empty()) {
            missing_fields.push("certification_id".to_string());
            issues.push(QualityIssue {
                severity: IssueSeverity::Warning,
                field_name: "certification_id".to_string(),
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

    pub fn validate_before_storage(&self, products: &[ProductDetail]) -> Result<Vec<ProductDetail>, String> {
    let report = self.analyze_product_quality(products)?;
        
        // Log quality report
        println!("Data Quality Analysis:");
        println!("- Total products: {}", report.total_products);
        println!("- Complete products: {}", report.complete_products);
        println!("- Quality score: {:.2}%", report.quality_score);
        
        if !report.issues.is_empty() {
            println!("Quality issues found:");
            for issue in &report.issues {
                println!("  - {} {:?} in '{}' for {}", 
                    match issue.severity {
                        IssueSeverity::Critical => "CRITICAL",
                        IssueSeverity::Warning => "WARNING",
                        IssueSeverity::Info => "INFO",
                    },
                    issue.issue_type,
                    issue.field_name,
                    issue.product_url
                );
            }
        }

        // Filter out products with critical issues if needed
        // For now, return all products but log the issues
        Ok(products.to_vec())
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
