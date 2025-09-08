use crate::domain::product::ProductDetail;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use tracing::{error, info, warn};

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

impl Default for DataQualityAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl DataQualityAnalyzer {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    pub fn analyze_product_quality(
        &self,
        products: &[ProductDetail],
    ) -> Result<DataQualityReport, String> {
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
        if product
            .manufacturer
            .as_ref()
            .is_none_or(|s| s.trim().is_empty())
        {
            missing_fields.push("manufacturer".to_string());
            issues.push(QualityIssue {
                severity: IssueSeverity::Critical,
                field_name: "manufacturer".to_string(),
                issue_type: IssueType::MissingRequired,
                product_url: product.url.clone(),
            });
        }

        if product.model.as_ref().is_none_or(|s| s.trim().is_empty()) {
            missing_fields.push("model".to_string());
            issues.push(QualityIssue {
                severity: IssueSeverity::Critical,
                field_name: "model".to_string(),
                issue_type: IssueType::MissingRequired,
                product_url: product.url.clone(),
            });
        }

        if product
            .device_type
            .as_ref()
            .is_none_or(|s| s.trim().is_empty())
        {
            missing_fields.push("device_type".to_string());
            issues.push(QualityIssue {
                severity: IssueSeverity::Warning,
                field_name: "device_type".to_string(),
                issue_type: IssueType::MissingRequired,
                product_url: product.url.clone(),
            });
        }

        if product
            .certificate_id
            .as_ref()
            .is_none_or(|s| s.trim().is_empty())
        {
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

    pub fn validate_before_storage(
        &self,
        products: &[ProductDetail],
    ) -> Result<Vec<ProductDetail>, String> {
        let report = self.analyze_product_quality(products)?;

        // 🚨 CRITICAL: 데이터 수집 실패 진단
        if report.total_products == 0 {
            error!("🚨 CRITICAL DATA COLLECTION FAILURE:");
            error!("  📊 Total products analyzed: {}", report.total_products);
            error!("  💥 ROOT CAUSE: No product data was extracted from the pipeline");
            error!("  🔍 LIKELY ISSUES:");
            error!(
                "    1. 🔗 Data pipeline break: Stage 3 (ProductDetailCrawling) → Stage 4 (DataValidation)"
            );
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

        // Decide logging mode (concise vs verbose) using env flag MC_CONCISE_DQ=1/true
        let concise = std::env::var("MC_CONCISE_DQ")
            .ok()
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));

        // Pre-compute issue counts
        let critical_count = report
            .issues
            .iter()
            .filter(|i| matches!(i.severity, IssueSeverity::Critical))
            .count();
        let warning_count = report
            .issues
            .iter()
            .filter(|i| matches!(i.severity, IssueSeverity::Warning))
            .count();
        let info_count = report
            .issues
            .iter()
            .filter(|i| matches!(i.severity, IssueSeverity::Info))
            .count();

        // Storage recommendation (short + long versions)
        let (storage_recommendation_short, storage_recommendation_long) =
            if report.quality_score >= 80.0 {
                (
                    "RECOMMENDED",
                    "🟢 RECOMMENDED - High quality data, safe to store",
                )
            } else if report.quality_score >= 60.0 {
                (
                    "CONDITIONAL",
                    "🟡 CONDITIONAL - Moderate quality, review critical issues before storage",
                )
            } else {
                (
                    "NOT_RECOMMENDED",
                    "� NOT RECOMMENDED - Low quality data, fix critical issues first",
                )
            };

        let json_mode = std::env::var("MC_DQ_JSON")
            .ok()
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
        if concise {
            // Build compact missing field summary (max 4 entries)
            let mut missing: Vec<(String, usize)> = report
                .missing_fields
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect();
            missing.sort_by(|a, b| b.1.cmp(&a.1));
            let missing_summary = if missing.is_empty() {
                "none".to_string()
            } else {
                missing
                    .iter()
                    .take(4)
                    .map(|(k, v)| format!("{}:{}", k, v))
                    .collect::<Vec<_>>()
                    .join(",")
                    + if missing.len() > 4 { ",…" } else { "" }
            };
            if json_mode {
                // Emit machine-readable JSON one-liner
                if let Ok(json) = serde_json::to_string(&serde_json::json!({
                    "kind":"dq_summary",
                    "products":report.total_products,
                    "complete":report.complete_products,
                    "incomplete":report.incomplete_products,
                    "score":format!("{:.1}", report.quality_score),
                    "missing_top":missing.iter().take(4).collect::<Vec<_>>(),
                    "critical":critical_count,
                    "warning":warning_count,
                    "info":info_count,
                    "storage":storage_recommendation_short
                })) {
                    info!("{}", json);
                }
            } else {
                info!(
                    "DQ summary products={} complete={} incomplete={} score={:.1}% miss=[{}] issues(c={} w={} i={}) storage={}",
                    report.total_products,
                    report.complete_products,
                    report.incomplete_products,
                    report.quality_score,
                    missing_summary,
                    critical_count,
                    warning_count,
                    info_count,
                    storage_recommendation_short
                );
            }
        } else {
            // Verbose (original) logging retained but condensed to reduce duplication
            info!("🔍 Data Quality Analysis Report:");
            info!("  � Total products analyzed: {}", report.total_products);
            info!(
                "  ✅ Complete products: {} ({:.1}%)",
                report.complete_products,
                if report.total_products > 0 {
                    (report.complete_products as f32 / report.total_products as f32) * 100.0
                } else {
                    0.0
                }
            );
            info!("  ⚠️  Incomplete products: {}", report.incomplete_products);
            info!("  � Overall quality score: {:.2}%", report.quality_score);
            if !report.missing_fields.is_empty() {
                info!("  📋 Missing field frequency:");
                for (field, count) in &report.missing_fields {
                    let pct = (*count as f32 / report.total_products as f32) * 100.0;
                    info!("    - {}: {} ({:.1}%)", field, count, pct);
                }
            }
            if !report.issues.is_empty() {
                warn!(
                    "  � Issues: critical={} warning={} info={} (showing first 3)",
                    critical_count, warning_count, info_count
                );
                for (i, issue) in report.issues.iter().take(3).enumerate() {
                    warn!(
                        "    {}. {} {} in '{}' [{}]",
                        i + 1,
                        match issue.severity {
                            IssueSeverity::Critical => "CRIT",
                            IssueSeverity::Warning => "WARN",
                            IssueSeverity::Info => "INFO",
                        },
                        match issue.issue_type {
                            IssueType::MissingRequired => "Missing",
                            IssueType::InvalidFormat => "Format",
                            IssueType::EmptyValue => "Empty",
                            IssueType::Duplicate => "Dup",
                        },
                        issue.field_name,
                        issue
                            .product_url
                            .split('/')
                            .next_back()
                            .unwrap_or(&issue.product_url)
                    );
                }
                if report.issues.len() > 3 {
                    info!("    ... {} more", report.issues.len() - 3);
                }
            }
            // Sample only if verbose and products available
            if let Some(sample_product) = products.first() {
                info!(
                    "  🧪 Sample: manf={:?} model={:?} device_type={:?} cert_id={:?} vid={:?} pid={:?}",
                    sample_product.manufacturer,
                    sample_product.model,
                    sample_product.device_type,
                    sample_product.certificate_id,
                    sample_product.vid,
                    sample_product.pid
                );
            }
            info!(
                "  💾 Storage Recommendation: {}",
                storage_recommendation_long
            );
        }

        // Filter out products with critical issues if needed
        // For now, return all products but log the assessment
        Ok(products.to_vec())
    }

    /// Analyze collected product data and provide storage recommendation
    pub fn assess_for_storage(
        &self,
        products: &[ProductDetail],
    ) -> Result<StorageAssessment, String> {
        let report = self.analyze_product_quality(products)?;

        let critical_issues = report
            .issues
            .iter()
            .filter(|i| matches!(i.severity, IssueSeverity::Critical))
            .count();
        let warning_issues = report
            .issues
            .iter()
            .filter(|i| matches!(i.severity, IssueSeverity::Warning))
            .count();

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
            report
                .issues
                .iter()
                .filter(|i| matches!(i.severity, IssueSeverity::Critical))
                .count(),
            report
                .issues
                .iter()
                .filter(|i| matches!(i.severity, IssueSeverity::Warning))
                .count()
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
            Self::Critical => write!(f, "Critical"),
            Self::Warning => write!(f, "Warning"),
            Self::Info => write!(f, "Info"),
        }
    }
}

impl fmt::Display for IssueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRequired => write!(f, "Missing Required Field"),
            Self::InvalidFormat => write!(f, "Invalid Format"),
            Self::EmptyValue => write!(f, "Empty Value"),
            Self::Duplicate => write!(f, "Duplicate"),
        }
    }
}
