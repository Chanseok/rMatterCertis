//! `StageActor`: 개별 스테이지 작업 처리 Actor
//!
//! Phase 3: Actor 구현 - 스테이지 레벨 작업 실행 및 관리
//! Modern Rust 2024 준수: 함수형 원칙, 명시적 의존성, 상태 최소화

use chrono::Utc;
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::crawl_engine::actors::traits::{Actor, ActorHealth, ActorStatus, ActorType};
use crate::crawl_engine::actors::types::{
    ActorCommand, ActorError, AppEvent, SimpleMetrics, StageError, StageItemResult, StageItemType,
    StageResult, StageType,
};
use crate::crawl_engine::channels::types::StageItem;
use crate::crawl_engine::integrated_context::AppContext;
use crate::crawl_engine::stages::traits::StageLogicFactory;
// Removed direct service imports; StageActor relies on strategy layer deps only
use crate::infrastructure::config::AppConfig;
use crate::infrastructure::{HttpClient, IntegratedProductRepository, MatterDataExtractor};

// 단일 아이템 실행 입력 값 묶음 (module scope)
struct TaskInput {
    ctx: AppContext,
    stage_type: StageType,
    item: StageItem,
    session_id: String,
    batch_id: Option<String>,
    strategy_factory: Arc<dyn StageLogicFactory + Send + Sync>,
    deps: Arc<StageDeps>,
    total_pages_hint: Option<u32>,
    products_on_last_page_hint: Option<u32>,
}

/// Dependency bundle for `StageActor` (to move construction out of the actor)
#[derive(Clone)]
pub struct StageDeps {
    pub http_client: Arc<HttpClient>,
    pub data_extractor: Arc<MatterDataExtractor>,
    pub product_repo: Arc<IntegratedProductRepository>,
    pub app_config: AppConfig,
    /// 중복 URL 저장 정책 (수동 실행 등에서 제어)
    pub duplicate_policy: crate::crawl_engine::actors::types::DuplicatePersistencePolicy,
}

// Duplicate-execution guard for DataSaving stage (session+batch scoped)
static DATA_SAVING_RUN_GUARD: Lazy<StdMutex<HashSet<String>>> =
    Lazy::new(|| StdMutex::new(HashSet::new()));

// Lightweight per-session metrics aggregator (throttled, best-effort)
#[derive(Default, Clone)]
struct MetricsWindow {
    last_emit: Option<Instant>,
    item_count: u64,
    sum_latency_ms: u128,
    success: u64,
    failure: u64,
}

static METRICS_BY_SESSION: Lazy<StdMutex<HashMap<String, MetricsWindow>>> =
    Lazy::new(|| StdMutex::new(HashMap::new()));

/// RAII 가드: 시작 시 StageItemStarted 이벤트, Drop 시 StageItemCompleted 이벤트 자동 발행
struct TaskExecutionGuard {
    ctx: AppContext,
    session_id: String,
    batch_id: Option<String>,
    stage_type: StageType,
    item_id: String,
    item_type: StageItemType,
    started_at: Instant,
    // 완료 정보 (없으면 Drop에서 best-effort로 실패 처리)
    outcome: Option<GuardOutcome>,
}

struct GuardOutcome {
    success: bool,
    error: Option<String>,
    duration_ms: u64,
    retry_count: u32,
    collected_count: Option<u32>,
}

impl TaskExecutionGuard {
    fn new(ctx: AppContext, session_id: String, batch_id: Option<String>, stage_type: StageType, item_id: String, item_type: StageItemType) -> Self {
        // 시작 이벤트 발행 (best-effort)
        let _ = ctx.emit_event(AppEvent::StageItemStarted {
            session_id: session_id.clone(),
            batch_id: batch_id.clone(),
            stage_type: stage_type.clone(),
            item_id: item_id.clone(),
            item_type: item_type.clone(),
            timestamp: Utc::now(),
        });
        Self {
            ctx,
            session_id,
            batch_id,
            stage_type,
            item_id,
            item_type,
            started_at: Instant::now(),
            outcome: None,
        }
    }
    fn record_ok(&mut self, retry_count: u32, collected_count: Option<u32>) {
        self.outcome = Some(GuardOutcome {
            success: true,
            error: None,
            duration_ms: self.started_at.elapsed().as_millis() as u64,
            retry_count,
            collected_count,
        });
    }
    fn record_err(&mut self, err: String) {
        self.outcome = Some(GuardOutcome {
            success: false,
            error: Some(err),
            duration_ms: self.started_at.elapsed().as_millis() as u64,
            retry_count: 0,
            collected_count: None,
        });
    }
}

impl Drop for TaskExecutionGuard {
    fn drop(&mut self) {
        // 완료 이벤트 발행 (best-effort, Drop 내에서 실패 무시)
        let (success, error, duration_ms, retry_count, collected_count) = if let Some(o) = &self.outcome {
            (o.success, o.error.clone(), o.duration_ms, o.retry_count, o.collected_count)
        } else {
            (false, Some("unknown_error_or_early_drop".into()), self.started_at.elapsed().as_millis() as u64, 0, None)
        };
        let _ = self.ctx.emit_event(AppEvent::StageItemCompleted {
            session_id: self.session_id.clone(),
            batch_id: self.batch_id.clone(),
            stage_type: self.stage_type.clone(),
            item_id: self.item_id.clone(),
            item_type: self.item_type.clone(),
            success,
            error,
            duration_ms,
            retry_count,
            collected_count,
            timestamp: Utc::now(),
        });
    }
}

/// 스테이지 상태 열거형 (local to `StageActor`)
#[derive(Debug, Clone, PartialEq)]
enum StageState {
    Idle,
    Starting,
    Processing,
    Completed,
    Failed { error: String },
    Timeout,
}

/// `StageActor`: 개별 스테이지 작업의 실행 및 관리
#[allow(clippy::struct_excessive_bools)]
pub struct StageActor {
    // 기본 메타데이터
    actor_id: String,
    pub batch_id: String,
    stage_id: Option<String>,
    stage_type: Option<StageType>,
    state: StageState,
    start_time: Option<Instant>,

    // 진행 카운터
    total_items: u32,
    completed_items: u32,
    success_count: u32,
    failure_count: u32,
    skipped_count: u32,
    item_results: Vec<StageItemResult>,

    // 실제 크롤링 엔진 의존성 (응집)
    deps: Arc<StageDeps>,

