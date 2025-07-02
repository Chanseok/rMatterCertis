//! 고급 데이터 처리 서비스 트레이트 정의
//! 
//! 이 모듈은 중복 제거, 유효성 검사, 충돌 해결 등의 
//! 고급 데이터 처리 기능을 제공하는 서비스들의 인터페이스를 정의합니다.

use async_trait::async_trait;
use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::domain::product::Product;

/// 중복 제거 서비스
#[async_trait]
pub trait DeduplicationService: Send + Sync {
    /// 제품 목록에서 중복 제거
    async fn remove_duplicates(&self, products: Vec<Product>) -> Result<Vec<Product>>;
    
    /// 중복 제품 분석
    async fn analyze_duplicates(&self, products: &[Product]) -> Result<DuplicationAnalysis>;
    
    /// 중복 검사 (단일 제품)
    async fn is_duplicate(&self, product: &Product, existing: &[Product]) -> Result<bool>;
}

/// 데이터 유효성 검사 서비스
#[async_trait]
pub trait ValidationService: Send + Sync {
    /// 모든 제품 유효성 검사
    async fn validate_all(&self, products: Vec<Product>) -> Result<ValidationResult>;
    
    /// 단일 제품 유효성 검사
    async fn validate_product(&self, product: &Product) -> Result<ProductValidation>;
    
    /// 필수 필드 검사
    async fn check_required_fields(&self, product: &Product) -> Result<FieldValidation>;
}

/// 데이터 충돌 해결 서비스
#[async_trait]
pub trait ConflictResolver: Send + Sync {
    /// 충돌하는 제품들 해결
    async fn resolve_conflicts(&self, products: Vec<Product>) -> Result<Vec<Product>>;
    
    /// 두 제품 간 충돌 해결
    async fn resolve_product_conflict(&self, existing: &Product, new: &Product) -> Result<Product>;
    
    /// 충돌 감지
    async fn detect_conflicts(&self, products: &[Product]) -> Result<Vec<ConflictGroup>>;
}

/// 배치 진행 추적 서비스
#[async_trait]
pub trait BatchProgressTracker: Send + Sync {
    /// 배치 진행 상황 업데이트
    async fn update_progress(&self, batch_id: &str, progress: BatchProgress) -> Result<()>;
    
    /// 현재 진행 상황 조회
    async fn get_current_progress(&self, batch_id: &str) -> Result<BatchProgress>;
    
    /// 배치 완료 처리
    async fn complete_batch(&self, batch_id: &str, result: BatchResult) -> Result<()>;
}

/// 배치 복구 서비스
#[async_trait]
pub trait BatchRecoveryService: Send + Sync {
    /// 실패한 배치 복구
    async fn recover_failed_batch(&self, batch_id: &str) -> Result<RecoveryResult>;
    
    /// 파싱 오류 복구
    async fn recover_parsing_error(&self, error: &str) -> Result<RecoveryAction>;
    
    /// 복구 가능성 평가
    async fn assess_recoverability(&self, error: &str) -> Result<RecoverabilityAssessment>;
}

/// 지능적 재시도 관리 서비스
#[async_trait]
pub trait RetryManager: Send + Sync {
    /// 재시도 실행
    async fn execute_with_retry<F, T>(&self, operation: F) -> Result<T>
    where
        F: Send + Fn() -> Result<T>,
        T: Send;
    
    /// 오류 분류
    async fn classify_error(&self, error: &str) -> Result<ErrorClassification>;
    
    /// 재시도 전략 결정
    async fn determine_retry_strategy(&self, error_type: ErrorType) -> Result<RetryStrategy>;
}

/// 오류 분류 서비스
#[async_trait]
pub trait ErrorClassifier: Send + Sync {
    /// 오류 분류
    async fn classify(&self, error: &str) -> Result<ErrorType>;
    
    /// 오류 심각도 평가
    async fn assess_severity(&self, error: &str) -> Result<ErrorSeverity>;
    
    /// 복구 가능성 평가
    async fn assess_recoverability(&self, error: &str) -> Result<bool>;
    
    /// 오류 처리 액션 결정
    async fn determine_action(&self, error_type: ErrorType, severity: ErrorSeverity) -> Result<ErrorAction>;
}

/// 오류 처리 액션
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ErrorAction {
    Retry,
    Skip,
    Abort,
    ManualIntervention,
}

