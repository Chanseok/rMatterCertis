//! Application use cases for Matter Certification crawling
//! 
//! Contains the application's use cases and business workflows specific to Matter domain.

use anyhow::{Result, anyhow};
use std::sync::Arc;
use std::collections::HashSet;
use uuid::Uuid;
use chrono::Utc;

use crate::domain::entities::{
    Vendor, Product, MatterProduct, 
    CrawlerConfig, ValidationResult, ValidationSummary, DatabaseSummary
};
use crate::domain::repositories::{VendorRepository, ProductRepository};
use crate::domain::session_manager::{SessionManager, CrawlingStage, SessionStatus, CrawlingSessionState};
use crate::application::dto::{
    CreateVendorDto, UpdateVendorDto, VendorResponseDto,
    CreateProductDto, ProductResponseDto,
    CreateMatterProductDto, MatterProductResponseDto,
    ProductSearchDto, MatterProductFilterDto, ProductSearchResultDto,
    DatabaseSummaryDto
};

// ============================================================================
// DTO-enabled Use Cases for Tauri Commands
// ============================================================================

/// DTO-enabled use cases for vendor management
pub struct VendorUseCases {
    vendor_repo: Arc<dyn VendorRepository>,
}

impl VendorUseCases {
    pub fn new(vendor_repo: Arc<dyn VendorRepository>) -> Self {
        Self { vendor_repo }
    }

    /// Create a new vendor with validation
    pub async fn create_vendor(&self, dto: CreateVendorDto) -> Result<VendorResponseDto> {
        // Input validation
        if dto.vendor_number == 0 {
            return Err(anyhow!("Vendor number must be greater than 0 for Matter certification"));
        }
        
        if dto.vendor_name.trim().is_empty() {
            return Err(anyhow!("Vendor name cannot be empty"));
        }

        if dto.company_legal_name.trim().is_empty() {
            return Err(anyhow!("Company legal name is required for Matter certification"));
        }

        // Check for duplicate vendor number
        if let Some(_) = self.vendor_repo.find_by_number(dto.vendor_number).await? {
            return Err(anyhow!("Vendor number already exists: {}", dto.vendor_number));
        }

        // Create vendor entity
        let vendor = Vendor {
            vendor_id: Uuid::new_v4().to_string(),
            vendor_number: dto.vendor_number,
            vendor_name: dto.vendor_name.trim().to_string(),
            company_legal_name: dto.company_legal_name.trim().to_string(),
            created_at: Utc::now(),
        };

        // Save to repository
        self.vendor_repo.create(&vendor).await?;

        Ok(VendorResponseDto::from(vendor))
    }

    /// Get all vendors
    pub async fn get_all_vendors(&self) -> Result<Vec<VendorResponseDto>> {
        let vendors = self.vendor_repo.find_all().await?;
        Ok(vendors.into_iter().map(VendorResponseDto::from).collect())
    }

    /// Get vendor by ID
    pub async fn get_vendor_by_id(&self, vendor_id: &str) -> Result<Option<VendorResponseDto>> {
        if let Some(vendor) = self.vendor_repo.find_by_id(vendor_id).await? {
            Ok(Some(VendorResponseDto::from(vendor)))
        } else {
            Ok(None)
        }
    }

    /// Search vendors by name
    pub async fn search_vendors_by_name(&self, name: &str) -> Result<Vec<VendorResponseDto>> {
        let vendors = self.vendor_repo.search_by_name(name).await?;
        Ok(vendors.into_iter().map(VendorResponseDto::from).collect())
    }

    /// Update vendor
    pub async fn update_vendor(&self, vendor_id: &str, dto: UpdateVendorDto) -> Result<VendorResponseDto> {
        let mut vendor = self.vendor_repo.find_by_id(vendor_id).await?
            .ok_or_else(|| anyhow!("Vendor not found: {}", vendor_id))?;

        // Update fields if provided
        if let Some(vendor_name) = dto.vendor_name {
            if vendor_name.trim().is_empty() {
                return Err(anyhow!("Vendor name cannot be empty"));
            }
            vendor.vendor_name = vendor_name.trim().to_string();
        }

        if let Some(company_legal_name) = dto.company_legal_name {
            if company_legal_name.trim().is_empty() {
                return Err(anyhow!("Company legal name cannot be empty"));
            }
            vendor.company_legal_name = company_legal_name.trim().to_string();
        }

        self.vendor_repo.update(&vendor).await?;
        Ok(VendorResponseDto::from(vendor))
    }