    // 상위에서 주입되는 페이지네이션 힌트
    site_total_pages_hint: Option<u32>,
    products_on_last_page_hint: Option<u32>,

    // 전략 분기 (Phase 3)
    strategy_factory: Arc<dyn StageLogicFactory + Send + Sync>,
}

// Extension trait to restore helper methods expected by per-item task logic
trait StageItemExt {
    fn id_string(&self) -> String;
    fn item_type_enum(&self) -> StageItemType;
}

impl StageItemExt for StageItem {
    fn id_string(&self) -> String {
        match self {
            Self::Page(p) => format!("page_{}", p),
            Self::Url(u) => u.clone(),
            Self::Product(p) => p.url.clone(),
            Self::ProductList(l) => format!("list_page_{}", l.page_number),
            Self::ProductUrls(urls) => format!("product_urls_{}", urls.urls.len()),
            Self::ProductDetails(d) => format!("product_details_{}", d.products.len()),
            Self::ValidatedProducts(v) => format!("validated_products_{}", v.products.len()),
            Self::ValidationTarget(v) => format!("validation_target_{}", v.len()),
        }
    }
    fn item_type_enum(&self) -> StageItemType {
        match self {
            Self::Page(page) => StageItemType::Page { page_number: *page },
            Self::Url(_u) => StageItemType::Url { url_type: "generic".into() },
            Self::Product(_p) => StageItemType::Url { url_type: "product".into() },
            Self::ProductList(_l) => StageItemType::ProductUrls { urls: vec![] },
            Self::ProductUrls(list) => StageItemType::ProductUrls {
                urls: list.urls.iter().map(|u| u.url.clone()).collect(),
            },
            Self::ProductDetails(_d) => StageItemType::Url { url_type: "product_details".into() },
            Self::ValidatedProducts(_v) => StageItemType::Url { url_type: "validated_products".into() },
            Self::ValidationTarget(_t) => StageItemType::Url { url_type: "validation_target".into() },
        }
    }
}

