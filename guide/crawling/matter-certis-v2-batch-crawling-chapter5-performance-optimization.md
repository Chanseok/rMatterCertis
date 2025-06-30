# Chapter 5.5: 데이터베이스 성능 최적화

## 개요

Matter Certis v2의 Rust/Tauri 구현에서 대용량 배치 처리의 성능을 극대화하기 위한 데이터베이스 최적화 전략을 다룹니다. 인덱싱, 쿼리 최적화, 메모리 튜닝, 배치 크기 조정 등을 포함합니다.

## 5.5.1 인덱스 전략 및 최적화

### 스마트 인덱싱 시스템

```rust
// src/services/index_optimizer.rs
use sea_orm::{DatabaseConnection, Statement, DbBackend};
use sqlx::Row;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct IndexStrategy {
    pub table_name: String,
    pub columns: Vec<String>,
    pub index_type: IndexType,
    pub is_unique: bool,
    pub is_partial: bool,
    pub condition: Option<String>,
}

#[derive(Debug, Clone)]
pub enum IndexType {
    BTree,
    Hash,
    Gin,
    Gist,
    Composite,
}

pub struct IndexOptimizer {
    db: Arc<DatabaseConnection>,
    query_analyzer: QueryAnalyzer,
}

impl IndexOptimizer {
    pub async fn analyze_and_optimize(&self) -> Result<OptimizationReport, DatabaseError> {
        // 1. 쿼리 패턴 분석
        let query_patterns = self.query_analyzer.analyze_recent_queries().await?;
        
        // 2. 기존 인덱스 분석
        let existing_indexes = self.get_existing_indexes().await?;
        
        // 3. 성능 병목 지점 식별
        let bottlenecks = self.identify_bottlenecks(&query_patterns).await?;
        
        // 4. 인덱스 제안 생성
        let recommendations = self.generate_index_recommendations(
            &query_patterns,
            &existing_indexes,
            &bottlenecks,
        ).await?;
        
        // 5. 인덱스 적용
        let applied = self.apply_recommendations(&recommendations).await?;
        
        Ok(OptimizationReport {
            analyzed_queries: query_patterns.len(),
            existing_indexes: existing_indexes.len(),
            bottlenecks_found: bottlenecks.len(),
            recommendations_generated: recommendations.len(),
            indexes_applied: applied.len(),
            performance_improvement: self.measure_performance_improvement().await?,
        })
    }

    async fn generate_index_recommendations(
        &self,
        patterns: &[QueryPattern],
        existing: &[IndexInfo],
        bottlenecks: &[PerformanceBottleneck],
    ) -> Result<Vec<IndexStrategy>, DatabaseError> {
        let mut recommendations = Vec::new();
        let existing_columns: HashSet<_> = existing
            .iter()
            .flat_map(|idx| idx.columns.iter().cloned())
            .collect();

        for pattern in patterns {
            if pattern.frequency < 10 {
                continue; // 자주 사용되지 않는 쿼리는 스킵
            }

            match &pattern.query_type {
                QueryType::Select { where_columns, order_columns, .. } => {
                    // WHERE 절에 자주 사용되는 컬럼들
                    for column in where_columns {
                        if !existing_columns.contains(column) {
                            recommendations.push(IndexStrategy {
                                table_name: pattern.table_name.clone(),
                                columns: vec![column.clone()],
                                index_type: IndexType::BTree,
                                is_unique: false,
                                is_partial: false,
                                condition: None,
                            });
                        }
                    }

                    // ORDER BY에 사용되는 컬럼들
                    if !order_columns.is_empty() {
                        let composite_key = order_columns.join(",");
                        if !existing_columns.contains(&composite_key) {
                            recommendations.push(IndexStrategy {
                                table_name: pattern.table_name.clone(),
                                columns: order_columns.clone(),
                                index_type: IndexType::Composite,
                                is_unique: false,
                                is_partial: false,
                                condition: None,
                            });
                        }
                    }
                }
                QueryType::Join { join_columns, .. } => {
                    // JOIN에 사용되는 컬럼들
                    for column in join_columns {
                        if !existing_columns.contains(column) {
                            recommendations.push(IndexStrategy {
                                table_name: pattern.table_name.clone(),
                                columns: vec![column.clone()],
                                index_type: IndexType::BTree,
                                is_unique: false,
                                is_partial: false,
                                condition: None,
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        // 부분 인덱스 추천
        self.recommend_partial_indexes(&mut recommendations, bottlenecks);
        
        Ok(recommendations)
    }

    async fn apply_recommendations(
        &self,
        recommendations: &[IndexStrategy],
    ) -> Result<Vec<String>, DatabaseError> {
        let mut applied = Vec::new();

        for strategy in recommendations {
            match self.create_index(strategy).await {
                Ok(index_name) => {
                    info!("Created index: {}", index_name);
                    applied.push(index_name);
                }
                Err(e) => {
                    warn!("Failed to create index for {:?}: {}", strategy, e);
                }
            }
        }

        Ok(applied)
    }
}
```

