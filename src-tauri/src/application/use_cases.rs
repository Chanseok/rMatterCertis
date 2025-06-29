//! Application use cases for Matter Certification crawling
//! 
//! Contains the application's use cases and business workflows specific to Matter domain.

use anyhow::{Result, anyhow};
use std::sync::Arc;
use std::collections::HashSet;
use uuid::Uuid;
use chrono::Utc;

use crate::domain::entities::{
    Vendor, Product, MatterProduct, ValidationResult, ValidationSummary, DatabaseSummary
};
use crate::domain::repositories::{VendorRepository, ProductRepository};
use crate::domain::session_manager::{SessionManager, CrawlingSessionState};
use crate::application::dto::{
    CreateVendorDto, UpdateVendorDto, VendorResponseDto, ProductResponseDto,
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
            return Err(anyhow!("Company legal name cannot be empty"));
        }

        // Create vendor entity
        let vendor = Vendor {
            id: Uuid::new_v4().to_string(),
            vendor_number: dto.vendor_number,
            vendor_name: dto.vendor_name,
            company_legal_name: dto.company_legal_name,
            vendor_url: dto.vendor_url,
            csa_assigned_number: dto.csa_assigned_number,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let saved_vendor = self.vendor_repo.save(&vendor).await?;
        Ok(VendorResponseDto::from(saved_vendor))
    }

    /// Update an existing vendor
    pub async fn update_vendor(&self, vendor_id: &str, dto: UpdateVendorDto) -> Result<VendorResponseDto> {
        let mut vendor = self.vendor_repo.find_by_id(vendor_id)
            .await?
            .ok_or_else(|| anyhow!("Vendor not found"))?;

        // Update fields if provided
        if let Some(vendor_name) = dto.vendor_name {
            if vendor_name.trim().is_empty() {
                return Err(anyhow!("Vendor name cannot be empty"));
            }
            vendor.vendor_name = vendor_name;
        }

        if let Some(company_legal_name) = dto.company_legal_name {
            if company_legal_name.trim().is_empty() {
                return Err(anyhow!("Company legal name cannot be empty"));
            }
            vendor.company_legal_name = company_legal_name;
        }

        if let Some(vendor_url) = dto.vendor_url {
            vendor.vendor_url = Some(vendor_url);
        }

        if let Some(csa_assigned_number) = dto.csa_assigned_number {
            vendor.csa_assigned_number = Some(csa_assigned_number);
        }

        vendor.updated_at = Utc::now();

        let updated_vendor = self.vendor_repo.save(&vendor).await?;
        Ok(VendorResponseDto::from(updated_vendor))
    }

    /// Get vendor by ID
    pub async fn get_vendor(&self, vendor_id: &str) -> Result<Option<VendorResponseDto>> {
        let vendor = self.vendor_repo.find_by_id(vendor_id).await?;
        Ok(vendor.map(VendorResponseDto::from))
    }

    /// List all vendors with pagination
    pub async fn list_vendors(&self, page: u32, limit: u32) -> Result<(Vec<VendorResponseDto>, u32)> {
        let (vendors, total) = self.vendor_repo.find_all_paginated(page, limit).await?;
        let vendor_dtos = vendors.into_iter().map(VendorResponseDto::from).collect();
        Ok((vendor_dtos, total))
    }

    /// Search vendors by name
    pub async fn search_vendors(&self, query: &str) -> Result<Vec<VendorResponseDto>> {
        let vendors = self.vendor_repo.search_by_name(query).await?;
        Ok(vendors.into_iter().map(VendorResponseDto::from).collect())
    }

    /// Delete vendor
    pub async fn delete_vendor(&self, vendor_id: &str) -> Result<()> {
        let exists = self.vendor_repo.find_by_id(vendor_id).await?.is_some();
        if !exists {
            return Err(anyhow!("Vendor not found"));
        }
        
        self.vendor_repo.delete(vendor_id).await
    }

    /// Get all vendors
    pub async fn get_all_vendors(&self) -> Result<Vec<VendorResponseDto>> {
        let vendors = self.vendor_repo.find_all().await?;
        Ok(vendors.into_iter().map(VendorResponseDto::from).collect())
    }
}

