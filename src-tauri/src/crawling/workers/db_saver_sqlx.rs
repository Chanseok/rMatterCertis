//! # Database Saver Worker
//!
//! Saves parsed product data to the database with optimized batch operations.

use std::sync::Arc;
use std::time::Instant;
use async_trait::async_trait;
use sqlx::{Pool, Postgres, Transaction};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::crawling::{tasks::*, state::*};
use crate::domain::value_objects::ProductData;
use crate::crawling::tasks::TaskProductData;
use super::{Worker, WorkerError};

/// Worker that saves product data to the database
pub struct DbSaver {
    pool: Pool<Postgres>,
    batch_size: usize,
    batch_buffer: Arc<Mutex<Vec<ProductData>>>,
    flush_interval: std::time::Duration,
}

impl DbSaver {
    /// Creates a new database saver
    pub fn new(
        pool: Pool<Postgres>,
        batch_size: usize,
        flush_interval: std::time::Duration,
    ) -> Self {
        Self {
            pool,
            batch_size,
            batch_buffer: Arc::new(Mutex::new(Vec::new())),
            flush_interval,
        }
    }

    /// Saves a single product to the database
    async fn save_product(&self, product: &ProductData) -> Result<(), WorkerError> {
        let mut tx = self.pool.begin().await
            .map_err(|e| WorkerError::DatabaseError(format!("Failed to begin transaction: {}", e)))?;

        self.insert_product(&mut tx, product).await?;

        tx.commit().await
            .map_err(|e| WorkerError::DatabaseError(format!("Failed to commit transaction: {}", e)))?;

        Ok(())
    }

    /// Saves multiple products in a batch
    async fn save_batch(&self, products: &[ProductData]) -> Result<(), WorkerError> {
        if products.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await
            .map_err(|e| WorkerError::DatabaseError(format!("Failed to begin transaction: {}", e)))?;

        for product in products {
            self.insert_product(&mut tx, product).await?;
        }

        tx.commit().await
            .map_err(|e| WorkerError::DatabaseError(format!("Failed to commit batch transaction: {}", e)))?;

        tracing::info!("Saved batch of {} products to database", products.len());
        Ok(())
    }

    /// Inserts a single product into the database within a transaction
    async fn insert_product(&self, tx: &mut Transaction<'_, Postgres>, product: &ProductData) -> Result<(), WorkerError> {
        // Check if product already exists (Modern Rust 2024 - 타입 안전한 쿼리)
        let existing = sqlx::query("SELECT id FROM products WHERE product_id = ?")
            .bind(&product.product_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(|e| WorkerError::DatabaseError(format!("Failed to check existing product: {}", e)))?;

        if existing.is_some() {
            // Update existing product
            sqlx::query!(
                r#"
                UPDATE products SET
                    product_name = $2,
                    company_name = $3,
                    license_number = $4,
                    registration_date = $5,
                    expiry_date = $6,
                    product_type = $7,
                    status = $8,
                    manufacturer = $9,
                    country_of_origin = $10,
                    ingredients = $11,
                    usage_instructions = $12,
                    warnings = $13,
                    storage_conditions = $14,
                    source_url = $15,
                    updated_at = NOW()
                WHERE product_id = $1
                "#,
                product.product_id,
                product.product_name,
                product.company_name,
                product.license_number,
                product.registration_date,
                product.expiry_date,
                product.product_type,
                product.status,
                product.manufacturer,
                product.country_of_origin,
                product.ingredients,
                product.usage_instructions,
                product.warnings,
                product.storage_conditions,
                product.source_url,
            )
            .execute(&mut **tx)
            .await
            .map_err(|e| WorkerError::DatabaseError(format!("Failed to update product: {}", e)))?;
        } else {
            // Insert new product
            sqlx::query!(
                r#"
                INSERT INTO products (
                    id, product_id, product_name, company_name, license_number,
                    registration_date, expiry_date, product_type, status,
                    manufacturer, country_of_origin, ingredients, usage_instructions,
                    warnings, storage_conditions, source_url, created_at, updated_at
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, NOW(), NOW()
                )
                "#,
                Uuid::new_v4(),
                product.product_id,
                product.product_name,
                product.company_name,
                product.license_number,
                product.registration_date,
                product.expiry_date,
                product.product_type,
                product.status,
                product.manufacturer,
                product.country_of_origin,
                product.ingredients,
                product.usage_instructions,
                product.warnings,
                product.storage_conditions,
                product.source_url,
            )
            .execute(&mut **tx)
            .await
            .map_err(|e| WorkerError::DatabaseError(format!("Failed to insert product: {}", e)))?;
        }

        Ok(())
    }

    /// Adds a product to the batch buffer
    async fn add_to_batch(&self, product: ProductData) -> Result<bool, WorkerError> {
        let mut buffer = self.batch_buffer.lock().await;
        buffer.push(product);
        
        Ok(buffer.len() >= self.batch_size)
    }

    /// Flushes the batch buffer to the database
    async fn flush_batch(&self) -> Result<usize, WorkerError> {
        let mut buffer = self.batch_buffer.lock().await;
        if buffer.is_empty() {
            return Ok(0);
        }

        let products = buffer.drain(..).collect::<Vec<_>>();
        let count = products.len();
        
        drop(buffer); // Release lock before database operation
        
        self.save_batch(&products).await?;
        Ok(count)
    }

    /// Validates that the database connection is working
    async fn validate_connection(&self) -> Result<(), WorkerError> {
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| WorkerError::DatabaseError(format!("Database connection validation failed: {}", e)))?;
        
        Ok(())
    }