### 동적 인덱스 관리

```rust
// src/services/dynamic_index_manager.rs
pub struct DynamicIndexManager {
    db: Arc<DatabaseConnection>,
    index_usage_tracker: Arc<IndexUsageTracker>,
    maintenance_scheduler: Arc<MaintenanceScheduler>,
}

impl DynamicIndexManager {
    pub async fn start_monitoring(&self) -> Result<(), DatabaseError> {
        // 인덱스 사용량 모니터링 시작
        self.index_usage_tracker.start_tracking().await?;
        
        // 정기적인 인덱스 유지보수 스케줄링
        self.maintenance_scheduler.schedule_maintenance().await?;
        
        Ok(())
    }

    pub async fn optimize_indexes_periodically(&self) -> Result<(), DatabaseError> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_hours(24));
            
            loop {
                interval.tick().await;
                
                match self.perform_index_maintenance().await {
                    Ok(report) => {
                        info!("Index maintenance completed: {:?}", report);
                    }
                    Err(e) => {
                        error!("Index maintenance failed: {}", e);
                    }
                }
            }
        });
        
        Ok(())
    }

    async fn perform_index_maintenance(&self) -> Result<MaintenanceReport, DatabaseError> {
        let mut report = MaintenanceReport::new();

        // 1. 사용되지 않는 인덱스 식별 및 제거
        let unused_indexes = self.identify_unused_indexes().await?;
        for index in unused_indexes {
            self.drop_index(&index.name).await?;
            report.dropped_indexes.push(index.name);
        }

        // 2. 인덱스 재구성 (REINDEX)
        let fragmented_indexes = self.identify_fragmented_indexes().await?;
        for index in fragmented_indexes {
            self.reindex(&index.name).await?;
            report.reindexed.push(index.name);
        }

        // 3. 인덱스 통계 업데이트
        self.update_index_statistics().await?;
        report.statistics_updated = true;

        Ok(report)
    }
}
```

## 5.5.2 쿼리 최적화

### 쿼리 플래너 및 분석기

```rust
// src/services/query_optimizer.rs
pub struct QueryOptimizer {
    db: Arc<DatabaseConnection>,
    execution_planner: ExecutionPlanner,
    cache_manager: Arc<QueryCacheManager>,
}

impl QueryOptimizer {
    pub async fn optimize_batch_queries(
        &self,
        queries: Vec<BatchQuery>,
    ) -> Result<Vec<OptimizedQuery>, DatabaseError> {
        let mut optimized = Vec::new();

        for query in queries {
            let plan = self.analyze_execution_plan(&query).await?;
            let optimized_query = self.apply_optimizations(&query, &plan).await?;
            optimized.push(optimized_query);
        }

        Ok(optimized)
    }

    async fn analyze_execution_plan(
        &self,
        query: &BatchQuery,
    ) -> Result<ExecutionPlan, DatabaseError> {
        let explain_query = format!("EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {}", query.sql);
        
        let result = self.db
            .query_one(Statement::from_string(
                DbBackend::Postgres,
                explain_query,
            ))
            .await?;

        let plan_json: serde_json::Value = result.try_get("", "QUERY PLAN")?;
        let execution_plan = ExecutionPlan::from_json(&plan_json)?;

        Ok(execution_plan)
    }

    async fn apply_optimizations(
        &self,
        query: &BatchQuery,
        plan: &ExecutionPlan,
    ) -> Result<OptimizedQuery, DatabaseError> {
        let mut optimized_sql = query.sql.clone();
        let mut optimizations_applied = Vec::new();

        // 1. 인덱스 힌트 추가
        if let Some(index_hint) = self.suggest_index_hint(plan) {
            optimized_sql = self.add_index_hint(&optimized_sql, &index_hint);
            optimizations_applied.push("index_hint".to_string());
        }

        // 2. JOIN 순서 최적화
        if plan.has_multiple_joins() {
            optimized_sql = self.optimize_join_order(&optimized_sql, plan)?;
            optimizations_applied.push("join_order".to_string());
        }

        // 3. WHERE 절 조건 순서 최적화
        if plan.has_complex_where_clause() {
            optimized_sql = self.optimize_where_clause(&optimized_sql, plan)?;
            optimizations_applied.push("where_clause".to_string());
        }

        // 4. LIMIT/OFFSET 최적화
        if query.has_pagination() {
            optimized_sql = self.optimize_pagination(&optimized_sql, plan)?;
            optimizations_applied.push("pagination".to_string());
        }

        Ok(OptimizedQuery {
            original_sql: query.sql.clone(),
            optimized_sql,
            estimated_improvement: plan.estimated_improvement,
            optimizations_applied,
        })
    }
}
```