// ============================================================================
// Matter Product Use Cases
// ============================================================================

/// Use cases for Matter-specific product management
pub struct MatterProductUseCases {
    product_repo: Arc<dyn ProductRepository>,
}

impl MatterProductUseCases {
    pub fn new(product_repo: Arc<dyn ProductRepository>) -> Self {
        Self { product_repo }
    }

    /// Create a new Matter product from DTO
    pub async fn create_matter_product(&self, dto: CreateMatterProductDto) -> Result<MatterProductResponseDto> {
        // Validate required fields
        if dto.url.trim().is_empty() {
            return Err(anyhow!("Product URL cannot be empty"));
        }

        // Parse application_categories from JSON string to Vec<String>
        let application_categories = if let Some(categories_json) = &dto.application_categories {
            serde_json::from_str::<Vec<String>>(categories_json).unwrap_or_default()
        } else {
            Vec::new()
        };

        // Create MatterProduct entity
        let matter_product = MatterProduct {
            url: dto.url,
            page_id: dto.page_id,
            index_in_page: None, // Not provided in this DTO
            id: None,            // Not provided in this DTO
            manufacturer: dto.manufacturer,
            model: None,         // Not provided in this DTO
            device_type: dto.device_type,
            certificate_id: None, // Not provided in this DTO
            certification_date: dto.certification_date,
            software_version: None,    // Not provided in this DTO
            hardware_version: None,    // Not provided in this DTO
            vid: dto.vid,
            pid: dto.pid,
            family_sku: None,          // Not provided in this DTO
            family_variant_sku: None,  // Not provided in this DTO
            firmware_version: None,    // Not provided in this DTO
            family_id: None,           // Not provided in this DTO
            tis_trp_tested: None,      // Not provided in this DTO
            specification_version: None, // Not provided in this DTO
            transport_interface: None,   // Not provided in this DTO
            primary_device_type_id: None, // Not provided in this DTO
            application_categories,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        self.product_repo.save_matter_product(&matter_product).await?;
        Ok(MatterProductResponseDto::from(matter_product))
    }

    /// Get Matter product by URL
    pub async fn get_matter_product(&self, url: &str) -> Result<Option<MatterProductResponseDto>> {
        let product = self.product_repo.find_matter_product_by_url(url).await?;
        Ok(product.map(MatterProductResponseDto::from))
    }

    /// List Matter products with advanced filtering
    pub async fn filter_matter_products(&self, filter: MatterProductFilterDto) -> Result<ProductSearchResultDto> {
        let products = self.product_repo.filter_matter_products(
            filter.manufacturer.as_deref(),
            filter.device_type.as_deref(),
            filter.vid.as_deref(),
            filter.certification_date_start.as_deref(),
            filter.certification_date_end.as_deref(),
        ).await?;

        // Apply pagination
        let total_count = products.len();
        let page = filter.page.unwrap_or(1);
        let page_size = filter.page_size.unwrap_or(10) as usize;
        let offset = ((page - 1) * page_size as u32) as usize;

        let paginated_products: Vec<MatterProduct> = products
            .into_iter()
            .skip(offset)
            .take(page_size)
            .collect();

        Ok(ProductSearchResultDto {
            products: paginated_products.into_iter().map(MatterProductResponseDto::from).collect(),
            total_count: total_count as u32,
            page,
            page_size: page_size as u32,
            total_pages: ((total_count as f64) / (page_size as f64)).ceil() as u32,
        })
    }

    /// Get recent Matter products
    pub async fn get_recent_matter_products(&self, limit: u32) -> Result<Vec<MatterProductResponseDto>> {
        let products = self.product_repo.get_recent_matter_products(limit).await?;
        Ok(products.into_iter().map(MatterProductResponseDto::from).collect())
    }

    /// Get unique manufacturers
    pub async fn get_manufacturers(&self) -> Result<Vec<String>> {
        self.product_repo.get_unique_manufacturers().await
    }

    /// Get unique device types
    pub async fn get_device_types(&self) -> Result<Vec<String>> {
        self.product_repo.get_unique_device_types().await
    }

    /// Get Matter products by manufacturer with DTO conversion
    pub async fn get_matter_products_by_manufacturer(&self, manufacturer: &str) -> Result<Vec<MatterProductResponseDto>> {
        let products = self.product_repo.find_by_manufacturer(manufacturer).await?;
        Ok(products.into_iter().map(MatterProductResponseDto::from).collect())
    }

    /// Search Matter products by text with DTO conversion
    pub async fn search_matter_products(&self, search_dto: ProductSearchDto) -> Result<ProductSearchResultDto> {
        let query = search_dto.query.as_deref().unwrap_or("");
        let products = self.product_repo.search_products(query).await?;
        let product_dtos: Vec<MatterProductResponseDto> = products.into_iter().map(MatterProductResponseDto::from).collect();
        
        let total_count = product_dtos.len() as u32;
        let page_size = search_dto.page_size.unwrap_or(50);
        let total_pages = total_count.div_ceil(page_size); // Calculate ceiling division
        
        Ok(ProductSearchResultDto {
            products: product_dtos,
            total_count,
            page: search_dto.page.unwrap_or(1),
            page_size,
            total_pages,
        })
    }

    /// Get database summary with DTO conversion
    pub async fn get_database_summary(&self) -> Result<DatabaseSummaryDto> {
        let summary = self.product_repo.get_database_summary().await?;
        Ok(DatabaseSummaryDto::from(summary))
    }

    /// Get all products (basic)
    pub async fn get_all_products(&self) -> Result<Vec<ProductResponseDto>> {
        let products = self.product_repo.get_all_products().await?;
        Ok(products.into_iter().map(ProductResponseDto::from).collect())
    }

    /// Get all Matter products
    pub async fn get_all_matter_products(&self) -> Result<Vec<MatterProductResponseDto>> {
        let products = self.product_repo.get_all_matter_products().await?;
        Ok(products.into_iter().map(MatterProductResponseDto::from).collect())
    }

    /// Search products with pagination
    pub async fn search_products(&self, dto: ProductSearchDto) -> Result<ProductSearchResultDto> {
        let all_matter_products = if let Some(query) = &dto.query {
            if !query.trim().is_empty() {
                self.product_repo.search_products(query).await?
            } else {
                self.product_repo.get_all_matter_products().await?
            }
        } else {
            self.product_repo.get_all_matter_products().await?
        };

        let total_count = all_matter_products.len();
        let page = dto.page.unwrap_or(1);
        let page_size = dto.page_size.unwrap_or(10) as usize;
        let offset = ((page - 1) * page_size as u32) as usize;

        let paginated_products: Vec<MatterProduct> = all_matter_products
            .into_iter()
            .skip(offset)
            .take(page_size)
            .collect();

        Ok(ProductSearchResultDto {
            products: paginated_products.into_iter().map(MatterProductResponseDto::from).collect(),
            total_count: total_count as u32,
            page,
            page_size: page_size as u32,
            total_pages: ((total_count as f64) / (page_size as f64)).ceil() as u32,
        })
    }

    /// Get products by manufacturer
    pub async fn get_products_by_manufacturer(&self, manufacturer: &str) -> Result<Vec<MatterProductResponseDto>> {
        let products = self.product_repo.find_by_manufacturer(manufacturer).await?;
        Ok(products.into_iter().map(MatterProductResponseDto::from).collect())
    }

    /// Get recent products
    pub async fn get_recent_products(&self, limit: u32) -> Result<Vec<MatterProductResponseDto>> {
        let products = self.product_repo.get_recent_matter_products(limit).await?;
        Ok(products.into_iter().map(MatterProductResponseDto::from).collect())
    }

    /// Delete a product by URL
    pub async fn delete_product(&self, url: &str) -> Result<()> {
        self.product_repo.delete_matter_product(url).await
    }
}

// ============================================================================
// Crawling Use Cases
// ============================================================================

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
            