    /// Gets database statistics
    async fn get_db_stats(&self) -> Result<DatabaseStats, WorkerError> {
        let result = sqlx::query!(
            "SELECT COUNT(*) as total_products, MAX(updated_at) as last_updated FROM products"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| WorkerError::DatabaseError(format!("Failed to get database stats: {}", e)))?;

        Ok(DatabaseStats {
            total_products: result.total_products.unwrap_or(0) as u64,
            last_updated: result.last_updated,
        })
    }

    /// Convert TaskProductData to domain ProductData
    fn convert_task_product_to_domain(&self, task_product: &TaskProductData) -> Result<ProductData, WorkerError> {
        // Create a validated URL
        let validated_url = crate::domain::value_objects::ValidatedUrl::new(&task_product.source_url)
            .map_err(|e| WorkerError::InvalidInput(format!("Invalid URL: {}", e)))?;

        // Create the domain product data
        ProductData::new(
            task_product.product_id.clone(),
            task_product.name.clone(),
            validated_url,
        )
        .map_err(|e| WorkerError::InvalidInput(format!("Invalid product data: {}", e)))
    }
}

#[derive(Debug, Clone)]
pub struct DatabaseStats {
    pub total_products: u64,
    pub last_updated: Option<chrono::DateTime<chrono::Utc>>,
}

#[async_trait]
impl Worker for DbSaver {
    type Task = CrawlingTask;

    async fn process_task(
        &self,
        task: Self::Task,
        shared_state: Arc<SharedState>,
    ) -> Result<TaskResult, WorkerError> {
        let start_time = Instant::now();

        match task {
            CrawlingTask::SaveProduct { task_id, product_data } => {
                if shared_state.is_shutdown_requested() {
                    return Err(WorkerError::Cancelled);
                }

                tracing::info!("Saving product: {} - {}", product_data.product_id, product_data.name);

                // Convert TaskProductData to ProductData
                let domain_product = self.convert_task_product_to_domain(&product_data)?;

                // Validate database connection
                self.validate_connection().await?;

                // Check if we should use batch processing
                let should_flush = self.add_to_batch(domain_product.clone()).await?;

                if should_flush {
                    let flushed_count = self.flush_batch().await?;
                    tracing::info!("Flushed batch of {} products", flushed_count);
                } else {
                    // For critical products or when batch is not full, save immediately
                    self.save_product(&domain_product).await?;
                }

                // Update statistics
                let mut stats = shared_state.stats.write().await;
                stats.products_saved += 1;
                
                let duration = start_time.elapsed();
                stats.record_task_completion("save_product", duration);

                tracing::info!(
                    "Successfully saved product: {} (ID: {})",
                    product_data.product_name,
                    product_data.product_id
                );

                Ok(TaskResult::Success {
                    task_id,
                    output: TaskOutput::SaveConfirmation {
                        product_id: product_data.product_id,
                        saved_at: chrono::Utc::now(),
                    },
                    duration,
                })
            }
            _ => Err(WorkerError::ValidationError(
                "DbSaver can only process SaveProduct tasks".to_string()
            )),
        }
    }

