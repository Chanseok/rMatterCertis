//! 작업 큐 기반 크롤링 엔진
//! 
//! 이 모듈은 독립적인 작업 단위들을 큐로 관리하며,
//! 병렬 처리와 즉시 취소를 지원하는 고성능 크롤링 엔진입니다.

use std::sync::Arc;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use anyhow::{Result, anyhow};
use tracing::{info, warn, debug};
use tokio::sync::{Mutex, RwLock, Semaphore};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// 독립적인 작업 단위 정의
#[derive(Debug, Clone)]
pub enum WorkItem {
    /// 사이트 상태 확인
    CheckSiteStatus,
    
    /// 데이터베이스 분석
    AnalyzeDatabase,
    
    /// 단일 페이지 크롤링 (페이지 번호)
    CrawlPage(u32),
    
    /// 단일 제품 상세정보 수집 (제품 URL)
    CollectProductDetail(String),
    
    /// 제품 데이터베이스 저장 (제품 데이터)
    SaveProduct(crate::domain::product::Product),
}

/// 작업 상태
#[derive(Debug, Clone, PartialEq)]
pub enum WorkStatus {
    Pending,
    InProgress,
    Completed,
    Failed(String),
    Cancelled,
}

/// 작업 항목 (작업 + 메타데이터)
#[derive(Debug, Clone)]
pub struct WorkTask {
    pub id: Uuid,
    pub item: WorkItem,
    pub status: WorkStatus,
    pub priority: u8, // 0=highest, 255=lowest
    pub created_at: Instant,
    pub started_at: Option<Instant>,
    pub completed_at: Option<Instant>,
    pub retry_count: u8,
    pub max_retries: u8,
}

impl WorkTask {
    pub fn new(item: WorkItem, priority: u8) -> Self {
        Self {
            id: Uuid::new_v4(),
            item,
            status: WorkStatus::Pending,
            priority,
            created_at: Instant::now(),
            started_at: None,
            completed_at: None,
            retry_count: 0,
            max_retries: 3,
        }
    }
    
    pub fn with_max_retries(mut self, max_retries: u8) -> Self {
        self.max_retries = max_retries;
        self
    }
}

/// 작업 큐 통계
#[derive(Debug, Clone)]
pub struct QueueStats {
    pub total_tasks: usize,
    pub pending_tasks: usize,
    pub in_progress_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub cancelled_tasks: usize,
}

/// 작업 큐 매니저
pub struct WorkQueueManager {
    /// 대기 중인 작업들 (우선순위 큐)
    pending_queue: Arc<Mutex<VecDeque<WorkTask>>>,
    
    /// 진행 중인 작업들
    in_progress_tasks: Arc<RwLock<Vec<WorkTask>>>,
    
    /// 완료된 작업들
    completed_tasks: Arc<RwLock<Vec<WorkTask>>>,
    
    /// 동시 실행 제한
    semaphore: Arc<Semaphore>,
    
    /// 취소 토큰
    cancellation_token: CancellationToken,
    
    /// 작업 처리 설정
    config: WorkQueueConfig,
}

#[derive(Debug, Clone)]
pub struct WorkQueueConfig {
    pub max_concurrent_workers: usize,
    pub worker_delay_ms: u64,
    pub queue_check_interval_ms: u64,
}

impl Default for WorkQueueConfig {
    fn default() -> Self {
        Self {
            max_concurrent_workers: 10,
            worker_delay_ms: 100,
            queue_check_interval_ms: 50,
        }
    }
}

impl WorkQueueManager {
    pub fn new(config: WorkQueueConfig) -> Self {
        Self {
            pending_queue: Arc::new(Mutex::new(VecDeque::new())),
            in_progress_tasks: Arc::new(RwLock::new(Vec::new())),
            completed_tasks: Arc::new(RwLock::new(Vec::new())),
            semaphore: Arc::new(Semaphore::new(config.max_concurrent_workers)),
            cancellation_token: CancellationToken::new(),
            config,
        }
    }
    
    /// 작업 추가
    pub async fn enqueue_task(&self, task: WorkTask) -> Result<()> {
        let mut queue = self.pending_queue.lock().await;
        
        // 우선순위 기반 삽입 (낮은 숫자가 높은 우선순위)
        let insert_position = queue.iter()
            .position(|existing| existing.priority > task.priority)
            .unwrap_or(queue.len());
            
        queue.insert(insert_position, task);
        debug!("작업이 큐에 추가되었습니다. 대기 중인 작업 수: {}", queue.len());
        
        Ok(())
    }
    