impl StageActor {
    // 개별 태스크 실행 헬퍼 (spawn 대상)
    async fn execute_single_item_task(
        sem: Arc<tokio::sync::Semaphore>,
        input: TaskInput,
    ) -> Result<StageItemResult, StageError> {
        let TaskInput {
            ctx,
            stage_type,
            item,
            session_id,
            batch_id,
            strategy_factory,
            deps,
            total_pages_hint,
            products_on_last_page_hint,
        } = input;

        let _permit = sem.acquire().await.map_err(|e| StageError::GenericError {
            message: format!("Semaphore error: {}", e),
        })?;

        // 미들웨어 + RAII 가드로 이벤트 자동화
        StageActor::before_each_item_hook(&ctx, &stage_type, &item).await;
        let mut guard = TaskExecutionGuard::new(
            ctx.clone(),
            session_id.clone(),
            batch_id.clone(),
            stage_type.clone(),
            item.id_string(),
            item.item_type_enum(),
        );

        // coarse lifecycle 프리-이벤트
        let lifecycle_item = item.clone();
        match (&stage_type, &item) {
            (StageType::ListPageCrawling, StageItem::Page(pn)) => {
                let _ = ctx.emit_event(AppEvent::PageLifecycle {
                    session_id: session_id.clone(),
                    batch_id: batch_id.clone(),
                    page_number: *pn,
                    status: "fetch_started".into(),
                    metrics: None,
                    timestamp: Utc::now(),
                });
            }
            (StageType::ProductDetailCrawling, StageItem::ProductUrls(urls_wrapper)) => {
                // best-effort DB 체크로 스케줄 수 추정 후 이벤트
                let mut ct = 0u32;
                for u in &urls_wrapper.urls {
                    match deps.product_repo.get_product_detail_by_url(&u.url).await {
                        Ok(existing) => if existing.is_none() { ct += 1 },
                        Err(e) => {
                            warn!("[DetailFilter] DB check failed url={} err={}", u.url, e);
                            ct += 1;
                        }
                    }
                }
                if let Some(first) = urls_wrapper.urls.first() {
                    let _ = ctx.emit_event(AppEvent::PageLifecycle {
                        session_id: session_id.clone(),
                        batch_id: batch_id.clone(),
                        page_number: first.page_id as u32,
                        status: "detail_mapping_emitted".into(),
                        metrics: Some(SimpleMetrics::Page {
                            url_count: Some(urls_wrapper.urls.len() as u32),
                            scheduled_details: Some(ct),
                            error: None,
                        }),
                        timestamp: Utc::now(),
                    });
                }
            }
            _ => {}
        }

        let item_start = Instant::now();

        // 전략 실행
        let result = if let Some(logic) = strategy_factory.logic_for(&stage_type) {
            let deps_in = crate::crawl_engine::stages::traits::Deps {
                http: deps.http_client.clone(),
                extractor: deps.data_extractor.clone(),
                repo: deps.product_repo.clone(),
                duplicate_policy: deps.duplicate_policy.clone(),
                list_collector: None,
                detail_collector: None,
            };
            let input = crate::crawl_engine::stages::traits::StageInput {
                stage_type: stage_type.clone(),
                item: item.clone(),
                config: deps.app_config.clone(),
                deps: deps_in,
                total_pages_hint,
                products_on_last_page_hint,
            };
            match logic.execute(input).await {
                Ok(crate::crawl_engine::stages::traits::StageOutput { result }) => Ok(result),
                Err(e) => Err(StageError::GenericError {
                    message: format!("Strategy error: {}", e),
                }),
            }
        } else {
            Err(StageError::GenericError {
                message: format!("No strategy registered for stage {:?}", stage_type),
            })
        };

        // 미들웨어 사후 훅
        StageActor::after_each_item_hook(&ctx, &stage_type, &item, &result, item_start).await;

        // 결과 기반 이벤트 및 퍼시스턴스
        match &result {
            Ok(r) => {
                // Validation 집계 이벤트
                if matches!(stage_type, StageType::DataValidation) {
                    let (products_found, products_checked, divergences, anomalies) = (|| {
                        if let Some(json) = &r.collected_data {
                            if let Ok(validated) = serde_json::from_str::<
                                Vec<crate::domain::product::ProductDetail>,
                            >(json)
                            {
                                let found = validated.len() as u32;
                                let report = crate::crawl_engine::services::data_quality_analyzer::DataQualityAnalyzer::new()
                                    .analyze_product_quality(&validated)
                                    .ok();
                                let (div_ct, anom_ct) = if let Some(rep) = report {
                                    let dup = rep
                                        .issues
                                        .iter()
                                        .filter(|i| matches!(i.issue_type, crate::crawl_engine::services::data_quality_analyzer::IssueType::Duplicate))
                                        .count() as u32;
                                    let anom = rep
                                        .issues
                                        .iter()
                                        .filter(|i| matches!(i.severity, crate::crawl_engine::services::data_quality_analyzer::IssueSeverity::Critical | crate::crawl_engine::services::data_quality_analyzer::IssueSeverity::Warning))
                                        .count() as u32;
                                    (dup, anom)
                                } else { (0, 0) };
                                return (found, u64::from(found), div_ct, anom_ct);
                            }
                        }
                        (0, 0, 0, 0)
                    })();
                    let _ = ctx.emit_event(AppEvent::ValidationStarted {
                        session_id: session_id.clone(),
                        scan_pages: 1,
                        total_pages_site: None,
                        timestamp: Utc::now(),
                    });
                    let _ = ctx.emit_event(AppEvent::ValidationPageScanned {
                        session_id: session_id.clone(),
                        physical_page: 0,
                        products_found,
                        assigned_start_offset: 0,
                        assigned_end_offset: u64::from(products_found.saturating_sub(1)),
                        timestamp: Utc::now(),
                    });
                    let _ = ctx.emit_event(AppEvent::ValidationCompleted {
                        session_id: session_id.clone(),
                        pages_scanned: 1,
                        products_checked,
                        divergences,
                        anomalies,
                        duration_ms: item_start.elapsed().as_millis() as u64,
                        timestamp: Utc::now(),
                    });
                }

                // ProductDetail 크롤링 그룹 완료 이벤트
                if matches!(stage_type, StageType::ProductDetailCrawling) {
                    if let StageItem::ProductUrls(ref urls) = lifecycle_item {
                        let page_hint = urls.urls.first().map_or(0u32, |u| u.page_id as u32);
                        let total = urls.urls.len() as u32;
                        let duration_ms = item_start.elapsed().as_millis() as u64;
                        let _ = ctx.emit_event(AppEvent::ProductLifecycleGroup {
                            session_id: session_id.clone(),
                            batch_id: batch_id.clone(),
                            page_number: Some(page_hint),
                            group_size: total,
                            started: total,
                            succeeded: total,
                            failed: 0,
                            duplicates: 0,
                            duration_ms,
                            phase: "fetch".into(),
                            timestamp: Utc::now(),
                        });
                    }
                }

                // DataSaving 퍼시스턴스
                if matches!(stage_type, StageType::DataSaving) {
                    let is_persist_target = matches!(lifecycle_item, StageItem::ProductDetails(_))
                        || matches!(lifecycle_item, StageItem::ValidatedProducts(_));
                    if is_persist_target {
                        let guard_key = format!(
                            "{}:{}:data_saving",
                            session_id,
                            batch_id.clone().unwrap_or_else(|| "none".into())
                        );
                        if let Ok(mut guard) = DATA_SAVING_RUN_GUARD.lock() {
                            if guard.contains(&guard_key) {
                                return Ok(StageItemResult {
                                    item_id: "data_saving_guard".into(),
                                    item_type: StageItemType::Url { url_type: "data_saving".into() },
                                    success: true,
                                    error: None,
                                    duration_ms: item_start.elapsed().as_millis() as u64,
                                    retry_count: 0,
                                    collected_data: None,
                                });
                            }
                            guard.insert(guard_key);
                        }
                        let skip_save = std::env::var("MC_SKIP_DB_SAVE")
                            .ok()
                            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
                        if skip_save {
                            let _ = ctx.emit_event(AppEvent::ProductLifecycle {
                                session_id: session_id.clone(),
                                batch_id: batch_id.clone(),
                                page_number: None,
                                product_ref: "_batch_persist".into(),
                                status: "persist_skipped".into(),
                                retry: None,
                                duration_ms: None,
                                metrics: Some(SimpleMetrics::Generic { key: "reason".into(), value: "MC_SKIP_DB_SAVE".into() }),
                                timestamp: Utc::now(),
                            });
                        } else {
                            let attempted_count = match &lifecycle_item {
                                StageItem::ValidatedProducts(v) => v.products.len() as u32,
                                StageItem::ProductDetails(d) => d.products.len() as u32,
                                _ => 0,
                            };
                            let _ = ctx.emit_event(AppEvent::ProductLifecycle {
                                session_id: session_id.clone(),
                                batch_id: batch_id.clone(),
                                page_number: None,
                                product_ref: "_batch_persist".into(),
                                status: "persist_started".into(),
                                retry: None,
                                duration_ms: None,
                                metrics: Some(SimpleMetrics::Generic { key: "attempted_count".into(), value: attempted_count.to_string() }),
                                timestamp: Utc::now(),
                            });
                            if attempted_count == 0 {
                                let _ = ctx.emit_event(AppEvent::ProductLifecycle {
                                    session_id: session_id.clone(),
                                    batch_id: batch_id.clone(),
                                    page_number: None,
                                    product_ref: "_batch_persist".into(),
                                    status: "persist_empty".into(),
                                    retry: None,
                                    duration_ms: Some(0),
                                    metrics: Some(SimpleMetrics::Generic { key: "persist_result".into(), value: "attempted=0".into() }),
                                    timestamp: Utc::now(),
                                });
                                return Ok(StageItemResult { item_id: "data_saving_empty".into(), item_type: StageItemType::Url { url_type: "data_saving".into() }, success: true, error: None, duration_ms: item_start.elapsed().as_millis() as u64, retry_count: 0, collected_data: None });
                            }
                            let persist_start = Instant::now();
                            match Self::execute_real_database_storage(&lifecycle_item, deps.product_repo.clone(), deps.duplicate_policy.clone()).await {
                                Ok((inserted, updated, duplicates_ct)) => {
                                    let attempted = attempted_count;
                                    let consumed = inserted + updated + duplicates_ct;
                                    let unchanged = attempted.saturating_sub(consumed);
                                    let status = if inserted > 0 && updated == 0 { "persist_inserted" }
                                        else if updated > 0 && inserted == 0 { "persist_updated" }
                                        else if inserted == 0 && updated == 0 { if duplicates_ct == attempted { "persist_noop_all_duplicate" } else { "persist_noop" } }
                                        else { "persist_mixed" };
                                    let metrics = SimpleMetrics::Generic { key: "persist_result".into(), value: format!("attempted={},inserted={},updated={},duplicates={},unchanged={}", attempted, inserted, updated, duplicates_ct, unchanged) };
                                    let _ = ctx.emit_event(AppEvent::ProductLifecycle { session_id: session_id.clone(), batch_id: batch_id.clone(), page_number: None, product_ref: "_batch_persist".into(), status: status.into(), retry: None, duration_ms: Some(persist_start.elapsed().as_millis() as u64), metrics: Some(metrics), timestamp: Utc::now() });
                                }
                                Err(e) => {
                                    let _ = ctx.emit_event(AppEvent::ProductLifecycle { session_id: session_id.clone(), batch_id: batch_id.clone(), page_number: None, product_ref: "_batch_persist".into(), status: "persist_failed".into(), retry: None, duration_ms: Some(persist_start.elapsed().as_millis() as u64), metrics: Some(SimpleMetrics::Generic { key: "error".into(), value: e }), timestamp: Utc::now() });
                                }
                            }
                        }
                    }
                }

                guard.record_ok(
                    r.retry_count,
                    r.collected_data.as_ref().map(|d| if d.starts_with('[') { d.matches('"').count() as u32 / 2 } else { 1 }),
                );
            }
            Err(err) => {
                guard.record_err(format!("{:?}", err));
                if let (StageType::ListPageCrawling, StageItem::Page(pn)) = (&stage_type, &lifecycle_item) {
                    let _ = ctx.emit_event(AppEvent::PageLifecycle { session_id: session_id.clone(), batch_id: batch_id.clone(), page_number: *pn, status: "failed".into(), metrics: Some(SimpleMetrics::Page { url_count: None, scheduled_details: None, error: Some(format!("{:?}", err)) }), timestamp: Utc::now() });
                }
                if let (StageType::ProductDetailCrawling, StageItem::ProductUrls(urls)) = (&stage_type, &lifecycle_item) {
                    for pu in &urls.urls {
                        let _ = ctx.emit_event(AppEvent::ProductLifecycle { session_id: session_id.clone(), batch_id: batch_id.clone(), page_number: Some(pu.page_id as u32), product_ref: pu.url.clone(), status: "failed".into(), retry: None, duration_ms: Some(item_start.elapsed().as_millis() as u64), metrics: Some(SimpleMetrics::Product { fields: None, size_bytes: None, error: Some(format!("{:?}", err)) }), timestamp: Utc::now() });
                    }
                }
            }
        }

        result
    }
    /// Before-each-item hook (middleware slot): emit logs/metrics or modify context in future.
    async fn before_each_item_hook(
        _: &AppContext,
        _: &StageType,
        _: &StageItem,
    ) {
        // No-op by default. Reserved for cross-cutting concerns (logging/metrics/instrumentation).
    }

