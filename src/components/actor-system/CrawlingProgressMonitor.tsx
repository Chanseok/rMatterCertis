/**
 * CrawlingProgressMonitor - 실시간 크롤링 진행률 시각화
 * Phase C: UI 개선 - OneShot Actor 기반 크롤링 진행률 모니터링
 */

import { Component, createSignal, onMount, onCleanup, For } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import './CrawlingProgressMonitor.css';

// 크롤링 진행률 타입 정의
interface CrawlingProgress {
  session_id: string;
  status: 'preparing' | 'running' | 'paused' | 'completed' | 'error' | 'cancelled';
  overall_progress: {
    total_items: number;
    processed_items: number;
    success_items: number;
    failed_items: number;
    progress_percentage: number;
    estimated_remaining_time_secs: number;
  };
  stage_progress: Array<{
    stage_type: string;
    status: 'pending' | 'running' | 'completed' | 'error';
    processed_items: number;
    total_items: number;
    success_rate: number;
    avg_processing_time_ms: number;
    current_batch_size: number;
  }>;
  performance_stats: {
    items_per_second: number;
    memory_usage_mb: number;
    active_connections: number;
    error_rate_percent: number;
  };
  recent_events: Array<{
    timestamp: string;
    stage: string;
    event_type: 'started' | 'completed' | 'error' | 'retry';
    message: string;
    details?: any;
  }>;
  time_stats: {
    start_time: string;
    elapsed_time_secs: number;
    estimated_total_time_secs: number;
  };
}

interface CrawlingConfig {
  target_pages: number[];
  concurrency_limit: number;
  batch_size: number;
  retry_attempts: number;
  timeout_secs: number;
}

