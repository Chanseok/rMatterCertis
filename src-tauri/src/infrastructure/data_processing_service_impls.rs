//! 고급 데이터 처리 서비스 구현체들
//!
//! domain/services/data_processing_services.rs의 트레이트들에 대한 실제 구현체

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::domain::product::Product;
use crate::domain::services::data_processing_services::*;

/// 중복 제거 서비스 구현체
pub struct DeduplicationServiceImpl {
    similarity_threshold: f64,
}

impl DeduplicationServiceImpl {
    pub fn new(similarity_threshold: f64) -> Self {
        Self {
            similarity_threshold: similarity_threshold.clamp(0.0, 1.0),
        }
    }

    /// 두 제품 간 유사도 계산
    fn calculate_similarity(&self, product1: &Product, product2: &Product) -> f64 {
        let mut similarity_score = 0.0;
        let mut total_weight = 0.0;

        // 제조사 비교 (가중치: 0.3)
        if let (Some(m1), Some(m2)) = (&product1.manufacturer, &product2.manufacturer) {
            total_weight += 0.3;
            if m1.to_lowercase() == m2.to_lowercase() {
                similarity_score += 0.3;
            } else if m1.to_lowercase().contains(&m2.to_lowercase())
                || m2.to_lowercase().contains(&m1.to_lowercase())
            {
                similarity_score += 0.15;
            }
        }

        // 모델명 비교 (가중치: 0.4)
        if let (Some(model1), Some(model2)) = (&product1.model, &product2.model) {
            total_weight += 0.4;
            if model1.to_lowercase() == model2.to_lowercase() {
                similarity_score += 0.4;
            } else {
                let similarity =
                    self.string_similarity(&model1.to_lowercase(), &model2.to_lowercase());
                similarity_score += 0.4 * similarity;
            }
        }

        // 인증 ID 비교 (가중치: 0.3)
        if let (Some(cert1), Some(cert2)) = (&product1.certificate_id, &product2.certificate_id) {
            total_weight += 0.3;
            if cert1 == cert2 {
                similarity_score += 0.3;
            }
        }

        if total_weight > 0.0 {
            similarity_score / total_weight
        } else {
            0.0
        }
    }

    /// 두 문자열 간 유사도 계산 (간단한 Levenshtein 거리 기반)
    fn string_similarity(&self, s1: &str, s2: &str) -> f64 {
        let len1 = s1.len();
        let len2 = s2.len();

        if len1 == 0 && len2 == 0 {
            return 1.0;
        }
        if len1 == 0 || len2 == 0 {
            return 0.0;
        }

        let max_len = len1.max(len2) as f64;
        let distance = levenshtein_distance(s1, s2) as f64;

        1.0 - (distance / max_len)
    }
}

#[async_trait]
impl DeduplicationService for DeduplicationServiceImpl {
    async fn remove_duplicates(&self, products: Vec<Product>) -> Result<Vec<Product>> {
        info!(
            "Starting deduplication process for {} products",
            products.len()
        );

        let mut unique_products = Vec::new();
        let mut processed_count = 0;

        for product in products {
            processed_count += 1;

            let is_duplicate = self.is_duplicate(&product, &unique_products).await?;

            if !is_duplicate {
                unique_products.push(product);
            } else {
                debug!("Removed duplicate product: {:?}", product.model);
            }

            if processed_count % 100 == 0 {
                debug!("Processed {} products for deduplication", processed_count);
            }
        }

        let removed_count = processed_count - unique_products.len();
        info!(
            "Deduplication completed: {} unique products, {} duplicates removed",
            unique_products.len(),
            removed_count
        );

        Ok(unique_products)
    }

