//! Modern HTML parsing infrastructure for Matter Certis v2
//!
//! This module provides trait-based HTML parsing architecture as per the guide,
//! with comprehensive error handling and robust selector strategies.

pub mod config;
pub mod context;
pub mod error;
pub mod product_detail_parser;
pub mod product_list_parser;

// Re-export public types
pub use config::ParsingConfig;
pub use context::ParseContext;
pub use error::{ParsingError, ParsingResult};
pub use product_detail_parser::ProductDetailParser;
pub use product_list_parser::ProductListParser;

use anyhow::Result;
use scraper::Html;

/// Generic HTML parser trait for type-safe parsing
pub trait HtmlParser {
    type Output;
    type Config;

    /// Parse HTML content with given configuration
    fn parse(&self, html: &str, config: &Self::Config) -> Result<Self::Output>;
}

/// Enhanced parser trait with context support
pub trait ContextualParser {
    type Output;
    type Context;

    /// Parse HTML with contextual information
    fn parse_with_context(
        &self,
        html: &Html,
        context: &Self::Context,
    ) -> ParsingResult<Self::Output>;
}

/// Validation trait for parsed results
pub trait Validator<T> {
    /// Validate parsed data for completeness and correctness
    fn validate(&self, data: &T) -> ParsingResult<()>;
}