    /// Delete vendor
    pub async fn delete_vendor(&self, vendor_id: &str) -> Result<()> {
        // Check if vendor exists
        if self.vendor_repo.find_by_id(vendor_id).await?.is_none() {
            return Err(anyhow!("Vendor not found: {}", vendor_id));
        }

        self.vendor_repo.delete(vendor_id).await?;
        Ok(())
    }
}

/// DTO-enabled use cases for Matter product management
pub struct MatterProductUseCases {
    product_repo: Arc<dyn ProductRepository>,
}

impl MatterProductUseCases {
    pub fn new(product_repo: Arc<dyn ProductRepository>) -> Self {
        Self { product_repo }
    }

    /// Create a basic product
    pub async fn create_product(&self, dto: CreateProductDto) -> Result<ProductResponseDto> {
        // URL validation
        if dto.url.trim().is_empty() {
            return Err(anyhow!("Product URL is required"));
        }

        if !dto.url.starts_with("http") {
            return Err(anyhow!("Invalid URL format: must start with http"));
        }

        // Check for duplicate URL
        if let Some(_) = self.product_repo.find_product_by_url(&dto.url).await? {
            return Err(anyhow!("Product already exists: {}", dto.url));
        }

        // Create product entity
        let product = Product {
            url: dto.url.trim().to_string(),
            manufacturer: dto.manufacturer,
            model: dto.model,
            certificate_id: dto.certificate_id,
            page_id: dto.page_id,
            index_in_page: dto.index_in_page,
            created_at: Utc::now(),
        };

        self.product_repo.save_product(&product).await?;
        Ok(ProductResponseDto::from(product))
    }

