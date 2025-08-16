//! Enhanced parsing error types following the guide's comprehensive error handling
//!
//! This module provides detailed error types for HTML parsing operations,
//! with context-aware error reporting and recovery strategies.

use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum ParsingError {
    #[error("Required field '{field}' not found in HTML")]
    RequiredFieldMissing {
        field: String,
        context: Option<String>,
    },

    #[error("Invalid CSS selector: {selector} - {reason}")]
    InvalidSelector {
        selector: String,
        reason: String,
        alternatives: Vec<String>,
    },

    #[error("HTML parsing failed: {message}")]
    HtmlParsingFailed {
        message: String,
        url: Option<String>,
    },

    #[error("No products found on page {page_id}")]
    NoProductsFound {
        page_id: u32,
        tried_selectors: Vec<String>,
    },

    #[error("Product validation failed: {reason}")]
    ProductValidationFailed {
        reason: String,
        field_errors: Vec<String>,
    },

    #[error("URL resolution failed: {url} - {reason}")]
    UrlResolutionFailed {
        url: String,
        reason: String,
        base_url: Option<String>,
    },

    #[error("Matter field extraction failed: {field} - {reason}")]
    MatterFieldExtractionFailed {
        field: String,
        reason: String,
        attempted_selectors: Vec<String>,
    },

    #[error("HTTP request failed: {status} - {message}")]
    HttpRequestFailed {
        status: u16,
        message: String,
        url: String,
    },

    #[error("Content validation failed: {reason}")]
    ContentValidationFailed {
        reason: String,
        content_length: usize,
        expected_indicators: Vec<String>,
    },

    #[error("Rate limit exceeded: {retry_after_seconds}s")]
    RateLimitExceeded {
        retry_after_seconds: u64,
        url: String,
    },

    #[error("Configuration error: {message}")]
    ConfigurationError { message: String, field: String },
}

impl ParsingError {
    /// Create a required field missing error with context
    pub fn required_field_missing(field: &str, context: Option<&str>) -> Self {
        Self::RequiredFieldMissing {
            field: field.to_string(),
            context: context.map(|s| s.to_string()),
        }
    }

    /// Create an invalid selector error with alternatives
    pub fn invalid_selector(selector: &str, reason: &str, alternatives: Vec<String>) -> Self {
        Self::InvalidSelector {
            selector: selector.to_string(),
            reason: reason.to_string(),
            alternatives,
        }
    }

    /// Create a no products found error with tried selectors
    pub fn no_products_found(page_id: u32, tried_selectors: Vec<String>) -> Self {
        Self::NoProductsFound {
            page_id,
            tried_selectors,
        }
    }

    /// Create a Matter field extraction error with attempted selectors
    pub fn matter_field_extraction_failed(
        field: &str,
        reason: &str,
        attempted_selectors: Vec<String>,
    ) -> Self {
        Self::MatterFieldExtractionFailed {
            field: field.to_string(),
            reason: reason.to_string(),
            attempted_selectors,
        }
    }

    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::RequiredFieldMissing { .. } => true,
            Self::InvalidSelector { .. } => true,
            Self::NoProductsFound { .. } => false,
            Self::ProductValidationFailed { .. } => true,
            Self::UrlResolutionFailed { .. } => true,
            Self::MatterFieldExtractionFailed { .. } => true,
            Self::HttpRequestFailed { status, .. } => *status < 500,
            Self::ContentValidationFailed { .. } => true,
            Self::RateLimitExceeded { .. } => true,
            Self::ConfigurationError { .. } => false,
            Self::HtmlParsingFailed { .. } => false,
        }
    }

    /// Get retry delay in seconds for recoverable errors
    pub fn retry_delay_seconds(&self) -> Option<u64> {
        match self {
            Self::RateLimitExceeded {
                retry_after_seconds,
                ..
            } => Some(*retry_after_seconds),
            Self::HttpRequestFailed { status, .. } if *status >= 500 => Some(5),
            _ => None,
        }
    }
}

pub type ParsingResult<T> = Result<T, ParsingError>;
