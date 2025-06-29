//! Repository implementations for Matter Certification domain
//! 
//! Contains concrete implementations of repository traits for Matter product data persistence.

use async_trait::async_trait;
use sqlx::{SqlitePool, Row};
use chrono::{DateTime, Utc};
use anyhow::{Result, anyhow};
use std::collections::HashSet;
use crate::domain::{
    entities::{Vendor, Product, MatterProduct, DatabaseSummary}, 
    repositories::{VendorRepository, ProductRepository}
};

// ============================================================================
// VendorRepository Implementation for Matter Certification
// ============================================================================

pub struct SqliteVendorRepository {
    pool: SqlitePool,
}

impl SqliteVendorRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Helper method to convert database row to Vendor entity
    fn row_to_vendor(row: &sqlx::sqlite::SqliteRow) -> Result<Vendor> {
        let created_at: String = row.try_get("created_at")?;
        let created_at = DateTime::parse_from_rfc3339(&created_at)
            .map_err(|e| anyhow!("Failed to parse created_at: {}", e))?
            .with_timezone(&Utc);

        Ok(Vendor {
            id: row.try_get("vendor_id")?,
            vendor_number: row.try_get("vendor_number")?,
            vendor_name: row.try_get("vendor_name")?,
            company_legal_name: row.try_get("company_legal_name")?,
            vendor_url: None, // Not in current schema
            csa_assigned_number: None, // Not in current schema
            created_at,
            updated_at: created_at, // Use created_at for updated_at for now
        })
    }
}