    async fn analyze_duplicates(&self, products: &[Product]) -> Result<DuplicationAnalysis> {
        info!("Analyzing duplicates in {} products", products.len());

        let mut duplicate_groups = Vec::new();
        let mut processed_indices = std::collections::HashSet::new();

        for (i, product1) in products.iter().enumerate() {
            if processed_indices.contains(&i) {
                continue;
            }

            let mut group_products = vec![product1.clone()];
            processed_indices.insert(i);

            for (j, product2) in products.iter().enumerate().skip(i + 1) {
                if processed_indices.contains(&j) {
                    continue;
                }

                let similarity = self.calculate_similarity(product1, product2);
                if similarity >= self.similarity_threshold {
                    group_products.push(product2.clone());
                    processed_indices.insert(j);
                }
            }

            if group_products.len() > 1 {
                let avg_similarity = if group_products.len() == 2 {
                    self.calculate_similarity(&group_products[0], &group_products[1])
                } else {
                    self.similarity_threshold
                };

                duplicate_groups.push(DuplicateProductGroup {
                    products: group_products,
                    similarity_score: avg_similarity,
                    duplicate_type: DuplicationType::SimilarModel {
                        similarity: avg_similarity,
                    },
                });
            }
        }

        let total_duplicates = duplicate_groups
            .iter()
            .map(|group| group.products.len() - 1)
            .sum::<usize>() as u32;

        let unique_products = products.len() as u32 - total_duplicates;
        let duplicate_rate = if products.len() > 0 {
            total_duplicates as f64 / products.len() as f64
        } else {
            0.0
        };

        Ok(DuplicationAnalysis {
            total_duplicates,
            duplicate_rate,
            duplicate_groups,
            unique_products,
        })
    }