export const CrawlingProgressMonitor: Component = () => {
  const [progress, setProgress] = createSignal<CrawlingProgress | null>(null);
  const [config, setConfig] = createSignal<CrawlingConfig | null>(null);
  const [isRunning, setIsRunning] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [autoRefresh, setAutoRefresh] = createSignal(true);
  
  let refreshTimer: number | null = null;

  // 진행률 조회
  const fetchProgress = async () => {
    try {
      const progressData = await invoke<CrawlingProgress>('get_actor_crawling_progress');
      const configData = await invoke<CrawlingConfig>('get_actor_crawling_config');
      
      setProgress(progressData);
      setConfig(configData);
      setIsRunning(progressData.status === 'running' || progressData.status === 'preparing');
      setError(null);
    } catch (err) {
      setError(`진행률 조회 실패: ${err}`);
    }
  };

  // 크롤링 시작
  const startCrawling = async () => {
    try {
      setError(null);
      await invoke('start_crawling_session');
      setIsRunning(true);
      await fetchProgress();
    } catch (err) {
      setError(`크롤링 시작 실패: ${err}`);
    }
  };

  // 크롤링 일시정지
  const pauseCrawling = async () => {
    try {
      await invoke('pause_crawling_session');
      await fetchProgress();
    } catch (err) {
      setError(`크롤링 일시정지 실패: ${err}`);
    }
  };

  // 크롤링 재개
  const resumeCrawling = async () => {
    try {
      await invoke('resume_crawling_session');
      await fetchProgress();
    } catch (err) {
      setError(`크롤링 재개 실패: ${err}`);
    }
  };

  // 크롤링 중단
  const stopCrawling = async () => {
    try {
      await invoke('stop_crawling_session');
      setIsRunning(false);
      await fetchProgress();
    } catch (err) {
      setError(`크롤링 중단 실패: ${err}`);
    }
  };

  // 자동 새로고침 설정
  const setupAutoRefresh = () => {
    if (refreshTimer) clearInterval(refreshTimer);
    
    if (autoRefresh() && isRunning()) {
      refreshTimer = setInterval(fetchProgress, 1000); // 1초마다
    }
  };

  // 컴포넌트 마운트/언마운트
  onMount(async () => {
    await fetchProgress();
    setupAutoRefresh();
  });

  onCleanup(() => {
    if (refreshTimer) clearInterval(refreshTimer);
  });

  // 자동 새로고침 토글
  const toggleAutoRefresh = () => {
    setAutoRefresh(!autoRefresh());
    setupAutoRefresh();
  };

  // 상태에 따른 색상
  const getStatusColor = (status: string): string => {
    switch (status) {
      case 'running': return '#10B981';
      case 'completed': return '#3B82F6';
      case 'paused': return '#F59E0B';
      case 'error': return '#EF4444';
      case 'cancelled': return '#6B7280';
      case 'preparing': return '#8B5CF6';
      default: return '#6B7280';
    }
  };

  // 시간 포맷팅
  const formatTime = (seconds: number): string => {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = Math.floor(seconds % 60);
    
    if (hours > 0) {
      return `${hours}시간 ${minutes}분 ${secs}초`;
    } else if (minutes > 0) {
      return `${minutes}분 ${secs}초`;
    } else {
      return `${secs}초`;
    }
  };

  // 파일 크기 포맷팅
  const formatFileSize = (mb: number): string => {
    if (mb >= 1024) {
      return `${(mb / 1024).toFixed(1)} GB`;
    }
    return `${mb.toFixed(1)} MB`;
  };

  return (
    <div class="crawling-progress-monitor">
      {/* 헤더 */}
      <div class="monitor-header">
        <h2 class="monitor-title">
          <span class="icon">🚀</span>
          실시간 크롤링 진행률 모니터
        </h2>
        
        <div class="monitor-controls">
          <button 
            class={`auto-refresh-toggle ${autoRefresh() ? 'active' : ''}`}
            onClick={toggleAutoRefresh}
          >
            {autoRefresh() ? '⏸️ 자동갱신' : '▶️ 자동갱신'}
          </button>
          
          <button 
            class="manual-refresh"
            onClick={fetchProgress}
          >
            🔄 새로고침
          </button>
        </div>
      </div>

      {error() && (
        <div class="error-banner">
          <span class="error-icon">⚠️</span>
          {error()}
        </div>
      )}

      {progress() && (
        <>
          {/* 전체 진행률 */}
          <div class="overall-progress-section">
            <div class="progress-card">
              <div class="progress-header">
                <h3>전체 진행률</h3>
                <div class="progress-status">
                  <span 
                    class="status-indicator"
                    style={{ "background-color": getStatusColor(progress()!.status) }}
                  ></span>
                  <span class="status-text">{progress()!.status.toUpperCase()}</span>
                </div>
              </div>
              
              <div class="progress-bar-container">
                <div class="progress-bar">
                  <div 
                    class="progress-fill"
                    style={{ width: `${progress()!.overall_progress.progress_percentage}%` }}
                  ></div>
                </div>
                <span class="progress-percentage">
                  {progress()!.overall_progress.progress_percentage.toFixed(1)}%
                </span>
              </div>
              
              <div class="progress-stats">
                <div class="stat">
                  <span class="stat-label">처리 완료</span>
                  <span class="stat-value">
                    {progress()!.overall_progress.processed_items} / {progress()!.overall_progress.total_items}
                  </span>
                </div>
                <div class="stat">
                  <span class="stat-label">성공</span>
                  <span class="stat-value success">{progress()!.overall_progress.success_items}</span>
                </div>
                <div class="stat">
                  <span class="stat-label">실패</span>
                  <span class="stat-value error">{progress()!.overall_progress.failed_items}</span>
                </div>
                <div class="stat">
                  <span class="stat-label">남은 시간</span>
                  <span class="stat-value">
                    {formatTime(progress()!.overall_progress.estimated_remaining_time_secs)}
                  </span>
                </div>
              </div>
            </div>

            {/* 크롤링 제어 */}
            <div class="control-panel">
              <h3>크롤링 제어</h3>
              <div class="control-buttons">
                {!isRunning() && (
                  <button class="control-btn start-btn" onClick={startCrawling}>
                    ▶️ 시작
                  </button>
                )}
                
                {isRunning() && progress()!.status === 'running' && (
                  <button class="control-btn pause-btn" onClick={pauseCrawling}>
                    ⏸️ 일시정지
                  </button>
                )}
                
                {isRunning() && progress()!.status === 'paused' && (
                  <button class="control-btn resume-btn" onClick={resumeCrawling}>
                    ▶️ 재개
                  </button>
                )}
                
                {isRunning() && (
                  <button class="control-btn stop-btn" onClick={stopCrawling}>
                    ⏹️ 중단
                  </button>
                )}
              </div>
            </div>
          </div>

          {/* 단계별 진행률 */}
          <div class="stage-progress-section">
            <h3>단계별 진행률</h3>
            <div class="stage-grid">
              <For each={progress()!.stage_progress}>
                {(stage) => (
                  <div class="stage-card">
                    <div class="stage-header">
                      <span class="stage-name">{stage.stage_type}</span>
                      <span 
                        class="stage-status"
                        style={{ "background-color": getStatusColor(stage.status) }}
                      >
                        {stage.status}
                      </span>
                    </div>
                    
                    <div class="stage-progress-bar">
                      <div 
                        class="stage-progress-fill"
                        style={{ 
                          width: `${stage.total_items > 0 ? (stage.processed_items / stage.total_items) * 100 : 0}%` 
                        }}
                      ></div>
                    </div>
                    
                    <div class="stage-details">
                      <div class="stage-detail">
                        <span>진행:</span>
                        <span>{stage.processed_items} / {stage.total_items}</span>
                      </div>
                      <div class="stage-detail">
                        <span>성공률:</span>
                        <span>{stage.success_rate.toFixed(1)}%</span>
                      </div>
                      <div class="stage-detail">
                        <span>평균 처리시간:</span>
                        <span>{stage.avg_processing_time_ms}ms</span>
                      </div>
                      <div class="stage-detail">
                        <span>배치 크기:</span>
                        <span>{stage.current_batch_size}</span>
                      </div>
                    </div>
                  </div>
                )}
              </For>
            </div>
          </div>

          {/* 성능 통계 */}
          <div class="performance-section">
            <h3>성능 통계</h3>
            <div class="performance-grid">
              <div class="perf-card">
                <div class="perf-header">처리량</div>
                <div class="perf-value">{progress()!.performance_stats.items_per_second.toFixed(1)}</div>
                <div class="perf-unit">items/sec</div>
              </div>
              
              <div class="perf-card">
                <div class="perf-header">메모리 사용량</div>
                <div class="perf-value">{formatFileSize(progress()!.performance_stats.memory_usage_mb)}</div>
                <div class="perf-unit">RAM</div>
              </div>
              
              <div class="perf-card">
                <div class="perf-header">활성 연결</div>
                <div class="perf-value">{progress()!.performance_stats.active_connections}</div>
                <div class="perf-unit">connections</div>
              </div>
              
              <div class="perf-card">
                <div class="perf-header">오류율</div>
                <div class="perf-value error-rate">{progress()!.performance_stats.error_rate_percent.toFixed(2)}%</div>
                <div class="perf-unit">errors</div>
              </div>
            </div>
          </div>

          {/* 시간 통계 */}
          <div class="time-section">
            <h3>시간 통계</h3>
            <div class="time-stats">
              <div class="time-stat">
                <span class="time-label">시작 시간:</span>
                <span class="time-value">{new Date(progress()!.time_stats.start_time).toLocaleString()}</span>
              </div>
              <div class="time-stat">
                <span class="time-label">경과 시간:</span>
                <span class="time-value">{formatTime(progress()!.time_stats.elapsed_time_secs)}</span>
              </div>
              <div class="time-stat">
                <span class="time-label">예상 총 시간:</span>
                <span class="time-value">{formatTime(progress()!.time_stats.estimated_total_time_secs)}</span>
              </div>
            </div>
          </div>

          {/* 최근 이벤트 */}
          <div class="events-section">
            <h3>최근 이벤트</h3>
            <div class="events-list">
              <For each={progress()!.recent_events.slice(0, 10)}>
                {(event) => (
                  <div class="event-item">
                    <div class="event-timestamp">
                      {new Date(event.timestamp).toLocaleTimeString()}
                    </div>
                    <div class="event-stage">{event.stage}</div>
                    <div class={`event-type ${event.event_type}`}>
                      {event.event_type}
                    </div>
                    <div class="event-message">{event.message}</div>
                  </div>
                )}
              </For>
            </div>
          </div>

          {/* 구성 정보 */}
          {config() && (
            <div class="config-section">
              <h3>크롤링 구성</h3>
              <div class="config-grid">
                <div class="config-item">
                  <span class="config-label">대상 페이지 수:</span>
                  <span class="config-value">{config()!.target_pages.length}</span>
                </div>
                <div class="config-item">
                  <span class="config-label">동시 실행 제한:</span>
                  <span class="config-value">{config()!.concurrency_limit}</span>
                </div>
                <div class="config-item">
                  <span class="config-label">배치 크기:</span>
                  <span class="config-value">{config()!.batch_size}</span>
                </div>
                <div class="config-item">
                  <span class="config-label">재시도 횟수:</span>
                  <span class="config-value">{config()!.retry_attempts}</span>
                </div>
                <div class="config-item">
                  <span class="config-label">타임아웃:</span>
                  <span class="config-value">{config()!.timeout_secs}초</span>
                </div>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
};