#[async_trait]
impl VendorRepository for SqliteVendorRepository {
    async fn create(&self, vendor: &Vendor) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO vendors (vendor_id, vendor_number, vendor_name, company_legal_name, created_at)
            VALUES ($1, $2, $3, $4, $5)
            "#
        )
        .bind(&vendor.id)
        .bind(vendor.vendor_number)
        .bind(&vendor.vendor_name)
        .bind(&vendor.company_legal_name)
        .bind(vendor.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn find_by_id(&self, vendor_id: &str) -> Result<Option<Vendor>> {
        let row = sqlx::query(
            "SELECT vendor_id, vendor_number, vendor_name, company_legal_name, created_at FROM vendors WHERE vendor_id = $1"
        )
        .bind(vendor_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(Self::row_to_vendor(&row)?)),
            None => Ok(None),
        }
    }

    async fn find_by_number(&self, vendor_number: u32) -> Result<Option<Vendor>> {
        let row = sqlx::query(
            "SELECT vendor_id, vendor_number, vendor_name, company_legal_name, created_at FROM vendors WHERE vendor_number = $1"
        )
        .bind(vendor_number)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(Self::row_to_vendor(&row)?)),
            None => Ok(None),
        }
    }

    async fn find_all(&self) -> Result<Vec<Vendor>> {
        let rows = sqlx::query(
            "SELECT vendor_id, vendor_number, vendor_name, company_legal_name, created_at FROM vendors ORDER BY vendor_name"
        )
        .fetch_all(&self.pool)
        .await?;

        let vendors = rows.iter()
            .map(Self::row_to_vendor)
            .collect::<Result<Vec<_>>>()?;

        Ok(vendors)
    }

    async fn search_by_name(&self, name: &str) -> Result<Vec<Vendor>> {
        let search_pattern = format!("%{name}%");
        let rows = sqlx::query(
            "SELECT vendor_id, vendor_number, vendor_name, company_legal_name, created_at FROM vendors WHERE vendor_name LIKE $1 ORDER BY vendor_name"
        )
        .bind(&search_pattern)
        .fetch_all(&self.pool)
        .await?;

        let vendors = rows.iter()
            .map(Self::row_to_vendor)
            .collect::<Result<Vec<_>>>()?;

        Ok(vendors)
    }

    async fn update(&self, vendor: &Vendor) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE vendors 
            SET vendor_number = $2, vendor_name = $3, company_legal_name = $4
            WHERE vendor_id = $1
            "#
        )
        .bind(&vendor.id)
        .bind(vendor.vendor_number)
        .bind(&vendor.vendor_name)
        .bind(&vendor.company_legal_name)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn delete(&self, vendor_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM vendors WHERE vendor_id = $1")
            .bind(vendor_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn save(&self, vendor: &Vendor) -> Result<Vendor> {
        // Use create for INSERT and update for UPDATE
        // For now, just do an INSERT OR REPLACE
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO vendors (vendor_id, vendor_number, vendor_name, company_legal_name, created_at)
            VALUES ($1, $2, $3, $4, $5)
            "#
        )
        .bind(&vendor.id)
        .bind(vendor.vendor_number)
        .bind(&vendor.vendor_name)
        .bind(&vendor.company_legal_name)
        .bind(vendor.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(vendor.clone())
    }

    async fn find_all_paginated(&self, page: u32, limit: u32) -> Result<(Vec<Vendor>, u32)> {
        let offset = (page - 1) * limit;
        
        // Get total count
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM vendors")
            .fetch_one(&self.pool)
            .await?;
        
        // Get paginated results
        let rows = sqlx::query("SELECT * FROM vendors ORDER BY vendor_name LIMIT $1 OFFSET $2")
            .bind(limit as i64)
            .bind(offset as i64)
            .fetch_all(&self.pool)
            .await?;

        let vendors = rows.iter()
            .map(Self::row_to_vendor)
            .collect::<Result<Vec<_>>>()?;

        Ok((vendors, total as u32))
    }
}

// ============================================================================
// ProductRepository Implementation for Matter Certification
// ============================================================================

pub struct SqliteProductRepository {
    pool: SqlitePool,
}

impl SqliteProductRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Helper method to convert database row to Product entity
    fn row_to_product(row: &sqlx::sqlite::SqliteRow) -> Result<Product> {
        let created_at: String = row.try_get("created_at")?;
        let created_at = DateTime::parse_from_rfc3339(&created_at)
            .map_err(|e| anyhow!("Failed to parse created_at: {}", e))?
            .with_timezone(&Utc);

        Ok(Product {
            url: row.try_get("url")?,
            manufacturer: row.try_get("manufacturer")?,
            model: row.try_get("model")?,
            certificate_id: row.try_get("certificate_id")?,
            page_id: row.try_get("page_id")?,
            index_in_page: row.try_get("index_in_page")?,
            created_at,
        })
    }

    /// Helper method to convert database row to MatterProduct entity
    fn row_to_matter_product(row: &sqlx::sqlite::SqliteRow) -> Result<MatterProduct> {
        let created_at: String = row.try_get("created_at")?;
        let updated_at: String = row.try_get("updated_at")?;
        
        let created_at = DateTime::parse_from_rfc3339(&created_at)
            .map_err(|e| anyhow!("Failed to parse created_at: {}", e))?
            .with_timezone(&Utc);
        
        let updated_at = DateTime::parse_from_rfc3339(&updated_at)
            .map_err(|e| anyhow!("Failed to parse updated_at: {}", e))?
            .with_timezone(&Utc);

        // Parse application_categories JSON
        let application_categories_json: Option<String> = row.try_get("application_categories")?;
        let application_categories = if let Some(json) = application_categories_json {
            serde_json::from_str(&json).unwrap_or_default()
        } else {
            Vec::new()
        };

        Ok(MatterProduct {
            url: row.try_get("url")?,
            page_id: row.try_get("page_id")?,
            index_in_page: row.try_get("index_in_page")?,
            id: row.try_get("id")?,
            manufacturer: row.try_get("manufacturer")?,
            model: row.try_get("model")?,
            device_type: row.try_get("device_type")?,
            certificate_id: row.try_get("certificate_id")?,
            certification_date: row.try_get("certification_date")?,
            software_version: row.try_get("software_version")?,
            hardware_version: row.try_get("hardware_version")?,
            vid: row.try_get("vid")?,
            pid: row.try_get("pid")?,
            family_sku: row.try_get("family_sku")?,
            family_variant_sku: row.try_get("family_variant_sku")?,
            firmware_version: row.try_get("firmware_version")?,
            family_id: row.try_get("family_id")?,
            tis_trp_tested: row.try_get("tis_trp_tested")?,
            specification_version: row.try_get("specification_version")?,
            transport_interface: row.try_get("transport_interface")?,
            primary_device_type_id: row.try_get("primary_device_type_id")?,
            application_categories,
            created_at,
            updated_at,
        })
    }
}

