//! Repository implementations
//! 
//! Contains concrete implementations of repository traits for data persistence.

use async_trait::async_trait;
use sqlx::{SqlitePool, Row};
use chrono::{DateTime, Utc};
use anyhow::{Result, anyhow};
use crate::domain::{
    entities::{Vendor, Product}, 
    repositories::{VendorRepository, ProductRepository}
};

// ============================================================================
// VendorRepository Implementation
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
        // Parse the crawling_config JSON
        let config_json: String = row.try_get("crawling_config")?;
        let crawling_config = serde_json::from_str(&config_json)
            .map_err(|e| anyhow!("Failed to parse crawling_config: {}", e))?;

        // Parse timestamps
        let created_at: String = row.try_get("created_at")?;
        let updated_at: String = row.try_get("updated_at")?;
        let last_crawled_at: Option<String> = row.try_get("last_crawled_at")?;

        let created_at = DateTime::parse_from_rfc3339(&created_at)
            .map_err(|e| anyhow!("Failed to parse created_at: {}", e))?
            .with_timezone(&Utc);
        
        let updated_at = DateTime::parse_from_rfc3339(&updated_at)
            .map_err(|e| anyhow!("Failed to parse updated_at: {}", e))?
            .with_timezone(&Utc);

        let last_crawled_at = if let Some(timestamp) = last_crawled_at {
            Some(DateTime::parse_from_rfc3339(&timestamp)
                .map_err(|e| anyhow!("Failed to parse last_crawled_at: {}", e))?
                .with_timezone(&Utc))
        } else {
            None
        };

        Ok(Vendor {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            base_url: row.try_get("base_url")?,
            crawling_config,
            is_active: row.try_get("is_active")?,
            created_at,
            updated_at,
            last_crawled_at,
        })
    }
}