    /// Create a Matter product (with automatic Product creation if needed)
    pub async fn create_matter_product(&self, dto: CreateMatterProductDto) -> Result<MatterProductResponseDto> {
        // URL validation
        if dto.url.trim().is_empty() {
            return Err(anyhow!("Product URL is required"));
        }

        // Ensure basic Product exists first
        if self.product_repo.find_product_by_url(&dto.url).await?.is_none() {
            let basic_product = Product {
                url: dto.url.clone(),
                manufacturer: dto.manufacturer.clone(),
                model: dto.model.clone(),
                certificate_id: dto.certificate_id.clone(),
                page_id: dto.page_id,
                index_in_page: dto.index_in_page,
                created_at: Utc::now(),
            };
            self.product_repo.save_product(&basic_product).await?;
        }

        // Create MatterProduct entity
        let matter_product = MatterProduct {
            url: dto.url.trim().to_string(),
            page_id: dto.page_id,
            index_in_page: dto.index_in_page,
            id: dto.id,
            manufacturer: dto.manufacturer,
            model: dto.model,
            device_type: dto.device_type,
            certificate_id: dto.certificate_id,
            certification_date: dto.certification_date,
            software_version: dto.software_version,
            hardware_version: dto.hardware_version,
            vid: dto.vid,
            pid: dto.pid,
            family_sku: dto.family_sku,
            family_variant_sku: dto.family_variant_sku,
            firmware_version: dto.firmware_version,
            family_id: dto.family_id,
            tis_trp_tested: dto.tis_trp_tested,
            specification_version: dto.specification_version,
            transport_interface: dto.transport_interface,
            primary_device_type_id: dto.primary_device_type_id,
            application_categories: dto.application_categories,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        self.product_repo.save_matter_product(&matter_product).await?;
        Ok(MatterProductResponseDto::from(matter_product))
    }

    /// Search Matter products with pagination
    pub async fn search_matter_products(&self, dto: ProductSearchDto) -> Result<ProductSearchResultDto> {
        let page = dto.page.unwrap_or(1);
        let page_size = dto.page_size.unwrap_or(20);

        if page == 0 {
            return Err(anyhow!("Page number must be greater than 0"));
        }

        if page_size == 0 || page_size > 100 {
            return Err(anyhow!("Page size must be between 1 and 100"));
        }

        let products = self.product_repo.search_products(&dto.query).await?;
        let total_count = products.len() as u32;
        let total_pages = (total_count + page_size - 1) / page_size;

        // Simple pagination (could be optimized with repository-level pagination)
        let start_idx = ((page - 1) * page_size) as usize;
        let end_idx = (start_idx + page_size as usize).min(products.len());
        
        let paginated_products = if start_idx < products.len() {
            products[start_idx..end_idx].to_vec()
        } else {
            Vec::new()
        };

        Ok(ProductSearchResultDto {
            products: paginated_products.into_iter().map(MatterProductResponseDto::from).collect(),
            total_count,
            page,
            page_size,
            total_pages,
        })
    }

    /// Filter Matter products by criteria
    pub async fn filter_matter_products(&self, filter: MatterProductFilterDto) -> Result<ProductSearchResultDto> {
        let page = filter.page.unwrap_or(1);
        let page_size = filter.page_size.unwrap_or(20);

        if page == 0 {
            return Err(anyhow!("Page number must be greater than 0"));
        }

        // Apply filters sequentially (could be optimized with compound queries)
        let mut products = if let Some(manufacturer) = &filter.manufacturer {
            self.product_repo.find_by_manufacturer(manufacturer).await?
        } else if let Some(device_type) = &filter.device_type {
            self.product_repo.find_by_device_type(device_type).await?
        } else if let Some(vid) = &filter.vid {
            self.product_repo.find_by_vid(vid).await?
        } else {
            // Get all matter products with pagination
            let (all_products, _) = self.product_repo.get_matter_products_paginated(page, page_size).await?;
            all_products
        };

        // Apply date range filter if provided
        if let (Some(start_date), Some(end_date)) = (&filter.certification_date_start, &filter.certification_date_end) {
            let date_filtered = self.product_repo.find_by_certification_date_range(start_date, end_date).await?;
            
            // Find intersection of date filter and other filters
            let date_urls: HashSet<String> = date_filtered.into_iter().map(|p| p.url).collect();
            products.retain(|p| date_urls.contains(&p.url));
        }

        let total_count = products.len() as u32;
        let total_pages = (total_count + page_size - 1) / page_size;

        // Apply pagination
        let start_idx = ((page - 1) * page_size) as usize;
        let end_idx = (start_idx + page_size as usize).min(products.len());
        
        let paginated_products = if start_idx < products.len() {
            products[start_idx..end_idx].to_vec()
        } else {
            Vec::new()
        };

        Ok(ProductSearchResultDto {
            products: paginated_products.into_iter().map(MatterProductResponseDto::from).collect(),
            total_count,
            page,
            page_size,
            total_pages,
        })
    }

    /// Get database summary
    pub async fn get_database_summary(&self) -> Result<DatabaseSummaryDto> {
        let summary = self.product_repo.get_database_summary().await?;
        Ok(DatabaseSummaryDto::from(summary))
    }

    /// Delete product by URL
    pub async fn delete_product(&self, url: &str) -> Result<()> {
        // Delete both MatterProduct and basic Product
        self.product_repo.delete_matter_product(url).await?;
        self.product_repo.delete_product(url).await?;
        Ok(())
    }
}

/// Use cases for starting and managing crawling operations with memory-based session management
pub struct CrawlingUseCases {
    product_repo: Arc<dyn ProductRepository>,
    #[allow(dead_code)]
    vendor_repo: Arc<dyn VendorRepository>,
    session_manager: Arc<SessionManager>,
}

impl CrawlingUseCases {
    pub fn new(
        product_repo: Arc<dyn ProductRepository>,
        vendor_repo: Arc<dyn VendorRepository>,
        session_manager: Arc<SessionManager>,
    ) -> Self {
        Self {
            product_repo,
            vendor_repo,
            session_manager,
        }
    }

    /// Start a new crawling session (in-memory)
    pub async fn start_crawling(&self, session_id: &str, start_url: &str, target_domains: Vec<String>) -> Result<()> {
        self.session_manager.start_session_simple(session_id, start_url, target_domains).await
    }

