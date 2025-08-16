//! Parsing context and configuration for HTML extraction
//!
//! Provides context objects for maintaining state during parsing operations.

/// Context information for parsing operations
#[derive(Debug, Clone)]
pub struct ParseContext {
    /// Current page being parsed
    pub page_id: u32,

    /// Base URL for resolving relative links
    pub base_url: String,

    /// Expected number of products per page (for validation)
    pub expected_products_per_page: u32,

    /// Additional metadata
    pub metadata: std::collections::HashMap<String, String>,
}

impl ParseContext {
    /// Create new parse context
    pub fn new(page_id: u32, base_url: String) -> Self {
        use crate::infrastructure::config::defaults::DEFAULT_PRODUCTS_PER_PAGE;

        Self {
            page_id,
            base_url,
            expected_products_per_page: DEFAULT_PRODUCTS_PER_PAGE,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Add metadata to context
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Set expected products per page
    pub fn with_expected_products(mut self, count: u32) -> Self {
        self.expected_products_per_page = count;
        self
    }
}

/// Detail parsing context for product detail pages
#[derive(Debug, Clone)]
pub struct DetailParseContext {
    /// Product URL being parsed
    pub url: String,

    /// Source page where this product was found
    pub source_page_id: Option<u32>,

    /// Index within source page
    pub source_index: Option<u32>,

    /// Base URL for resolving relative resources
    pub base_url: String,
}

impl DetailParseContext {
    /// Create new detail parse context
    pub fn new(url: String, base_url: String) -> Self {
        Self {
            url,
            base_url,
            source_page_id: None,
            source_index: None,
        }
    }

    /// Set source information
    pub fn with_source(mut self, page_id: u32, index: u32) -> Self {
        self.source_page_id = Some(page_id);
        self.source_index = Some(index);
        self
    }
}
