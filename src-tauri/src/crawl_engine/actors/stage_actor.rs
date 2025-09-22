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

use crate::crawl_engine::actors::stage_batcher::{DefaultStageBatcher, StageBatcher};
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
    fn new(
        ctx: AppContext,
        session_id: String,
        batch_id: Option<String>,
        stage_type: StageType,
        item_id: String,
        item_type: StageItemType,
    ) -> Self {
        // 시작 이벤트 발행 (best-effort)
        StageActor::emit_best_effort(
            &ctx,
            AppEvent::StageItemStarted {
                session_id: session_id.clone(),
                batch_id: batch_id.clone(),
                stage_type: stage_type.clone(),
                item_id: item_id.clone(),
                item_type: item_type.clone(),
                timestamp: Utc::now(),
            },
        );
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
        let (success, error, duration_ms, retry_count, collected_count) =
            if let Some(o) = &self.outcome {
                (
                    o.success,
                    o.error.clone(),
                    o.duration_ms,
                    o.retry_count,
                    o.collected_count,
                )
            } else {
                (
                    false,
                    Some("unknown_error_or_early_drop".into()),
                    self.started_at.elapsed().as_millis() as u64,
                    0,
                    None,
                )
            };
        StageActor::emit_best_effort(
            &self.ctx,
            AppEvent::StageItemCompleted {
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
            },
        );
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

    // 배치 정책 헬퍼(액터 아님) - 기본은 no-op
    batcher: Arc<dyn StageBatcher>,
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
            Self::ProductUrl(url) => url.url.clone(),
            Self::ProductDetails(d) => format!("product_details_{}", d.products.len()),
            Self::ValidatedProducts(v) => format!("validated_products_{}", v.products.len()),
            Self::ValidationTarget(v) => format!("validation_target_{}", v.len()),
        }
    }
    fn item_type_enum(&self) -> StageItemType {
        match self {
            Self::Page(page) => StageItemType::Page { page_number: *page },
            Self::Url(_u) => StageItemType::Url {
                url_type: "generic".into(),
            },
            Self::Product(_p) => StageItemType::Url {
                url_type: "product".into(),
            },
            Self::ProductList(_l) => StageItemType::ProductUrls { urls: vec![] },
            Self::ProductUrls(list) => StageItemType::ProductUrls {
                urls: list.urls.iter().map(|u| u.url.clone()).collect(),
            },
            Self::ProductUrl(url) => StageItemType::ProductDetail {
                url: url.url.clone(),
                page_id: url.page_id,
                index_in_page: url.index_in_page,
            },
            Self::ProductDetails(_d) => StageItemType::Url {
                url_type: "product_details".into(),
            },
            Self::ValidatedProducts(_v) => StageItemType::Url {
                url_type: "validated_products".into(),
            },
            Self::ValidationTarget(_t) => StageItemType::Url {
                url_type: "validation_target".into(),
            },
        }
    }
}

impl StageActor {
    /// Helper to emit AppEvent with StageError mapping.
    #[inline]
    fn emit(&self, context: &AppContext, evt: AppEvent) -> Result<(), StageError> {
        context
            .emit_event(evt)
            .map(|_| ())
            .map_err(|e| StageError::GenericError {
                message: e.to_string(),
            })
    }
    /// Helper to emit AppEvent best-effort (ignore errors). Used in per-item paths and RAII.
    #[inline]
    fn emit_best_effort(context: &AppContext, evt: AppEvent) {
        let _ = context.emit_event(evt);
    }

    /// Clean up DataSaving guard entries for a specific session.
    /// This helps prevent stale guard entries from blocking future operations.
    pub fn cleanup_data_saving_guards_for_session(session_id: &str) {
        if let Ok(mut guard) = DATA_SAVING_RUN_GUARD.lock() {
            let keys_to_remove: Vec<String> = guard
                .iter()
                .filter(|key| key.starts_with(&format!("{}:", session_id)))
                .cloned()
                .collect();

            for key in keys_to_remove {
                guard.remove(&key);
                tracing::info!(target: "data_saving_diag", "[DataSaving] cleanup: removed stale guard key={key}");
            }
        }
    }

    /// ProductUrls 번들을 개별 ProductUrl 아이템으로 분할
    /// 실시간 개별 URL 진행상황을 위한 핵심 로직
    fn expand_product_urls_to_individual_items(items: Vec<StageItem>) -> Vec<StageItem> {
        let mut expanded = Vec::new();
        let mut bundle_count = 0;
        let mut total_urls = 0;
        
        for item in items {
            match item {
                StageItem::ProductUrls(product_urls) => {
                    bundle_count += 1;
                    let url_count = product_urls.urls.len();
                    total_urls += url_count;
                    info!(
                        "🔄 Expanding ProductUrls bundle {} with {} URLs", 
                        bundle_count, url_count
                    );
                    
                    // ProductUrls 번들의 각 URL을 개별 ProductUrl 아이템으로 변환
                    for product_url in product_urls.urls {
                        expanded.push(StageItem::ProductUrl(product_url));
                    }
                }
                // 다른 아이템 타입들은 그대로 유지
                other => expanded.push(other),
            }
        }
        
        if bundle_count > 0 {
            info!(
                "✅ Expansion complete: {} bundles → {} individual URLs", 
                bundle_count, total_urls
            );
        }
        
        expanded
    }