    /// Update crawling session progress
    pub async fn update_crawling_progress(
        &self,
        session_id: &str,
        progress: u32,
        current_step: &str,
    ) -> Result<()> {
        self.session_manager.update_session_progress(session_id, progress, current_step.to_string()).await
    }

    /// Complete crawling session
    pub async fn complete_crawling(&self, session_id: &str) -> Result<()> {
        self.session_manager.complete_session_simple(session_id).await
    }

    /// Get session status
    pub async fn get_session_status(&self, session_id: &str) -> Result<Option<CrawlingSessionState>> {
        self.session_manager.get_session_state(session_id).await
    }

    /// Validate products against existing database (Stage 1.5)
    pub async fn validate_products(&self, products: Vec<Product>) -> Result<ValidationResult> {
        let start_time = std::time::Instant::now();
        
        // Get existing URLs from database
        let existing_urls = self.product_repo.get_existing_urls().await?;
        
        // Classify products
        let mut new_products = Vec::new();
        let mut existing_products = Vec::new();
        let mut duplicate_products = Vec::new();
        let mut seen_urls = HashSet::new();
        
        for product in products {
            // Check for duplicates within the current collection
            if seen_urls.contains(&product.url) {
                duplicate_products.push(product);
                continue;
            }
            seen_urls.insert(product.url.clone());
            
            // Check against database
            if existing_urls.contains(&product.url) {
                existing_products.push(product);
            } else {
                new_products.push(product);
            }
        }
        
        let validation_time_ms = start_time.elapsed().as_millis() as u64;
        
        Ok(ValidationResult {
            summary: ValidationSummary {
                total_products: seen_urls.len(),
                new_products: new_products.len(),
                existing_products: existing_products.len(),
                duplicate_products: duplicate_products.len(),
                validation_time_ms,
            },
            new_products,
            existing_products,
            duplicate_products,
        })
    }

    /// Save products to database (Stage 1 results)
    pub async fn save_products(&self, products: &[Product]) -> Result<()> {
        self.product_repo.save_products_batch(products).await
    }

    /// Save Matter products to database (Stage 2 results)
    pub async fn save_matter_products(&self, products: &[MatterProduct]) -> Result<()> {
        self.product_repo.save_matter_products_batch(products).await
    }
}

/// Use cases for product management and querying
pub struct ProductUseCases {
    product_repo: Arc<dyn ProductRepository>,
}

impl ProductUseCases {
    pub fn new(product_repo: Arc<dyn ProductRepository>) -> Self {
        Self { product_repo }
    }

    /// Get paginated products
    pub async fn get_products(&self, page: u32, limit: u32) -> Result<(Vec<Product>, u32)> {
        self.product_repo.get_products_paginated(page, limit).await
    }

    /// Get paginated Matter products with full details
    pub async fn get_matter_products(&self, page: u32, limit: u32) -> Result<(Vec<MatterProduct>, u32)> {
        self.product_repo.get_matter_products_paginated(page, limit).await
    }

    /// Search products by text query
    pub async fn search_products(&self, query: &str) -> Result<Vec<MatterProduct>> {
        self.product_repo.search_products(query).await
    }

    /// Find products by manufacturer
    pub async fn find_by_manufacturer(&self, manufacturer: &str) -> Result<Vec<MatterProduct>> {
        self.product_repo.find_by_manufacturer(manufacturer).await
    }

    /// Find products by device type
    pub async fn find_by_device_type(&self, device_type: &str) -> Result<Vec<MatterProduct>> {
        self.product_repo.find_by_device_type(device_type).await
    }

    /// Find products by Vendor ID
    pub async fn find_by_vid(&self, vid: &str) -> Result<Vec<MatterProduct>> {
        self.product_repo.find_by_vid(vid).await
    }

    /// Get database summary statistics
    pub async fn get_database_summary(&self) -> Result<DatabaseSummary> {
        self.product_repo.get_database_summary().await
    }

    /// Delete a product by URL
    pub async fn delete_product(&self, url: &str) -> Result<()> {
        self.product_repo.delete_product(url).await
    }
}

// Note: Session management is now handled by SessionManager in the domain layer
// Final crawling results are persisted using CrawlingResultRepository