            // Check against existing database
            if existing_urls.contains(&product.url) {
                existing_products.push(product);
            } else {
                new_products.push(product);
            }
        }
        
        let processing_time = start_time.elapsed();
        
        let new_products_len = new_products.len();
        let existing_products_len = existing_products.len();
        let duplicate_products_len = duplicate_products.len();
        
        Ok(ValidationResult {
            new_products,
            existing_products,
            duplicate_products,
            summary: ValidationSummary {
                total_products: seen_urls.len() + duplicate_products_len,
                new_products: new_products_len,
                existing_products: existing_products_len,
                duplicate_products: duplicate_products_len,
                validation_time_ms: processing_time.as_millis() as u64,
            },
        })
    }

    /// Save products to database (Stage 2 results)
    pub async fn save_products(&self, products: &[Product]) -> Result<()> {
        self.product_repo.save_products_batch(products).await
    }

    /// Save Matter products to database (Stage 2 results)
    pub async fn save_matter_products(&self, products: &[MatterProduct]) -> Result<()> {
        self.product_repo.save_matter_products_batch(products).await
    }
}

// ============================================================================
// Product Use Cases
// ============================================================================

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

    /// Get all basic products with DTO conversion
    pub async fn get_all_products(&self) -> Result<Vec<ProductResponseDto>> {
        let products = self.product_repo.get_all_products().await?;
        Ok(products.into_iter().map(ProductResponseDto::from).collect())
    }

    /// Get all Matter products with DTO conversion
    pub async fn get_all_matter_products(&self) -> Result<Vec<MatterProductResponseDto>> {
        let matter_products = self.product_repo.get_all_matter_products().await?;
        Ok(matter_products.into_iter().map(MatterProductResponseDto::from).collect())
    }

    /// Search products with pagination and DTO conversion
    pub async fn search_products_paginated(&self, dto: ProductSearchDto) -> Result<ProductSearchResultDto> {
        let all_matter_products = if let Some(query) = &dto.query {
            if !query.trim().is_empty() {
                self.product_repo.search_products(query).await?
            } else {
                self.product_repo.get_all_matter_products().await?
            }
        } else {
            self.product_repo.get_all_matter_products().await?
        };

        let total_count = all_matter_products.len();
        let page = dto.page.unwrap_or(1);
        let page_size = dto.page_size.unwrap_or(10) as usize;
        let offset = ((page - 1) * page_size as u32) as usize;

        let paginated_products: Vec<MatterProduct> = all_matter_products
            .into_iter()
            .skip(offset)
            .take(page_size)
            .collect();

        Ok(ProductSearchResultDto {
            products: paginated_products.into_iter().map(MatterProductResponseDto::from).collect(),
            total_count: total_count as u32,
            page,
            page_size: page_size as u32,
            total_pages: ((total_count as f64) / (page_size as f64)).ceil() as u32,
        })
    }

    /// Get products by manufacturer with DTO conversion
    pub async fn get_products_by_manufacturer(&self, manufacturer: &str) -> Result<Vec<MatterProductResponseDto>> {
        let products = self.product_repo.find_by_manufacturer(manufacturer).await?;
        Ok(products.into_iter().map(MatterProductResponseDto::from).collect())
    }

    /// Get recent products with DTO conversion
    pub async fn get_recent_products(&self, limit: u32) -> Result<Vec<MatterProductResponseDto>> {
        let products = self.product_repo.get_recent_matter_products(limit).await?;
        Ok(products.into_iter().map(MatterProductResponseDto::from).collect())
    }

    /// Get database summary with DTO conversion
    pub async fn get_database_summary_dto(&self) -> Result<DatabaseSummaryDto> {
        let summary = self.product_repo.get_database_summary().await?;
        Ok(DatabaseSummaryDto::from(summary))
    }
}

// Note: Session management is now handled by SessionManager in the domain layer
// Final crawling results are persisted using CrawlingResultRepository