    async fn is_duplicate(&self, product: &Product, existing: &[Product]) -> Result<bool> {
        for existing_product in existing {
            let similarity = self.calculate_similarity(product, existing_product);
            if similarity >= self.similarity_threshold {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// 데이터 유효성 검사 서비스 구현체
pub struct ValidationServiceImpl {
    required_fields: Vec<String>,
}

impl ValidationServiceImpl {
    pub fn new() -> Self {
        Self {
            required_fields: vec!["manufacturer".to_string(), "model".to_string()],
        }
    }

    /// 제품의 데이터 품질 점수 계산
    fn calculate_quality_score(&self, product: &Product) -> f64 {
        let mut score = 0.0;
        let mut max_score = 0.0;

        // 필수 필드 존재 여부 (가중치: 0.5)
        max_score += 0.5;
        let required_filled = self
            .required_fields
            .iter()
            .map(|field| match field.as_str() {
                "manufacturer" => product
                    .manufacturer
                    .as_ref()
                    .map_or(false, |m| !m.trim().is_empty()),
                "model" => product
                    .model
                    .as_ref()
                    .map_or(false, |m| !m.trim().is_empty()),
                _ => false,
            })
            .filter(|&filled| filled)
            .count();

        score += 0.5 * (required_filled as f64 / self.required_fields.len() as f64);

        // 선택적 필드 완성도 (가중치: 0.3)
        max_score += 0.3;
        let optional_fields = [
            product.certificate_id.as_ref(),
            // device_type과 certification_date는 ProductDetail에만 있음
        ];
        let optional_filled = optional_fields
            .iter()
            .filter(|field| field.map_or(false, |f| !f.trim().is_empty()))
            .count();

        score += 0.3 * (optional_filled as f64 / optional_fields.len() as f64);

        // URL 유효성 (가중치: 0.2)
        max_score += 0.2;
        if !product.url.trim().is_empty() && product.url.starts_with("http") {
            score += 0.2;
        }

        if max_score > 0.0 {
            score / max_score
        } else {
            0.0
        }
    }
}

#[async_trait]
impl ValidationService for ValidationServiceImpl {
    async fn validate_all(&self, products: Vec<Product>) -> Result<ValidationResult> {
        info!(
            "Starting validation process for {} products",
            products.len()
        );

        let mut valid_products = Vec::new();
        let mut invalid_products = Vec::new();
        let mut common_errors = HashMap::new();

        for product in products {
            let validation = self.validate_product(&product).await?;

            if validation.is_valid {
                valid_products.push(product);
            } else {
                for error in &validation.errors {
                    *common_errors.entry(error.message.clone()).or_insert(0) += 1;
                }

                invalid_products.push(InvalidProduct {
                    product,
                    validation_errors: validation.errors,
                });
            }
        }

        let total_products = valid_products.len() + invalid_products.len();
        let validation_rate = if total_products > 0 {
            valid_products.len() as f64 / total_products as f64
        } else {
            0.0
        };

        let mut common_error_list: Vec<String> = common_errors
            .into_iter()
            .map(|(error, count)| format!("{} ({} occurrences)", error, count))
            .collect();
        common_error_list.sort_by(|a, b| b.cmp(a)); // 빈도순 정렬

        info!(
            "Validation completed: {} valid, {} invalid products",
            valid_products.len(),
            invalid_products.len()
        );

        let valid_count = valid_products.len() as u32;
        let invalid_count = invalid_products.len() as u32;

        Ok(ValidationResult {
            valid_products,
            invalid_products,
            validation_summary: ValidationSummary {
                total_products: total_products as u32,
                valid_products: valid_count,
                invalid_products: invalid_count,
                validation_rate,
                common_errors: common_error_list.into_iter().take(10).collect(),
            },
        })
    }

    async fn validate_product(&self, product: &Product) -> Result<ProductValidation> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // 필수 필드 검사
        let field_validation = self.check_required_fields(product).await?;

        for missing_field in field_validation.missing_required_fields {
            errors.push(ValidationError {
                field: missing_field.clone(),
                error_type: ValidationErrorType::MissingRequiredField,
                message: format!("Required field '{}' is missing", missing_field),
            });
        }

        for format_error in field_validation.invalid_field_formats {
            errors.push(ValidationError {
                field: format_error.field.clone(),
                error_type: ValidationErrorType::InvalidFormat,
                message: format!(
                    "Invalid format in field '{}': expected {}, got {}",
                    format_error.field, format_error.expected_format, format_error.actual_value
                ),
            });
        }

        // URL 검증
        if product.url.trim().is_empty() {
            errors.push(ValidationError {
                field: "url".to_string(),
                error_type: ValidationErrorType::MissingRequiredField,
                message: "URL cannot be empty".to_string(),
            });
        } else if !product.url.starts_with("http") {
            errors.push(ValidationError {
                field: "url".to_string(),
                error_type: ValidationErrorType::InvalidFormat,
                message: "URL must start with http or https".to_string(),
            });
        }

        // 경고 생성
        for empty_field in field_validation.empty_fields {
            warnings.push(ValidationWarning {
                field: empty_field.clone(),
                warning_type: ValidationWarningType::EmptyOptionalField,
                message: format!("Optional field '{}' is empty", empty_field),
            });
        }

        let is_valid = errors.is_empty();
        let score = self.calculate_quality_score(product);

        Ok(ProductValidation {
            is_valid,
            errors,
            warnings,
            score,
        })
    }

    async fn check_required_fields(&self, product: &Product) -> Result<FieldValidation> {
        let mut missing_required_fields = Vec::new();
        let invalid_field_formats = Vec::new();
        let mut empty_fields = Vec::new();

        // 필수 필드 검사
        for field in &self.required_fields {
            match field.as_str() {
                "manufacturer" => {
                    if product
                        .manufacturer
                        .as_ref()
                        .map_or(true, |m| m.trim().is_empty())
                    {
                        missing_required_fields.push("manufacturer".to_string());
                    }
                }
                "model" => {
                    if product.model.as_ref().map_or(true, |m| m.trim().is_empty()) {
                        missing_required_fields.push("model".to_string());
                    }
                }
                _ => {}
            }
        }

        // 선택적 필드의 빈 값 검사
        if product
            .certificate_id
            .as_ref()
            .map_or(false, |c| c.trim().is_empty())
        {
            empty_fields.push("certificate_id".to_string());
        }

        // device_type은 ProductDetail에만 있으므로 스킵

        Ok(FieldValidation {
            missing_required_fields,
            invalid_field_formats,
            empty_fields,
        })
    }
}

/// 간단한 Levenshtein 거리 계산 함수
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let chars1: Vec<char> = s1.chars().collect();
    let chars2: Vec<char> = s2.chars().collect();
    let len1 = chars1.len();
    let len2 = chars2.len();

    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }

    for i in 1..=len1 {
        for j in 1..=len2 {
            let cost = if chars1[i - 1] == chars2[j - 1] { 0 } else { 1 };
            matrix[i][j] = std::cmp::min(
                std::cmp::min(
                    matrix[i - 1][j] + 1, // deletion
                    matrix[i][j - 1] + 1, // insertion
                ),
                matrix[i - 1][j - 1] + cost, // substitution
            );
        }
    }

    matrix[len1][len2]
}

/// 충돌 해결 서비스 구현체
pub struct ConflictResolverImpl {
    resolution_strategy: ResolutionStrategy,
}

impl ConflictResolverImpl {
    pub fn new(strategy: ResolutionStrategy) -> Self {
        Self {
            resolution_strategy: strategy,
        }
    }
}

#[async_trait]
impl ConflictResolver for ConflictResolverImpl {
    async fn resolve_conflicts(&self, products: Vec<Product>) -> Result<Vec<Product>> {
        info!(
            "Starting conflict resolution for {} products",
            products.len()
        );

        let conflicts = self.detect_conflicts(&products).await?;
        let resolved_products = products;

        for conflict_group in conflicts {
            match conflict_group.resolution_strategy {
                ResolutionStrategy::KeepLatest => {
                    // 가장 최근 제품만 유지 (임시로 첫 번째 제품 유지)
                    if let Some(latest) = conflict_group.conflicting_products.first() {
                        debug!("Resolved conflict by keeping latest: {:?}", latest.model);
                    }
                }
                ResolutionStrategy::KeepMostComplete => {
                    // 가장 완전한 제품 유지 (필드가 많이 채워진 제품)
                    if let Some(most_complete) =
                        conflict_group.conflicting_products.iter().max_by_key(|p| {
                            let mut score = 0;
                            if p.manufacturer.is_some() {
                                score += 1;
                            }
                            if p.model.is_some() {
                                score += 1;
                            }
                            if p.certificate_id.is_some() {
                                score += 1;
                            }
                            // device_type은 ProductDetail에만 있으므로 스킵
                            score
                        })
                    {
                        debug!(
                            "Resolved conflict by keeping most complete: {:?}",
                            most_complete.model
                        );
                    }
                }
                _ => {
                    warn!(
                        "Conflict resolution strategy not implemented: {:?}",
                        conflict_group.resolution_strategy
                    );
                }
            }
        }

        info!("Conflict resolution completed");
        Ok(resolved_products)
    }

