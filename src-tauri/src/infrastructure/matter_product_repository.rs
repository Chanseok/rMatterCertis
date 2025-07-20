use anyhow::{Context, Result};
use sqlx::{Row, SqlitePool};
use crate::domain::matter_product::MatterProduct;

pub struct MatterProductRepository {
    pool: SqlitePool,
}

impl MatterProductRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
    
    /// Insert a new Matter product
    pub async fn create(&self, product: &MatterProduct) -> Result<i64> {
        let id = sqlx::query!(
            r"
            INSERT INTO matter_products (
                certificate_id, company_name, product_name, description,
                firmware_version, hardware_version, specification_version,
                product_id, vendor_id, primary_device_type_id, transport_interface,
                certified_date, tis_trp_tested, compliance_document_url,
                program_type, device_type, detail_url, listing_url,
                page_number, position_in_page
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20
            )
            ",
            product.certificate_id,
            product.company_name,
            product.product_name,
            product.description,
            product.firmware_version,
            product.hardware_version,
            product.specification_version,
            product.product_id,
            product.vendor_id,
            product.primary_device_type_id,
            product.transport_interface,
            product.certified_date,
            product.tis_trp_tested,
            product.compliance_document_url,
            product.program_type,
            product.device_type,
            product.detail_url,
            product.listing_url,
            product.page_number,
            product.position_in_page
        )
        .execute(&self.pool)
        .await
        .context("Failed to insert Matter product")?
        .last_insert_rowid();
        
        Ok(id)
    }
    
    /// Update an existing Matter product
    pub async fn update(&self, product: &MatterProduct) -> Result<()> {
        sqlx::query!(
            r"
            UPDATE matter_products SET
                company_name = ?2, product_name = ?3, description = ?4,
                firmware_version = ?5, hardware_version = ?6, specification_version = ?7,
                product_id = ?8, vendor_id = ?9, primary_device_type_id = ?10,
                transport_interface = ?11, certified_date = ?12, tis_trp_tested = ?13,
                compliance_document_url = ?14, program_type = ?15, device_type = ?16,
                detail_url = ?17, listing_url = ?18, page_number = ?19, position_in_page = ?20,
                updated_at = CURRENT_TIMESTAMP
            WHERE certificate_id = ?1
            ",
            product.certificate_id,
            product.company_name,
            product.product_name,
            product.description,
            product.firmware_version,
            product.hardware_version,
            product.specification_version,
            product.product_id,
            product.vendor_id,
            product.primary_device_type_id,
            product.transport_interface,
            product.certified_date,
            product.tis_trp_tested,
            product.compliance_document_url,
            product.program_type,
            product.device_type,
            product.detail_url,
            product.listing_url,
            product.page_number,
            product.position_in_page
        )
        .execute(&self.pool)
        .await
        .context("Failed to update Matter product")?;
        
        Ok(())
    }
    
    /// Find a Matter product by certificate ID
    pub async fn find_by_certificate_id(&self, certificate_id: &str) -> Result<Option<MatterProduct>> {
        let product = sqlx::query_as!(
            MatterProduct,
            "SELECT * FROM matter_products WHERE certificate_id = ?1",
            certificate_id
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to find Matter product by certificate ID")?;
        
        Ok(product)
    }
    
    /// Upsert a Matter product (insert if not exists, update if exists)
    pub async fn upsert(&self, product: &MatterProduct) -> Result<i64> {
        match self.find_by_certificate_id(&product.certificate_id).await? {
            Some(existing) => {
                self.update(product).await?;
                Ok(existing.id.unwrap_or(0))
            }
            None => self.create(product).await,
        }
    }
    
    /// Get all Matter products with pagination
    pub async fn find_all(&self, limit: i32, offset: i32) -> Result<Vec<MatterProduct>> {
        let products = sqlx::query_as!(
            MatterProduct,
            "SELECT * FROM matter_products ORDER BY crawled_at DESC LIMIT ?1 OFFSET ?2",
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch Matter products")?;
        
        Ok(products)
    }
    
    /// Count total Matter products
    pub async fn count(&self) -> Result<i64> {
        let count = sqlx::query!("SELECT COUNT(*) as count FROM matter_products")
            .fetch_one(&self.pool)
            .await
            .context("Failed to count Matter products")?
            .count;
        
        Ok(count)
    }
    
    /// Get products by company name
    pub async fn find_by_company(&self, company_name: &str) -> Result<Vec<MatterProduct>> {
        let products = sqlx::query_as!(
            MatterProduct,
            "SELECT * FROM matter_products WHERE company_name LIKE ?1 ORDER BY crawled_at DESC",
            format!("%{}%", company_name)
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to find Matter products by company")?;
        
        Ok(products)
    }
    
    /// Get products by specification version
    pub async fn find_by_specification_version(&self, version: &str) -> Result<Vec<MatterProduct>> {
        let products = sqlx::query_as!(
            MatterProduct,
            "SELECT * FROM matter_products WHERE specification_version = ?1 ORDER BY crawled_at DESC",
            version
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to find Matter products by specification version")?;
        
        Ok(products)
    }
    
    /// Get crawling statistics
    pub async fn get_statistics(&self) -> Result<MatterProductStatistics> {
        let total_count = self.count().await?;
        
        let companies_count = sqlx::query!("SELECT COUNT(DISTINCT company_name) as count FROM matter_products")
            .fetch_one(&self.pool)
            .await
            .context("Failed to count unique companies")?
            .count;
            
        let latest_crawl = sqlx::query!("SELECT MAX(crawled_at) as latest FROM matter_products")
            .fetch_one(&self.pool)
            .await
            .context("Failed to get latest crawl time")?
            .latest;
            
        Ok(MatterProductStatistics {
            total_products: total_count,
            unique_companies: companies_count,
            latest_crawl_time: latest_crawl,
        })
    }
    
    /// Delete products older than specified days
    pub async fn cleanup_old_products(&self, days: i32) -> Result<u64> {
        let deleted = sqlx::query!(
            "DELETE FROM matter_products WHERE crawled_at < datetime('now', '-' || ?1 || ' days')",
            days
        )
        .execute(&self.pool)
        .await
        .context("Failed to cleanup old products")?
        .rows_affected();
        
        Ok(deleted)
    }
}

#[derive(Debug)]
pub struct MatterProductStatistics {
    pub total_products: i64,
    pub unique_companies: i64,
    pub latest_crawl_time: Option<String>,
}