    /// After-each-item hook (middleware slot): observe result/error for telemetry.
    async fn after_each_item_hook(
        context: &AppContext,
        _stage_type: &StageType,
        _item: &StageItem,
        result: &Result<StageItemResult, StageError>,
        started_at: std::time::Instant,
    ) {
        // Best-effort, low-noise telemetry: keep a tiny rolling window per session and occasionally emit
        let latency_ms = started_at.elapsed().as_millis();
        let key = context.session_id.to_string();
        if let Ok(mut map) = METRICS_BY_SESSION.lock() {
            let entry = map.entry(key.clone()).or_default();
            entry.item_count = entry.item_count.saturating_add(1);
            entry.sum_latency_ms = entry.sum_latency_ms.saturating_add(latency_ms);
            match result {
                Ok(_) => entry.success = entry.success.saturating_add(1),
                Err(_) => entry.failure = entry.failure.saturating_add(1),
            }
            let now = Instant::now();
            let should_emit = match entry.last_emit {
                None => true,
                Some(prev) => now.duration_since(prev) >= Duration::from_millis(1000),
            };
            if should_emit && entry.item_count > 0 {
                let avg_ms = (entry.sum_latency_ms as f64) / (entry.item_count as f64);
                let throughput = if avg_ms > 0.0 { 1000.0 / avg_ms } else { 0.0 };
                // Compose high-level metrics snapshot; unknown fields left conservative
                let snapshot = crate::crawl_engine::actors::types::PerformanceMetrics {
                    memory_usage_mb: 0.0,
                    cpu_usage_percent: 0.0,
                    active_tasks_count: 0,
                    queued_tasks_count: 0,
                    avg_response_time_ms: avg_ms,
                    throughput_per_second: throughput,
                };
                // Emit as AppEvent::PerformanceMetrics (additive, consumed by UI)
                let _ = context.emit_event(AppEvent::PerformanceMetrics {
                    session_id: key,
                    metrics: snapshot,
                    timestamp: Utc::now(),
                });
                entry.last_emit = Some(now);
                // Keep window from growing unbounded
                entry.item_count = 0;
                entry.sum_latency_ms = 0;
                entry.success = 0;
                entry.failure = 0;
            }
        }
    }
    // Removed: deprecated constructors new() and new_with_oneshot()

    /// New constructor that takes explicit dependencies and a strategy factory.
    /// This supports proper DI and makes `StageActor` focused on orchestration only.
    #[must_use]
    pub fn new_with_deps(
        actor_id: String,
        batch_id: String,
        deps: StageDeps,
        strategy_factory: Arc<dyn StageLogicFactory + Send + Sync>,
    ) -> Self {
        Self {
            actor_id,
            batch_id,
            stage_id: None,
            stage_type: None,
            state: StageState::Idle,
            start_time: None,
            total_items: 0,
            completed_items: 0,
            success_count: 0,
            failure_count: 0,
            skipped_count: 0,
            item_results: Vec::new(),
            deps: Arc::new(deps),
            site_total_pages_hint: None,
            products_on_last_page_hint: None,
            strategy_factory,
        }
    }


    /// 사이트 페이지네이션 힌트 설정 (`StatusCheck` 결과를 상위에서 주입)
    pub fn set_site_pagination_hints(&mut self, total_pages: u32, products_on_last_page: u32) {
        self.site_total_pages_hint = Some(total_pages);
        self.products_on_last_page_hint = Some(products_on_last_page);
        info!(
            "🔧 Applied site pagination hints: total_pages={}, products_on_last_page={}",
            total_pages, products_on_last_page
        );
    }


    /// 크롤링 엔진 초기화 (임시 구현)
    /// 현재는 시뮬레이션 모드이므로 실제 엔진 초기화는 건너뛰기
    pub fn initialize_default_engines(&mut self) -> Result<(), StageError> {
    // No-op in production. Historical simulation path removed.
        Ok(())
    }

