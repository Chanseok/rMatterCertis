//! Repository implementation for integrated product operations
//!
//! This module provides database operations that combine product and product_detail
//! tables to provide comprehensive product information with advanced search and
//! filtering capabilities.

#![allow(clippy::uninlined_format_args)]
#![allow(missing_docs)]
#![allow(clippy::unnecessary_qualification)]
#![allow(unused_must_use)]

use crate::domain::product::{
    Product, ProductDetail, ProductWithDetails, ProductSearchCriteria, 
    ProductSearchResult, Vendor
};
use crate::domain::session_manager::CrawlingResult;
use crate::domain::integrated_product::DatabaseStatistics;
use anyhow::Result;
use sqlx::{SqlitePool, Row};
use std::sync::Arc;
use chrono::{DateTime, Utc};

/// Repository for the integrated schema (products + product_details + vendors + crawling_results)
#[derive(Clone)]
pub struct IntegratedProductRepository {
    pool: Arc<SqlitePool>,
}

impl IntegratedProductRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool: Arc::new(pool) }
    }

    // ===============================
    // PRODUCT OPERATIONS
    // ===============================

    /// Insert or update basic product information from listing page
    pub async fn create_or_update_product(&self, product: &Product) -> Result<()> {
        let now = chrono::Utc::now();
        
        // Check if product already exists
        let existing = self.get_product_by_url(&product.url).await?;
        
        if let Some(existing_product) = existing {
            // Update existing product, preserve created_at
            sqlx::query(
                r#"
                UPDATE products 
                SET manufacturer = ?, model = ?, certificateId = ?, pageId = ?, indexInPage = ?, updatedAt = ?
                WHERE url = ?
                "#,
            )
            .bind(&product.manufacturer)
            .bind(&product.model)
            .bind(&product.certificate_id)
            .bind(product.page_id)
            .bind(product.index_in_page)
            .bind(now)
            .bind(&product.url)
            .execute(&*self.pool)
            .await?;
        } else {
            // Insert new product
            sqlx::query(
                r#"
                INSERT INTO products 
                (url, manufacturer, model, certificateId, pageId, indexInPage, createdAt, updatedAt)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&product.url)
            .bind(&product.manufacturer)
            .bind(&product.model)
            .bind(&product.certificate_id)
            .bind(product.page_id)
            .bind(product.index_in_page)
            .bind(now)
            .bind(now)
            .execute(&*self.pool)
            .await?;
        }
        
        Ok(())
    }

    /// Insert or update detailed product specifications
    pub async fn create_or_update_product_detail(&self, detail: &ProductDetail) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO product_details 
            (url, page_id, index_in_page, id, manufacturer, model, device_type,
             certificate_id, certification_date, software_version, hardware_version,
             vid, pid, family_sku, family_variant_sku, firmware_version, family_id,
             tis_trp_tested, specification_version, transport_interface, 
             primary_device_type_id, application_categories, description,
             compliance_document_url, program_type, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&detail.url)
        .bind(detail.page_id)
        .bind(detail.index_in_page)
        .bind(&detail.id)
        .bind(&detail.manufacturer)
        .bind(&detail.model)
        .bind(&detail.device_type)
        .bind(&detail.certification_id)
        .bind(&detail.certification_date)
        .bind(&detail.software_version)
        .bind(&detail.hardware_version)
        .bind(detail.vid)
        .bind(detail.pid)
        .bind(&detail.family_sku)
        .bind(&detail.family_variant_sku)
        .bind(&detail.firmware_version)
        .bind(&detail.family_id)
        .bind(&detail.tis_trp_tested)
        .bind(&detail.specification_version)
        .bind(&detail.transport_interface)
        .bind(&detail.primary_device_type_id)
        .bind(&detail.application_categories)
        .bind(&detail.description)
        .bind(&detail.compliance_document_url)
        .bind(&detail.program_type)
        .bind(detail.created_at)
        .bind(detail.updated_at)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    /// Get all products with pagination
    pub async fn get_products_paginated(&self, page: i32, limit: i32) -> Result<Vec<Product>> {
        let offset = (page - 1) * limit;
        let rows = sqlx::query(
            r#"
            SELECT url, manufacturer, model, certificateId, pageId, indexInPage, createdAt, updatedAt
            FROM products 
            ORDER BY pageId DESC, indexInPage ASC 
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&*self.pool)
        .await?;

        let products = rows
            .into_iter()
            .map(|row| Product {
                url: row.get("url"),
                manufacturer: row.get("manufacturer"),
                model: row.get("model"),
                certificate_id: row.get("certificateId"),
                page_id: row.get("pageId"),
                index_in_page: row.get("indexInPage"),
                created_at: row.get("createdAt"),
                updated_at: row.get("updatedAt"),
            })
            .collect();

        Ok(products)
    }

    /// Get product by URL
    pub async fn get_product_by_url(&self, url: &str) -> Result<Option<Product>> {
        let row = sqlx::query(
            r#"
            SELECT url, manufacturer, model, certificateId, pageId, indexInPage, createdAt, updatedAt
            FROM products WHERE url = ?
            "#,
        )
        .bind(url)
        .fetch_optional(&*self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(Product {
                url: row.get("url"),
                manufacturer: row.get("manufacturer"),
                model: row.get("model"),
                certificate_id: row.get("certificateId"),
                page_id: row.get("pageId"),
                index_in_page: row.get("indexInPage"),
                created_at: row.get("createdAt"),
                updated_at: row.get("updatedAt"),
            })),
            None => Ok(None),
        }
    }

    /// Get product with details by URL
    pub async fn get_product_with_details(&self, url: &str) -> Result<Option<ProductWithDetails>> {
        let product = self.get_product_by_url(url).await?;
        
        if let Some(product) = product {
            let detail = self.get_product_detail_by_url(url).await?;
            Ok(Some(ProductWithDetails { product, details: detail }))
        } else {
            Ok(None)
        }
    }

    /// Get product detail by URL
    pub async fn get_product_detail_by_url(&self, url: &str) -> Result<Option<ProductDetail>> {
        let row = sqlx::query(
            r#"
            SELECT url, page_id, index_in_page, id, manufacturer, model, device_type,
                   certificate_id, certification_date, software_version, hardware_version,
                   vid, pid, family_sku, family_variant_sku, firmware_version, family_id,
                   tis_trp_tested, specification_version, transport_interface, 
                   primary_device_type_id, application_categories, description,
                   compliance_document_url, program_type, created_at, updated_at
            FROM product_details WHERE url = ?
            "#,
        )
        .bind(url)
        .fetch_optional(&*self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(ProductDetail {
                url: row.get("url"),
                page_id: row.get("page_id"),
                index_in_page: row.get("index_in_page"),
                id: row.get("id"),
                manufacturer: row.get("manufacturer"),
                model: row.get("model"),
                device_type: row.get("device_type"),
                certification_id: row.get("certificate_id"),
                certification_date: row.get("certification_date"),
                software_version: row.get("software_version"),
                hardware_version: row.get("hardware_version"),
                vid: row.get("vid"),
                pid: row.get("pid"),
                family_sku: row.get("family_sku"),
                family_variant_sku: row.get("family_variant_sku"),
                firmware_version: row.get("firmware_version"),
                family_id: row.get("family_id"),
                tis_trp_tested: row.get("tis_trp_tested"),
                specification_version: row.get("specification_version"),
                transport_interface: row.get("transport_interface"),
                primary_device_type_id: row.get("primary_device_type_id"),
                application_categories: row.get("application_categories"),
                description: row.get("description"),
                compliance_document_url: row.get("compliance_document_url"),
                program_type: row.get("program_type"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })),
            None => Ok(None),
        }
    }

    /// Search products with criteria and pagination
    pub async fn search_products(&self, criteria: &ProductSearchCriteria) -> Result<ProductSearchResult> {
        let mut conditions = Vec::new();
        let mut bind_values = Vec::new();

        // Build WHERE clause based on criteria
        if let Some(manufacturer) = &criteria.manufacturer {
            conditions.push("p.manufacturer LIKE ?");
            bind_values.push(format!("%{}%", manufacturer));
        }

        if let Some(device_type) = &criteria.device_type {
            conditions.push("pd.device_type LIKE ?");
            bind_values.push(format!("%{}%", device_type));
        }

        if let Some(certificate_id) = &criteria.certification_id {
            conditions.push("p.certificate_id LIKE ?");
            bind_values.push(format!("%{}%", certificate_id));
        }

        if let Some(specification_version) = &criteria.specification_version {
            conditions.push("pd.specification_version = ?");
            bind_values.push(specification_version.clone());
        }

        if let Some(program_type) = &criteria.program_type {
            conditions.push("pd.program_type = ?");
            bind_values.push(program_type.clone());
        }

        let where_clause = if conditions.is_empty() {
            "".to_string()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let page = criteria.page.unwrap_or(1);
        let limit = criteria.limit.unwrap_or(50);
        let offset = (page - 1) * limit;

        // Get total count
        let count_query = format!(
            r#"
            SELECT COUNT(*) as total
            FROM products p
            LEFT JOIN product_details pd ON p.url = pd.url
            {}
            "#,
            where_clause
        );

        let mut count_query_builder = sqlx::query_scalar::<_, i32>(&count_query);
        for value in &bind_values {
            count_query_builder = count_query_builder.bind(value);
        }
        let total_count = count_query_builder.fetch_one(&*self.pool).await?;

        // Get paginated results
        let data_query = format!(
            r#"
            SELECT p.url, p.manufacturer, p.model, p.certificateId, p.pageId, p.indexInPage, 
                   p.createdAt as p_created_at, p.updatedAt as p_updated_at,
                   pd.id, pd.device_type as pd_device_type, pd.certification_date as pd_certification_date, pd.software_version, pd.hardware_version,
                   pd.vid, pd.pid, pd.family_sku, pd.family_variant_sku, pd.firmware_version, pd.family_id,
                   pd.tis_trp_tested, pd.specification_version, pd.transport_interface, 
                   pd.primary_device_type_id, pd.application_categories, pd.description,
                   pd.compliance_document_url, pd.program_type,
                   pd.created_at as pd_created_at, pd.updated_at as pd_updated_at
            FROM products p
            LEFT JOIN product_details pd ON p.url = pd.url
            {}
            ORDER BY p.pageId DESC, p.indexInPage ASC
            LIMIT ? OFFSET ?
            "#,
            where_clause
        );

        let mut data_query_builder = sqlx::query(&data_query);
        for value in &bind_values {
            data_query_builder = data_query_builder.bind(value);
        }
        data_query_builder = data_query_builder.bind(limit).bind(offset);
        let rows = data_query_builder.fetch_all(&*self.pool).await?;

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
                    created_at: row.get("p_created_at"),
                    updated_at: row.get("p_updated_at"),
                };

                let details = if row.get::<Option<String>, _>("id").is_some() {
                    Some(ProductDetail {
                        url: row.get("url"),
                        page_id: row.get("page_id"),
                        index_in_page: row.get("index_in_page"),
                        id: row.get("id"),
                        manufacturer: row.get("manufacturer"),
                        model: row.get("model"),
                        device_type: row.get("pd_device_type"),
                        certification_id: row.get("certificate_id"),
                        certification_date: row.get("pd_certification_date"),
                        software_version: row.get("software_version"),
                        hardware_version: row.get("hardware_version"),
                        vid: row.get("vid"),
                        pid: row.get("pid"),
                        family_sku: row.get("family_sku"),
                        family_variant_sku: row.get("family_variant_sku"),
                        firmware_version: row.get("firmware_version"),
                        family_id: row.get("family_id"),
                        tis_trp_tested: row.get("tis_trp_tested"),
                        specification_version: row.get("specification_version"),
                        transport_interface: row.get("transport_interface"),
                        primary_device_type_id: row.get("primary_device_type_id"),
                        application_categories: row.get("application_categories"),
                        description: row.get("description"),
                        compliance_document_url: row.get("compliance_document_url"),
                        program_type: row.get("program_type"),
                        created_at: row.get("pd_created_at"),
                        updated_at: row.get("pd_updated_at"),
                    })
                } else {
                    None
                };

                ProductWithDetails { product, details }
            })
            .collect();

        let total_pages = (total_count + limit - 1) / limit;

        Ok(ProductSearchResult {
            products,
            total_count,
            page,
            limit,
            total_pages,
        })
    }

    /// JSON 형식의 제품 데이터를 DB에 추가 또는 업데이트
    /// 
    /// 새로운 제품인 경우 true, 기존 제품 업데이트인 경우 false 반환
    pub async fn upsert_product(&self, product_json: serde_json::Value) -> Result<bool> {
        let url = product_json["url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Product JSON must contain a url field"))?
            .to_string();
        
        // 기존 제품 확인
        let existing_product = self.get_product_by_url(&url).await?;
        let is_new = existing_product.is_none();
        
        // 기본 제품 정보 추출
        let basic_product = Product {
            url: url.clone(),
            manufacturer: product_json["manufacturer"].as_str().map(|s| s.to_string()),
            model: product_json["model"].as_str().map(|s| s.to_string()),
            certificate_id: product_json["certification_id"].as_str().map(|s| s.to_string()),
            page_id: product_json["page_id"].as_i64().map(|i| i as i32),
            index_in_page: product_json["index_in_page"].as_i64().map(|i| i as i32),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        // 기본 제품 정보 저장
        self.create_or_update_product(&basic_product).await?;
        
        // 상세 정보가 있는 경우 저장
        if product_json.get("device_type").is_some() || product_json.get("hardware_version").is_some() {
            let vid = product_json["vid"].as_i64().map(|i| i as i32);
            let pid = product_json["pid"].as_i64().map(|i| i as i32);
            
            let detail = ProductDetail {
                url: url.clone(),
                page_id: basic_product.page_id,
                index_in_page: basic_product.index_in_page,
                id: product_json["id"].as_str().map(|s| s.to_string()),
                manufacturer: basic_product.manufacturer.clone(),
                model: basic_product.model.clone(),
                device_type: product_json["device_type"].as_str().map(|s| s.to_string()),
                certification_id: basic_product.certificate_id.clone(),
                certification_date: product_json["certification_date"].as_str().map(|s| s.to_string()),
                software_version: product_json["software_version"].as_str().map(|s| s.to_string()),
                hardware_version: product_json["hardware_version"].as_str().map(|s| s.to_string()),
                vid,
                pid,
                family_sku: product_json["family_sku"].as_str().map(|s| s.to_string()),
                family_variant_sku: product_json["family_variant_sku"].as_str().map(|s| s.to_string()),
                firmware_version: product_json["firmware_version"].as_str().map(|s| s.to_string()),
                family_id: product_json["family_id"].as_str().map(|s| s.to_string()),
                tis_trp_tested: product_json["tis_trp_tested"].as_str().map(|s| s.to_string()),
                specification_version: product_json["specification_version"].as_str().map(|s| s.to_string()),
                transport_interface: product_json["transport_interface"].as_str().map(|s| s.to_string()),
                primary_device_type_id: product_json["primary_device_type_id"].as_str().map(|s| s.to_string()),
                application_categories: product_json["application_categories"].as_array().map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                }),
                description: product_json["description"].as_str().map(|s| s.to_string()),
                compliance_document_url: product_json["compliance_document_url"].as_str().map(|s| s.to_string()),
                program_type: product_json["program_type"].as_str().map(|s| s.to_string()),
                created_at: basic_product.created_at,
                updated_at: basic_product.updated_at,
            };
            
            self.create_or_update_product_detail(&detail).await?;
        }
        
        Ok(is_new)
    }

    /// Get all vendors
    pub async fn get_vendors(&self) -> Result<Vec<Vendor>> {
        let rows = sqlx::query(
            "SELECT vendor_id, vendor_number, vendor_name, company_legal_name, created_at, updated_at FROM vendors ORDER BY vendor_name"
        )
        .fetch_all(&*self.pool)
        .await?;

        let vendors = rows
            .into_iter()
            .map(|row| Vendor {
                vendor_id: row.get("vendor_id"),
                vendor_number: row.get("vendor_number"),
                vendor_name: row.get("vendor_name"),
                company_legal_name: row.get("company_legal_name"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

        Ok(vendors)
    }

    /// Create a new vendor
    pub async fn create_vendor(&self, vendor: &crate::domain::product::Vendor) -> Result<i32> {
        let vendor_id = vendor.vendor_id.max(0); // 새 ID인 경우 자동 생성
        
        sqlx::query(
            r#"
            INSERT INTO vendors 
            (vendor_id, vendor_number, vendor_name, company_legal_name, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(vendor_id)
        .bind(vendor.vendor_number)
        .bind(&vendor.vendor_name)
        .bind(&vendor.company_legal_name)
        .bind(vendor.created_at)
        .bind(vendor.updated_at)
        .execute(&*self.pool)
        .await?;
        
        // 새 벤더 ID 반환 (자동 생성된 경우 조회)
        if vendor_id <= 0 {
            let row = sqlx::query("SELECT last_insert_rowid() as id")
                .fetch_one(&*self.pool)
                .await?;
            
            let new_id: i32 = row.get("id");
            Ok(new_id)
        } else {
            Ok(vendor_id)
        }
    }

    // ===============================
    // CRAWLING RESULTS OPERATIONS
    // ===============================

    /// Save crawling result
    pub async fn save_crawling_result(&self, result: &CrawlingResult) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO crawling_results 
            (session_id, status, stage, total_pages, products_found, details_fetched, errors_count,
             started_at, completed_at, execution_time_seconds, config_snapshot, error_details, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&result.session_id)
        .bind(&result.status)
        .bind(&result.stage)
        .bind(result.total_pages)
        .bind(result.products_found)
        .bind(result.details_fetched)
        .bind(result.errors_count)
        .bind(result.started_at)
        .bind(result.completed_at)
        .bind(result.execution_time_seconds)
        .bind(&result.config_snapshot)
        .bind(&result.error_details)
        .bind(result.created_at)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    /// Get crawling results with pagination
    pub async fn get_crawling_results(&self, page: i32, limit: i32) -> Result<Vec<CrawlingResult>> {
        let offset = (page - 1) * limit;
        let rows = sqlx::query(
            r#"
            SELECT session_id, status, stage, total_pages, products_found, details_fetched, errors_count,
                   started_at, completed_at, execution_time_seconds, config_snapshot, error_details, created_at
            FROM crawling_results 
            ORDER BY started_at DESC 
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&*self.pool)
        .await?;

        let results = rows
            .into_iter()
            .map(|row| CrawlingResult {
                session_id: row.get("session_id"),
                status: row.get("status"),
                stage: row.get("stage"),
                total_pages: row.get("total_pages"),
                products_found: row.get("products_found"),
                details_fetched: row.get("details_fetched"),
                errors_count: row.get("errors_count"),
                started_at: row.get("started_at"),
                completed_at: row.get("completed_at"),
                execution_time_seconds: row.get("execution_time_seconds"),
                config_snapshot: row.get("config_snapshot"),
                error_details: row.get("error_details"),
                created_at: row.get("created_at"),
            })
            .collect();

        Ok(results)
    }

    // ===============================
    // STATISTICS AND ANALYTICS
    // ===============================

    /// Get database statistics
    pub async fn get_database_statistics(&self) -> Result<DatabaseStatistics> {
        let total_products: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
            .fetch_one(&*self.pool)
            .await?;

        let total_details: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM product_details")
            .fetch_one(&*self.pool)
            .await?;

        let unique_manufacturers: i32 = sqlx::query_scalar("SELECT COUNT(DISTINCT manufacturer) FROM products WHERE manufacturer IS NOT NULL")
            .fetch_one(&*self.pool)
            .await?;

        let unique_device_types: i32 = sqlx::query_scalar("SELECT COUNT(DISTINCT device_type) FROM product_details WHERE device_type IS NOT NULL")
            .fetch_one(&*self.pool)
            .await?;

        let latest_crawl_date: Option<String> = sqlx::query_scalar("SELECT MAX(started_at) FROM crawling_results")
            .fetch_optional(&*self.pool)
            .await?;

        let matter_products_count: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM product_details WHERE program_type = 'Matter' OR program_type IS NULL")
            .fetch_one(&*self.pool)
            .await?;

        let completion_rate = if total_products > 0 {
            (total_details as f32 / total_products as f32) * 100.0
        } else {
            0.0
        };

        Ok(DatabaseStatistics {
            total_products: total_products.into(),
            active_products: total_products.into(), // TODO: Add proper active_products query
            unique_vendors: unique_manufacturers.into(),
            unique_categories: unique_device_types.into(),
            avg_rating: None, // TODO: Add rating calculation
            total_reviews: 0, // TODO: Add review count
            last_crawled: latest_crawl_date.as_ref().and_then(|s| DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.with_timezone(&Utc))),
            total_details: total_details.into(),
            unique_manufacturers: unique_manufacturers.into(),
            unique_device_types: unique_device_types.into(),
            latest_crawl_date: latest_crawl_date.as_ref().and_then(|s| DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.with_timezone(&Utc))),
            matter_products_count: matter_products_count.into(),
            completion_rate: completion_rate as f64,
        })
    }

    /// Get products without details (for crawling prioritization)
    pub async fn get_products_without_details(&self, limit: i32) -> Result<Vec<Product>> {
        let rows = sqlx::query(
            r#"
            SELECT p.url, p.manufacturer, p.model, p.certificateId, p.pageId, p.indexInPage, p.createdAt, p.updatedAt
            FROM products p
            LEFT JOIN product_details pd ON p.url = pd.url
            WHERE pd.url IS NULL
            ORDER BY p.pageId DESC, p.indexInPage ASC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&*self.pool)
        .await?;

        let products = rows
            .into_iter()
            .map(|row| Product {
                url: row.get("url"),
                manufacturer: row.get("manufacturer"),
                model: row.get("model"),
                certificate_id: row.get("certificateId"),
                page_id: row.get("pageId"),
                index_in_page: row.get("indexInPage"),
                created_at: row.get("createdAt"),
                updated_at: row.get("updatedAt"),
            })
            .collect();

        Ok(products)
    }

    /// Get all products from the database
    pub async fn get_all_products(&self) -> Result<Vec<Product>> {
        let rows = sqlx::query(
            r#"
            SELECT url, manufacturer, model, certificateId, 
                   pageId, indexInPage, createdAt, updatedAt
            FROM products
            ORDER BY pageId DESC, indexInPage ASC
            "#,
        )
        .fetch_all(&*self.pool)
        .await?;

        let products = rows
            .into_iter()
            .map(|row| Product {
                url: row.get("url"),
                manufacturer: row.get("manufacturer"),
                model: row.get("model"),
                certificate_id: row.get("certificateId"),
                page_id: row.get("pageId"),
                index_in_page: row.get("indexInPage"),
                created_at: row.get("createdAt"),
                updated_at: row.get("updatedAt"),
            })
            .collect();

        Ok(products)
    }

    /// Get the latest updated product
    pub async fn get_latest_updated_product(&self) -> Result<Option<Product>> {
        let row = sqlx::query(
            r#"
            SELECT url, manufacturer, model, certificateId, 
                   pageId, indexInPage, createdAt, updatedAt
            FROM products
            ORDER BY updatedAt DESC
            LIMIT 1
            "#,
        )
        .fetch_optional(&*self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(Product {
                url: row.get("url"),
                manufacturer: row.get("manufacturer"),
                model: row.get("model"),
                certificate_id: row.get("certificateId"),
                page_id: row.get("pageId"),
                index_in_page: row.get("indexInPage"),
                created_at: row.get("createdAt"),
                updated_at: row.get("updatedAt"),
            }))
        } else {
            Ok(None)
        }
    }

    // ===============================
    // CRAWLING RANGE CALCULATION
    // ===============================

    /// Get the maximum pageId and indexInPage from the database
    /// Returns (max_page_id, max_index_in_page) or (None, None) if no data
    pub async fn get_max_page_id_and_index(&self) -> Result<(Option<i32>, Option<i32>)> {
        let row = sqlx::query(
            r#"
            SELECT MAX(pageId) as max_page_id, 
                   MAX(CASE WHEN pageId = (SELECT MAX(pageId) FROM products) THEN indexInPage END) as max_index_in_page
            FROM products
            WHERE pageId IS NOT NULL AND indexInPage IS NOT NULL
            "#,
        )
        .fetch_optional(&*self.pool)
        .await?;

        if let Some(row) = row {
            let max_page_id: Option<i32> = row.get("max_page_id");
            let max_index_in_page: Option<i32> = row.get("max_index_in_page");
            Ok((max_page_id, max_index_in_page))
        } else {
            Ok((None, None))
        }
    }

    /// Get the count of products stored in the database
    pub async fn get_product_count(&self) -> Result<i32> {
        let count: i32 = sqlx::query_scalar("SELECT COUNT(*) FROM products")
            .fetch_one(&*self.pool)
            .await?;
        Ok(count)
    }

    /// Calculate the next crawling range based on local DB state and site information
    /// Returns (start_page, end_page) for the next crawling range
    /// 
    /// This implements the logic from prompts6:
    /// 1. Get the last saved product's reverse absolute index
    /// 2. Calculate the next product index to crawl
    /// 3. Convert to website page numbers
    /// 4. Apply crawl page limit
    pub async fn calculate_next_crawling_range(
        &self,
        total_pages_on_site: u32,
        products_on_last_page: u32,
        crawl_page_limit: u32,
        products_per_page: u32,
    ) -> Result<Option<(u32, u32)>> {
        // Step 1: Get the last saved product's pageId and indexInPage
        let (max_page_id, max_index_in_page) = self.get_max_page_id_and_index().await?;
        
        // If no data exists, start from the oldest page (highest page number)
        if max_page_id.is_none() || max_index_in_page.is_none() {
            let start_page = total_pages_on_site;
            let end_page = if start_page >= crawl_page_limit {
                start_page - crawl_page_limit + 1
            } else {
                1
            };
            tracing::info!("🆕 No existing data, starting from page {} to {}", start_page, end_page);
            return Ok(Some((start_page, end_page)));
        }

        let max_page_id = max_page_id.unwrap();
        let max_index_in_page = max_index_in_page.unwrap();

        // Step 2: Calculate the last saved product's reverse absolute index
        // Formula: lastSavedIndex = (max_pageId * productsPerPage) + max_indexInPage
        let last_saved_index = (max_page_id as u32 * products_per_page) + max_index_in_page as u32;
        
        // Step 3: Calculate the next product index to crawl
        // Formula: nextProductIndex = lastSavedIndex + 1
        let next_product_index = last_saved_index + 1;
        
        // Step 4: Calculate total products on the site
        // Formula: totalProducts = ((totalPagesOnSite - 1) * productsPerPage) + productsOnLastPage
        let total_products = ((total_pages_on_site - 1) * products_per_page) + products_on_last_page;
        
        // Check if we've already crawled all products
        if next_product_index >= total_products {
            tracing::info!("🏁 All products have been crawled (next_index: {}, total: {})", next_product_index, total_products);
            return Ok(None);
        }
        
        // Step 5: Convert next product index to website page number
        // Formula: forwardIndex = (totalProducts - 1) - nextProductIndex
        let forward_index = (total_products - 1) - next_product_index;
        
        // Formula: targetPageNumber = floor(forwardIndex / productsPerPage) + 1
        let target_page_number = (forward_index / products_per_page) + 1;
        
        // Step 6: Apply crawl page limit
        // Crawling goes from older pages (higher numbers) to newer pages (lower numbers)
        let start_page = target_page_number;
        let end_page = if start_page >= crawl_page_limit {
            start_page - crawl_page_limit + 1
        } else {
            1
        };
        
        tracing::info!("📊 Crawling range calculation:");
        tracing::info!("  Last saved: pageId={}, indexInPage={}", max_page_id, max_index_in_page);
        tracing::info!("  Last saved index: {}", last_saved_index);
        tracing::info!("  Next product index: {}", next_product_index);
        tracing::info!("  Total products on site: {}", total_products);
        tracing::info!("  Forward index: {}", forward_index);
        tracing::info!("  Target page: {}", target_page_number);
        tracing::info!("  Crawl range: {} to {} (limit: {})", start_page, end_page, crawl_page_limit);
        
        Ok(Some((start_page, end_page)))
    }

    /// Check if a specific page range has already been crawled
    pub async fn is_page_range_crawled(&self, start_page: u32, end_page: u32, products_per_page: u32) -> Result<bool> {
        // Convert website page numbers to our internal pageId system
        // Website pages are in reverse order (1 = newest, high number = oldest)
        // Our pageId starts from 0 for the oldest products
        
        // For the check, we need to see if we have products in the corresponding pageId range
        let start_page_id = if start_page > end_page {
            // Normal case: start_page is higher (older) than end_page
            end_page - 1  // Convert to 0-based pageId
        } else {
            start_page - 1
        };
        
        let end_page_id = if start_page > end_page {
            start_page - 1
        } else {
            end_page - 1
        };
        
        let count: i32 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) 
            FROM products 
            WHERE pageId >= ? AND pageId <= ?
            "#,
        )
        .bind(start_page_id as i32)
        .bind(end_page_id as i32)
        .fetch_one(&*self.pool)
        .await?;
        
        // Expected number of products in this range
        let expected_products = (end_page_id - start_page_id + 1) * products_per_page;
        
        tracing::debug!("Range check: pageId {} to {} has {} products (expected: {})", 
                       start_page_id, end_page_id, count, expected_products);
        
        Ok(count >= expected_products as i32)
    }
}