### 배치 쿼리 최적화

```rust
// src/services/batch_query_optimizer.rs
pub struct BatchQueryOptimizer {
    db: Arc<DatabaseConnection>,
    connection_pool: Arc<ConnectionPool>,
}

impl BatchQueryOptimizer {
    pub async fn execute_optimized_batch(
        &self,
        batch: &BatchOperation,
    ) -> Result<BatchResult, DatabaseError> {
        match batch.operation_type {
            BatchOperationType::BulkInsert => {
                self.execute_bulk_insert_optimized(&batch.items).await
            }
            BatchOperationType::BulkUpdate => {
                self.execute_bulk_update_optimized(&batch.items).await
            }
            BatchOperationType::BulkUpsert => {
                self.execute_bulk_upsert_optimized(&batch.items).await
            }
        }
    }

    async fn execute_bulk_insert_optimized(
        &self,
        items: &[DeviceInfo],
    ) -> Result<BatchResult, DatabaseError> {
        // COPY 명령어를 사용한 대량 삽입 최적화
        let copy_sql = r#"
            COPY device_info (device_id, device_name, ip_address, status, created_at)
            FROM STDIN WITH (FORMAT CSV, HEADER false)
        "#;

        let mut csv_data = String::new();
        for item in items {
            csv_data.push_str(&format!(
                "{},{},{},{},{}\n",
                item.device_id,
                item.device_name,
                item.ip_address,
                item.status,
                item.created_at.format("%Y-%m-%d %H:%M:%S")
            ));
        }

        let mut conn = self.connection_pool.get().await?;
        let mut copy_in = conn.copy_in(&copy_sql).await?;
        copy_in.send_data(csv_data.as_bytes()).await?;
        let rows_affected = copy_in.finish().await?;

        Ok(BatchResult {
            success_count: rows_affected as usize,
            failure_count: 0,
            errors: Vec::new(),
        })
    }

    async fn execute_bulk_upsert_optimized(
        &self,
        items: &[DeviceInfo],
    ) -> Result<BatchResult, DatabaseError> {
        // VALUES 절과 ON CONFLICT를 사용한 대량 UPSERT
        let placeholders: Vec<String> = (0..items.len())
            .map(|i| {
                let base = i * 5;
                format!("(${}, ${}, ${}, ${}, ${})", 
                    base + 1, base + 2, base + 3, base + 4, base + 5)
            })
            .collect();

        let upsert_sql = format!(
            r#"
            INSERT INTO device_info (device_id, device_name, ip_address, status, created_at)
            VALUES {}
            ON CONFLICT (device_id) 
            DO UPDATE SET
                device_name = EXCLUDED.device_name,
                ip_address = EXCLUDED.ip_address,
                status = EXCLUDED.status,
                updated_at = CURRENT_TIMESTAMP
            "#,
            placeholders.join(", ")
        );

        let mut params = Vec::new();
        for item in items {
            params.push(&item.device_id);
            params.push(&item.device_name);
            params.push(&item.ip_address);
            params.push(&item.status);
            params.push(&item.created_at);
        }

        let result = sqlx::query(&upsert_sql)
            .bind_all(params)
            .execute(&*self.connection_pool.get().await?)
            .await?;

        Ok(BatchResult {
            success_count: result.rows_affected() as usize,
            failure_count: 0,
            errors: Vec::new(),
        })
    }
}
```

## 5.5.3 메모리 최적화

### 스마트 메모리 관리

