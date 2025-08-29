//! 크롤링 성능 최적화 서비스
//! Phase C: 실시간 성능 모니터링 및 자동 최적화

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use ts_rs::TS;

use crate::crawl_engine::config::SystemConfig;

/// 크롤링 성능 메트릭
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CrawlingPerformanceMetrics {
    /// 현재 세션 ID
    pub session_id: String,
    /// 평균 응답 시간 (밀리초)
    pub avg_response_time_ms: f64,
    /// 성공률 (0.0-1.0)
    pub success_rate: f64,
    /// 초당 처리량 (requests per second)
    pub throughput_rps: f64,
    /// 동시성 수준
    pub current_concurrency: u32,
    /// 권장 동시성
    pub recommended_concurrency: u32,
    /// 메모리 사용량 (KB)
    pub memory_usage_kb: u64,
    /// 네트워크 오류율
    pub network_error_rate: f64,
    /// 최적화 상태
    pub optimization_status: OptimizationStatus,
}

/// 최적화 상태
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum OptimizationStatus {
    /// 최적화된 상태
    Optimal,
    /// 동시성 증가 권장
    ShouldIncrease,
    /// 동시성 감소 권장
    ShouldDecrease,
    /// 불안정 상태
    Unstable,
}

/// 성능 최적화 추천사항
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OptimizationRecommendation {
    /// 추천 동시성
    pub recommended_concurrency: u32,
    /// 추천 배치 크기
    pub recommended_batch_size: u32,
    /// 추천 지연 시간 (밀리초)
    pub recommended_delay_ms: u64,
    /// 추천 이유
    pub reason: String,
    /// 예상 성능 향상 (%)
    pub expected_improvement_percent: f64,
}

/// 성능 최적화 서비스
pub struct CrawlingPerformanceOptimizer {
    config: Arc<SystemConfig>,
    metrics_history: Arc<RwLock<Vec<CrawlingPerformanceMetrics>>>,
    current_session: Arc<RwLock<Option<String>>>,
}