    /// 공개 스테이지 실행 메서드 (`BatchActor에서` 사용)
    ///
    /// # Arguments
    /// * `stage_type` - 실행할 스테이지 타입
    /// * `items` - 처리할 아이템 리스트
    /// * `concurrency_limit` - 동시성 제한
    /// * `timeout_secs` - 타임아웃 (초)
    /// * `context` - Actor 컨텍스트
    pub async fn execute_stage(
        &mut self,
        stage_type: StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
        timeout_secs: u64,
        context: &AppContext,
    ) -> Result<StageResult, StageError> {
        self.handle_execute_stage(stage_type, items, concurrency_limit, timeout_secs, context)
            .await?;

        Ok(StageResult {
            processed_items: self.completed_items,
            successful_items: self.success_count,
            failed_items: self.failure_count,
            duration_ms: self
                .start_time
                .map_or(0, |start| start.elapsed().as_millis() as u64),
            details: self.item_results.clone(),
        })
    }

    /// 스테이지 실행 처리
    ///
    /// # Arguments
    /// * `stage_type` - 실행할 스테이지 타입
    /// * `items` - 처리할 아이템 리스트
    /// * `concurrency_limit` - 동시성 제한
    /// * `timeout_secs` - 타임아웃 (초)
    /// * `context` - Actor 컨텍스트
    async fn handle_execute_stage(
        &mut self,
        stage_type: StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
        timeout_secs: u64,
        context: &AppContext,
    ) -> Result<(), StageError> {
        // 상태 검증
        if !matches!(self.state, StageState::Idle) {
            let sid = self
                .stage_id
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            return Err(StageError::GenericError {
                message: format!("Stage already processing: {}", sid),
            });
        }

        let stage_id = Uuid::new_v4().to_string();

        info!(
            "🎯 StageActor {} executing stage {:?} with {} items",
            self.actor_id,
            stage_type,
            items.len()
        );

        // 상태 초기화
        self.stage_id = Some(stage_id.clone());
        self.stage_type = Some(stage_type.clone());
        self.state = StageState::Starting;
        self.start_time = Some(Instant::now());
        self.total_items = items.len() as u32;
        self.completed_items = 0;
        self.success_count = 0;
        self.failure_count = 0;
        self.skipped_count = 0;
        self.item_results.clear();

        // 스테이지 시작 이벤트 발행
        let start_event = AppEvent::StageStarted {
            stage_type: stage_type.clone(),
            session_id: context.session_id.clone(),
            batch_id: Some(self.batch_id.clone()),
            items_count: items.len() as u32,
            timestamp: Utc::now(),
        };

        context
            .emit_event(start_event)
            .map_err(|e| StageError::GenericError {
                message: e.to_string(),
            })?;

        // 상태를 Processing으로 전환
        self.state = StageState::Processing;

        // 내부 타임아웃/취소 지원이 포함된 처리 실행 (tasks abort 포함)
        let processing_result = self
            .process_stage_items(
                stage_type.clone(),
                items,
                concurrency_limit,
                context,
                Duration::from_secs(timeout_secs),
            )
            .await;
        // TODO: Introduce a generic retry wrapper for stage-level retries and emit AppEvent::StageRetrying accordingly.

        match processing_result {
            Ok(stage_result) => {
                self.state = StageState::Completed;
                let completion_event = AppEvent::StageCompleted {
                    stage_type: stage_type.clone(),
                    session_id: context.session_id.clone(),
                    batch_id: Some(self.batch_id.clone()),
                    result: stage_result,
                    timestamp: Utc::now(),
                };
                context
                    .emit_event(completion_event)
                    .map_err(|e| StageError::GenericError {
                        message: e.to_string(),
                    })?;
                info!(
                    "✅ Stage {:?} completed successfully: {}/{} items processed",
                    stage_type, self.success_count, self.total_items
                );
                Ok(())
            }
            Err(StageError::TimeoutError { .. }) => {
                self.state = StageState::Timeout;
                let error = StageError::TimeoutError {
                    timeout_ms: timeout_secs * 1000,
                };
                let timeout_event = AppEvent::StageFailed {
                    stage_type: stage_type.clone(),
                    session_id: context.session_id.clone(),
                    batch_id: Some(self.batch_id.clone()),
                    error: format!("{:?}", error),
                    timestamp: Utc::now(),
                };
                context
                    .emit_event(timeout_event)
                    .map_err(|e| StageError::GenericError {
                        message: e.to_string(),
                    })?;
                Err(error)
            }
            Err(e) => {
                let error_msg = format!("{:?}", e);
                self.state = StageState::Failed {
                    error: error_msg.clone(),
                };
                let failure_event = AppEvent::StageFailed {
                    stage_type: stage_type.clone(),
                    session_id: context.session_id.clone(),
                    batch_id: Some(self.batch_id.clone()),
                    error: error_msg,
                    timestamp: Utc::now(),
                };
                context
                    .emit_event(failure_event)
                    .map_err(|er| StageError::GenericError {
                        message: er.to_string(),
                    })?;
                Err(e)
            }
        }
    }