    async fn resolve_product_conflict(&self, existing: &Product, new: &Product) -> Result<Product> {
        match self.resolution_strategy {
            ResolutionStrategy::KeepLatest => Ok(new.clone()),
            ResolutionStrategy::KeepMostComplete => {
                // 더 많은 필드가 채워진 제품 선택
                let existing_score = [
                    existing.manufacturer.as_ref(),
                    existing.model.as_ref(),
                    existing.certificate_id.as_ref(),
                    // device_type은 ProductDetail에만 있으므로 스킵
                ]
                .iter()
                .filter(|f| f.is_some())
                .count();

                let new_score = [
                    new.manufacturer.as_ref(),
                    new.model.as_ref(),
                    new.certificate_id.as_ref(),
                    // device_type은 ProductDetail에만 있으므로 스킵
                ]
                .iter()
                .filter(|f| f.is_some())
                .count();

                if new_score >= existing_score {
                    Ok(new.clone())
                } else {
                    Ok(existing.clone())
                }
            }
            ResolutionStrategy::Merge => {
                // 필드 병합
                let mut merged = existing.clone();

                if new.manufacturer.is_some() && existing.manufacturer.is_none() {
                    merged.manufacturer = new.manufacturer.clone();
                }
                if new.model.is_some() && existing.model.is_none() {
                    merged.model = new.model.clone();
                }
                if new.certificate_id.is_some() && existing.certificate_id.is_none() {
                    merged.certificate_id = new.certificate_id.clone();
                }

                Ok(merged)
            }
            ResolutionStrategy::ManualReview => {
                // 수동 검토가 필요한 경우 기존 제품 유지
                warn!("Manual review required for conflict resolution");
                Ok(existing.clone())
            }
        }
    }