```rust
// src/services/memory_optimizer.rs
use std::sync::atomic::{AtomicUsize, Ordering};
use sysinfo::{System, SystemExt};

pub struct MemoryOptimizer {
    system_info: Arc<Mutex<System>>,
    memory_threshold: Arc<AtomicUsize>,
    gc_scheduler: Arc<GarbageCollectionScheduler>,
}

impl MemoryOptimizer {
    pub fn new() -> Self {
        Self {
            system_info: Arc::new(Mutex::new(System::new_all())),
            memory_threshold: Arc::new(AtomicUsize::new(80)), // 80% 임계값
            gc_scheduler: Arc::new(GarbageCollectionScheduler::new()),
        }
    }

    pub async fn monitor_and_optimize(&self) -> Result<(), MemoryError> {
        let memory_monitor = self.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                if let Err(e) = memory_monitor.check_and_optimize().await {
                    error!("Memory optimization failed: {}", e);
                }
            }
        });

        Ok(())
    }

    async fn check_and_optimize(&self) -> Result<(), MemoryError> {
        let memory_usage = self.get_memory_usage().await?;
        
        if memory_usage.used_percentage > self.memory_threshold.load(Ordering::Relaxed) {
            warn!("High memory usage detected: {}%", memory_usage.used_percentage);
            
            // 메모리 정리 수행
            self.perform_memory_cleanup().await?;
            
            // 캐시 크기 조정
            self.adjust_cache_sizes().await?;
            
            // 배치 크기 축소
            self.reduce_batch_sizes().await?;
        }

        Ok(())
    }

    async fn perform_memory_cleanup(&self) -> Result<(), MemoryError> {
        // 1. 사용하지 않는 연결 정리
        self.cleanup_idle_connections().await?;
        
        // 2. 캐시 정리
        self.cleanup_caches().await?;
        
        // 3. 임시 데이터 정리
        self.cleanup_temporary_data().await?;
        
        // 4. 강제 가비지 컬렉션 (필요한 경우)
        if self.get_memory_usage().await?.used_percentage > 90 {
            self.force_garbage_collection().await?;
        }

        Ok(())
    }
}
```

### 배치 크기 동적 조정

```rust
// src/services/batch_size_optimizer.rs
pub struct BatchSizeOptimizer {
    performance_history: Arc<Mutex<VecDeque<PerformanceMetric>>>,
    current_batch_size: Arc<AtomicUsize>,
    min_batch_size: usize,
    max_batch_size: usize,
}

impl BatchSizeOptimizer {
    pub fn new(initial_size: usize, min_size: usize, max_size: usize) -> Self {
        Self {
            performance_history: Arc::new(Mutex::new(VecDeque::with_capacity(100))),
            current_batch_size: Arc::new(AtomicUsize::new(initial_size)),
            min_batch_size: min_size,
            max_batch_size: max_size,
        }
    }

    pub async fn optimize_batch_size(
        &self,
        current_performance: PerformanceMetric,
    ) -> usize {
        let mut history = self.performance_history.lock().await;
        history.push_back(current_performance);
        
        // 최근 10개 메트릭 유지
        if history.len() > 10 {
            history.pop_front();
        }

        if history.len() < 3 {
            return self.current_batch_size.load(Ordering::Relaxed);
        }

        let avg_throughput = history.iter()
            .map(|m| m.throughput)
            .sum::<f64>() / history.len() as f64;

        let current_size = self.current_batch_size.load(Ordering::Relaxed);
        let new_size = self.calculate_optimal_size(avg_throughput, current_size);

        self.current_batch_size.store(new_size, Ordering::Relaxed);
        new_size
    }

    fn calculate_optimal_size(&self, avg_throughput: f64, current_size: usize) -> usize {
        let target_memory_usage = 70.0; // 70% 목표
        let current_memory_usage = self.get_current_memory_usage();

        let size_adjustment = if current_memory_usage > target_memory_usage {
            // 메모리 사용량이 높으면 배치 크기 감소
            (current_size as f64 * 0.8) as usize
        } else if current_memory_usage < target_memory_usage * 0.7 {
            // 메모리 여유가 있으면 배치 크기 증가
            (current_size as f64 * 1.2) as usize
        } else {
            current_size
        };

        size_adjustment.max(self.min_batch_size).min(self.max_batch_size)
    }
}
```

## 5.5.4 연결 풀 최적화

### 적응형 연결 풀