    /// Expose limited read-only access to product repo stats for external actors (e.g., SessionActor fallback emissions)
    /// Intentionally narrow to avoid leaking full deps struct.
    pub async fn try_product_detail_stats(
        &self,
    ) -> Result<(i64, Option<i32>, Option<i32>, Option<chrono::DateTime<chrono::Utc>>), ()> {
        self.deps
            .product_repo
            .get_product_detail_stats()
            .await
            .map_err(|_| ())
    }
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
                Self::emit_best_effort(
                    &ctx,
                    AppEvent::PageLifecycle {
                        session_id: session_id.clone(),
                        batch_id: batch_id.clone(),
                        page_number: *pn,
                        status: "fetch_started".into(),
                        metrics: None,
                        timestamp: Utc::now(),
                    },
                );
            }
            (StageType::ProductDetailCrawling, StageItem::ProductUrls(urls_wrapper)) => {
                // best-effort DB 체크로 스케줄 수 추정 후 이벤트
                let mut ct = 0u32;
                for u in &urls_wrapper.urls {
                    match deps.product_repo.get_product_detail_by_url(&u.url).await {
                        Ok(existing) => {
                            if existing.is_none() {
                                ct += 1;
                            }
                        }
                        Err(e) => {
                            warn!("[DetailFilter] DB check failed url={} err={}", u.url, e);
                            ct += 1;
                        }
                    }
                }
                if let Some(first) = urls_wrapper.urls.first() {
                    Self::emit_best_effort(
                        &ctx,
                        AppEvent::PageLifecycle {
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
                        },
                    );
                }
            }
            _ => {}
        }

        let item_start = Instant::now();

        // 재시도 & per-attempt 이벤트 wrapper
    async fn run_with_attempt_events(
            ctx: &AppContext,
            stage_type: &StageType,
            item: &StageItem,
            session_id: &str,
            batch_id: &Option<String>,
            strategy_factory: &Arc<dyn StageLogicFactory + Send + Sync>,
            deps: &Arc<StageDeps>,
            total_pages_hint: Option<u32>,
            products_on_last_page_hint: Option<u32>,
    ) -> Result<StageItemResult, StageError> {
            // Attempt-level lifecycle events always enabled (previously gated by MC_ATTEMPT_EVENTS)
            let attempt_events_enabled = true;
            // 기본 재시도 횟수 설정 (추후 설정에서 가져오도록 확장 가능)
            let max_attempts = 3u32;
            let per_attempt_timeout = Duration::from_secs(30);
            for attempt in 1..=max_attempts {
                let attempt_start = Instant::now();
                // attempt_started 이벤트
                if attempt_events_enabled && matches!(stage_type, StageType::ListPageCrawling | StageType::ProductDetailCrawling) {
                    let status = if matches!(stage_type, StageType::ListPageCrawling) {
                        "list_attempt_started"
                    } else {
                        "detail_attempt_started"
                    };
                    StageActor::emit_best_effort(
                        ctx,
                        AppEvent::ProductLifecycle {
                            session_id: session_id.to_string(),
                            batch_id: batch_id.clone(),
                            page_number: None,
                            product_ref: item.id_string(),
                            status: status.into(),
                            retry: Some(attempt.saturating_sub(1)),
                            duration_ms: None,
                            metrics: Some(SimpleMetrics::Generic {
                                key: "attempt_meta".into(),
                                value: format!(
                                    "attempt={} total={} per_timeout_s=30 start_ts_ms={}",
                                    attempt,
                                    max_attempts,
                                    chrono::Utc::now().timestamp_millis()
                                ),
                            }),
                            timestamp: Utc::now(),
                        },
                    );
                }
                let deps_in = crate::crawl_engine::stages::traits::Deps {
                    http: deps.http_client.clone(),
                    extractor: deps.data_extractor.clone(),
                    repo: deps.product_repo.clone(),
                    duplicate_policy: deps.duplicate_policy.clone(),
                    list_collector: None,
                    detail_collector: None,
                };
                // Legacy ProductDetail progress-emitter disabled (keyed events now authoritative)
                let progress_emitter = if matches!(stage_type, StageType::ProductDetailCrawling) {
                    None
                } else { None };
                let stage_input = crate::crawl_engine::stages::traits::StageInput {
                    stage_type: stage_type.clone(),
                    item: item.clone(),
                    config: deps.app_config.clone(),
                    deps: deps_in,
                    total_pages_hint,
                    products_on_last_page_hint,
                    session_id: session_id.to_string(),
                    batch_id: batch_id.clone(),
                    progress_emitter,
                    product_detail_event_emitter: if matches!(stage_type, StageType::ProductDetailCrawling) {
                        let ctx_clone = ctx.clone();
                        Some(Arc::new(move |evt: AppEvent| {
                            StageActor::emit_best_effort(&ctx_clone, evt);
                        }))
                    } else { None },
                };
                let logic_arc = if let Some(l) = strategy_factory.logic_for(stage_type) { l } else { return Err(StageError::GenericError { message: format!("No strategy registered for stage {:?}", stage_type) }); };
                let fut_exec = logic_arc.execute(stage_input);
                match tokio::time::timeout(per_attempt_timeout, fut_exec).await {
                    Ok(Ok(out)) => {
                        // attempt_succeeded
                        if attempt_events_enabled && matches!(stage_type, StageType::ListPageCrawling | StageType::ProductDetailCrawling) {
                            let status = if matches!(stage_type, StageType::ListPageCrawling) {
                                "list_attempt_succeeded"
                            } else { "detail_attempt_succeeded" };
                            let attempts_used = attempt; // 1-based
                            StageActor::emit_best_effort(
                                ctx,
                                AppEvent::ProductLifecycle {
                                    session_id: session_id.to_string(),
                                    batch_id: batch_id.clone(),
                                    page_number: None,
                                    product_ref: item.id_string(),
                                    status: status.into(),
                                    retry: Some(attempt.saturating_sub(1)),
                                    duration_ms: Some(attempt_start.elapsed().as_millis() as u64),
                                    metrics: Some(SimpleMetrics::Generic { key: "attempts_used".into(), value: attempts_used.to_string() }),
                                    timestamp: Utc::now(),
                                },
                            );
                        }
                        return Ok(out.result);
                    }
                    Ok(Err(e)) => {
                        // attempt_failed
                        if attempt_events_enabled && matches!(stage_type, StageType::ListPageCrawling | StageType::ProductDetailCrawling) {
                            let status = if matches!(stage_type, StageType::ListPageCrawling) {
                                "list_attempt_failed"
                            } else { "detail_attempt_failed" };
                            StageActor::emit_best_effort(
                                ctx,
                                AppEvent::ProductLifecycle {
                                    session_id: session_id.to_string(),
                                    batch_id: batch_id.clone(),
                                    page_number: None,
                                    product_ref: item.id_string(),
                                    status: status.into(),
                                    retry: Some(attempt.saturating_sub(1)),
                                    duration_ms: Some(attempt_start.elapsed().as_millis() as u64),
                                    metrics: Some(SimpleMetrics::Generic { key: "error".into(), value: format!("{}", e) }),
                                    timestamp: Utc::now(),
                                },
                            );
                        }
                        if attempt == max_attempts {
                            return Err(StageError::GenericError { message: format!("Strategy error after {} attempts: {}", attempt, e) });
                        } else {
                            // retry 이벤트
                            if attempt_events_enabled && matches!(stage_type, StageType::ListPageCrawling | StageType::ProductDetailCrawling) {
                                let status = if matches!(stage_type, StageType::ListPageCrawling) { "list_attempt_retry" } else { "detail_attempt_retry" };
                                StageActor::emit_best_effort(
                                    ctx,
                                    AppEvent::ProductLifecycle {
                                        session_id: session_id.to_string(),
                                        batch_id: batch_id.clone(),
                                        page_number: None,
                                        product_ref: item.id_string(),
                                        status: status.into(),
                                        retry: Some(attempt),
                                        duration_ms: None,
                                        metrics: Some(SimpleMetrics::Generic { key: "reason".into(), value: "error".into() }),
                                        timestamp: Utc::now(),
                                    },
                                );
                            }
                            continue;
                        }
                    }
                    Err(_timeout) => {
                        // attempt timeout
                        if attempt_events_enabled && matches!(stage_type, StageType::ListPageCrawling | StageType::ProductDetailCrawling) {
                            let status = if matches!(stage_type, StageType::ListPageCrawling) { "list_attempt_timeout" } else { "detail_attempt_timeout" };
                            StageActor::emit_best_effort(
                                ctx,
                                AppEvent::ProductLifecycle {
                                    session_id: session_id.to_string(),
                                    batch_id: batch_id.clone(),
                                    page_number: None,
                                    product_ref: item.id_string(),
                                    status: status.into(),
                                    retry: Some(attempt.saturating_sub(1)),
                                    duration_ms: Some(per_attempt_timeout.as_millis() as u64),
                                    metrics: Some(SimpleMetrics::Generic { key: "reason".into(), value: "timeout".into() }),
                                    timestamp: Utc::now(),
                                },
                            );
                        }
                        if attempt == max_attempts {
                            return Err(StageError::TimeoutError { timeout_ms: per_attempt_timeout.as_millis() as u64 });
                        } else {
                            if attempt_events_enabled && matches!(stage_type, StageType::ListPageCrawling | StageType::ProductDetailCrawling) {
                                let retry_status = if matches!(stage_type, StageType::ListPageCrawling) { "list_attempt_retry" } else { "detail_attempt_retry" };
                                StageActor::emit_best_effort(
                                    ctx,
                                    AppEvent::ProductLifecycle {
                                        session_id: session_id.to_string(),
                                        batch_id: batch_id.clone(),
                                        page_number: None,
                                        product_ref: item.id_string(),
                                        status: retry_status.into(),
                                        retry: Some(attempt),
                                        duration_ms: None,
                                        metrics: Some(SimpleMetrics::Generic { key: "reason".into(), value: "timeout".into() }),
                                        timestamp: Utc::now(),
                                    },
                                );
                            }
                            continue;
                        }
                    }
                }
            }
            // 이론상 도달하지 않음
            Err(StageError::GenericError { message: "attempt loop exited unexpectedly".into() })
        }

    let result: Result<StageItemResult, StageError> = if matches!(stage_type, StageType::ListPageCrawling | StageType::ProductDetailCrawling) {
            run_with_attempt_events(&ctx, &stage_type, &item, &session_id, &batch_id, &strategy_factory, &deps, total_pages_hint, products_on_last_page_hint).await
        } else if let Some(logic) = strategy_factory.logic_for(&stage_type) {
            let deps_in = crate::crawl_engine::stages::traits::Deps {
                http: deps.http_client.clone(),
                extractor: deps.data_extractor.clone(),
                repo: deps.product_repo.clone(),
                duplicate_policy: deps.duplicate_policy.clone(),
                list_collector: None,
                detail_collector: None,
            };
            let stage_input = crate::crawl_engine::stages::traits::StageInput {
                stage_type: stage_type.clone(),
                item: item.clone(),
                config: deps.app_config.clone(),
                deps: deps_in,
                total_pages_hint,
                products_on_last_page_hint,
                session_id: session_id.clone(),
                batch_id: batch_id.clone(),
                progress_emitter: None,
                product_detail_event_emitter: None,
            };
            match logic.execute(stage_input).await {
                Ok(crate::crawl_engine::stages::traits::StageOutput { result }) => Ok(result),
                Err(e) => Err(StageError::GenericError { message: format!("Strategy error: {}", e) }),
            }
        } else {
            Err(StageError::GenericError { message: format!("No strategy registered for stage {:?}", stage_type) })
        };

        // 미들웨어 사후 훅
        StageActor::after_each_item_hook(&ctx, &stage_type, &item, &result, item_start).await;

        // 결과 기반 이벤트 및 퍼시스턴스
        match &result {
            Ok(r) => {
                // Validation 집계 이벤트
                if matches!(stage_type, StageType::DataValidation) {
                    let (products_found, products_checked, divergences, anomalies) = {
                        use crate::crawl_engine::actors::types::StageResultData as SRD;
                        match &r.collected_data {
                            Some(SRD::ValidationResult { validated_count, .. }) => {
                                let found = *validated_count;
                                (found, u64::from(found), 0, 0)
                            }
                            Some(SRD::ProductDetails { details, .. }) => {
                                let found = details.len() as u32;
                                // Optional: light analysis
                                let report = crate::crawl_engine::services::data_quality_analyzer::DataQualityAnalyzer::new()
                                    .analyze_product_quality(&details)
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
                                } else {
                                    (0, 0)
                                };
                                (found, u64::from(found), div_ct, anom_ct)
                            }
                            _ => (0, 0, 0, 0),
                        }
                    };
                    Self::emit_best_effort(
                        &ctx,
                        AppEvent::ValidationStarted {
                            session_id: session_id.clone(),
                            scan_pages: 1,
                            total_pages_site: None,
                            timestamp: Utc::now(),
                        },
                    );
                    Self::emit_best_effort(
                        &ctx,
                        AppEvent::ValidationPageScanned {
                            session_id: session_id.clone(),
                            physical_page: 0,
                            products_found,
                            assigned_start_offset: 0,
                            assigned_end_offset: u64::from(products_found.saturating_sub(1)),
                            timestamp: Utc::now(),
                        },
                    );
                    Self::emit_best_effort(
                        &ctx,
                        AppEvent::ValidationCompleted {
                            session_id: session_id.clone(),
                            pages_scanned: 1,
                            products_checked,
                            divergences,
                            anomalies,
                            duration_ms: item_start.elapsed().as_millis() as u64,
                            timestamp: Utc::now(),
                        },
                    );
                }

                // ProductDetail 크롤링 그룹 완료 이벤트
                if matches!(stage_type, StageType::ProductDetailCrawling) {
                    if let StageItem::ProductUrls(ref urls) = lifecycle_item {
                        let page_hint = urls.urls.first().map_or(0u32, |u| u.page_id as u32);
                        let total = urls.urls.len() as u32;
                        let duration_ms = item_start.elapsed().as_millis() as u64;
                        Self::emit_best_effort(
                            &ctx,
                            AppEvent::ProductLifecycleGroup {
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
                                partial: None,
                                done: None,
                                timestamp: Utc::now(),
                            },
                        );
                    }
                }

                // DataSaving 퍼시스턴스
                if matches!(stage_type, StageType::DataSaving) {
                    tracing::info!(target: "data_saving_diag", "[DataSaving] Enter StageType::DataSaving for session={session_id} batch={:?}", batch_id);
                    let mut persist_events_count: u32 = 0; // count emitted persist related events (ProductLifecycle / Group)
                    let is_persist_target = matches!(lifecycle_item, StageItem::ProductDetails(_))
                        || matches!(lifecycle_item, StageItem::ValidatedProducts(_));
                    if is_persist_target {
                        tracing::info!(target: "data_saving_diag", "[DataSaving] lifecycle_item qualifies as persist target (ProductDetails|ValidatedProducts)");
                        let guard_key = format!(
                            "{}:{}:data_saving",
                            session_id,
                            batch_id.clone().unwrap_or_else(|| "none".into())
                        );
                        // Persist phase ProductLifecycleGroup semantics:
                        // - We emit exactly one final snapshot per persist attempt path (success, skip, empty, error, fallback, guard skip, unexpected)
                        // - partial is always None for persist-phase snapshots (no incremental streaming here yet)
                        // - done is ALWAYS Some(group_size) so UI can uniformly treat persist like other phases without heuristics
                        // - failed field currently carries "unchanged" count for success path so UI can derive true failures separately
                        // Pre-compute attempted_count early so guard skip path can also emit persist snapshot
                        let attempted_count = match &lifecycle_item {
                            StageItem::ValidatedProducts(v) => v.products.len() as u32,
                            StageItem::ProductDetails(d) => d.products.len() as u32,
                            _ => 0,
                        };
                        // Acquire + evaluate guard under a short scope so we don't hold the lock across awaits
                        let guard_hit = {
                            if let Ok(mut guard) = DATA_SAVING_RUN_GUARD.lock() {
                                if guard.contains(&guard_key) {
                                    true
                                } else {
                                    tracing::info!(target: "data_saving_diag", "[DataSaving] guard insert key={guard_key}");
                                    guard.insert(guard_key.clone());
                                    false
                                }
                            } else {
                                false
                            }
                        };

                        if guard_hit {
                            tracing::warn!(target: "data_saving_diag", "[DataSaving] guard hit; skipping duplicate save for key={guard_key}");
                            // Emit guard-skip persist snapshot so Stage 5 UI still updates
                            Self::emit_best_effort(
                                &ctx,
                                AppEvent::ProductLifecycle {
                                    session_id: session_id.clone(),
                                    batch_id: batch_id.clone(),
                                    page_number: None,
                                    product_ref: "_batch_persist".into(),
                                    status: "persist_guard_skipped".into(),
                                    retry: None,
                                    duration_ms: Some(item_start.elapsed().as_millis() as u64),
                                    metrics: Some(SimpleMetrics::Generic {
                                        key: "persist_result".into(),
                                        value: format!(
                                            "attempted={},inserted=0,updated=0,duplicates=0,unchanged={}",
                                            attempted_count, attempted_count
                                        ),
                                    }),
                                    timestamp: Utc::now(),
                                },
                            );
                            Self::emit_best_effort(
                                &ctx,
                                AppEvent::ProductLifecycleGroup {
                                    session_id: session_id.clone(),
                                    batch_id: batch_id.clone(),
                                    page_number: None,
                                    group_size: attempted_count,
                                    started: attempted_count,
                                    succeeded: 0,
                                    failed: attempted_count,
                                    duplicates: 0,
                                    duration_ms: item_start.elapsed().as_millis() as u64,
                                    phase: "persist".into(),
                                    partial: None,
                                    done: Some(attempted_count),
                                    timestamp: Utc::now(),
                                },
                            );
                            // Database stats snapshot (unchanged)
                            if let Ok((cnt, minp, maxp, _)) = deps.product_repo.get_product_detail_stats().await {
                                Self::emit_best_effort(
                                    &ctx,
                                    AppEvent::DatabaseStats {
                                        session_id: session_id.clone(),
                                        batch_id: batch_id.clone(),
                                        total_product_details: cnt,
                                        min_page: minp,
                                        max_page: maxp,
                                        note: Some("post_persist:guard_skip".into()),
                                        timestamp: Utc::now(),
                                    },
                                );
                            }
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
                        // We'll log the number of persist events emitted at the end of the DataSaving block (after all possible early returns except guard/empty cases)
                        // attempted_count already computed above
                        tracing::info!(target: "data_saving_diag", attempted_count, "[DataSaving] computed attempted_count");
                        let skip_save = std::env::var("MC_SKIP_DB_SAVE")
                            .ok()
                            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
                        tracing::info!(target: "data_saving_diag", skip_save, "[DataSaving] skip_save flag");
                        if skip_save {
                            tracing::warn!(target: "data_saving_diag", "[DataSaving] MC_SKIP_DB_SAVE active - emitting nosave events only");
                            Self::emit_best_effort(
                                &ctx,
                                AppEvent::ProductLifecycle {
                                    session_id: session_id.clone(),
                                    batch_id: batch_id.clone(),
                                    page_number: None,
                                    product_ref: "_batch_persist".into(),
                                    status: "persist_skipped".into(),
                                    retry: None,
                                    duration_ms: None,
                                    metrics: Some(SimpleMetrics::Generic {
                                        key: "reason".into(),
                                        value: "MC_SKIP_DB_SAVE".into(),
                                    }),
                                    timestamp: Utc::now(),
                                },
                            );
                            // Emit grouped snapshot for Stage 5 panel even when skipping DB save
                            Self::emit_best_effort(
                                &ctx,
                                AppEvent::ProductLifecycleGroup {
                                    session_id: session_id.clone(),
                                    batch_id: batch_id.clone(),
                                    page_number: None,
                                    group_size: attempted_count,
                                    started: attempted_count,
                                    succeeded: 0,
                                    failed: 0,
                                    duplicates: 0,
                                    duration_ms: item_start.elapsed().as_millis() as u64,
                                    phase: "persist".into(),
                                    partial: None,
                                    done: Some(attempted_count),
                                    timestamp: Utc::now(),
                                },
                            );
                            // Also emit a DatabaseStats snapshot (unchanged totals) so Stage 4 flashes
                            if let Ok((cnt, minp, maxp, _)) =
                                deps.product_repo.get_product_detail_stats().await
                            {
                                Self::emit_best_effort(
                                    &ctx,
                                    AppEvent::DatabaseStats {
                                        session_id: session_id.clone(),
                                        batch_id: batch_id.clone(),
                                        total_product_details: cnt,
                                        min_page: minp,
                                        max_page: maxp,
                                        note: Some("post_persist:nosave".into()),
                                        timestamp: Utc::now(),
                                    },
                                );
                            }
                        } else {
                            Self::emit_best_effort(
                                &ctx,
                                AppEvent::ProductLifecycle {
                                    session_id: session_id.clone(),
                                    batch_id: batch_id.clone(),
                                    page_number: None,
                                    product_ref: "_batch_persist".into(),
                                    status: "persist_started".into(),
                                    retry: None,
                                    duration_ms: None,
                                    metrics: Some(SimpleMetrics::Generic {
                                        key: "attempted_count".into(),
                                        value: attempted_count.to_string(),
                                    }),
                                    timestamp: Utc::now(),
                                },
                            );
                            if attempted_count == 0 {
                                tracing::info!(target: "data_saving_diag", "[DataSaving] attempted_count=0 => persist_empty path");
                                Self::emit_best_effort(
                                    &ctx,
                                    AppEvent::ProductLifecycle {
                                        session_id: session_id.clone(),
                                        batch_id: batch_id.clone(),
                                        page_number: None,
                                        product_ref: "_batch_persist".into(),
                                        status: "persist_empty".into(),
                                        retry: None,
                                        duration_ms: Some(0),
                                        metrics: Some(SimpleMetrics::Generic {
                                            key: "persist_result".into(),
                                            value: "attempted=0".into(),
                                        }),
                                        timestamp: Utc::now(),
                                    },
                                );
                                // Emit grouped snapshot (empty)
                                Self::emit_best_effort(
                                    &ctx,
                                    AppEvent::ProductLifecycleGroup {
                                        session_id: session_id.clone(),
                                        batch_id: batch_id.clone(),
                                        page_number: None,
                                        group_size: 0,
                                        started: 0,
                                        succeeded: 0,
                                        failed: 0,
                                        duplicates: 0,
                                        duration_ms: 0,
                                        phase: "persist".into(),
                                        partial: None,
                                        done: Some(0),
                                        timestamp: Utc::now(),
                                    },
                                );
                                // Emit DB stats snapshot as well
                                if let Ok((cnt, minp, maxp, _)) =
                                    deps.product_repo.get_product_detail_stats().await
                                {
                                    tracing::info!(target: "data_saving_diag", cnt, minp, maxp, "[DataSaving] post_persist:empty stats fetched");
                                    Self::emit_best_effort(
                                        &ctx,
                                        AppEvent::DatabaseStats {
                                            session_id: session_id.clone(),
                                            batch_id: batch_id.clone(),
                                            total_product_details: cnt,
                                            min_page: minp,
                                            max_page: maxp,
                                            note: Some("post_persist:empty".into()),
                                            timestamp: Utc::now(),
                                        },
                                    );
                                }
                                return Ok(StageItemResult {
                                    item_id: "data_saving_empty".into(),
                                    item_type: StageItemType::Url {
                                        url_type: "data_saving".into(),
                                    },
                                    success: true,
                                    error: None,
                                    duration_ms: item_start.elapsed().as_millis() as u64,
                                    retry_count: 0,
                                    collected_data: None,
                                });
                            }
                            let persist_start = Instant::now();
                            match Self::execute_real_database_storage(
                                &lifecycle_item,
                                deps.product_repo.clone(),
                                deps.duplicate_policy.clone(),
                            )
                            .await
                            {
                                Ok((inserted, updated, duplicates_ct)) => {
                                    tracing::info!(target: "data_saving_diag", inserted, updated, duplicates_ct, attempted = attempted_count, "[DataSaving] storage result success");
                                    let attempted = attempted_count;
                                    let consumed = inserted + updated + duplicates_ct;
                                    let unchanged = attempted.saturating_sub(consumed);
                                    let status = if inserted > 0 && updated == 0 {
                                        "persist_inserted"
                                    } else if updated > 0 && inserted == 0 {
                                        "persist_updated"
                                    } else if inserted == 0 && updated == 0 {
                                        if duplicates_ct == attempted {
                                            "persist_noop_all_duplicate"
                                        } else {
                                            "persist_noop"
                                        }
                                    } else {
                                        "persist_mixed"
                                    };
                                    let metrics = SimpleMetrics::Generic {
                                        key: "persist_result".into(),
                                        value: format!(
                                            "attempted={},inserted={},updated={},duplicates={},unchanged={}",
                                            attempted, inserted, updated, duplicates_ct, unchanged
                                        ),
                                    };
                                    Self::emit_best_effort(
                                        &ctx,
                                        AppEvent::ProductLifecycle {
                                            session_id: session_id.clone(),
                                            batch_id: batch_id.clone(),
                                            page_number: None,
                                            product_ref: "_batch_persist".into(),
                                            status: status.into(),
                                            retry: None,
                                            duration_ms: Some(
                                                persist_start.elapsed().as_millis() as u64
                                            ),
                                            metrics: Some(metrics),
                                            timestamp: Utc::now(),
                                        },
                                    );
                                    // Emit grouped snapshot for UI Stage 5 panel
                                    Self::emit_best_effort(
                                        &ctx,
                                        AppEvent::ProductLifecycleGroup {
                                            session_id: session_id.clone(),
                                            batch_id: batch_id.clone(),
                                            page_number: None,
                                            group_size: attempted,
                                            started: attempted,
                                            succeeded: inserted + updated,
                                            failed: unchanged, // "true" failures are derived in UI; provide unchanged as baseline here
                                            duplicates: duplicates_ct,
                                            duration_ms: persist_start.elapsed().as_millis() as u64,
                                            phase: "persist".into(),
                                            partial: None,
                                            done: Some(attempted),
                                            timestamp: Utc::now(),
                                        },
                                    );
                                    // Emit DatabaseStats snapshot to drive Stage 4 panel
                                    if let Ok((cnt, minp, maxp, _)) =
                                        deps.product_repo.get_product_detail_stats().await
                                    {
                                        tracing::info!(target: "data_saving_diag", cnt, minp, maxp, "[DataSaving] post_persist stats fetched");
                                        Self::emit_best_effort(
                                            &ctx,
                                            AppEvent::DatabaseStats {
                                                session_id: session_id.clone(),
                                                batch_id: batch_id.clone(),
                                                total_product_details: cnt,
                                                min_page: minp,
                                                max_page: maxp,
                                                note: Some("post_persist".into()),
                                                timestamp: Utc::now(),
                                            },
                                        );
                                    }
                                    persist_events_count += 3; // ProductLifecycle + Group + DB stats
                                }
                                Err(e) => {
                                    tracing::error!(target: "data_saving_diag", error=?e, "[DataSaving] storage result error");
                                    Self::emit_best_effort(
                                        &ctx,
                                        AppEvent::ProductLifecycle {
                                            session_id: session_id.clone(),
                                            batch_id: batch_id.clone(),
                                            page_number: None,
                                            product_ref: "_batch_persist".into(),
                                            status: "persist_failed".into(),
                                            retry: None,
                                            duration_ms: Some(
                                                persist_start.elapsed().as_millis() as u64
                                            ),
                                            metrics: Some(SimpleMetrics::Generic {
                                                key: "error".into(),
                                                value: e,
                                            }),
                                            timestamp: Utc::now(),
                                        },
                                    );
                                    // Emit grouped failure snapshot and DB stats (unchanged)
                                    Self::emit_best_effort(
                                        &ctx,
                                        AppEvent::ProductLifecycleGroup {
                                            session_id: session_id.clone(),
                                            batch_id: batch_id.clone(),
                                            page_number: None,
                                            group_size: attempted_count,
                                            started: attempted_count,
                                            succeeded: 0,
                                            failed: attempted_count,
                                            duplicates: 0,
                                            duration_ms: persist_start.elapsed().as_millis() as u64,
                                            phase: "persist".into(),
                                            partial: None,
                                            done: Some(attempted_count),
                                            timestamp: Utc::now(),
                                        },
                                    );
                                    if let Ok((cnt, minp, maxp, _)) =
                                        deps.product_repo.get_product_detail_stats().await
                                    {
                                        tracing::info!(target: "data_saving_diag", cnt, minp, maxp, "[DataSaving] post_persist:error stats fetched");
                                        Self::emit_best_effort(
                                            &ctx,
                                            AppEvent::DatabaseStats {
                                                session_id: session_id.clone(),
                                                batch_id: batch_id.clone(),
                                                total_product_details: cnt,
                                                min_page: minp,
                                                max_page: maxp,
                                                note: Some("post_persist:error".into()),
                                                timestamp: Utc::now(),
                                            },
                                        );
                                    }
                                    persist_events_count += 3; // ProductLifecycle + Group + DB stats
                                }
                            }
                            // Fallback: if nothing was emitted (unexpected early exit path), emit minimal failure snapshot
                            if persist_events_count == 0 {
                                tracing::warn!(target: "data_saving_diag", "[DataSaving] fallback trigger: no persist events emitted; emitting minimal snapshot");
                                let attempted_count = match &lifecycle_item {
                                    StageItem::ValidatedProducts(v) => v.products.len() as u32,
                                    StageItem::ProductDetails(d) => d.products.len() as u32,
                                    _ => 0,
                                };
                                Self::emit_best_effort(
                                    &ctx,
                                    AppEvent::ProductLifecycle {
                                        session_id: session_id.clone(),
                                        batch_id: batch_id.clone(),
                                        page_number: None,
                                        product_ref: "_batch_persist".into(),
                                        status: "persist_failed_fallback".into(),
                                        retry: None,
                                        duration_ms: Some(item_start.elapsed().as_millis() as u64),
                                        metrics: Some(SimpleMetrics::Generic {
                                            key: "persist_result".into(),
                                            value: format!("attempted={},inserted=0,updated=0,duplicates=0,unchanged={}", attempted_count, attempted_count),
                                        }),
                                        timestamp: Utc::now(),
                                    },
                                );
                                Self::emit_best_effort(
                                    &ctx,
                                    AppEvent::ProductLifecycleGroup {
                                        session_id: session_id.clone(),
                                        batch_id: batch_id.clone(),
                                        page_number: None,
                                        group_size: attempted_count,
                                        started: attempted_count,
                                        succeeded: 0,
                                        failed: attempted_count,
                                        duplicates: 0,
                                        duration_ms: item_start.elapsed().as_millis() as u64,
                                        phase: "persist".into(),
                                        partial: None,
                                        done: Some(attempted_count),
                                        timestamp: Utc::now(),
                                    },
                                );
                                if let Ok((cnt, minp, maxp, _)) = deps.product_repo.get_product_detail_stats().await {
                                    Self::emit_best_effort(
                                        &ctx,
                                        AppEvent::DatabaseStats {
                                            session_id: session_id.clone(),
                                            batch_id: batch_id.clone(),
                                            total_product_details: cnt,
                                            min_page: minp,
                                            max_page: maxp,
                                            note: Some("post_persist:fallback".into()),
                                            timestamp: Utc::now(),
                                        },
                                    );
                                }
                                persist_events_count += 3; // ProductLifecycle + Group + DB stats
                            }
                            tracing::debug!(target: "data_saving_diag", persist_events_count, "[DataSaving] total persist-related events emitted in block");
                        }
                    } else {
                        // Non-persist target variant encountered (unexpected) – emit minimal snapshot so UI won't stay blank
                        tracing::warn!(target: "data_saving_diag", "[DataSaving] non-persist StageItem variant encountered; emitting minimal persist_unexpected snapshot");
                        Self::emit_best_effort(
                            &ctx,
                            AppEvent::ProductLifecycle {
                                session_id: session_id.clone(),
                                batch_id: batch_id.clone(),
                                page_number: None,
                                product_ref: "_batch_persist".into(),
                                status: "persist_unexpected".into(),
                                retry: None,
                                duration_ms: Some(item_start.elapsed().as_millis() as u64),
                                metrics: Some(SimpleMetrics::Generic {
                                    key: "persist_result".into(),
                                    value: "attempted=0,inserted=0,updated=0,duplicates=0,unchanged=0".into(),
                                }),
                                timestamp: Utc::now(),
                            },
                        );
                        Self::emit_best_effort(
                            &ctx,
                            AppEvent::ProductLifecycleGroup {
                                session_id: session_id.clone(),
                                batch_id: batch_id.clone(),
                                page_number: None,
                                group_size: 0,
                                started: 0,
                                succeeded: 0,
                                failed: 0,
                                duplicates: 0,
                                duration_ms: item_start.elapsed().as_millis() as u64,
                                phase: "persist".into(),
                                partial: None,
                                done: Some(0),
                                timestamp: Utc::now(),
                            },
                        );
                    }
                }

                // Clean up DataSaving guard after successful completion
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
                            if guard.remove(&guard_key) {
                                tracing::info!(target: "data_saving_diag", "[DataSaving] guard cleanup successful for key={guard_key}");
                            } else {
                                tracing::warn!(target: "data_saving_diag", "[DataSaving] guard cleanup: key not found key={guard_key}");
                            }
                        }
                    }
                }

                // Estimate collected count from typed result data for telemetry
                let collected_count = {
                    use crate::crawl_engine::actors::types::StageResultData as SRD;
                    match &r.collected_data {
                        Some(SRD::ProductUrls { urls, .. }) => Some(urls.len() as u32),
                        Some(SRD::ProductDetails { details, .. }) => Some(details.len() as u32),
                        Some(SRD::ValidationResult { validated_count, .. }) => Some(*validated_count),
                        Some(SRD::SavingResult { saved_count, .. }) => Some(*saved_count),
                        Some(SRD::StatusCheck { .. }) => Some(1),
                        Some(SRD::QualityAnalysis { total_analyzed, .. }) => Some(*total_analyzed),
                        Some(SRD::Empty) => Some(0),
                        None => None,
                    }
                };
                guard.record_ok(r.retry_count, collected_count);
            }
            Err(err) => {
                guard.record_err(format!("{:?}", err));
                
                // Clean up DataSaving guard after failed completion
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
                            if guard.remove(&guard_key) {
                                tracing::info!(target: "data_saving_diag", "[DataSaving] guard cleanup after error for key={guard_key}");
                            } else {
                                tracing::warn!(target: "data_saving_diag", "[DataSaving] guard cleanup after error: key not found key={guard_key}");
                            }
                        }
                    }
                }
                
                if let (StageType::ListPageCrawling, StageItem::Page(pn)) =
                    (&stage_type, &lifecycle_item)
                {
                    Self::emit_best_effort(
                        &ctx,
                        AppEvent::PageLifecycle {
                            session_id: session_id.clone(),
                            batch_id: batch_id.clone(),
                            page_number: *pn,
                            status: "failed".into(),
                            metrics: Some(SimpleMetrics::Page {
                                url_count: None,
                                scheduled_details: None,
                                error: Some(format!("{:?}", err)),
                            }),
                            timestamp: Utc::now(),
                        },
                    );
                }
                if let (StageType::ProductDetailCrawling, StageItem::ProductUrls(urls)) =
                    (&stage_type, &lifecycle_item)
                {
                    for pu in &urls.urls {
                        Self::emit_best_effort(
                            &ctx,
                            AppEvent::ProductLifecycle {
                                session_id: session_id.clone(),
                                batch_id: batch_id.clone(),
                                page_number: Some(pu.page_id as u32),
                                product_ref: pu.url.clone(),
                                status: "failed".into(),
                                retry: None,
                                duration_ms: Some(item_start.elapsed().as_millis() as u64),
                                metrics: Some(SimpleMetrics::Product {
                                    fields: None,
                                    size_bytes: None,
                                    error: Some(format!("{:?}", err)),
                                }),
                                timestamp: Utc::now(),
                            },
                        );
                    }
                }
            }
        }

        result
    }
    /// Before-each-item hook (middleware slot): emit logs/metrics or modify context in future.
    async fn before_each_item_hook(_: &AppContext, _: &StageType, _: &StageItem) {
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
                StageActor::emit_best_effort(
                    context,
                    AppEvent::PerformanceMetrics {
                        session_id: key,
                        metrics: snapshot,
                        timestamp: Utc::now(),
                    },
                );
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
            // Default to a no-op batcher; callers can replace via a setter in the future if needed
            batcher: Arc::new(DefaultStageBatcher),
        }
    }

    /// Optionally replace the batch planning helper. This allows custom chunking/concurrency policies.
    pub fn set_batcher(&mut self, batcher: Arc<dyn StageBatcher>) {
        self.batcher = batcher;
    }

    /// Builder-style API to replace the batcher and return self for chaining.
    #[must_use]
    pub fn with_batcher(mut self, batcher: Arc<dyn StageBatcher>) -> Self {
        self.batcher = batcher;
        self
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
    ///
    /// # Errors
    /// 현재 구현은 항상 `Ok(())`를 반환합니다. 실제 엔진 초기화가 도입되면
    /// 초기화 실패 사유를 `StageError`로 반환해야 합니다.
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
    ///
    /// # Errors
    /// - 스테이지가 이미 실행 중인 경우(상태 불일치)
    /// - 내부 처리에서 타임아웃이 발생한 경우
    /// - 전략 실행 실패 혹은 이벤트 발행 실패가 `StageError`로 매핑된 경우
    pub async fn execute_stage(
        &mut self,
        stage_type: StageType,
        items: Vec<StageItem>,
        concurrency_limit: u32,
        timeout_secs: u64,
        context: &AppContext,
    ) -> Result<StageResult, StageError> {
        // Execute the stage and produce a result snapshot from current actor state
        self.handle_execute_stage(stage_type, items, concurrency_limit, timeout_secs, context)
            .await?;

        let result = StageResult {
            processed_items: self.completed_items,
            successful_items: self.success_count,
            failed_items: self.failure_count,
            duration_ms: self
                .start_time
                .map_or(0, |start| start.elapsed().as_millis() as u64),
            // Now StageResult.details is typed; return as-is
            details: self.item_results.clone(),
        };

        // Important: reset internal state to Idle so caller can run subsequent stages sequentially
        self.cleanup_stage();

        Ok(result)
    }

    /// 스테이지 실행 처리
    ///
    /// # Arguments
    /// * `stage_type` - 실행할 스테이지 타입
    /// * `items` - 처리할 아이템 리스트
    /// * `concurrency_limit` - 동시성 제한
    /// * `timeout_secs` - 타임아웃 (초)
    /// * `context` - Actor 컨텍스트
    ///
    /// # Errors
    /// - 상태가 `Idle`이 아닌 경우(중복 실행 시도)
    /// - 아이템 처리 중 타임아웃 발생
    /// - 스테이지 완료/실패 이벤트 발행 실패(컨텍스트 브로드캐스트 오류 등)
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

        // Clean up any stale DataSaving guards for this session at the start of stage processing
        if matches!(stage_type, StageType::DataSaving) {
            if let Some(ref session_id) = &context.session_id {
                Self::cleanup_data_saving_guards_for_session(session_id);
            }
        }

        let stage_id = Uuid::new_v4().to_string();

        // ProductDetailCrawling의 경우 StageStarted 이벤트 이전에 번들을 개별 URL로 확장해야 UI total이 정확해진다.
        let (maybe_expanded_items, expanded_log) = if matches!(stage_type, StageType::ProductDetailCrawling) {
            let original = items.len();
            let expanded = Self::expand_product_urls_to_individual_items(items);
            let expanded_cnt = expanded.len();
            (
                expanded,
                format!(
                    "🔄 Expanded ProductUrls bundles before StageStarted: {} -> {} ProductUrl items",
                    original, expanded_cnt
                ),
            )
        } else {
            (items, String::new())
        };

        if !expanded_log.is_empty() {
            info!(target: "stage_actor", "{}", expanded_log);
        }

        info!(
            "🎯 StageActor {} executing stage {:?} with {} items",
            self.actor_id,
            stage_type,
            maybe_expanded_items.len()
        );

        // 상태 초기화
        self.stage_id = Some(stage_id.clone());
        self.stage_type = Some(stage_type.clone());
        self.state = StageState::Starting;
        self.start_time = Some(Instant::now());
    self.total_items = maybe_expanded_items.len() as u32;
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
            items_count: self.total_items,
            timestamp: Utc::now(),
        };

        self.emit(context, start_event)?;

        // 상태를 Processing으로 전환
        self.state = StageState::Processing;

        // 내부 타임아웃/취소 지원이 포함된 처리 실행 (tasks abort 포함)
        let processing_result = self
            .process_stage_items(
                stage_type.clone(),
                maybe_expanded_items,
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
                self.emit(context, completion_event)?;
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
                self.emit(context, timeout_event)?;
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
                self.emit(context, failure_event)?;
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
    /// * `overall_timeout` - 전체 처리 타임아웃
    ///
    /// # Errors
    /// - 전체 타임아웃 초과 시 `StageError::TimeoutError`
    /// - 태스크 조인 오류/전략 실행 오류는 실패 아이템으로 집계되어 반환 결과에 포함됩니다.
    async fn process_stage_items(
        &mut self,
        stage_type: StageType,
        raw_items: Vec<StageItem>,
        raw_concurrency_limit: u32,
        _context: &AppContext,
        raw_overall_timeout: Duration,
    ) -> Result<StageResult, StageError> {
        debug!(
            "Processing {} raw items for stage {:?}",
            raw_items.len(),
            stage_type
        );

        // 배치 정책 적용 (현재는 no-op 계획)
        let plan = self.batcher.plan(
            &stage_type,
            raw_items,
            raw_concurrency_limit,
            raw_overall_timeout,
        );
        let items = plan.items; // ProductDetail 확장은 handle_execute_stage 단계에서 이미 수행됨
        let concurrency_limit = plan.concurrency_limit;
        let overall_timeout = plan.overall_timeout;

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
                products_on_last_page_hint,
            };
            join_set.spawn(Self::execute_single_item_task(sem, input));
        }

        // 모든 태스크 완료 대기 (전체 타임아웃 관리 및 잔여 task abort)
        let mut results = Vec::new();
    // Progress tracking (ListPageCrawling only). Emit every page completion (no throttling); UI aggregates.
    // ListPageCrawling: one StageItem per page -> total_items_count = scheduled pages
    // ProductDetailCrawling: each StageItem currently bundles many product URLs; detail progress handled elsewhere
    let progress_enabled = matches!(stage_type, StageType::ListPageCrawling);
    let total_items_count = self.total_items; // pages scheduled
    let mut succeeded_count: u32 = 0; // pages succeeded
    let mut failed_count: u32 = 0;    // pages failed (retry-exhausted)
        loop {
            let now = Instant::now();
            if now >= deadline {
                // 남은 작업들 중단
                while let Some(h) = join_set.join_next().await {
                    if let Ok(Err(e)) = h {
                        error!("Aborted after timeout; task error: {:?}", e);
                    }
                }
                return Err(StageError::TimeoutError {
                    timeout_ms: overall_timeout.as_millis() as u64,
                });
            }
            let remaining = deadline.saturating_duration_since(now);
            match tokio::time::timeout(remaining, join_set.join_next()).await {
                Ok(Some(Ok(Ok(res)))) => {
                    if progress_enabled {
                        if res.success { succeeded_count += 1; } else { failed_count += 1; }
                        let emit_due_count = succeeded_count + failed_count; // pages completed (success+fail)
                        let is_final = emit_due_count == total_items_count;
                        Self::emit_best_effort(
                            &_context,
                            AppEvent::ProductLifecycleGroup {
                                session_id: _context.session_id.clone(),
                                batch_id: Some(batch_id_owned.clone()),
                                page_number: None,
                                group_size: total_items_count,
                                started: total_items_count, // planned
                                succeeded: succeeded_count,
                                failed: failed_count,
                                duplicates: 0,
                                duration_ms: (overall_timeout.as_millis() as u64).saturating_sub(deadline.saturating_duration_since(Instant::now()).as_millis() as u64),
                                phase: "fetch".into(),
                                partial: Some(!is_final),
                                done: Some(emit_due_count),
                                timestamp: Utc::now(),
                            },
                        );
                    }
                    results.push(res)
                },
                Ok(Some(Ok(Err(e)))) => {
                    error!("Item processing failed: {:?}", e);
                    // If this stage is DataSaving, emit a minimal persist failure snapshot so Stage 5 UI updates
                    if matches!(stage_type, StageType::DataSaving) {
                        Self::emit_best_effort(
                            &_context,
                            AppEvent::ProductLifecycle {
                                session_id: _context.session_id.clone(),
                                batch_id: Some(batch_id_owned.clone()),
                                page_number: None,
                                product_ref: "_batch_persist".into(),
                                status: "persist_failed_exec".into(),
                                retry: None,
                                duration_ms: None,
                                metrics: Some(SimpleMetrics::Generic {
                                    key: "error".into(),
                                    value: format!("{:?}", e),
                                }),
                                timestamp: Utc::now(),
                            },
                        );
                        Self::emit_best_effort(
                            &_context,
                            AppEvent::ProductLifecycleGroup {
                                session_id: _context.session_id.clone(),
                                batch_id: Some(batch_id_owned.clone()),
                                page_number: None,
                                group_size: 0,
                                started: 0,
                                succeeded: 0,
                                failed: 1,
                                duplicates: 0,
                                duration_ms: 0,
                                phase: "persist".into(),
                                partial: None,
                                done: None,
                                timestamp: Utc::now(),
                            },
                        );
                    }
                    results.push(StageItemResult {
                        item_id: "unknown".into(),
                        item_type: StageItemType::Url { url_type: "unknown".into() },
                        success: false,
                        error: Some(format!("{:?}", e)),
                        duration_ms: 0,
                        retry_count: 0,
                        collected_data: None,
                    });
                }
                Ok(Some(Err(join_err))) => {
                    error!("Task join error: {}", join_err);
                    results.push(StageItemResult {
                        item_id: "unknown".into(),
                        item_type: StageItemType::Url {
                            url_type: "unknown".into(),
                        },
                        success: false,
                        error: Some(format!("Task join error: {}", join_err)),
                        duration_ms: 0,
                        retry_count: 0,
                        collected_data: None,
                    });
                }
                Ok(None) => break, // all done
                Err(_elapsed) => {
                    // timed out waiting for next; loop checks deadline and exits with timeout handling above
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
    ///
    /// # Errors
    /// - 데이터베이스에 저장/업데이트 중 오류가 발생하면 `Err(String)`으로 상세 메시지가 반환됩니다.
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
    use crate::crawl_engine::stages::DefaultStageLogicFactory;
    use crate::crawl_engine::system_config::SystemConfig;
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
        let extractor =
            Arc::new(crate::infrastructure::MatterDataExtractor::new().expect("extractor"));
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
            url: "https://e/p1".into(),
            page_id: Some(1),
            index_in_page: Some(1),
            id: None,
            manufacturer: Some("A".into()),
            model: Some("M".into()),
            device_type: None,
            certificate_id: None,
            certification_date: None,
            software_version: None,
            hardware_version: None,
            vid: None,
            pid: None,
            family_sku: None,
            family_variant_sku: None,
            firmware_version: None,
            family_id: None,
            tis_trp_tested: None,
            specification_version: None,
            transport_interface: None,
            primary_device_type_ids: None,
            application_categories: None,
            description: None,
            compliance_document_url: None,
            program_type: Some("Matter".into()),
            created_at: now,
            updated_at: now,
        };
        let items = vec![ch::StageItem::ProductDetails(ch::ProductDetails {
            products: vec![pd1],
            source_urls: vec![],
            extraction_stats: ch::ExtractionStats {
                attempted: 1,
                successful: 1,
                failed: 0,
                empty_responses: 0,
            },
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
                        Ok(AppEvent::StageCompleted { .. }) => {
                            got_stage_completed = true;
                        }
                        Ok(AppEvent::PerformanceMetrics { .. }) => {
                            metrics_seen = true;
                        }
                        Ok(AppEvent::StageStarted { .. }) => {
                            started_seen = true;
                        }
                        Ok(AppEvent::ValidationCompleted { .. }) => {
                            validation_completed_seen = true;
                        }
                        _ => {}
                    }
                    if got_stage_completed && validation_completed_seen {
                        break;
                    }
                } else {
                    // timeout, continue loop until overall deadline
                }
            }
            (
                got_stage_completed,
                metrics_seen,
                started_seen,
                validation_completed_seen,
            )
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

        let (got_completed, metrics_seen, started_seen, validation_done) =
            collector.await.expect("collector join");
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_data_saving_guard_cleanup() {
        // Initialize the guard with some test keys
        {
            if let Ok(mut guard) = DATA_SAVING_RUN_GUARD.lock() {
                guard.insert("session1:batch1:data_saving".to_string());
                guard.insert("session1:batch2:data_saving".to_string());
                guard.insert("session2:batch1:data_saving".to_string());
                guard.insert("other_session:batch1:data_saving".to_string());
            }
        }

        // Verify initial state
        {
            if let Ok(guard) = DATA_SAVING_RUN_GUARD.lock() {
                assert_eq!(guard.len(), 4);
                assert!(guard.contains("session1:batch1:data_saving"));
                assert!(guard.contains("session1:batch2:data_saving"));
                assert!(guard.contains("session2:batch1:data_saving"));
                assert!(guard.contains("other_session:batch1:data_saving"));
            }
        }

        // Test cleanup for session1
        StageActor::cleanup_data_saving_guards_for_session("session1");

        // Verify session1 keys are removed, others remain
        {
            if let Ok(guard) = DATA_SAVING_RUN_GUARD.lock() {
                assert_eq!(guard.len(), 2);
                assert!(!guard.contains("session1:batch1:data_saving"));
                assert!(!guard.contains("session1:batch2:data_saving"));
                assert!(guard.contains("session2:batch1:data_saving"));
                assert!(guard.contains("other_session:batch1:data_saving"));
            }
        }

        // Test cleanup for session2
        StageActor::cleanup_data_saving_guards_for_session("session2");

        // Verify session2 keys are removed
        {
            if let Ok(guard) = DATA_SAVING_RUN_GUARD.lock() {
                assert_eq!(guard.len(), 1);
                assert!(guard.contains("other_session:batch1:data_saving"));
            }
        }

        // Clean up remaining test data
        StageActor::cleanup_data_saving_guards_for_session("other_session");

        {
            if let Ok(guard) = DATA_SAVING_RUN_GUARD.lock() {
                assert_eq!(guard.len(), 0);
            }
        }
    }

    #[test]
    fn test_data_saving_guard_cleanup_empty_session() {
        // Test cleanup on non-existent session should not panic
        StageActor::cleanup_data_saving_guards_for_session("non_existent_session");

        // Verify no keys are affected
        {
            if let Ok(guard) = DATA_SAVING_RUN_GUARD.lock() {
                assert_eq!(guard.len(), 0);
            }
        }
    }

    #[test]
    fn test_data_saving_guard_cleanup_partial_match() {
        // Test that only exact session prefix matches are cleaned
        {
            if let Ok(mut guard) = DATA_SAVING_RUN_GUARD.lock() {
                guard.insert("session123:batch1:data_saving".to_string());
                guard.insert("session1:batch1:data_saving".to_string());
                guard.insert("session12:batch1:data_saving".to_string());
            }
        }

        // Clean up session1 - should only remove exact prefix matches
        StageActor::cleanup_data_saving_guards_for_session("session1");

        {
            if let Ok(guard) = DATA_SAVING_RUN_GUARD.lock() {
                assert_eq!(guard.len(), 2);
                assert!(guard.contains("session123:batch1:data_saving"));
                assert!(guard.contains("session12:batch1:data_saving"));
                assert!(!guard.contains("session1:batch1:data_saving"));
            }
        }

        // Clean up remaining test data
        if let Ok(mut guard) = DATA_SAVING_RUN_GUARD.lock() {
            guard.clear();
        }
    }
}