    /// 스테이지 아이템들 처리
    ///
    /// # Arguments
    /// * `stage_type` - 스테이지 타입
    /// * `items` - 처리할 아이템들
    /// * `concurrency_limit` - 동시성 제한
    /// * `context` - Actor 컨텍스트
    async fn process_stage_items(
        &mut self,
        stage_type: StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
        _context: &AppContext,
        overall_timeout: Duration,
    ) -> Result<StageResult, StageError> {
        debug!(
            "Processing {} items for stage {:?}",
            items.len(),
            stage_type
        );

    // 의존성/설정 클론 (Arc 복사)
    let deps_arc = self.deps.clone();

        // 동시성 제어를 위한 세마포어
        let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency_limit as usize));
        // 전략/설정 및 의존성 복사
        let strategy_factory_clone = self.strategy_factory.clone();
        // 페이지네이션 힌트 복사 (Copy types; safe to move into tasks)
        let site_total_pages_hint = self.site_total_pages_hint;
        let products_on_last_page_hint = self.products_on_last_page_hint;
        // Duplicate policy는 deps에서 참조

    // 각 아이템을 병렬로 처리 (StageItemStarted를 먼저 emit하여 이벤트 순서 보장)
    let deadline = Instant::now() + overall_timeout;
    let mut join_set = tokio::task::JoinSet::new();
        let batch_id_owned = self.batch_id.clone();
        for item in items {
            let sem = semaphore.clone();
            let input = TaskInput {
                ctx: _context.clone(),
                stage_type: stage_type.clone(),
                item: item.clone(),
                session_id: _context.session_id.clone(),
                batch_id: Some(batch_id_owned.clone()),
                strategy_factory: strategy_factory_clone.clone(),
                deps: deps_arc.clone(),
                total_pages_hint: site_total_pages_hint,
                products_on_last_page_hint: products_on_last_page_hint,
            };
            join_set.spawn(Self::execute_single_item_task(sem, input));
        }

        // 모든 태스크 완료 대기 (전체 타임아웃 관리 및 잔여 task abort)
        let mut results = Vec::new();
        loop {
            let now = Instant::now();
            if now >= deadline {
                // 남은 작업들 중단
                while let Some(h) = join_set.join_next().await {
                    if let Ok(Err(e)) = h {
                        error!("Aborted after timeout; task error: {:?}", e);
                    }
                }
                return Err(StageError::TimeoutError { timeout_ms: overall_timeout.as_millis() as u64 });
            }
            let remaining = deadline.saturating_duration_since(now);
            match tokio::time::timeout(remaining, join_set.join_next()).await {
                Ok(Some(Ok(Ok(res)))) => results.push(res),
                Ok(Some(Ok(Err(e)))) => {
                    error!("Item processing failed: {:?}", e);
                    results.push(StageItemResult { item_id: "unknown".into(), item_type: StageItemType::Url { url_type: "unknown".into() }, success: false, error: Some(format!("{:?}", e)), duration_ms: 0, retry_count: 0, collected_data: None });
                }
                Ok(Some(Err(join_err))) => {
                    error!("Task join error: {}", join_err);
                    results.push(StageItemResult { item_id: "unknown".into(), item_type: StageItemType::Url { url_type: "unknown".into() }, success: false, error: Some(format!("Task join error: {}", join_err)), duration_ms: 0, retry_count: 0, collected_data: None });
                }
                Ok(None) => break, // all done
                Err(_elapsed) => {
                    // timed out waiting for next; loop checks deadline and exits with timeout handling above
                    continue;
                }
            }
        }

        // 결과 집계
        self.item_results = results;
        self.completed_items = self.item_results.len() as u32;
        self.success_count = self.item_results.iter().filter(|r| r.success).count() as u32;
        self.failure_count = self.item_results.iter().filter(|r| !r.success).count() as u32;

        let duration = self
            .start_time
            .map_or(Duration::ZERO, |start| start.elapsed());

        Ok(StageResult {
            processed_items: self.completed_items,
            successful_items: self.success_count,
            failed_items: self.failure_count,
            duration_ms: duration.as_millis() as u64,
            details: self.item_results.clone(),
        })
    }

    // Legacy per-item path removed; strategy-only execution is enforced in process_stage_items.

    // === 실제 서비스 기반 처리 함수들 (Critical Issue #1) ===

    // Removed unused real-execution helpers: execute_real_status_check, execute_real_list_page_processing,
    // execute_real_product_detail_processing, execute_real_data_validation. The actively used implementations
    // live under new_architecture/actors/stage_actor.rs.

    /// 실제 데이터베이스 저장 처리
    async fn execute_real_database_storage(
        item: &StageItem,
        product_repo: Arc<IntegratedProductRepository>,
        duplicate_policy: crate::crawl_engine::actors::types::DuplicatePersistencePolicy,
    ) -> Result<(u32, u32, u32), String> {
        // (inserted, updated, duplicates)
        match item {
            StageItem::ProductDetails(wrapper) => {
                info!(
                    "[PersistExec] handling ProductDetails count={} extraction_stats=attempted:{} success:{} failed:{}",
                    wrapper.products.len(),
                    wrapper.extraction_stats.attempted,
                    wrapper.extraction_stats.successful,
                    wrapper.extraction_stats.failed
                );
                // Debug a small sample of pagination coordinates to verify propagation
                for (i, d) in wrapper.products.iter().take(3).enumerate() {
                    debug!(
                        "[PersistExecSample] idx={} url={} page_id={:?} index_in_page={:?}",
                        i, d.url, d.page_id, d.index_in_page
                    );
                }
                let products = &wrapper.products;
                if products.is_empty() {
                    return Ok((0, 0, 0));
                }
                // Duplicate detection by URL
                let mut seen = std::collections::HashSet::new();
                let mut duplicates: Vec<String> = Vec::new();
                for d in products {
                    if !seen.insert(d.url.clone()) {
                        duplicates.push(d.url.clone());
                    }
                }
                if !duplicates.is_empty() {
                    warn!(
                        "[PersistExec] duplicate urls detected count={} urls={:?}",
                        duplicates.len(),
                        duplicates
                    );
                }
                let mut inserted = 0u32;
                let mut updated = 0u32;
                let mut duplicates_ct = 0u32;
                for (idx, detail) in products.iter().enumerate() {
                    let start = std::time::Instant::now();
                    debug!(
                        "[PersistExec] upsert detail idx={} url={} page_id={:?} index_in_page={:?}",
                        idx, detail.url, detail.page_id, detail.index_in_page
                    );
                    match product_repo.create_or_update_product_detail(detail).await {
                        Ok((was_updated, was_created)) => {
                            if was_created {
                                inserted += 1;
                            }
                            if was_updated {
                                updated += 1;
                            }
                            if !was_created && !was_updated {
                                // 중복(no-op) → 정책 적용
                                if matches!(duplicate_policy, crate::crawl_engine::actors::types::DuplicatePersistencePolicy::UpdateIdIndexOnly) {
                                    if let (Some(pid), Some(idx)) =
                                        (detail.page_id, detail.index_in_page)
                                    {
                                        if let Ok((prod_rows, det_rows)) = product_repo
                                            .force_update_position_by_url(&detail.url, pid, idx)
                                            .await
                                        {
                                            if det_rows > 0 || prod_rows > 0 {
                                                updated += 1;
                                                debug!(
                                                    "[PersistExecDetail] forced pos update idx={} url={} pid={} idx_in_page={} (prod_rows={}, det_rows={})",
                                                    idx, detail.url, pid, idx, prod_rows, det_rows
                                                );
                                                continue;
                                            }
                                        }
                                    }
                                }
                                duplicates_ct += 1;
                            }
                            debug!(
                                "[PersistExecDetail] idx={} url={} created={} updated={} elapsed_ms={}",
                                idx,
                                detail.url,
                                was_created,
                                was_updated,
                                start.elapsed().as_millis()
                            );
                        }
                        Err(e) => return Err(format!("Database save failed: {}", e)),
                    }
                }
                Ok((inserted, updated, duplicates_ct))
            }
            StageItem::ValidatedProducts(wrapper) => {
                info!(
                    "[PersistExec] handling ValidatedProducts count={}",
                    wrapper.products.len()
                );
                for (i, d) in wrapper.products.iter().take(3).enumerate() {
                    debug!(
                        "[PersistExecSample] validated idx={} url={} page_id={:?} index_in_page={:?}",
                        i, d.url, d.page_id, d.index_in_page
                    );
                }
                let products = &wrapper.products;
                if products.is_empty() {
                    return Ok((0, 0, 0));
                }
                let mut seen = std::collections::HashSet::new();
                let mut duplicates: Vec<String> = Vec::new();
                for d in products {
                    if !seen.insert(d.url.clone()) {
                        duplicates.push(d.url.clone());
                    }
                }
                if !duplicates.is_empty() {
                    warn!(
                        "[PersistExec] duplicate validated urls detected count={} urls={:?}",
                        duplicates.len(),
                        duplicates
                    );
                }
                let mut inserted = 0u32;
                let mut updated = 0u32;
                let mut duplicates_ct = 0u32;
                for (idx, detail) in products.iter().enumerate() {
                    let start = std::time::Instant::now();
                    debug!(
                        "[PersistExec] upsert validated detail idx={} url={} page_id={:?} index_in_page={:?}",
                        idx, detail.url, detail.page_id, detail.index_in_page
                    );
                    match product_repo.create_or_update_product_detail(detail).await {
                        Ok((was_updated, was_created)) => {
                            if was_created {
                                inserted += 1;
                            }
                            if was_updated {
                                updated += 1;
                            }
                            if !was_created && !was_updated {
                                if matches!(duplicate_policy, crate::crawl_engine::actors::types::DuplicatePersistencePolicy::UpdateIdIndexOnly) {
                                    if let (Some(pid), Some(idx)) =
                                        (detail.page_id, detail.index_in_page)
                                    {
                                        if let Ok((prod_rows, det_rows)) = product_repo
                                            .force_update_position_by_url(&detail.url, pid, idx)
                                            .await
                                        {
                                            if det_rows > 0 || prod_rows > 0 {
                                                updated += 1;
                                                debug!(
                                                    "[PersistExecDetail] forced pos update(validated) idx={} url={} pid={} idx_in_page={} (prod_rows={}, det_rows={})",
                                                    idx, detail.url, pid, idx, prod_rows, det_rows
                                                );
                                                continue;
                                            }
                                        }
                                    }
                                }
                                duplicates_ct += 1;
                            }
                            debug!(
                                "[PersistExecDetail] validated idx={} url={} created={} updated={} elapsed_ms={}",
                                idx,
                                detail.url,
                                was_created,
                                was_updated,
                                start.elapsed().as_millis()
                            );
                        }
                        Err(e) => return Err(format!("Database save failed: {}", e)),
                    }
                }
                Ok((inserted, updated, duplicates_ct))
            }
            _ => Ok((0, 0, 0)),
        }
    }

    // === 시뮬레이션 함수들 (기존) ===

    // (이전) 리스트 페이지 처리 시뮬레이션 함수는 미사용으로 제거됨

    /// 스테이지 정리
    fn cleanup_stage(&mut self) {
        self.stage_id = None;
        self.stage_type = None;
        self.state = StageState::Idle;
        self.start_time = None;
        self.total_items = 0;
        self.completed_items = 0;
        self.success_count = 0;
        self.failure_count = 0;
        self.skipped_count = 0;
        self.item_results.clear();
    }

    /// 진행 상황 계산
    ///
    /// # Returns
    /// * `f64` - 진행률 (0.0 ~ 1.0)
    fn calculate_progress(&self) -> f64 {
        if self.total_items == 0 {
            0.0
        } else {
            f64::from(self.completed_items) / f64::from(self.total_items)
        }
    }

    /// 성공률 계산
    ///
    /// # Returns
    /// * `f64` - 성공률 (0.0 ~ 1.0)
    fn calculate_success_rate(&self) -> f64 {
        if self.completed_items == 0 {
            0.0
        } else {
            f64::from(self.success_count) / f64::from(self.completed_items)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crawl_engine::channels::types as ch;
    use crate::crawl_engine::integrated_context::IntegratedContextFactory;
    use crate::crawl_engine::system_config::SystemConfig;
    use crate::crawl_engine::stages::DefaultStageLogicFactory;
    use std::sync::Arc;

    async fn memory_repo() -> Arc<IntegratedProductRepository> {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory pool");
        Arc::new(IntegratedProductRepository::new(pool))
    }

    #[tokio::test]
    async fn stage_actor_emits_validation_and_metrics_with_hermetic_context() {
        // Context and channels
        let config = Arc::new(SystemConfig::default());
        let factory = IntegratedContextFactory::new(config);
        let (context, channels) = factory
            .create_session_context("sess-itg".to_string())
            .expect("context");
    let event_rx = channels.event_tx.subscribe();

        // DI deps
        let app_config = crate::infrastructure::config::AppConfig::default();
        let http_client = Arc::new(app_config.create_http_client().expect("http"));
        let extractor = Arc::new(crate::infrastructure::MatterDataExtractor::new().expect("extractor"));
        let repo = memory_repo().await;

    let deps = StageDeps {
            http_client: Arc::clone(&http_client),
            data_extractor: Arc::clone(&extractor),
            product_repo: Arc::clone(&repo),
            app_config: app_config.clone(),
            duplicate_policy: crate::crawl_engine::actors::types::DuplicatePersistencePolicy::Skip,
        };

        let mut actor = StageActor::new_with_deps(
            "actor-itg".into(),
            "batch-itg".into(),
            deps,
            Arc::new(DefaultStageLogicFactory),
        );

        // Build a minimal valid ProductDetails payload for DataValidation
        let now = chrono::Utc::now();
        let pd1 = crate::domain::product::ProductDetail {
            url: "https://e/p1".into(), page_id: Some(1), index_in_page: Some(1), id: None,
            manufacturer: Some("A".into()), model: Some("M".into()), device_type: None, certificate_id: None, certification_date: None, software_version: None, hardware_version: None, vid: None, pid: None,
            family_sku: None, family_variant_sku: None, firmware_version: None, family_id: None, tis_trp_tested: None, specification_version: None, transport_interface: None,
            primary_device_type_id: None, application_categories: None, description: None, compliance_document_url: None, program_type: Some("Matter".into()), created_at: now, updated_at: now
        };
        let items = vec![ch::StageItem::ProductDetails(ch::ProductDetails {
            products: vec![pd1],
            source_urls: vec![],
            extraction_stats: ch::ExtractionStats { attempted: 1, successful: 1, failed: 0, empty_responses: 0 },
        })];

        // Spawn a task to collect a handful of events until StageCompleted received
        let mut rx = event_rx;
        let collector = tokio::spawn(async move {
            use crate::crawl_engine::actors::types::AppEvent;
            let mut got_stage_completed = false;
            let mut metrics_seen = false;
            let mut started_seen = false;
            let mut validation_completed_seen = false;
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline {
                if let Ok(ev) = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
                    match ev {
                        Ok(AppEvent::StageCompleted { .. }) => { got_stage_completed = true; },
                        Ok(AppEvent::PerformanceMetrics { .. }) => { metrics_seen = true; },
                        Ok(AppEvent::StageStarted { .. }) => { started_seen = true; },
                        Ok(AppEvent::ValidationCompleted { .. }) => { validation_completed_seen = true; },
                        _ => {}
                    }
                    if got_stage_completed && validation_completed_seen { break; }
                } else {
                    // timeout, continue loop until overall deadline
                }
            }
            (got_stage_completed, metrics_seen, started_seen, validation_completed_seen)
        });

        // Run the stage
        let res = actor
            .execute_stage(
                StageType::DataValidation,
                items,
                2,
                3, // seconds
                &context,
            )
            .await
            .expect("stage ok");
        assert!(res.successful_items >= 1);

        let (got_completed, metrics_seen, started_seen, validation_done) = collector.await.expect("collector join");
        assert!(started_seen, "StageStarted not seen");
        assert!(validation_done, "ValidationCompleted not seen");
        assert!(got_completed, "StageCompleted not seen");
        assert!(metrics_seen, "PerformanceMetrics not seen");
    }
}