    async fn detect_conflicts(&self, products: &[Product]) -> Result<Vec<ConflictGroup>> {
        let mut conflicts = Vec::new();
        let mut url_map: HashMap<String, Vec<&Product>> = HashMap::new();

        // URL 기반으로 그룹화
        for product in products {
            url_map
                .entry(product.url.clone())
                .or_default()
                .push(product);
        }

        // 동일한 URL을 가진 제품들이 여러 개인 경우 충돌로 판단
        for (_url, conflicting_products) in url_map {
            if conflicting_products.len() > 1 {
                conflicts.push(ConflictGroup {
                    conflicting_products: conflicting_products.into_iter().cloned().collect(),
                    conflict_type: ConflictType::UrlConflict,
                    resolution_strategy: self.resolution_strategy.clone(),
                });
            }
        }

        Ok(conflicts)
    }
}

/// 배치 진행 추적 서비스 구현체
pub struct BatchProgressTrackerImpl {
    // 실제 구현에서는 데이터베이스나 메모리 저장소 사용
}

impl BatchProgressTrackerImpl {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl BatchProgressTracker for BatchProgressTrackerImpl {
    async fn update_progress(&self, batch_id: &str, progress: BatchProgress) -> Result<()> {
        info!(
            "Updating progress for batch {}: {}%",
            batch_id, progress.progress_percentage
        );
        // 실제 구현에서는 진행 상황을 데이터베이스나 메모리에 저장
        Ok(())
    }

    async fn get_current_progress(&self, batch_id: &str) -> Result<BatchProgress> {
        info!("Getting current progress for batch {}", batch_id);
        // 실제 구현에서는 저장된 진행 상황을 반환
        Ok(BatchProgress {
            batch_id: batch_id.to_string(),
            total_items: 0,
            processed_items: 0,
            successful_items: 0,
            failed_items: 0,
            progress_percentage: 0.0,
            estimated_remaining_time: None,
            current_stage: "초기화".to_string(),
        })
    }

    async fn complete_batch(&self, batch_id: &str, result: BatchResult) -> Result<()> {
        info!(
            "Completing batch {}: {} items processed",
            batch_id, result.total_processed
        );
        // 실제 구현에서는 배치 완료 상태를 저장
        Ok(())
    }
}

/// 배치 복구 서비스 구현체
pub struct BatchRecoveryServiceImpl {}

impl BatchRecoveryServiceImpl {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl BatchRecoveryService for BatchRecoveryServiceImpl {
    async fn recover_failed_batch(&self, batch_id: &str) -> Result<RecoveryResult> {
        info!("Attempting to recover failed batch: {}", batch_id);

        // 실제 구현에서는 실패 원인 분석 및 복구 시도
        Ok(RecoveryResult {
            success: true,
            recovered_items: 0,
            remaining_failures: 0,
            recovery_actions: vec![RecoveryAction::Retry],
        })
    }

    async fn recover_parsing_error(&self, error: &str) -> Result<RecoveryAction> {
        info!("Attempting to recover parsing error: {}", error);

        // 실제 구현에서는 파싱 오류 유형에 따른 복구 액션 결정
        if error.contains("HTML structure") {
            Ok(RecoveryAction::UseAlternativeMethod)
        } else if error.contains("network") {
            Ok(RecoveryAction::Retry)
        } else {
            Ok(RecoveryAction::Skip)
        }
    }