    /// 작업 큐 통계 조회
    pub async fn get_stats(&self) -> QueueStats {
        let pending = self.pending_queue.lock().await.len();
        let in_progress = self.in_progress_tasks.read().await.len();
        let completed_tasks = self.completed_tasks.read().await;
        
        let completed = completed_tasks.iter()
            .filter(|t| t.status == WorkStatus::Completed)
            .count();
            
        let failed = completed_tasks.iter()
            .filter(|t| matches!(t.status, WorkStatus::Failed(_)))
            .count();
            
        let cancelled = completed_tasks.iter()
            .filter(|t| t.status == WorkStatus::Cancelled)
            .count();
        
        QueueStats {
            total_tasks: pending + in_progress + completed_tasks.len(),
            pending_tasks: pending,
            in_progress_tasks: in_progress,
            completed_tasks: completed,
            failed_tasks: failed,
            cancelled_tasks: cancelled,
        }
    }
    
    /// 모든 작업 취소
    pub async fn cancel_all(&self) {
        info!("🛑 모든 작업을 취소하는 중...");
        self.cancellation_token.cancel();
        
        // 대기 중인 작업들을 취소됨으로 마킹
        let mut pending = self.pending_queue.lock().await;
        let mut completed = self.completed_tasks.write().await;
        
        while let Some(mut task) = pending.pop_front() {
            task.status = WorkStatus::Cancelled;
            task.completed_at = Some(Instant::now());
            completed.push(task);
        }
        
        info!("🛑 모든 작업이 취소되었습니다");
    }
    
    /// 작업 큐 시작 - 워커들을 백그라운드로 실행
    pub async fn start_workers(&self, worker_context: Arc<WorkerContext>) -> Result<()> {
        info!("🚀 {} 개의 워커를 시작합니다", self.config.max_concurrent_workers);
        
        // 여러 워커를 동시에 실행
        let mut worker_handles = Vec::new();
        
        for worker_id in 0..self.config.max_concurrent_workers {
            let worker_handle = self.spawn_worker(worker_id, Arc::clone(&worker_context));
            worker_handles.push(worker_handle);
        }
        
        // 모든 워커가 완료될 때까지 대기
        futures::future::try_join_all(worker_handles).await?;
        
        info!("✅ 모든 워커가 완료되었습니다");
        Ok(())
    }
    
    /// 단일 워커 실행
    async fn spawn_worker(&self, worker_id: usize, context: Arc<WorkerContext>) -> Result<()> {
        info!("👷 워커 {} 시작", worker_id);
        
        loop {
            // 취소 확인
            if self.cancellation_token.is_cancelled() {
                info!("🛑 워커 {} 취소됨", worker_id);
                break;
            }
            
            // 세마포어 획득 (동시 실행 제한)
            let permit = match self.semaphore.clone().try_acquire_owned() {
                Ok(permit) => permit,
                Err(_) => {
                    // 사용 가능한 슬롯이 없으면 잠시 대기
                    tokio::time::sleep(Duration::from_millis(self.config.queue_check_interval_ms)).await;
                    continue;
                }
            };
            
            // 다음 작업 가져오기
            let task = {
                let mut queue = self.pending_queue.lock().await;
                queue.pop_front()
            };
            
            if let Some(mut task) = task {
                // 작업을 진행 중으로 이동
                task.status = WorkStatus::InProgress;
                task.started_at = Some(Instant::now());
                
                {
                    let mut in_progress = self.in_progress_tasks.write().await;
                    in_progress.push(task.clone());
                }
                
                debug!("👷 워커 {} 작업 시작: {:?}", worker_id, task.item);
                
                // 실제 작업 실행
                let result = self.execute_task(&task, &context).await;
                
                // 작업 완료 처리
                self.complete_task(task, result).await;
                
                // 세마포어 해제
                drop(permit);
            } else {
                // 작업이 없으면 잠시 대기
                tokio::time::sleep(Duration::from_millis(self.config.queue_check_interval_ms)).await;
            }
        }
        
        info!("👷 워커 {} 종료", worker_id);
        Ok(())
    }
    
