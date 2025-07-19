/**
 * ActorSystemDashboard - OneShot Actor 시스템 실시간 모니터링 대시보드
 * Phase C: UI 개선 - Actor 시스템 상태 시각화
 */

import { Component, createSignal, onMount, onCleanup, For } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import './ActorSystemDashboard.css';

// Actor 시스템 상태 타입 정의
interface ActorSystemStatus {
  session_actor: {
    id: string;
    status: 'idle' | 'running' | 'completed' | 'error';
    active_batches: number;
    total_processed: number;
    uptime_seconds: number;
  };
  batch_actors: Array<{
    id: string;
    status: 'idle' | 'processing' | 'waiting' | 'completed' | 'error';
    current_stage: string | null;
    processed_items: number;
    success_rate: number;
    error_count: number;
  }>;
  stage_actors: Array<{
    id: string;
    stage_type: string;
    status: 'idle' | 'executing' | 'completed' | 'error';
    current_batch_size: number;
    avg_processing_time_ms: number;
    total_executions: number;
  }>;
  channel_status: {
    control_channel_pending: number;
    event_channel_pending: number;
    oneshot_channels_active: number;
  };
  performance_metrics: {
    memory_usage_mb: number;
    cpu_usage_percent: number;
    throughput_items_per_second: number;
    error_rate_percent: number;
  };
}

interface SystemHealth {
  overall_status: 'healthy' | 'warning' | 'critical' | 'offline';
  health_score: number; // 0-100
  issues: string[];
  recommendations: string[];
}