// (기존 상세 Actor trait 구현은 파일 하단 원본 영역 유지)

#[async_trait::async_trait]
impl Actor for StageActor {
    type Command = ActorCommand;
    type Error = ActorError;

    fn actor_id(&self) -> &str {
        self.stage_id.as_deref().unwrap_or("unknown")
    }

    fn actor_type(&self) -> ActorType {
        ActorType::Stage
    }

    async fn run(
        &mut self,
        mut context: AppContext,
        mut command_rx: mpsc::Receiver<Self::Command>,
    ) -> Result<(), Self::Error> {
        info!("🎯 StageActor {} starting execution loop", self.actor_id);

        loop {
            tokio::select! {
                biased;
                // 취소 신호 확인 (우선 처리)
                _ = context.cancellation_token.changed() => {
                    if *context.cancellation_token.borrow() {
                        warn!("🚫 StageActor {} received cancellation signal", self.actor_id);
                        break;
                    }
                }
                // 명령 처리
                maybe_cmd = command_rx.recv() => {
                    if let Some(cmd) = maybe_cmd {
                        debug!("📨 StageActor {} received command: {:?}", self.actor_id, cmd);
                        match cmd {
                            ActorCommand::ExecuteStage { stage_type, items: _, concurrency_limit, timeout_secs } => {
                                // Temporary: this control path is not used in production; ignore payload type and run with empty items
                                let empty: Vec<StageItem> = Vec::new();
                                if let Err(e) = self.execute_stage(stage_type.clone(), empty, concurrency_limit, timeout_secs, &context).await {
                                    error!("Failed to execute stage: {:?}", e);
                                }
                            }
                            ActorCommand::Shutdown => {
                                info!("🛑 StageActor {} received shutdown command", self.actor_id);
                                break;
                            }
                            _ => {
                                debug!("StageActor {} ignoring non-stage command", self.actor_id);
                            }
                        }
                    } else {
                        warn!("📪 StageActor {} command channel closed", self.actor_id);
                        break;
                    }
                }
            }
        }

        info!("🏁 StageActor {} execution loop ended", self.actor_id);
        Ok(())
    }