    fn worker_name(&self) -> &'static str {
        "DbSaver"
    }

    fn max_concurrency(&self) -> usize {
        4 // Database I/O bound, moderate concurrency to avoid overwhelming DB
    }
}

impl Drop for DbSaver {
    fn drop(&mut self) {
        // Attempt to flush any remaining items in the buffer
        // Note: This is a best-effort cleanup, doesn't handle errors
        let buffer = self.batch_buffer.clone();
        let pool = self.pool.clone();
        
        tokio::spawn(async move {
            if let Ok(mut buffer) = buffer.try_lock() {
                if !buffer.is_empty() {
                    let products = buffer.drain(..).collect::<Vec<_>>();
                    if !products.is_empty() {
                        let mut tx = pool.begin().await.ok();
                        if let Some(ref mut tx) = tx {
                            for product in &products {
                                let _ = sqlx::query!(
                                    r#"
                                    INSERT INTO products (
                                        id, product_id, product_name, company_name, license_number,
                                        registration_date, expiry_date, product_type, status,
                                        manufacturer, country_of_origin, ingredients, usage_instructions,
                                        warnings, storage_conditions, source_url, created_at, updated_at
                                    ) VALUES (
                                        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, NOW(), NOW()
                                    )
                                    ON CONFLICT (product_id) DO UPDATE SET
                                        product_name = EXCLUDED.product_name,
                                        company_name = EXCLUDED.company_name,
                                        updated_at = NOW()
                                    "#,
                                    Uuid::new_v4(),
                                    product.product_id,
                                    product.product_name,
                                    product.company_name,
                                    product.license_number,
                                    product.registration_date,
                                    product.expiry_date,
                                    product.product_type,
                                    product.status,
                                    product.manufacturer,
                                    product.country_of_origin,
                                    product.ingredients,
                                    product.usage_instructions,
                                    product.warnings,
                                    product.storage_conditions,
                                    product.source_url,
                                ).execute(&mut **tx).await;
                            }
                            let _ = tx.commit().await;
                            tracing::info!("Flushed {} products during cleanup", products.len());
                        }
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::ProductData;

    #[tokio::test]
    async fn batch_management() {
        // Note: This test would need a real database connection
        // For now, we test the batch buffer logic
        
        let mock_pool = Pool::<Postgres>::connect("postgresql://test").await;
        if mock_pool.is_err() {
            // Skip test if no database available
            return;
        }

        let db_saver = DbSaver::new(
            mock_pool.unwrap(),
            3, // batch size
            std::time::Duration::from_secs(10),
        );

        // Test batch buffer
        let product = ProductData::builder()
            .product_id("test123".to_string())
            .product_name("Test Product".to_string())
            .company_name("Test Company".to_string())
            .build()
            .unwrap();

        // Add products to batch
        let should_flush_1 = db_saver.add_to_batch(product.clone()).await.unwrap();
        assert!(!should_flush_1);

        let should_flush_2 = db_saver.add_to_batch(product.clone()).await.unwrap();
        assert!(!should_flush_2);

        let should_flush_3 = db_saver.add_to_batch(product.clone()).await.unwrap();
        assert!(should_flush_3); // Should flush when batch size is reached
    }

    #[test]
    fn worker_properties() {
        let mock_pool = Pool::<Postgres>::connect("postgresql://test");
        if mock_pool.is_err() {
            return;
        }

        let db_saver = DbSaver::new(
            mock_pool.unwrap(),
            10,
            std::time::Duration::from_secs(30),
        );

        assert_eq!(db_saver.worker_name(), "DbSaver");
        assert_eq!(db_saver.max_concurrency(), 4);
    }
}