```rust
// src/services/adaptive_connection_pool.rs
use sqlx::{Pool, Postgres, PoolOptions};
use std::sync::Arc;

pub struct AdaptiveConnectionPool {
    pool: Arc<Pool<Postgres>>,
    pool_config: Arc<Mutex<PoolConfig>>,
    metrics: Arc<PoolMetrics>,
}

#[derive(Debug, Clone)]
pub struct PoolConfig {
    pub min_connections: u32,
    pub max_connections: u32,
    pub acquire_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_lifetime: Duration,
}

impl AdaptiveConnectionPool {
    pub async fn new(database_url: &str, initial_config: PoolConfig) -> Result<Self, sqlx::Error> {
        let pool = PoolOptions::new()
            .min_connections(initial_config.min_connections)
            .max_connections(initial_config.max_connections)
            .acquire_timeout(initial_config.acquire_timeout)
            .idle_timeout(Some(initial_config.idle_timeout))
            .max_lifetime(Some(initial_config.max_lifetime))
            .connect(database_url)
            .await?;

        Ok(Self {
            pool: Arc::new(pool),
            pool_config: Arc::new(Mutex::new(initial_config)),
            metrics: Arc::new(PoolMetrics::new()),
        })
    }

    pub async fn start_optimization(&self) -> Result<(), DatabaseError> {
        let pool_optimizer = self.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                
                if let Err(e) = pool_optimizer.optimize_pool_settings().await {
                    error!("Pool optimization failed: {}", e);
                }
            }
        });

        Ok(())
    }

    async fn optimize_pool_settings(&self) -> Result<(), DatabaseError> {
        let current_metrics = self.metrics.get_current_metrics().await;
        let mut config = self.pool_config.lock().await;

        // 연결 대기 시간이 길면 최대 연결 수 증가
        if current_metrics.avg_acquire_time > Duration::from_millis(500) {
            config.max_connections = (config.max_connections + 2).min(50);
            info!("Increased max connections to {}", config.max_connections);
        }

        // 유휴 연결이 많으면 최소 연결 수 감소
        if current_metrics.idle_connections > config.max_connections / 2 {
            config.min_connections = (config.min_connections.saturating_sub(1)).max(1);
            info!("Decreased min connections to {}", config.min_connections);
        }

        // 연결 실패율이 높으면 타임아웃 증가
        if current_metrics.connection_failure_rate > 0.05 {
            config.acquire_timeout = (config.acquire_timeout * 2).min(Duration::from_secs(30));
            info!("Increased acquire timeout to {:?}", config.acquire_timeout);
        }

        Ok(())
    }
}
```

## 5.5.5 성능 모니터링 및 튜닝

### 종합 성능 모니터

```rust
// src/services/performance_monitor.rs
use prometheus::{Histogram, Counter, Gauge, Registry};

pub struct PerformanceMonitor {
    db_query_duration: Histogram,
    batch_processing_duration: Histogram,
    memory_usage: Gauge,
    connection_pool_usage: Gauge,
    transaction_rate: Counter,
    error_rate: Counter,
    registry: Arc<Registry>,
}

impl PerformanceMonitor {
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Arc::new(Registry::new());
        
        let db_query_duration = Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "db_query_duration_seconds",
                "Database query execution time"
            ).buckets(vec![0.001, 0.01, 0.1, 1.0, 10.0])
        )?;

        let batch_processing_duration = Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "batch_processing_duration_seconds", 
                "Batch processing execution time"
            ).buckets(vec![1.0, 10.0, 30.0, 60.0, 300.0])
        )?;

        let memory_usage = Gauge::new(
            "memory_usage_bytes",
            "Current memory usage in bytes"
        )?;

        let connection_pool_usage = Gauge::new(
            "connection_pool_active",
            "Active database connections"
        )?;

        let transaction_rate = Counter::new(
            "transactions_total",
            "Total number of transactions"
        )?;

        let error_rate = Counter::new(
            "errors_total", 
            "Total number of errors"
        )?;

        registry.register(Box::new(db_query_duration.clone()))?;
        registry.register(Box::new(batch_processing_duration.clone()))?;
        registry.register(Box::new(memory_usage.clone()))?;
        registry.register(Box::new(connection_pool_usage.clone()))?;
        registry.register(Box::new(transaction_rate.clone()))?;
        registry.register(Box::new(error_rate.clone()))?;

        Ok(Self {
            db_query_duration,
            batch_processing_duration,
            memory_usage,
            connection_pool_usage,
            transaction_rate,
            error_rate,
            registry,
        })
    }

    pub async fn start_monitoring(&self) -> Result<(), MonitoringError> {
        let monitor = self.clone();
        
        // 시스템 메트릭 수집
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            
            loop {
                interval.tick().await;
                monitor.collect_system_metrics().await;
            }
        });

        // 성능 리포트 생성
        self.start_performance_reporting().await?;

        Ok(())
    }

    async fn collect_system_metrics(&self) {
        // 메모리 사용량 수집
        if let Ok(memory_info) = self.get_memory_info().await {
            self.memory_usage.set(memory_info.used_bytes as f64);
        }

        // 연결 풀 상태 수집
        if let Ok(pool_info) = self.get_pool_info().await {
            self.connection_pool_usage.set(pool_info.active_connections as f64);
        }
    }

    pub async fn generate_performance_report(&self) -> Result<PerformanceReport, MonitoringError> {
        let metrics = self.registry.gather();
        
        let report = PerformanceReport {
            timestamp: chrono::Utc::now(),
            query_performance: self.analyze_query_performance(&metrics)?,
            batch_performance: self.analyze_batch_performance(&metrics)?,
            resource_usage: self.analyze_resource_usage(&metrics)?,
            recommendations: self.generate_recommendations(&metrics)?,
        };

        Ok(report)
    }

    fn generate_recommendations(
        &self,
        metrics: &[prometheus::proto::MetricFamily],
    ) -> Result<Vec<PerformanceRecommendation>, MonitoringError> {
        let mut recommendations = Vec::new();

        // 쿼리 성능 기반 추천
        if let Some(avg_query_time) = self.get_average_query_time(metrics) {
            if avg_query_time > 1.0 {
                recommendations.push(PerformanceRecommendation {
                    category: RecommendationCategory::Query,
                    priority: Priority::High,
                    description: "Average query time is too high".to_string(),
                    action: "Consider adding indexes or optimizing queries".to_string(),
                });
            }
        }

        // 메모리 사용량 기반 추천
        if let Some(memory_usage) = self.get_memory_usage_percentage(metrics) {
            if memory_usage > 80.0 {
                recommendations.push(PerformanceRecommendation {
                    category: RecommendationCategory::Memory,
                    priority: Priority::Medium,
                    description: "High memory usage detected".to_string(),
                    action: "Reduce batch sizes or increase available memory".to_string(),
                });
            }
        }

        Ok(recommendations)
    }
}
```