export const ActorSystemDashboard: Component = () => {
  const [systemStatus, setSystemStatus] = createSignal<ActorSystemStatus | null>(null);
  const [systemHealth, setSystemHealth] = createSignal<SystemHealth | null>(null);
  const [loading, setLoading] = createSignal(true);
  const [autoRefresh, setAutoRefresh] = createSignal(true);
  const [refreshInterval, setRefreshInterval] = createSignal(2000); // 2초
  
  let refreshTimer: number | null = null;

  // 시스템 상태 조회
  const fetchSystemStatus = async () => {
    try {
      const status = await invoke<ActorSystemStatus>('get_actor_system_status');
      const health = await invoke<SystemHealth>('get_actor_system_health');
      
      setSystemStatus(status);
      setSystemHealth(health);
    } catch (error) {
      console.error('Failed to fetch actor system status:', error);
      setSystemHealth({
        overall_status: 'offline',
        health_score: 0,
        issues: [`시스템 연결 실패: ${error}`],
        recommendations: ['백엔드 서비스 상태를 확인하세요']
      });
    }
  };

  // 자동 새로고침 설정
  const setupAutoRefresh = () => {
    if (refreshTimer) clearInterval(refreshTimer);
    
    if (autoRefresh()) {
      refreshTimer = setInterval(() => {
        fetchSystemStatus();
      }, refreshInterval());
    }
  };

  // 컴포넌트 마운트/언마운트 처리
  onMount(async () => {
    setLoading(true);
    await fetchSystemStatus();
    setLoading(false);
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

  // 새로고침 간격 변경
  const updateRefreshInterval = (interval: number) => {
    setRefreshInterval(interval);
    setupAutoRefresh();
  };

  // 상태에 따른 색상 반환
  const getStatusColor = (status: string): string => {
    switch (status) {
      case 'running': case 'executing': case 'processing': return '#10B981'; // Green
      case 'idle': case 'waiting': return '#6B7280'; // Gray
      case 'completed': return '#3B82F6'; // Blue
      case 'error': return '#EF4444'; // Red
      case 'healthy': return '#10B981'; // Green
      case 'warning': return '#F59E0B'; // Yellow
      case 'critical': return '#EF4444'; // Red
      case 'offline': return '#6B7280'; // Gray
      default: return '#6B7280';
    }
  };

  // 건강 점수에 따른 색상
  const getHealthColor = (score: number): string => {
    if (score >= 80) return '#10B981'; // Green
    if (score >= 60) return '#F59E0B'; // Yellow
    if (score >= 40) return '#FB923C'; // Orange
    return '#EF4444'; // Red
  };

  return (
    <div class="actor-system-dashboard">
      {/* 헤더 */}
      <div class="dashboard-header">
        <h2 class="dashboard-title">
          <span class="icon">🎭</span>
          OneShot Actor System Dashboard
        </h2>
        
        <div class="dashboard-controls">
          <div class="refresh-controls">
            <button 
              class={`refresh-toggle ${autoRefresh() ? 'active' : ''}`}
              onClick={toggleAutoRefresh}
            >
              {autoRefresh() ? '⏸️ 자동 새로고침' : '▶️ 자동 새로고침'}
            </button>
            
            <select 
              value={refreshInterval()} 
              onChange={(e) => updateRefreshInterval(Number(e.target.value))}
              disabled={!autoRefresh()}
            >
              <option value={1000}>1초</option>
              <option value={2000}>2초</option>
              <option value={5000}>5초</option>
              <option value={10000}>10초</option>
            </select>
          </div>
          
          <button 
            class="manual-refresh"
            onClick={fetchSystemStatus}
            disabled={loading()}
          >
            🔄 새로고침
          </button>
        </div>
      </div>

      {loading() && (
        <div class="loading-state">
          <div class="spinner"></div>
          <p>Actor 시스템 상태 로딩 중...</p>
        </div>
      )}

      {!loading() && systemHealth() && (
        <>
          {/* 시스템 건강 상태 */}
          <div class="health-overview">
            <div class="health-card">
              <div class="health-indicator">
                <div 
                  class="health-circle"
                  style={{ 
                    background: `conic-gradient(${getHealthColor(systemHealth()!.health_score)} ${systemHealth()!.health_score * 3.6}deg, #e5e7eb 0deg)` 
                  }}
                >
                  <div class="health-score">
                    {systemHealth()!.health_score}
                  </div>
                </div>
                <div class="health-status">
                  <span 
                    class="status-badge"
                    style={{ "background-color": getStatusColor(systemHealth()!.overall_status) }}
                  >
                    {systemHealth()!.overall_status.toUpperCase()}
                  </span>
                </div>
              </div>
              
              {systemHealth()!.issues.length > 0 && (
                <div class="health-issues">
                  <h4>⚠️ 문제사항</h4>
                  <For each={systemHealth()!.issues}>
                    {(issue) => <div class="issue-item">{issue}</div>}
                  </For>
                </div>
              )}
              
              {systemHealth()!.recommendations.length > 0 && (
                <div class="health-recommendations">
                  <h4>💡 권장사항</h4>
                  <For each={systemHealth()!.recommendations}>
                    {(rec) => <div class="recommendation-item">{rec}</div>}
                  </For>
                </div>
              )}
            </div>
          </div>

          {systemStatus() && (
            <>
              {/* Session Actor 상태 */}
              <div class="actor-section">
                <h3>🎯 Session Actor</h3>
                <div class="session-actor-card">
                  <div class="actor-status">
                    <span 
                      class="status-indicator"
                      style={{ "background-color": getStatusColor(systemStatus()!.session_actor.status) }}
                    ></span>
                    <span class="actor-id">{systemStatus()!.session_actor.id}</span>
                    <span class="status-text">{systemStatus()!.session_actor.status}</span>
                  </div>
                  
                  <div class="session-metrics">
                    <div class="metric">
                      <span class="metric-label">활성 배치</span>
                      <span class="metric-value">{systemStatus()!.session_actor.active_batches}</span>
                    </div>
                    <div class="metric">
                      <span class="metric-label">총 처리</span>
                      <span class="metric-value">{systemStatus()!.session_actor.total_processed}</span>
                    </div>
                    <div class="metric">
                      <span class="metric-label">가동시간</span>
                      <span class="metric-value">{Math.floor(systemStatus()!.session_actor.uptime_seconds / 60)}분</span>
                    </div>
                  </div>
                </div>
              </div>

              {/* Batch Actors 상태 */}
              <div class="actor-section">
                <h3>📦 Batch Actors ({systemStatus()!.batch_actors.length})</h3>
                <div class="batch-actors-grid">
                  <For each={systemStatus()!.batch_actors}>
                    {(batchActor) => (
                      <div class="batch-actor-card">
                        <div class="actor-header">
                          <span 
                            class="status-indicator"
                            style={{ "background-color": getStatusColor(batchActor.status) }}
                          ></span>
                          <span class="actor-id">{batchActor.id}</span>
                        </div>
                        
                        <div class="actor-details">
                          <div class="detail-row">
                            <span>상태:</span>
                            <span>{batchActor.status}</span>
                          </div>
                          {batchActor.current_stage && (
                            <div class="detail-row">
                              <span>현재 단계:</span>
                              <span>{batchActor.current_stage}</span>
                            </div>
                          )}
                          <div class="detail-row">
                            <span>처리 항목:</span>
                            <span>{batchActor.processed_items}</span>
                          </div>
                          <div class="detail-row">
                            <span>성공률:</span>
                            <span>{batchActor.success_rate.toFixed(1)}%</span>
                          </div>
                          <div class="detail-row">
                            <span>오류 수:</span>
                            <span class={batchActor.error_count > 0 ? 'error-count' : ''}>
                              {batchActor.error_count}
                            </span>
                          </div>
                        </div>
                      </div>
                    )}
                  </For>
                </div>
              </div>

              {/* Stage Actors 상태 */}
              <div class="actor-section">
                <h3>⚙️ Stage Actors ({systemStatus()!.stage_actors.length})</h3>
                <div class="stage-actors-grid">
                  <For each={systemStatus()!.stage_actors}>
                    {(stageActor) => (
                      <div class="stage-actor-card">
                        <div class="actor-header">
                          <span 
                            class="status-indicator"
                            style={{ "background-color": getStatusColor(stageActor.status) }}
                          ></span>
                          <span class="stage-type">{stageActor.stage_type}</span>
                        </div>
                        
                        <div class="actor-details">
                          <div class="detail-row">
                            <span>상태:</span>
                            <span>{stageActor.status}</span>
                          </div>
                          <div class="detail-row">
                            <span>배치 크기:</span>
                            <span>{stageActor.current_batch_size}</span>
                          </div>
                          <div class="detail-row">
                            <span>평균 처리시간:</span>
                            <span>{stageActor.avg_processing_time_ms}ms</span>
                          </div>
                          <div class="detail-row">
                            <span>총 실행:</span>
                            <span>{stageActor.total_executions}</span>
                          </div>
                        </div>
                      </div>
                    )}
                  </For>
                </div>
              </div>

              {/* 채널 상태 */}
              <div class="actor-section">
                <h3>🔀 Channel Status</h3>
                <div class="channel-status-grid">
                  <div class="channel-card">
                    <div class="channel-header">Control Channel</div>
                    <div class="channel-value">{systemStatus()!.channel_status.control_channel_pending}</div>
                    <div class="channel-label">대기 중인 메시지</div>
                  </div>
                  
                  <div class="channel-card">
                    <div class="channel-header">Event Channel</div>
                    <div class="channel-value">{systemStatus()!.channel_status.event_channel_pending}</div>
                    <div class="channel-label">대기 중인 이벤트</div>
                  </div>
                  
                  <div class="channel-card">
                    <div class="channel-header">OneShot Channels</div>
                    <div class="channel-value">{systemStatus()!.channel_status.oneshot_channels_active}</div>
                    <div class="channel-label">활성 OneShot</div>
                  </div>
                </div>
              </div>

              {/* 성능 메트릭 */}
              <div class="actor-section">
                <h3>📊 Performance Metrics</h3>
                <div class="performance-grid">
                  <div class="metric-card">
                    <div class="metric-header">메모리 사용량</div>
                    <div class="metric-value">{systemStatus()!.performance_metrics.memory_usage_mb.toFixed(1)} MB</div>
                  </div>
                  
                  <div class="metric-card">
                    <div class="metric-header">CPU 사용량</div>
                    <div class="metric-value">{systemStatus()!.performance_metrics.cpu_usage_percent.toFixed(1)}%</div>
                  </div>
                  
                  <div class="metric-card">
                    <div class="metric-header">처리량</div>
                    <div class="metric-value">{systemStatus()!.performance_metrics.throughput_items_per_second.toFixed(1)} items/s</div>
                  </div>
                  
                  <div class="metric-card">
                    <div class="metric-header">오류율</div>
                    <div class="metric-value error-rate">{systemStatus()!.performance_metrics.error_rate_percent.toFixed(2)}%</div>
                  </div>
                </div>
              </div>
            </>
          )}
        </>
      )}
    </div>
  );
};