impl CrawlingPerformanceOptimizer {
    /// 새 최적화 서비스 생성
    #[must_use] pub fn new(config: Arc<SystemConfig>) -> Self {
        Self {
            config,
            metrics_history: Arc::new(RwLock::new(Vec::new())),
            current_session: Arc::new(RwLock::new(None)),
        }
    }

    /// 새 세션 시작
    pub async fn start_session(&self, session_id: String) {
        let mut current = self.current_session.write().await;
        *current = Some(session_id.clone());

        info!(session_id = %session_id, "🔧 Performance optimization session started");
    }

    /// 세션 종료
    pub async fn end_session(&self) {
        let mut current = self.current_session.write().await;
        if let Some(session_id) = current.take() {
            info!(session_id = %session_id, "✅ Performance optimization session ended");
        }
    }

    /// 성능 메트릭 기록
    pub async fn record_metrics(
        &self,
        response_time_ms: u64,
        success: bool,
        concurrency: u32,
        memory_usage_kb: u64,
        network_error: bool,
    ) {
        let session_id = {
            let current = self.current_session.read().await;
            current.clone().unwrap_or_else(|| "unknown".to_string())
        };

        // 성능 메트릭 계산
        let metrics = self
            .calculate_current_metrics(
                session_id.clone(),
                response_time_ms,
                success,
                concurrency,
                memory_usage_kb,
                network_error,
            )
            .await;

        // 히스토리에 추가
        let mut history = self.metrics_history.write().await;
        history.push(metrics.clone());

        // 최대 100개 항목 유지
        if history.len() > 100 {
            history.remove(0);
        }

        debug!(
            session_id = %session_id,
            avg_response_time = metrics.avg_response_time_ms,
            success_rate = metrics.success_rate,
            throughput = metrics.throughput_rps,
            "📊 Performance metrics recorded"
        );
    }

    /// 현재 성능 메트릭 계산
    async fn calculate_current_metrics(
        &self,
        session_id: String,
        response_time_ms: u64,
        success: bool,
        concurrency: u32,
        memory_usage_kb: u64,
        network_error: bool,
    ) -> CrawlingPerformanceMetrics {
        let history = self.metrics_history.read().await;

        // 최근 10개 항목으로 평균 계산
        let recent_metrics: Vec<_> = history.iter().rev().take(10).collect();

        let avg_response_time_ms = if recent_metrics.is_empty() {
            response_time_ms as f64
        } else {
            let sum: f64 = recent_metrics.iter().map(|m| m.avg_response_time_ms).sum();
            (sum + response_time_ms as f64) / (recent_metrics.len() + 1) as f64
        };

        let success_rate = if recent_metrics.is_empty() {
            if success { 1.0 } else { 0.0 }
        } else {
            let success_sum: f64 = recent_metrics.iter().map(|m| m.success_rate).sum();
            let current_success = if success { 1.0 } else { 0.0 };
            (success_sum + current_success) / (recent_metrics.len() + 1) as f64
        };

        let throughput_rps = if avg_response_time_ms > 0.0 {
            (1000.0 / avg_response_time_ms) * f64::from(concurrency)
        } else {
            0.0
        };

        let network_error_rate = if recent_metrics.is_empty() {
            if network_error { 1.0 } else { 0.0 }
        } else {
            let error_sum: f64 = recent_metrics.iter().map(|m| m.network_error_rate).sum();
            let current_error = if network_error { 1.0 } else { 0.0 };
            (error_sum + current_error) / (recent_metrics.len() + 1) as f64
        };

        // 최적화 상태 결정
        let optimization_status = self.determine_optimization_status(
            avg_response_time_ms,
            success_rate,
            throughput_rps,
            network_error_rate,
            concurrency,
        );

        // 권장 동시성 계산
        let recommended_concurrency = self.calculate_recommended_concurrency(
            avg_response_time_ms,
            success_rate,
            throughput_rps,
            concurrency,
        );

        CrawlingPerformanceMetrics {
            session_id,
            avg_response_time_ms,
            success_rate,
            throughput_rps,
            current_concurrency: concurrency,
            recommended_concurrency,
            memory_usage_kb,
            network_error_rate,
            optimization_status,
        }
    }

    /// 최적화 상태 결정
    fn determine_optimization_status(
        &self,
        avg_response_time_ms: f64,
        success_rate: f64,
        _throughput_rps: f64,
        network_error_rate: f64,
        current_concurrency: u32,
    ) -> OptimizationStatus {
        // 불안정 상태 체크
        if success_rate < 0.8 || network_error_rate > 0.2 {
            return OptimizationStatus::Unstable;
        }

        // 응답 시간 기반 판단
        let target_response_time = 1000.0; // 1초 목표
        let max_concurrency = self.config.performance.concurrency.max_concurrent_tasks;

        if avg_response_time_ms > target_response_time * 2.0 {
            // 너무 느림, 동시성 감소 권장
            OptimizationStatus::ShouldDecrease
        } else if avg_response_time_ms < target_response_time * 0.5
            && current_concurrency < max_concurrency
        {
            // 충분히 빠름, 동시성 증가 가능
            OptimizationStatus::ShouldIncrease
        } else {
            // 적절한 상태
            OptimizationStatus::Optimal
        }
    }

    /// 권장 동시성 계산
    fn calculate_recommended_concurrency(
        &self,
        avg_response_time_ms: f64,
        success_rate: f64,
        _throughput_rps: f64,
        current_concurrency: u32,
    ) -> u32 {
        let min_concurrency = self.config.performance.concurrency.min_concurrent_batches;
        let max_concurrency = self.config.performance.concurrency.max_concurrent_batches;

        // 성공률이 낮으면 동시성 감소
        if success_rate < 0.9 {
            return (current_concurrency.saturating_sub(1)).max(min_concurrency);
        }

        // 응답 시간 기반 조정
        let target_response_time = 1000.0; // 1초 목표

        if avg_response_time_ms > target_response_time * 1.5 {
            // 너무 느림, 동시성 감소
            (current_concurrency.saturating_sub(1)).max(min_concurrency)
        } else if avg_response_time_ms < target_response_time * 0.7 {
            // 충분히 빠름, 동시성 증가
            (current_concurrency + 1).min(max_concurrency)
        } else {
            // 현재 상태 유지
            current_concurrency
        }
    }

    /// 현재 성능 메트릭 조회
    pub async fn get_current_metrics(&self) -> Option<CrawlingPerformanceMetrics> {
        let history = self.metrics_history.read().await;
        history.last().cloned()
    }

    /// 최적화 추천사항 계산
    pub async fn get_optimization_recommendation(&self) -> Option<OptimizationRecommendation> {
        let current_metrics = self.get_current_metrics().await?;

        let recommended_concurrency = current_metrics.recommended_concurrency;
        let current_concurrency = current_metrics.current_concurrency;

        // 배치 크기 권장사항
        let recommended_batch_size = if current_metrics.success_rate > 0.95 {
            self.config.performance.batch_sizes.initial_size.min(50)
        } else {
            self.config.performance.batch_sizes.initial_size.min(20)
        };

        // 지연 시간 권장사항
        let recommended_delay_ms = if current_metrics.network_error_rate > 0.1 {
            1000 // 네트워크 오류가 많으면 지연 증가
        } else if current_metrics.success_rate > 0.98 {
            200 // 성공률이 높으면 지연 감소
        } else {
            500 // 기본값
        };

        // 개선 효과 추정
        let expected_improvement_percent = if recommended_concurrency == current_concurrency {
            0.0
        } else {
            let concurrency_diff = f64::from(recommended_concurrency) / f64::from(current_concurrency);
            (concurrency_diff - 1.0) * 50.0 // 동시성 변화의 50% 효과 가정
        };

        let reason = match current_metrics.optimization_status {
            OptimizationStatus::Optimal => "현재 설정이 최적입니다".to_string(),
            OptimizationStatus::ShouldIncrease => {
                "동시성을 증가시켜 처리량을 향상시킬 수 있습니다".to_string()
            }
            OptimizationStatus::ShouldDecrease => {
                "동시성을 감소시켜 안정성을 향상시켜야 합니다".to_string()
            }
            OptimizationStatus::Unstable => {
                "시스템이 불안정합니다. 설정을 보수적으로 조정하세요".to_string()
            }
        };

        Some(OptimizationRecommendation {
            recommended_concurrency,
            recommended_batch_size,
            recommended_delay_ms,
            reason,
            expected_improvement_percent,
        })
    }

    /// 성능 히스토리 조회
    pub async fn get_performance_history(&self) -> Vec<CrawlingPerformanceMetrics> {
        let history = self.metrics_history.read().await;
        history.clone()
    }

    /// 성능 히스토리 초기화
    pub async fn clear_performance_history(&self) {
        let mut history = self.metrics_history.write().await;
        history.clear();
        info!("🧹 Performance history cleared");
    }
}

/// Type alias for compatibility
pub type PerformanceOptimizer = CrawlingPerformanceOptimizer;