#[async_trait]
impl ProductRepository for SqliteProductRepository {
    // Basic product operations (Stage 1 collection)
    async fn save_product(&self, product: &Product) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO products (url, manufacturer, model, certificate_id, page_id, index_in_page, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#
        )
        .bind(&product.url)
        .bind(&product.manufacturer)
        .bind(&product.model)
        .bind(&product.certificate_id)
        .bind(product.page_id)
        .bind(product.index_in_page)
        .bind(product.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn save_products_batch(&self, products: &[Product]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        
        for product in products {
            sqlx::query(
                r#"
                INSERT OR REPLACE INTO products (url, manufacturer, model, certificate_id, page_id, index_in_page, created_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#
            )
            .bind(&product.url)
            .bind(&product.manufacturer)
            .bind(&product.model)
            .bind(&product.certificate_id)
            .bind(product.page_id)
            .bind(product.index_in_page)
            .bind(product.created_at.to_rfc3339())
            .execute(&mut *tx)
            .await?;
        }
        
        tx.commit().await?;
        Ok(())
    }

    async fn find_product_by_url(&self, url: &str) -> Result<Option<Product>> {
        let row = sqlx::query(
            "SELECT url, manufacturer, model, certificate_id, page_id, index_in_page, created_at FROM products WHERE url = $1"
        )
        .bind(url)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(Self::row_to_product(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_existing_urls(&self) -> Result<HashSet<String>> {
        let rows = sqlx::query("SELECT url FROM products")
            .fetch_all(&self.pool)
            .await?;

        let urls = rows.into_iter()
            .map(|row| row.try_get::<String, _>("url"))
            .collect::<Result<HashSet<String>, _>>()?;

        Ok(urls)
    }

    async fn get_products_paginated(&self, page: u32, limit: u32) -> Result<(Vec<Product>, u32)> {
        let offset = page * limit;
        
        // Get total count
        let count_row = sqlx::query("SELECT COUNT(*) as count FROM products")
            .fetch_one(&self.pool)
            .await?;
        let total: i64 = count_row.try_get("count")?;
        
        // Get paginated results
        let rows = sqlx::query(
            r#"
            SELECT url, manufacturer, model, certificate_id, page_id, index_in_page, created_at
            FROM products 
            ORDER BY created_at DESC 
            LIMIT $1 OFFSET $2
            "#
        )
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await?;

        let products = rows.iter()
            .map(Self::row_to_product)
            .collect::<Result<Vec<_>>>()?;

        Ok((products, total as u32))
    }

    // Matter product operations (Stage 2 collection)
    async fn save_matter_product(&self, product: &MatterProduct) -> Result<()> {
        let application_categories_json = serde_json::to_string(&product.application_categories)?;
        
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO matter_products (
                url, page_id, index_in_page, id, manufacturer, model, device_type,
                certificate_id, certification_date, software_version, hardware_version,
                vid, pid, family_sku, family_variant_sku, firmware_version,
                family_id, tis_trp_tested, specification_version, transport_interface,
                primary_device_type_id, application_categories, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24)
            "#
        )
        .bind(&product.url)
        .bind(product.page_id)
        .bind(product.index_in_page)
        .bind(&product.id)
        .bind(&product.manufacturer)
        .bind(&product.model)
        .bind(&product.device_type)
        .bind(&product.certificate_id)
        .bind(&product.certification_date)
        .bind(&product.software_version)
        .bind(&product.hardware_version)
        .bind(&product.vid)
        .bind(&product.pid)
        .bind(&product.family_sku)
        .bind(&product.family_variant_sku)
        .bind(&product.firmware_version)
        .bind(&product.family_id)
        .bind(&product.tis_trp_tested)
        .bind(&product.specification_version)
        .bind(&product.transport_interface)
        .bind(&product.primary_device_type_id)
        .bind(&application_categories_json)
        .bind(product.created_at.to_rfc3339())
        .bind(product.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn save_matter_products_batch(&self, products: &[MatterProduct]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        
        for product in products {
            let application_categories_json = serde_json::to_string(&product.application_categories)?;
            
            sqlx::query(
                r#"
                INSERT OR REPLACE INTO matter_products (
                    url, page_id, index_in_page, id, manufacturer, model, device_type,
                    certificate_id, certification_date, software_version, hardware_version,
                    vid, pid, family_sku, family_variant_sku, firmware_version,
                    family_id, tis_trp_tested, specification_version, transport_interface,
                    primary_device_type_id, application_categories, created_at, updated_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24)
                "#
            )
            .bind(&product.url)
            .bind(product.page_id)
            .bind(product.index_in_page)
            .bind(&product.id)
            .bind(&product.manufacturer)
            .bind(&product.model)
            .bind(&product.device_type)
            .bind(&product.certificate_id)
            .bind(&product.certification_date)
            .bind(&product.software_version)
            .bind(&product.hardware_version)
            .bind(&product.vid)
            .bind(&product.pid)
            .bind(&product.family_sku)
            .bind(&product.family_variant_sku)
            .bind(&product.firmware_version)
            .bind(&product.family_id)
            .bind(&product.tis_trp_tested)
            .bind(&product.specification_version)
            .bind(&product.transport_interface)
            .bind(&product.primary_device_type_id)
            .bind(&application_categories_json)
            .bind(product.created_at.to_rfc3339())
            .bind(product.updated_at.to_rfc3339())
            .execute(&mut *tx)
            .await?;
        }
        
        tx.commit().await?;
        Ok(())
    }

    async fn find_matter_product_by_url(&self, url: &str) -> Result<Option<MatterProduct>> {
        let row = sqlx::query(
            r#"
            SELECT url, page_id, index_in_page, id, manufacturer, model, device_type,
                   certificate_id, certification_date, software_version, hardware_version,
                   vid, pid, family_sku, family_variant_sku, firmware_version,
                   family_id, tis_trp_tested, specification_version, transport_interface,
                   primary_device_type_id, application_categories, created_at, updated_at
            FROM matter_products WHERE url = $1
            "#
        )
        .bind(url)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(Self::row_to_matter_product(&row)?)),
            None => Ok(None),
        }
    }

    async fn get_matter_products_paginated(&self, page: u32, limit: u32) -> Result<(Vec<MatterProduct>, u32)> {
        let offset = page * limit;
        
        // Get total count
        let count_row = sqlx::query("SELECT COUNT(*) as count FROM matter_products")
            .fetch_one(&self.pool)
            .await?;
        let total: i64 = count_row.try_get("count")?;
        
        // Get paginated results
        let rows = sqlx::query(
            r#"
            SELECT url, page_id, index_in_page, id, manufacturer, model, device_type,
                   certificate_id, certification_date, software_version, hardware_version,
                   vid, pid, family_sku, family_variant_sku, firmware_version,
                   family_id, tis_trp_tested, specification_version, transport_interface,
                   primary_device_type_id, application_categories, created_at, updated_at
            FROM matter_products 
            ORDER BY created_at DESC 
            LIMIT $1 OFFSET $2
            "#
        )
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await?;

        let products = rows.iter()
            .map(Self::row_to_matter_product)
            .collect::<Result<Vec<_>>>()?;

        Ok((products, total as u32))
    }

    // Search and filtering
    async fn search_products(&self, query: &str) -> Result<Vec<MatterProduct>> {
        let search_pattern = format!("%{query}%");
        
        let rows = sqlx::query(
            r#"
            SELECT url, page_id, index_in_page, id, manufacturer, model, device_type,
                   certificate_id, certification_date, software_version, hardware_version,
                   vid, pid, family_sku, family_variant_sku, firmware_version,
                   family_id, tis_trp_tested, specification_version, transport_interface,
                   primary_device_type_id, application_categories, created_at, updated_at
            FROM matter_products 
            WHERE manufacturer LIKE $1 
               OR model LIKE $1 
               OR device_type LIKE $1 
               OR certificate_id LIKE $1
               OR vid LIKE $1
               OR pid LIKE $1
            ORDER BY created_at DESC
            LIMIT 100
            "#
        )
        .bind(&search_pattern)
        .fetch_all(&self.pool)
        .await?;

        let products = rows.iter()
            .map(Self::row_to_matter_product)
            .collect::<Result<Vec<_>>>()?;

        Ok(products)
    }

    async fn find_by_manufacturer(&self, manufacturer: &str) -> Result<Vec<MatterProduct>> {
        let rows = sqlx::query(
            r#"
            SELECT url, page_id, index_in_page, id, manufacturer, model, device_type,
                   certificate_id, certification_date, software_version, hardware_version,
                   vid, pid, family_sku, family_variant_sku, firmware_version,
                   family_id, tis_trp_tested, specification_version, transport_interface,
                   primary_device_type_id, application_categories, created_at, updated_at
            FROM matter_products 
            WHERE manufacturer = $1
            ORDER BY created_at DESC
            "#
        )
        .bind(manufacturer)
        .fetch_all(&self.pool)
        .await?;

        let products = rows.iter()
            .map(Self::row_to_matter_product)
            .collect::<Result<Vec<_>>>()?;

        Ok(products)
    }

    async fn find_by_device_type(&self, device_type: &str) -> Result<Vec<MatterProduct>> {
        let rows = sqlx::query(
            r#"
            SELECT url, page_id, index_in_page, id, manufacturer, model, device_type,
                   certificate_id, certification_date, software_version, hardware_version,
                   vid, pid, family_sku, family_variant_sku, firmware_version,
                   family_id, tis_trp_tested, specification_version, transport_interface,
                   primary_device_type_id, application_categories, created_at, updated_at
            FROM matter_products 
            WHERE device_type = $1
            ORDER BY created_at DESC
            "#
        )
        .bind(device_type)
        .fetch_all(&self.pool)
        .await?;

        let products = rows.iter()
            .map(Self::row_to_matter_product)
            .collect::<Result<Vec<_>>>()?;

        Ok(products)
    }

    async fn find_by_vid(&self, vid: &str) -> Result<Vec<MatterProduct>> {
        let rows = sqlx::query(
            r#"
            SELECT url, page_id, index_in_page, id, manufacturer, model, device_type,
                   certificate_id, certification_date, software_version, hardware_version,
                   vid, pid, family_sku, family_variant_sku, firmware_version,
                   family_id, tis_trp_tested, specification_version, transport_interface,
                   primary_device_type_id, application_categories, created_at, updated_at
            FROM matter_products 
            WHERE vid = $1
            ORDER BY created_at DESC
            "#
        )
        .bind(vid)
        .fetch_all(&self.pool)
        .await?;

        let products = rows.iter()
            .map(Self::row_to_matter_product)
            .collect::<Result<Vec<_>>>()?;

        Ok(products)
    }

    async fn find_by_certification_date_range(&self, start: &str, end: &str) -> Result<Vec<MatterProduct>> {
        let rows = sqlx::query(
            r#"
            SELECT url, page_id, index_in_page, id, manufacturer, model, device_type,
                   certificate_id, certification_date, software_version, hardware_version,
                   vid, pid, family_sku, family_variant_sku, firmware_version,
                   family_id, tis_trp_tested, specification_version, transport_interface,
                   primary_device_type_id, application_categories, created_at, updated_at
            FROM matter_products 
            WHERE certification_date >= $1 AND certification_date <= $2
            ORDER BY certification_date DESC
            "#
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;

        let products = rows.iter()
            .map(Self::row_to_matter_product)
            .collect::<Result<Vec<_>>>()?;

        Ok(products)
    }

    // Statistics and summary
    async fn get_database_summary(&self) -> Result<DatabaseSummary> {
        // Get total products count
        let products_row = sqlx::query("SELECT COUNT(*) as count FROM products")
            .fetch_one(&self.pool)
            .await?;
        let total_products: i64 = products_row.try_get("count")?;

        // Get total matter products count
        let matter_products_row = sqlx::query("SELECT COUNT(*) as count FROM matter_products")
            .fetch_one(&self.pool)
            .await?;
        let total_matter_products: i64 = matter_products_row.try_get("count")?;

        // Get total vendors count
        let vendors_row = sqlx::query("SELECT COUNT(*) as count FROM vendors")
            .fetch_one(&self.pool)
            .await?;
        let total_vendors: i64 = vendors_row.try_get("count")?;

        // Get last crawling date from crawling_sessions
        let last_session_row = sqlx::query(
            "SELECT started_at FROM crawling_sessions WHERE status = 'Completed' ORDER BY started_at DESC LIMIT 1"
        )
        .fetch_optional(&self.pool)
        .await?;

        let last_crawling_date = if let Some(row) = last_session_row {
            let timestamp: String = row.try_get("started_at")?;
            Some(DateTime::parse_from_rfc3339(&timestamp)?.with_timezone(&Utc))
        } else {
            None
        };

        // Estimate database size (simplified calculation)
        let database_size_mb = (total_products + total_matter_products + total_vendors) as f64 * 0.001; // Rough estimate

        Ok(DatabaseSummary {
            total_products: total_products as u32,
            total_matter_products: total_matter_products as u32,
            total_vendors: total_vendors as u32,
            last_crawling_date,
            database_size_mb,
        })
    }

    async fn count_products(&self) -> Result<u32> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM products")
            .fetch_one(&self.pool)
            .await?;
        let count: i64 = row.try_get("count")?;
        Ok(count as u32)
    }

    async fn count_matter_products(&self) -> Result<u32> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM matter_products")
            .fetch_one(&self.pool)
            .await?;
        let count: i64 = row.try_get("count")?;
        Ok(count as u32)
    }

    // Cleanup operations
    async fn delete_product(&self, url: &str) -> Result<()> {
        sqlx::query("DELETE FROM products WHERE url = $1")
            .bind(url)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete_matter_product(&self, url: &str) -> Result<()> {
        sqlx::query("DELETE FROM matter_products WHERE url = $1")
            .bind(url)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Get all basic products
    async fn get_all_products(&self) -> Result<Vec<Product>> {
        let rows = sqlx::query(
            "SELECT url, manufacturer, model, certificate_id, page_id, index_in_page, created_at FROM products ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        let products = rows.iter()
            .map(Self::row_to_product)
            .collect::<Result<Vec<_>>>()?;

        Ok(products)
    }

    /// Get all Matter products
    async fn get_all_matter_products(&self) -> Result<Vec<MatterProduct>> {
        let rows = sqlx::query(
            "SELECT url, page_id, index_in_page, id, manufacturer, model, device_type, certificate_id, certification_date, software_version, hardware_version, vid, pid, family_sku, family_variant_sku, firmware_version, family_id, tis_trp_tested, specification_version, transport_interface, primary_device_type_id, application_categories, created_at, updated_at FROM matter_products ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        let products = rows.iter()
            .map(Self::row_to_matter_product)
            .collect::<Result<Vec<_>>>()?;

        Ok(products)
    }

    /// Get recent products with limit
    async fn get_recent_products(&self, limit: u32) -> Result<Vec<Product>> {
        let rows = sqlx::query(
            "SELECT url, manufacturer, model, certificate_id, page_id, index_in_page, created_at FROM products ORDER BY created_at DESC LIMIT $1"
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let products = rows.iter()
            .map(Self::row_to_product)
            .collect::<Result<Vec<_>>>()?;

        Ok(products)
    }

    /// Get recent Matter products with limit
    async fn get_recent_matter_products(&self, limit: u32) -> Result<Vec<MatterProduct>> {
        let rows = sqlx::query(
            "SELECT url, page_id, index_in_page, id, manufacturer, model, device_type, certificate_id, certification_date, software_version, hardware_version, vid, pid, family_sku, family_variant_sku, firmware_version, family_id, tis_trp_tested, specification_version, transport_interface, primary_device_type_id, application_categories, created_at, updated_at FROM matter_products ORDER BY created_at DESC LIMIT $1"
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let products = rows.iter()
            .map(Self::row_to_matter_product)
            .collect::<Result<Vec<_>>>()?;

        Ok(products)
    }

    /// Filter Matter products with advanced criteria
    async fn filter_matter_products(
        &self,
        manufacturer: Option<&str>,
        device_type: Option<&str>,
        vid: Option<&str>,
        certification_date_start: Option<&str>,
        certification_date_end: Option<&str>,
    ) -> Result<Vec<MatterProduct>> {
        let mut query = "SELECT url, page_id, index_in_page, id, manufacturer, model, device_type, certificate_id, certification_date, software_version, hardware_version, vid, pid, family_sku, family_variant_sku, firmware_version, family_id, tis_trp_tested, specification_version, transport_interface, primary_device_type_id, application_categories, created_at, updated_at FROM matter_products WHERE 1=1".to_string();
        let mut bind_count = 0;

        if manufacturer.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND manufacturer = ${bind_count}"));
        }
        if device_type.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND device_type = ${bind_count}"));
        }
        if vid.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND vid = ${bind_count}"));
        }
        if certification_date_start.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND certification_date >= ${bind_count}"));
        }
        if certification_date_end.is_some() {
            bind_count += 1;
            query.push_str(&format!(" AND certification_date <= ${bind_count}"));
        }

        query.push_str(" ORDER BY created_at DESC");

        let mut db_query = sqlx::query(&query);

        if let Some(m) = manufacturer {
            db_query = db_query.bind(m);
        }
        if let Some(dt) = device_type {
            db_query = db_query.bind(dt);
        }
        if let Some(v) = vid {
            db_query = db_query.bind(v);
        }
        if let Some(start) = certification_date_start {
            db_query = db_query.bind(start);
        }
        if let Some(end) = certification_date_end {
            db_query = db_query.bind(end);
        }

        let rows = db_query.fetch_all(&self.pool).await?;

        let products = rows.iter()
            .map(Self::row_to_matter_product)
            .collect::<Result<Vec<_>>>()?;

        Ok(products)
    }

    /// Get unique manufacturers from Matter products
    async fn get_unique_manufacturers(&self) -> Result<Vec<String>> {
        let rows = sqlx::query("SELECT DISTINCT manufacturer FROM matter_products WHERE manufacturer IS NOT NULL ORDER BY manufacturer")
            .fetch_all(&self.pool)
            .await?;

        let manufacturers = rows.iter()
            .map(|row| row.try_get::<String, _>("manufacturer"))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(manufacturers)
    }

    /// Get unique device types from Matter products
    async fn get_unique_device_types(&self) -> Result<Vec<String>> {
        let rows = sqlx::query("SELECT DISTINCT device_type FROM matter_products WHERE device_type IS NOT NULL ORDER BY device_type")
            .fetch_all(&self.pool)
            .await?;

        let device_types = rows.iter()
            .map(|row| row.try_get::<String, _>("device_type"))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(device_types)
    }
}

// Note: CrawlingSessionRepository removed - using memory-based SessionManager instead
// Final results are stored using CrawlingResultRepository

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::DatabaseConnection;
    use chrono::Utc;

    async fn setup_test_db() -> Result<SqlitePool> {
        // Use in-memory database for tests to avoid file permission issues
        let database_url = "sqlite::memory:";

        let db = DatabaseConnection::new(database_url).await?;
        db.migrate().await?;
        Ok(db.pool().clone())
    }

    #[tokio::test]
    async fn test_vendor_repository() -> Result<()> {
        let pool = setup_test_db().await?;
        let repo = SqliteVendorRepository::new(pool);

        let vendor = Vendor {
            id: "0x1234".to_string(),
            vendor_number: 4660,
            vendor_name: "Test Vendor".to_string(),
            company_legal_name: "Test Vendor Inc.".to_string(),
            vendor_url: None,
            csa_assigned_number: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Test create
        repo.create(&vendor).await?;

        // Test find by id
        let found = repo.find_by_id("0x1234").await?;
        assert!(found.is_some());
        assert_eq!(found.unwrap().vendor_name, "Test Vendor");

        // Test search by name
        let found_by_name = repo.search_by_name("Test Vendor").await?;
        assert_eq!(found_by_name.len(), 1);

        // Test find all
        let all_vendors = repo.find_all().await?;
        assert_eq!(all_vendors.len(), 1);

        // Test save (insert or replace)
        let updated_vendor = Vendor {
            id: "0x1234".to_string(),
            vendor_number: 4660,
            vendor_name: "Updated Vendor".to_string(),
            company_legal_name: "Updated Vendor Inc.".to_string(),
            vendor_url: None,
            csa_assigned_number: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        repo.save(&updated_vendor).await?;

        // Verify update
        let found_updated = repo.find_by_id("0x1234").await?;
        assert!(found_updated.is_some());
        assert_eq!(found_updated.unwrap().vendor_name, "Updated Vendor");

        // Test find all paginated
        let (vendors_paginated, total_count) = repo.find_all_paginated(1, 10).await?;
        assert_eq!(vendors_paginated.len(), 1);
        assert_eq!(total_count, 1);

        println!("✅ Vendor repository test passed!");
        Ok(())
    }

    #[tokio::test]
    async fn test_product_repository() -> Result<()> {
        let pool = setup_test_db().await?;
        let repo = SqliteProductRepository::new(pool);

        let product = Product {
            url: "https://example.com/product/1".to_string(),
            manufacturer: Some("Test Manufacturer".to_string()),
            model: Some("Test Model".to_string()),
            certificate_id: Some("CERT-123".to_string()),
            page_id: Some(1),
            index_in_page: Some(0),
            created_at: Utc::now(),
        };

        // Test save product
        repo.save_product(&product).await?;

        // Test get existing URLs
        let existing_urls = repo.get_existing_urls().await?;
        assert!(existing_urls.contains("https://example.com/product/1"));

        // Test database summary
        let summary = repo.get_database_summary().await?;
        assert_eq!(summary.total_products, 1);

        println!("✅ Product repository test passed!");
        Ok(())
    }

    #[tokio::test]
    async fn test_matter_product_repository() -> Result<()> {
        let pool = setup_test_db().await?;
        let repo = SqliteProductRepository::new(pool);

        // First create a basic product (required for foreign key)
        let basic_product = Product {
            url: "https://example.com/product/matter/1".to_string(),
            manufacturer: Some("Matter Corp".to_string()),
            model: Some("Smart Device".to_string()),
            certificate_id: Some("CERT-MATTER-123".to_string()),
            page_id: Some(1),
            index_in_page: Some(0),
            created_at: Utc::now(),
        };

        // Save the basic product first
        repo.save_product(&basic_product).await?;

        let matter_product = MatterProduct {
            url: "https://example.com/product/matter/1".to_string(),
            page_id: Some(1),
            index_in_page: Some(0),
            id: Some("MATTER-001".to_string()),
            manufacturer: Some("Matter Corp".to_string()),
            model: Some("Smart Device".to_string()),
            device_type: Some("Light".to_string()),
            certificate_id: Some("CERT-MATTER-123".to_string()),
            certification_date: Some("2024-01-01".to_string()),
            software_version: Some("1.0.0".to_string()),
            hardware_version: Some("1.0".to_string()),
            vid: Some("0x1234".to_string()),
            pid: Some("0x5678".to_string()),
            family_sku: Some("SKU-123".to_string()),
            family_variant_sku: None,
            firmware_version: Some("1.0.0".to_string()),
            family_id: Some("FAM-123".to_string()),
            tis_trp_tested: Some("Yes".to_string()),
            specification_version: Some("1.0".to_string()),
            transport_interface: Some("Wi-Fi".to_string()),
            primary_device_type_id: Some("0x0100".to_string()),
            application_categories: vec!["Lighting".to_string(), "Smart Home".to_string()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Test save matter product
        repo.save_matter_product(&matter_product).await?;

        // Test search products
        let search_results = repo.search_products("Matter").await?;
        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].manufacturer, Some("Matter Corp".to_string()));

        // Test get products by manufacturer
        let manufacturer_products = repo.find_by_manufacturer("Matter Corp").await?;
        assert_eq!(manufacturer_products.len(), 1);

        // Test get products by device type
        let device_type_products = repo.find_by_device_type("Light").await?;
        assert_eq!(device_type_products.len(), 1);

        // Test get products by VID
        let vid_products = repo.find_by_vid("0x1234").await?;
        assert_eq!(vid_products.len(), 1);

        println!("✅ Matter product repository test passed!");
        Ok(())
    }
}
