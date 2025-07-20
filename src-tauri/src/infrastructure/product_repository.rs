use crate::domain::integrated_product::{Product, ProductDetail, ProductWithDetails, ProductSearchCriteria, ProductSearchResult, Vendor};
use anyhow::Result;
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
pub struct ProductRepository {
    pool: Arc<SqlitePool>,
}

impl ProductRepository {
    pub fn new(pool: Arc<SqlitePool>) -> Self {
        Self { pool }
    }

    /// Insert or update basic product information from listing page
    pub async fn create_or_update_product(&self, product: &Product) -> Result<()> {
        sqlx::query!(
            r"
            INSERT OR REPLACE INTO products 
            (url, manufacturer, model, certificate_id, page_id, index_in_page, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ",
            product.url,
            product.manufacturer,
            product.model,
            product.certificate_id,
            product.page_id,
            product.index_in_page,
            product.created_at,
            product.updated_at
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    /// Insert or update detailed product specifications
    pub async fn create_or_update_product_detail(&self, detail: &ProductDetail) -> Result<()> {
        sqlx::query!(
            r"
            INSERT OR REPLACE INTO product_details 
            (url, pageId, indexInPage, id, manufacturer, model, deviceType,
             certificationId, certificationDate, softwareVersion, hardwareVersion,
             vid, pid, familySku, familyVariantSku, firmwareVersion, familyId,
             tisTrpTested, specificationVersion, transportInterface, 
             primaryDeviceTypeId, applicationCategories)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ",
            detail.url,
            detail.page_id,
            detail.index_in_page,
            detail.id,
            detail.manufacturer,
            detail.model,
            detail.device_type,
            detail.certificate_id,
            detail.certification_date,
            detail.software_version,
            detail.hardware_version,
            detail.vid,
            detail.pid,
            detail.family_sku,
            detail.family_variant_sku,
            detail.firmware_version,
            detail.family_id,
            detail.tis_trp_tested,
            detail.specification_version,
            detail.transport_interface,
            detail.primary_device_type_id,
            detail.application_categories
        )
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    /// Get all products with pagination
    pub async fn get_products_paginated(&self, page: i32, limit: i32) -> Result<Vec<Product>> {
        let offset = (page - 1) * limit;
        let products = sqlx::query!(
            r"
            SELECT url, manufacturer, model, certificateId, pageId, indexInPage
            FROM products 
            ORDER BY pageId DESC, indexInPage ASC 
            LIMIT ? OFFSET ?
            ",
            limit,
            offset
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(products
            .into_iter()
            .map(|row| Product {
                url: row.url,
                manufacturer: row.manufacturer,
                model: row.model,
                certificate_id: row.certificateId,
                page_id: row.pageId,
                index_in_page: row.indexInPage,
            })
            .collect())
    }

    /// Get product with details by URL
    pub async fn get_product_with_details(&self, url: &str) -> Result<Option<ProductWithDetails>> {
        let product_row = sqlx::query!(
            r"
            SELECT url, manufacturer, model, certificateId, pageId, indexInPage
            FROM products WHERE url = ?
            ",
            url
        )
        .fetch_optional(&*self.pool)
        .await?;

        if let Some(product_row) = product_row {
            let product = Product {
                url: product_row.url,
                manufacturer: product_row.manufacturer,
                model: product_row.model,
                certificate_id: product_row.certificateId,
                page_id: product_row.pageId,
                index_in_page: product_row.indexInPage,
            };

            let detail_row = sqlx::query!(
                r"
                SELECT url, pageId, indexInPage, id, manufacturer, model, deviceType,
                       certificationId, certificationDate, softwareVersion, hardwareVersion,
                       vid, pid, familySku, familyVariantSku, firmwareVersion, familyId,
                       tisTrpTested, specificationVersion, transportInterface,
                       primaryDeviceTypeId, applicationCategories
                FROM product_details WHERE url = ?
                ",
                url
            )
            .fetch_optional(&*self.pool)
            .await?;

            let details = detail_row.map(|row| ProductDetail {
                url: row.url,
                page_id: row.pageId,
                index_in_page: row.indexInPage,
                id: row.id,
                manufacturer: row.manufacturer,
                model: row.model,
                device_type: row.deviceType,
                certification_id: row.certificationId,
                certification_date: row.certificationDate,
                software_version: row.softwareVersion,
                hardware_version: row.hardwareVersion,
                vid: row.vid,
                pid: row.pid,
                family_sku: row.familySku,
                family_variant_sku: row.familyVariantSku,
                firmware_version: row.firmwareVersion,
                family_id: row.familyId,
                tis_trp_tested: row.tisTrpTested,
                specification_version: row.specificationVersion,
                transport_interface: row.transportInterface,
                primary_device_type_id: row.primaryDeviceTypeId,
                application_categories: row.applicationCategories,
            });

            Ok(Some(ProductWithDetails { product, details }))
        } else {
            Ok(None)
        }
    }

    /// Search products with criteria
    pub async fn search_products(&self, criteria: &ProductSearchCriteria) -> Result<ProductSearchResult> {
        let page = criteria.page.unwrap_or(1);
        let limit = criteria.limit.unwrap_or(20);
        let offset = (page - 1) * limit;

        let mut query_conditions = Vec::new();
        let mut params: Vec<String> = Vec::new();

        if let Some(manufacturer) = &criteria.manufacturer {
            query_conditions.push("p.manufacturer LIKE ?");
            params.push(format!("%{}%", manufacturer));
        }

        if let Some(device_type) = &criteria.device_type {
            query_conditions.push("pd.deviceType LIKE ?");
            params.push(format!("%{}%", device_type));
        }

        if let Some(cert_id) = &criteria.certification_id {
            query_conditions.push("p.certificateId = ?");
            params.push(cert_id.clone());
        }

        let where_clause = if query_conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", query_conditions.join(" AND "))
        };

        // Get total count
        let count_query = format!(
            r"
            SELECT COUNT(*) as count
            FROM products p
            LEFT JOIN product_details pd ON p.url = pd.url
            {}
            ",
            where_clause
        );

        let total_count = sqlx::query_scalar::<_, i32>(&count_query)
            .bind_all(params.iter().map(|p| p.as_str()).collect::<Vec<_>>())
            .fetch_one(&*self.pool)
            .await?;

        // Get products with details
        let products_query = format!(
            r"
            SELECT p.url, p.manufacturer, p.model, p.certificateId, p.pageId, p.indexInPage,
                   pd.id, pd.deviceType, pd.certificationDate, pd.specificationVersion
            FROM products p
            LEFT JOIN product_details pd ON p.url = pd.url
            {}
            ORDER BY p.pageId DESC, p.indexInPage ASC
            LIMIT ? OFFSET ?
            ",
            where_clause
        );

        let rows = sqlx::query(&products_query)
            .bind_all(params.iter().map(|p| p.as_str()).collect::<Vec<_>>())
            .bind(limit)
            .bind(offset)
            .fetch_all(&*self.pool)
            .await?;

        let products = rows
            .into_iter()
            .map(|row| {
                let product = Product {
                    url: row.get("url"),
                    manufacturer: row.get("manufacturer"),
                    model: row.get("model"),
                    certificate_id: row.get("certificateId"),
                    page_id: row.get("pageId"),
                    index_in_page: row.get("indexInPage"),
                };

                let details = if let Some(id) = row.get::<Option<String>, _>("id") {
                    Some(ProductDetail {
                        url: row.get("url"),
                        page_id: row.get("pageId"),
                        index_in_page: row.get("indexInPage"),
                        id: Some(id),
                        manufacturer: row.get("manufacturer"),
                        model: row.get("model"),
                        device_type: row.get("deviceType"),
                        certification_id: row.get("certificateId"),
                        certification_date: row.get("certificationDate"),
                        specification_version: row.get("specificationVersion"),
                        // Other fields would need similar extraction...
                        software_version: None,
                        hardware_version: None,
                        vid: None,
                        pid: None,
                        family_sku: None,
                        family_variant_sku: None,
                        firmware_version: None,
                        family_id: None,
                        tis_trp_tested: None,
                        transport_interface: None,
                        primary_device_type_id: None,
                        application_categories: None,
                    })
                } else {
                    None
                };

                ProductWithDetails { product, details }
            })
            .collect();

        let total_pages = (total_count as f32 / limit as f32).ceil() as i32;

        Ok(ProductSearchResult {
            products,
            total_count,
            page,
            limit,
            total_pages,
        })
    }

    /// Get statistics
    pub async fn get_statistics(&self) -> Result<(i32, i32, i32)> {
        let total_products = sqlx::query_scalar::<_, i32>("SELECT COUNT(*) FROM products")
            .fetch_one(&*self.pool)
            .await?;

        let total_details = sqlx::query_scalar::<_, i32>("SELECT COUNT(*) FROM product_details")
            .fetch_one(&*self.pool)
            .await?;

        let unique_manufacturers = sqlx::query_scalar::<_, i32>(
            "SELECT COUNT(DISTINCT manufacturer) FROM products WHERE manufacturer IS NOT NULL"
        )
        .fetch_one(&*self.pool)
        .await?;

        Ok((total_products, total_details, unique_manufacturers))
    }

    /// Get URLs that need detail crawling (products without details)
    pub async fn get_urls_needing_details(&self, limit: i32) -> Result<Vec<String>> {
        let urls = sqlx::query_scalar::<_, String>(
            r"
            SELECT p.url
            FROM products p
            LEFT JOIN product_details pd ON p.url = pd.url
            WHERE pd.url IS NULL
            LIMIT ?
            "
        )
        .bind(limit)
        .fetch_all(&*self.pool)
        .await?;

        Ok(urls)
    }
}

/// Vendor repository for managing vendor data
#[derive(Clone)]
pub struct VendorRepository {
    pool: Arc<SqlitePool>,
}

impl VendorRepository {
    pub fn new(pool: Arc<SqlitePool>) -> Self {
        Self { pool }
    }

    pub async fn create_vendor(&self, vendor_name: &str, company_legal_name: &str) -> Result<i32> {
        let result = sqlx::query!(
            r"
            INSERT INTO vendors (vendorName, companyLegalName)
            VALUES (?, ?)
            ",
            vendor_name,
            company_legal_name
        )
        .execute(&*self.pool)
        .await?;

        Ok(result.last_insert_rowid() as i32)
    }

    pub async fn get_all_vendors(&self) -> Result<Vec<Vendor>> {
        let vendors = sqlx::query!(
            "SELECT vendorId, vendorName, companyLegalName FROM vendors ORDER BY vendorName"
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(vendors
            .into_iter()
            .map(|row| Vendor {
                vendor_id: row.vendorId as i32,
                vendor_name: row.vendorName,
                company_legal_name: row.companyLegalName,
            })
            .collect())
    }
}