### 자동 튜닝 시스템

```rust
// src/services/auto_tuner.rs
pub struct AutoTuner {
    performance_monitor: Arc<PerformanceMonitor>,
    batch_size_optimizer: Arc<BatchSizeOptimizer>,
    connection_pool: Arc<AdaptiveConnectionPool>,
    index_optimizer: Arc<IndexOptimizer>,
}

impl AutoTuner {
    pub async fn start_auto_tuning(&self) -> Result<(), TuningError> {
        let tuner = self.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5분마다
            
            loop {
                interval.tick().await;
                
                if let Err(e) = tuner.perform_tuning_cycle().await {
                    error!("Auto-tuning cycle failed: {}", e);
                }
            }
        });

        Ok(())
    }

    async fn perform_tuning_cycle(&self) -> Result<(), TuningError> {
        // 1. 성능 리포트 생성
        let performance_report = self.performance_monitor.generate_performance_report().await?;
        
        // 2. 추천사항 분석
        for recommendation in &performance_report.recommendations {
            match recommendation.category {
                RecommendationCategory::Query => {
                    self.apply_query_optimization(recommendation).await?;
                }
                RecommendationCategory::Memory => {
                    self.apply_memory_optimization(recommendation).await?;
                }
                RecommendationCategory::ConnectionPool => {
                    self.apply_connection_pool_optimization(recommendation).await?;
                }
                RecommendationCategory::Index => {
                    self.apply_index_optimization(recommendation).await?;
                }
            }
        }

        // 3. 튜닝 결과 로깅
        info!("Auto-tuning cycle completed with {} recommendations applied", 
              performance_report.recommendations.len());

        Ok(())
    }
}
```

이 섹션에서는 Rust/Tauri 환경에서의 포괄적인 데이터베이스 성능 최적화 전략을 다루었습니다. 인덱싱, 쿼리 최적화, 메모리 관리, 연결 풀 튜닝, 그리고 자동 성능 모니터링까지 모든 측면을 포함하여 대용량 배치 처리의 성능을 극대화할 수 있습니다.

Chapter 5의 모든 섹션이 완성되었습니다:
- 5.1: 데이터베이스 스키마 및 모델 정의
- 5.2: 배치 저장 및 업데이트 로직  
- 5.3: 진행 상태 영속화 시스템
- 5.4: 트랜잭션 관리 및 에러 복구
- 5.5: 데이터베이스 성능 최적화