// === 데이터 구조체들 ===

/// 중복 분석 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicationAnalysis {
    pub total_duplicates: u32,
    pub duplicate_rate: f64,
    pub duplicate_groups: Vec<DuplicateProductGroup>,
    pub unique_products: u32,
}

/// 중복 제품 그룹
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateProductGroup {
    pub products: Vec<Product>,
    pub similarity_score: f64,
    pub duplicate_type: DuplicationType,
}

/// 중복 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DuplicationType {
    ExactMatch,
    SimilarModel { similarity: f64 },
    SameManufacturerModel,
    SimilarCertificationId,
}

/// 유효성 검사 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid_products: Vec<Product>,
    pub invalid_products: Vec<InvalidProduct>,
    pub validation_summary: ValidationSummary,
}

/// 유효하지 않은 제품 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidProduct {
    pub product: Product,
    pub validation_errors: Vec<ValidationError>,
}

/// 제품 유효성 검사 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductValidation {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
    pub score: f64, // 0.0 ~ 1.0
}

/// 필드 유효성 검사 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldValidation {
    pub missing_required_fields: Vec<String>,
    pub invalid_field_formats: Vec<FieldFormatError>,
    pub empty_fields: Vec<String>,
}

/// 유효성 검사 오류
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub field: String,
    pub error_type: ValidationErrorType,
    pub message: String,
}

/// 유효성 검사 경고
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationWarning {
    pub field: String,
    pub warning_type: ValidationWarningType,
    pub message: String,
}

/// 필드 형식 오류
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldFormatError {
    pub field: String,
    pub expected_format: String,
    pub actual_value: String,
}

/// 유효성 검사 요약
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSummary {
    pub total_products: u32,
    pub valid_products: u32,
    pub invalid_products: u32,
    pub validation_rate: f64,
    pub common_errors: Vec<String>,
}

/// 유효성 검사 오류 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationErrorType {
    MissingRequiredField,
    InvalidFormat,
    OutOfRange,
    InvalidValue,
}

/// 유효성 검사 경고 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationWarningType {
    EmptyOptionalField,
    SuspiciousValue,
    InconsistentData,
}

/// 충돌 그룹
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictGroup {
    pub conflicting_products: Vec<Product>,
    pub conflict_type: ConflictType,
    pub resolution_strategy: ResolutionStrategy,
}

/// 충돌 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictType {
    DuplicateId,
    InconsistentData,
    VersionConflict,
    UrlConflict,
}

/// 해결 전략
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResolutionStrategy {
    KeepLatest,
    KeepMostComplete,
    Merge,
    ManualReview,
}

/// 배치 진행 상황
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProgress {
    pub batch_id: String,
    pub total_items: u32,
    pub processed_items: u32,
    pub successful_items: u32,
    pub failed_items: u32,
    pub progress_percentage: f64,
    pub estimated_remaining_time: Option<u64>,
    pub current_stage: String,
}

/// 배치 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    pub batch_id: String,
    pub total_processed: u32,
    pub successful: u32,
    pub failed: u32,
    pub duration_ms: u64,
    pub errors: Vec<String>,
}

/// 복구 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryResult {
    pub success: bool,
    pub recovered_items: u32,
    pub remaining_failures: u32,
    pub recovery_actions: Vec<RecoveryAction>,
}

/// 복구 액션
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryAction {
    Retry,
    Skip,
    ManualIntervention,
    UseAlternativeMethod,
}

/// 복구 가능성 평가
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoverabilityAssessment {
    pub is_recoverable: bool,
    pub confidence: f64,
    pub recommended_action: RecoveryAction,
    pub estimated_success_rate: f64,
}

/// 오류 분류
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorClassification {
    pub error_type: ErrorType,
    pub severity: ErrorSeverity,
    pub is_recoverable: bool,
    pub recommended_action: RecoveryAction,
}

/// 오류 타입
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ErrorType {
    Network,
    Parsing,
    Database,
    RateLimit,
    Authentication,
    Timeout,
    Unknown,
}

/// 오류 심각도
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ErrorSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// 재시도 전략
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryStrategy {
    pub max_attempts: u32,
    pub initial_delay_ms: u64,
    pub backoff_multiplier: f64,
    pub max_delay_ms: u64,
    pub should_retry: bool,
}
