//! # Database Saver Worker
//!
//! Saves parsed product data to the database with optimized batch operations.

use std::sync::Arc;
use std::time::Instant;
use async_trait::async_trait;
use sqlx::{Pool, Sqlite, Postgres, Transaction, Row};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::crawling::{tasks::*, state::*};
use crate::domain::value_objects::ProductData;
use crate::crawling::tasks::TaskProductData;
use super::{Worker, WorkerError};

/// Worker that saves product data to the database
pub struct DbSaver {
    pool: Pool<Sqlite>,
    batch_size: usize,
    batch_buffer: Arc<Mutex<Vec<ProductData>>>,
    flush_interval: std::time::Duration,
}

impl DbSaver {
    /// Creates a new database saver
    pub fn new(
        pool: Pool<Sqlite>,
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
    async fn insert_product(&self, tx: &mut Transaction<'_, Sqlite>, product: &ProductData) -> Result<(), WorkerError> {
        // Use parameterized query instead of sqlx! macro to avoid compilation-time database dependency
        let query = r#"
            INSERT INTO products (
                url, manufacturer, model, certificate_id, page_id, index_in_page, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(url) DO UPDATE SET
                manufacturer = excluded.manufacturer,
                model = excluded.model,
                certificate_id = excluded.certificate_id,
                updated_at = excluded.updated_at
        "#;
        
        sqlx::query(query)
            .bind(product.source_url.as_str())
            .bind(product.manufacturer.as_deref().unwrap_or(""))
            .bind(product.model.as_deref().unwrap_or(""))
            .bind(product.certification_number.as_deref().unwrap_or(""))
            .bind(0i32) // page_id - placeholder
            .bind(0i32) // index_in_page - placeholder  
            .bind(product.extracted_at)
            .bind(product.extracted_at)
            .execute(&mut **tx)
            .await
            .map_err(|e| WorkerError::DatabaseError(format!("Failed to insert product: {}", e)))?;
        
        tracing::info!("Successfully inserted product: {}", product.name);
        
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
        let query = "SELECT COUNT(*) as total_products, MAX(updated_at) as last_updated FROM products";
        
        let row = sqlx::query(query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| WorkerError::DatabaseError(format!("Failed to get database stats: {}", e)))?;
        
        let total_products: i64 = row.try_get("total_products").unwrap_or(0);
        let last_updated: Option<chrono::DateTime<chrono::Utc>> = row.try_get("last_updated").ok();
        
        Ok(DatabaseStats {
            total_products: total_products as u64,
            last_updated,
        })
    }

    /// Convert TaskProductData to domain ProductData
    fn convert_task_product_to_domain(&self, task_product: &TaskProductData) -> Result<ProductData, WorkerError> {
        // Create a validated URL
        let validated_url = crate::domain::value_objects::ValidatedUrl::new(task_product.source_url.clone())
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
impl Worker<CrawlingTask> for DbSaver {
    type Task = CrawlingTask;

    fn worker_id(&self) -> &'static str {
        "DbSaver"
    }

    fn worker_name(&self) -> &'static str {
        "Database Saver"
    }

    fn max_concurrency(&self) -> usize {
        4 // Database I/O, conservative concurrency
    }

    async fn process_task(
        &self,
        task: CrawlingTask,
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
                    product_data.name,
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
                        let tx = pool.begin().await.ok();
                        if let Some(mut tx) = tx {
                            for product in &products {
                                // Use parameterized query instead of sqlx! macro
                                let query = r#"
                                    INSERT INTO products (
                                        url, manufacturer, model, certificate_id, page_id, index_in_page, created_at, updated_at
                                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                                    ON CONFLICT(url) DO UPDATE SET
                                        manufacturer = excluded.manufacturer,
                                        model = excluded.model,
                                        certificate_id = excluded.certificate_id,
                                        updated_at = excluded.updated_at
                                "#;
                                
                                let _ = sqlx::query(query)
                                    .bind(product.source_url.as_str())
                                    .bind(product.manufacturer.as_deref().unwrap_or(""))
                                    .bind(product.model.as_deref().unwrap_or(""))
                                    .bind(product.certification_number.as_deref().unwrap_or(""))
                                    .bind(0i32) // page_id - placeholder
                                    .bind(0i32) // index_in_page - placeholder
                                    .bind(product.extracted_at)
                                    .bind(product.extracted_at)
                                    .execute(tx.as_mut())
                                    .await;
                                
                                tracing::info!("Inserted product during cleanup: {}", product.name);
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
        
        let mock_pool = Pool::<Sqlite>::connect("sqlite::memory:").await;
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
        let product = ProductData::new(
            "test123".to_string(),
            "Test Product".to_string(),
            crate::domain::ValidatedUrl::new("https://example.com/test".to_string()).unwrap(),
        ).unwrap()
        .with_manufacturer(Some("Test Company".to_string()));

        // Add products to batch
        let should_flush_1 = db_saver.add_to_batch(product.clone()).await.unwrap();
        assert!(!should_flush_1);

        let should_flush_2 = db_saver.add_to_batch(product.clone()).await.unwrap();
        assert!(!should_flush_2);

        let should_flush_3 = db_saver.add_to_batch(product.clone()).await.unwrap();
        assert!(should_flush_3); // Should flush when batch size is reached
    }

    #[tokio::test]
    async fn worker_properties() {
        let mock_pool = Pool::<Sqlite>::connect("sqlite::memory:").await;
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