    async fn health_check(&self) -> Result<ActorHealth, Self::Error> {
        let status = match &self.state {
            StageState::Idle => ActorStatus::Healthy,
            StageState::Processing => ActorStatus::Healthy,
            StageState::Completed => ActorStatus::Healthy,
            StageState::Timeout => ActorStatus::Degraded {
                reason: "Stage timed out".to_string(),
                since: Utc::now(),
            },
            StageState::Failed { error } => ActorStatus::Unhealthy {
                error: error.clone(),
                since: Utc::now(),
            },
            _ => ActorStatus::Degraded {
                reason: format!("In transition state: {:?}", self.state),
                since: Utc::now(),
            },
        };

        Ok(ActorHealth {
            actor_id: self.stage_id.clone().unwrap_or_default(),
            actor_type: ActorType::Stage,
            status,
            last_activity: Utc::now(),
            memory_usage_mb: 0, // TODO: 실제 메모리 사용량 계산
            active_tasks: if matches!(self.state, StageState::Processing) {
                self.total_items - self.completed_items
            } else {
                0
            },
            commands_processed: 0, // TODO: 실제 처리된 명령 수 계산
            errors_count: 0,       // TODO: 실제 에러 수 계산
            avg_command_processing_time_ms: 0.0, // TODO: 실제 평균 처리 시간 계산
            metadata: serde_json::json!({
                "stage_id": self.stage_id,
                "stage_type": self.stage_type,
                "state": format!("{:?}", self.state),
                "total_items": self.total_items,
                "completed_items": self.completed_items,
                "success_count": self.success_count,
                "failure_count": self.failure_count,
                "skipped_count": self.skipped_count,
                "progress": self.calculate_progress(),
                "success_rate": self.calculate_success_rate()
            })
            .to_string(),
        })
    }

    /// 데이터 품질 분석 실행
    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        info!("🔌 StageActor {} shutting down", self.actor_id);

        // 활성 스테이지가 있다면 정리
        if self.stage_id.is_some() {
            warn!("Cleaning up active stage during shutdown");
            self.cleanup_stage();
        }

        Ok(())
    }
}

impl StageActor {
    // ...existing code...
}