#[async_trait]
impl VendorRepository for SqliteVendorRepository {
    async fn create(&self, vendor: &Vendor) -> Result<()> {
        let config_json = serde_json::to_string(&vendor.crawling_config)?;
        
        sqlx::query(
            r#"
            INSERT INTO vendors (id, name, base_url, crawling_config, is_active, created_at, updated_at, last_crawled_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#
        )
        .bind(&vendor.id)
        .bind(&vendor.name)
        .bind(&vendor.base_url)
        .bind(&config_json)
        .bind(vendor.is_active)
        .bind(vendor.created_at.to_rfc3339())
        .bind(vendor.updated_at.to_rfc3339())
        .bind(vendor.last_crawled_at.map(|dt| dt.to_rfc3339()))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<Vendor>> {
        let row = sqlx::query(
            "SELECT id, name, base_url, crawling_config, is_active, created_at, updated_at, last_crawled_at FROM vendors WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(Self::row_to_vendor(&row)?)),
            None => Ok(None),
        }
    }

    async fn find_all(&self) -> Result<Vec<Vendor>> {
        let rows = sqlx::query(
            "SELECT id, name, base_url, crawling_config, is_active, created_at, updated_at, last_crawled_at FROM vendors ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        let mut vendors = Vec::new();
        for row in rows {
            vendors.push(Self::row_to_vendor(&row)?);
        }

        Ok(vendors)
    }

    async fn find_active(&self) -> Result<Vec<Vendor>> {
        let rows = sqlx::query(
            "SELECT id, name, base_url, crawling_config, is_active, created_at, updated_at, last_crawled_at FROM vendors WHERE is_active = 1 ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        let mut vendors = Vec::new();
        for row in rows {
            vendors.push(Self::row_to_vendor(&row)?);
        }

        Ok(vendors)
    }

    async fn update(&self, vendor: &Vendor) -> Result<()> {
        let config_json = serde_json::to_string(&vendor.crawling_config)?;
        
        sqlx::query(
            r#"
            UPDATE vendors 
            SET name = $2, base_url = $3, crawling_config = $4, is_active = $5, 
                updated_at = $6, last_crawled_at = $7
            WHERE id = $1
            "#
        )
        .bind(&vendor.id)
        .bind(&vendor.name)
        .bind(&vendor.base_url)
        .bind(&config_json)
        .bind(vendor.is_active)
        .bind(vendor.updated_at.to_rfc3339())
        .bind(vendor.last_crawled_at.map(|dt| dt.to_rfc3339()))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM vendors WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn update_last_crawled(&self, id: &str, timestamp: DateTime<Utc>) -> Result<()> {
        sqlx::query(
            "UPDATE vendors SET last_crawled_at = $2 WHERE id = $1"
        )
        .bind(id)
        .bind(timestamp.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

// ============================================================================
// ProductRepository Implementation
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
        // Parse timestamps
        let collected_at: String = row.try_get("collected_at")?;
        let updated_at: String = row.try_get("updated_at")?;

        let collected_at = DateTime::parse_from_rfc3339(&collected_at)
            .map_err(|e| anyhow!("Failed to parse collected_at: {}", e))?
            .with_timezone(&Utc);
        
        let updated_at = DateTime::parse_from_rfc3339(&updated_at)
            .map_err(|e| anyhow!("Failed to parse updated_at: {}", e))?
            .with_timezone(&Utc);

        Ok(Product {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            price: row.try_get("price")?,
            currency: row.try_get("currency")?,
            description: row.try_get("description")?,
            image_url: row.try_get("image_url")?,
            product_url: row.try_get("product_url")?,
            vendor_id: row.try_get("vendor_id")?,
            category: row.try_get("category")?,
            in_stock: row.try_get::<bool, _>("in_stock")?,
            collected_at,
            updated_at,
        })
    }
}

#[async_trait]
impl ProductRepository for SqliteProductRepository {
    async fn create(&self, product: &Product) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO products (id, name, price, currency, description, image_url, product_url, 
                                vendor_id, category, in_stock, collected_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#
        )
        .bind(&product.id)
        .bind(&product.name)
        .bind(product.price)
        .bind(&product.currency)
        .bind(&product.description)
        .bind(&product.image_url)
        .bind(&product.product_url)
        .bind(&product.vendor_id)
        .bind(&product.category)
        .bind(product.in_stock)
        .bind(product.collected_at.to_rfc3339())
        .bind(product.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<Product>> {
        let row = sqlx::query(
            r#"
            SELECT id, name, price, currency, description, image_url, product_url, 
                   vendor_id, category, in_stock, collected_at, updated_at 
            FROM products WHERE id = $1
            "#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(Self::row_to_product(&row)?)),
            None => Ok(None),
        }
    }

    async fn find_by_vendor(&self, vendor_id: &str) -> Result<Vec<Product>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, price, currency, description, image_url, product_url, 
                   vendor_id, category, in_stock, collected_at, updated_at 
            FROM products WHERE vendor_id = $1 ORDER BY collected_at DESC
            "#
        )
        .bind(vendor_id)
        .fetch_all(&self.pool)
        .await?;

        let mut products = Vec::new();
        for row in rows {
            products.push(Self::row_to_product(&row)?);
        }

        Ok(products)
    }

    async fn find_all(&self) -> Result<Vec<Product>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, price, currency, description, image_url, product_url, 
                   vendor_id, category, in_stock, collected_at, updated_at 
            FROM products ORDER BY collected_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let mut products = Vec::new();
        for row in rows {
            products.push(Self::row_to_product(&row)?);
        }

        Ok(products)
    }

    async fn find_in_stock(&self) -> Result<Vec<Product>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, price, currency, description, image_url, product_url, 
                   vendor_id, category, in_stock, collected_at, updated_at 
            FROM products WHERE in_stock = 1 ORDER BY collected_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let mut products = Vec::new();
        for row in rows {
            products.push(Self::row_to_product(&row)?);
        }

        Ok(products)
    }

    async fn find_by_category(&self, category: &str) -> Result<Vec<Product>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, price, currency, description, image_url, product_url, 
                   vendor_id, category, in_stock, collected_at, updated_at 
            FROM products WHERE category = $1 ORDER BY collected_at DESC
            "#
        )
        .bind(category)
        .fetch_all(&self.pool)
        .await?;

        let mut products = Vec::new();
        for row in rows {
            products.push(Self::row_to_product(&row)?);
        }

        Ok(products)
    }

    async fn search_by_name(&self, query: &str) -> Result<Vec<Product>> {
        let search_query = format!("%{}%", query);
        let rows = sqlx::query(
            r#"
            SELECT id, name, price, currency, description, image_url, product_url, 
                   vendor_id, category, in_stock, collected_at, updated_at 
            FROM products WHERE name LIKE $1 ORDER BY collected_at DESC
            "#
        )
        .bind(&search_query)
        .fetch_all(&self.pool)
        .await?;

        let mut products = Vec::new();
        for row in rows {
            products.push(Self::row_to_product(&row)?);
        }

        Ok(products)
    }

    async fn update(&self, product: &Product) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE products 
            SET name = $2, price = $3, currency = $4, description = $5, image_url = $6, 
                product_url = $7, category = $8, in_stock = $9, updated_at = $10
            WHERE id = $1
            "#
        )
        .bind(&product.id)
        .bind(&product.name)
        .bind(product.price)
        .bind(&product.currency)
        .bind(&product.description)
        .bind(&product.image_url)
        .bind(&product.product_url)
        .bind(&product.category)
        .bind(product.in_stock)
        .bind(product.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM products WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn delete_by_vendor(&self, vendor_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM products WHERE vendor_id = $1")
            .bind(vendor_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn count_by_vendor(&self, vendor_id: &str) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM products WHERE vendor_id = $1")
            .bind(vendor_id)
            .fetch_one(&self.pool)
            .await?;

        Ok(row.try_get("count")?)
    }

    async fn get_latest_by_vendor(&self, vendor_id: &str, limit: i64) -> Result<Vec<Product>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, price, currency, description, image_url, product_url, 
                   vendor_id, category, in_stock, collected_at, updated_at 
            FROM products WHERE vendor_id = $1 
            ORDER BY collected_at DESC LIMIT $2
            "#
        )
        .bind(vendor_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut products = Vec::new();
        for row in rows {
            products.push(Self::row_to_product(&row)?);
        }

        Ok(products)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database_connection::DatabaseConnection;
    use crate::domain::entities::{Vendor, Product, CrawlingConfig, ProductSelectors, PaginationConfig};
    use chrono::Utc;

    async fn setup_test_db() -> Result<SqlitePool> {
        // Use in-memory database for tests to avoid file permission issues
        let database_url = "sqlite::memory:";
        
        let db_connection = DatabaseConnection::new(database_url).await?;
        db_connection.migrate().await?;
        
        Ok(db_connection.pool().clone())
    }

    #[tokio::test]
    async fn test_vendor_repository_crud() -> Result<()> {
        let pool = setup_test_db().await?;
        let repo = SqliteVendorRepository::new(pool);
        
        // Create test vendor
        let vendor = Vendor {
            id: "test-vendor-id".to_string(),
            name: "Test Vendor".to_string(),
            base_url: "https://example.com".to_string(),
            crawling_config: CrawlingConfig {
                max_concurrent_requests: 10,
                delay_between_requests: 1000,
                user_agent: "test-agent".to_string(),
                selectors: ProductSelectors {
                    name: "h2.title".to_string(),
                    price: "span.price".to_string(),
                    description: None,
                    image_url: None,
                    product_url: "a.product-link".to_string(),
                    in_stock: None,
                    category: None,
                },
                pagination: PaginationConfig {
                    next_page_selector: Some(".next".to_string()),
                    page_url_pattern: None,
                    max_pages: Some(10),
                },
            },
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_crawled_at: None,
        };
        
        // Test create
        repo.create(&vendor).await?;
        
        // Test find_by_id
        let found = repo.find_by_id(&vendor.id).await?;
        assert!(found.is_some());
        let found_vendor = found.unwrap();
        assert_eq!(found_vendor.name, vendor.name);
        
        // Test find_all
        let all_vendors = repo.find_all().await?;
        assert_eq!(all_vendors.len(), 1);
        
        // Test update
        let mut updated_vendor = vendor.clone();
        updated_vendor.name = "Updated Vendor".to_string();
        updated_vendor.updated_at = Utc::now();
        
        repo.update(&updated_vendor).await?;
        
        let found = repo.find_by_id(&vendor.id).await?;
        assert_eq!(found.unwrap().name, "Updated Vendor");
        
        // Test delete
        repo.delete(&vendor.id).await?;
        let found = repo.find_by_id(&vendor.id).await?;
        assert!(found.is_none());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_product_repository_crud() -> Result<()> {
        let pool = setup_test_db().await?;
        let product_repo = SqliteProductRepository::new(pool.clone());
        let vendor_repo = SqliteVendorRepository::new(pool);
        
        // First create a vendor for foreign key constraint
        let vendor = Vendor {
            id: "test-vendor".to_string(),
            name: "Test Vendor".to_string(),
            base_url: "https://example.com".to_string(),
            crawling_config: CrawlingConfig {
                max_concurrent_requests: 10,
                delay_between_requests: 1000,
                user_agent: "test-agent".to_string(),
                selectors: ProductSelectors {
                    name: "h2.title".to_string(),
                    price: "span.price".to_string(),
                    description: None,
                    image_url: None,
                    product_url: "a.product-link".to_string(),
                    in_stock: None,
                    category: None,
                },
                pagination: PaginationConfig {
                    next_page_selector: Some(".next".to_string()),
                    page_url_pattern: None,
                    max_pages: Some(10),
                },
            },
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_crawled_at: None,
        };
        
        vendor_repo.create(&vendor).await?;
        
        // Create test product
        let product = Product {
            id: "test-product-id".to_string(),
            name: "Test Product".to_string(),
            price: Some(99.99),
            currency: "USD".to_string(),
            description: Some("Test description".to_string()),
            image_url: Some("https://example.com/image.jpg".to_string()),
            product_url: "https://example.com/product".to_string(),
            vendor_id: "test-vendor".to_string(),
            category: Some("Electronics".to_string()),
            in_stock: true,
            collected_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        // Test create
        product_repo.create(&product).await?;
        
        // Test find_by_id
        let found = product_repo.find_by_id(&product.id).await?;
        assert!(found.is_some());
        let found_product = found.unwrap();
        assert_eq!(found_product.name, product.name);
        
        // Test find_by_vendor
        let vendor_products = product_repo.find_by_vendor(&product.vendor_id).await?;
        assert_eq!(vendor_products.len(), 1);
        
        // Test search_by_name
        let search_results = product_repo.search_by_name("Test").await?;
        assert_eq!(search_results.len(), 1);
        
        // Test delete
        product_repo.delete(&product.id).await?;
        let found = product_repo.find_by_id(&product.id).await?;
        assert!(found.is_none());
        
        Ok(())
    }
}