    /// 작업 실행
    async fn execute_task(&self, task: &WorkTask, context: &WorkerContext) -> Result<WorkResult> {
        // 취소 확인
        if self.cancellation_token.is_cancelled() {
            return Err(anyhow!("작업이 취소되었습니다"));
        }
        
        match &task.item {
            WorkItem::CheckSiteStatus => {
                let status = context.status_checker.check_site_status().await?;
                Ok(WorkResult::SiteStatus(status))
            }
            
            WorkItem::AnalyzeDatabase => {
                let analysis = context.database_analyzer.analyze_current_state().await?;
                Ok(WorkResult::DatabaseAnalysis(analysis))
            }
            
            WorkItem::CrawlPage(page_num) => {
                let urls = context.product_list_collector.collect_single_page(*page_num).await?;
                Ok(WorkResult::ProductUrls(urls))
            }
            
            WorkItem::CollectProductDetail(url) => {
                let details = context.product_detail_collector.collect_details(&[url.clone()]).await?;
                if let Some(detail) = details.into_iter().next() {
                    Ok(WorkResult::ProductDetail(detail))
                } else {
                    Err(anyhow!("제품 상세 정보를 찾을 수 없습니다"))
                }
            }
            
            WorkItem::SaveProduct(product) => {
                context.product_repo.create_or_update_product(product).await?;
                Ok(WorkResult::ProductSaved)
            }
        }
    }
    
    /// 작업 완료 처리
    async fn complete_task(&self, mut task: WorkTask, result: Result<WorkResult>) {
        // 진행 중 목록에서 제거
        {
            let mut in_progress = self.in_progress_tasks.write().await;
            in_progress.retain(|t| t.id != task.id);
        }
        
        // 결과에 따라 상태 업데이트
        match result {
            Ok(work_result) => {
                task.status = WorkStatus::Completed;
                task.completed_at = Some(Instant::now());
                
                debug!("✅ 작업 완료: {:?}", task.item);
                
                // 후속 작업 생성 (필요한 경우)
                self.handle_work_result(work_result).await;
            }
            Err(e) => {
                warn!("❌ 작업 실패: {:?} - {}", task.item, e);
                
                // 재시도 가능한지 확인
                if task.retry_count < task.max_retries {
                    task.retry_count += 1;
                    task.status = WorkStatus::Pending;
                    task.started_at = None;
                    
                    // 재시도 큐에 추가
                    self.enqueue_task(task.clone()).await.unwrap_or_else(|e| {
                        warn!("재시도 큐 추가 실패: {}", e);
                    });
                    
                    info!("🔄 작업 재시도: {} 시도 중 {} 번째", task.max_retries, task.retry_count);
                } else {
                    task.status = WorkStatus::Failed(e.to_string());
                    task.completed_at = Some(Instant::now());
                }
            }
        }
        
        // 완료된 작업 목록에 추가
        {
            let mut completed = self.completed_tasks.write().await;
            completed.push(task);
        }
    }
    
    /// 작업 결과 처리 및 후속 작업 생성
    async fn handle_work_result(&self, result: WorkResult) {
        match result {
            WorkResult::ProductUrls(urls) => {
                // 각 제품 URL에 대해 상세 정보 수집 작업 생성
                for url in urls {
                    let task = WorkTask::new(WorkItem::CollectProductDetail(url), 1);
                    self.enqueue_task(task).await.unwrap_or_else(|e| {
                        warn!("제품 상세 정보 수집 작업 추가 실패: {}", e);
                    });
                }
            }
            
            WorkResult::ProductDetail(detail) => {
                // 제품 상세 정보를 제품 객체로 변환하여 저장 작업 생성
                let product = crate::infrastructure::crawling_service_impls::product_detail_to_product(detail);
                let task = WorkTask::new(WorkItem::SaveProduct(product), 2);
                self.enqueue_task(task).await.unwrap_or_else(|e| {
                    warn!("제품 저장 작업 추가 실패: {}", e);
                });
            }
            
            _ => {
                // 다른 결과들은 후속 작업이 필요하지 않음
            }
        }
    }
}

/// 작업 실행 결과
#[derive(Debug)]
pub enum WorkResult {
    SiteStatus(crate::domain::services::SiteStatus),
    DatabaseAnalysis(crate::domain::services::DatabaseAnalysis),
    ProductUrls(Vec<String>),
    ProductDetail(crate::domain::product::ProductDetail),
    ProductSaved,
}

/// 워커 실행 컨텍스트 (서비스들)
pub struct WorkerContext {
    pub status_checker: Arc<dyn crate::domain::services::StatusChecker>,
    pub database_analyzer: Arc<dyn crate::domain::services::DatabaseAnalyzer>,
    pub product_list_collector: Arc<dyn crate::domain::services::ProductListCollector>,
    pub product_detail_collector: Arc<dyn crate::domain::services::ProductDetailCollector>,
    pub product_repo: Arc<crate::infrastructure::IntegratedProductRepository>,
}