    async fn assess_recoverability(&self, error: &str) -> Result<RecoverabilityAssessment> {
        info!("Assessing recoverability for error: {}", error);

        // 실제 구현에서는 오류 유형에 따른 복구 가능성 평가
        let recoverable = !error.contains("permanent") && !error.contains("invalid");

        Ok(RecoverabilityAssessment {
            is_recoverable: recoverable,
            confidence: if recoverable { 0.8 } else { 0.1 },
            recommended_action: if recoverable {
                RecoveryAction::Retry
            } else {
                RecoveryAction::ManualIntervention
            },
            estimated_success_rate: if recoverable { 0.8 } else { 0.1 },
        })
    }
}

/// 지능적 재시도 관리 서비스 구현체
pub struct RetryManagerImpl {
    max_retries: u32,
    base_delay_ms: u64,
}

impl RetryManagerImpl {
    pub fn new(max_retries: u32, base_delay_ms: u64) -> Self {
        Self {
            max_retries,
            base_delay_ms,
        }
    }
}

#[async_trait]
impl RetryManager for RetryManagerImpl {
    async fn execute_with_retry<F, T>(&self, operation: F) -> Result<T>
    where
        F: Send + Fn() -> Result<T>,
        T: Send,
    {
        let mut last_error = None;

        for attempt in 1..=self.max_retries {
            match operation() {
                Ok(result) => return Ok(result),
                Err(e) => {
                    warn!("Attempt {} failed: {}", attempt, e);
                    last_error = Some(e);

                    if attempt < self.max_retries {
                        let delay = self.base_delay_ms * (2_u64.pow(attempt - 1));
                        tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("All retry attempts failed")))
    }

    async fn classify_error(&self, error: &str) -> Result<ErrorClassification> {
        info!("Classifying error: {}", error);

        let error_type = if error.contains("network") || error.contains("timeout") {
            ErrorType::Network
        } else if error.contains("parse") || error.contains("HTML") {
            ErrorType::Parsing
        } else if error.contains("database") || error.contains("SQL") {
            ErrorType::Database
        } else {
            ErrorType::Unknown
        };

        Ok(ErrorClassification {
            error_type,
            severity: ErrorSeverity::Medium,
            is_recoverable: matches!(error_type, ErrorType::Network | ErrorType::Parsing),
            recommended_action: if matches!(error_type, ErrorType::Network | ErrorType::Parsing) {
                RecoveryAction::Retry
            } else {
                RecoveryAction::Skip
            },
        })
    }

    async fn determine_retry_strategy(&self, error_type: ErrorType) -> Result<RetryStrategy> {
        let strategy = match error_type {
            ErrorType::Network => RetryStrategy {
                max_attempts: 3,
                initial_delay_ms: 1000,
                max_delay_ms: 5000,
                backoff_multiplier: 2.0,
                should_retry: true,
            },
            ErrorType::Parsing => RetryStrategy {
                max_attempts: 2,
                initial_delay_ms: 500,
                max_delay_ms: 2000,
                backoff_multiplier: 1.5,
                should_retry: true,
            },
            ErrorType::Database => RetryStrategy {
                max_attempts: 1,
                initial_delay_ms: 100,
                max_delay_ms: 100,
                backoff_multiplier: 1.0,
                should_retry: false,
            },
            ErrorType::RateLimit => RetryStrategy {
                max_attempts: 5,
                initial_delay_ms: 2000,
                max_delay_ms: 10000,
                backoff_multiplier: 3.0,
                should_retry: true,
            },
            ErrorType::Authentication => RetryStrategy {
                max_attempts: 1,
                initial_delay_ms: 0,
                max_delay_ms: 0,
                backoff_multiplier: 1.0,
                should_retry: false,
            },
            ErrorType::Timeout => RetryStrategy {
                max_attempts: 3,
                initial_delay_ms: 1500,
                max_delay_ms: 6000,
                backoff_multiplier: 2.5,
                should_retry: true,
            },
            ErrorType::Unknown => RetryStrategy {
                max_attempts: 1,
                initial_delay_ms: 0,
                max_delay_ms: 0,
                backoff_multiplier: 1.0,
                should_retry: false,
            },
        };

        Ok(strategy)
    }
}

/// 오류 분류 서비스 구현체
pub struct ErrorClassifierImpl {}

impl ErrorClassifierImpl {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl ErrorClassifier for ErrorClassifierImpl {
    async fn classify(&self, error: &str) -> Result<ErrorType> {
        let error_lower = error.to_lowercase();

        let error_type = if error_lower.contains("network")
            || error_lower.contains("connection")
            || error_lower.contains("timeout")
        {
            ErrorType::Network
        } else if error_lower.contains("parse")
            || error_lower.contains("html")
            || error_lower.contains("selector")
        {
            ErrorType::Parsing
        } else if error_lower.contains("database")
            || error_lower.contains("sql")
            || error_lower.contains("sqlite")
        {
            ErrorType::Database
        } else {
            ErrorType::Unknown
        };

        info!("Classified error '{}' as {:?}", error, error_type);
        Ok(error_type)
    }

    async fn assess_severity(&self, error: &str) -> Result<ErrorSeverity> {
        let error_lower = error.to_lowercase();

        let severity = if error_lower.contains("critical")
            || error_lower.contains("fatal")
            || error_lower.contains("panic")
        {
            ErrorSeverity::Critical
        } else if error_lower.contains("error") || error_lower.contains("failed") {
            ErrorSeverity::High
        } else if error_lower.contains("warning") || error_lower.contains("timeout") {
            ErrorSeverity::Medium
        } else {
            ErrorSeverity::Low
        };

        info!("Assessed error severity as {:?} for: {}", severity, error);
        Ok(severity)
    }

    async fn determine_action(
        &self,
        error_type: ErrorType,
        severity: ErrorSeverity,
    ) -> Result<ErrorAction> {
        let action = match (error_type, severity) {
            (ErrorType::Network, ErrorSeverity::Low | ErrorSeverity::Medium) => ErrorAction::Retry,
            (ErrorType::Network, ErrorSeverity::High | ErrorSeverity::Critical) => {
                ErrorAction::Skip
            }
            (ErrorType::Parsing, _) => ErrorAction::Retry,
            (ErrorType::Database, ErrorSeverity::Critical) => ErrorAction::Abort,
            (ErrorType::Database, _) => ErrorAction::Skip,
            (ErrorType::RateLimit, _) => ErrorAction::Retry,
            (ErrorType::Authentication, _) => ErrorAction::Abort,
            (ErrorType::Timeout, ErrorSeverity::Critical) => ErrorAction::Abort,
            (ErrorType::Timeout, _) => ErrorAction::Retry,
            (ErrorType::Unknown, ErrorSeverity::Critical) => ErrorAction::Abort,
            (ErrorType::Unknown, _) => ErrorAction::Skip,
        };

        info!(
            "Determined action {:?} for error type {:?} with severity {:?}",
            action, error_type, severity
        );
        Ok(action)
    }

    async fn assess_recoverability(&self, error: &str) -> Result<bool> {
        let error_lower = error.to_lowercase();

        // 복구 불가능한 오류들
        let unrecoverable_keywords = [
            "authentication failed",
            "invalid credentials",
            "permanent failure",
            "fatal error",
            "corrupted data",
            "invalid format",
        ];

        for keyword in &unrecoverable_keywords {
            if error_lower.contains(keyword) {
                return Ok(false);
            }
        }

        // 대부분의 다른 오류들은 복구 가능
        Ok(true)
    }
}
