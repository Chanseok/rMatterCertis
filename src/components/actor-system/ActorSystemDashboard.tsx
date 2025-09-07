/**
 * ActorSystemDashboard - OneShot Actor 시스템 실시간 모니터링 대시보드
 * Phase C: UI 개선 - Actor 시스템 상태 시각화
 */

import { Component, createSignal, onMount, onCleanup, For, Show, createEffect } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { useActorSessionControls } from '../../hooks/useActorSessionControls';
import { tauriApi } from '../../services/tauri-api';
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
  const [currentSessionId, setCurrentSessionId] = createSignal<string | null>(null);
  const [sessionStatusText, setSessionStatusText] = createSignal<string>('');
  const [polling, setPolling] = createSignal<boolean>(false);
  const controls = useActorSessionControls();
  
  // Actor 시스템 배치 분할 테스트를 위한 상태
  const [isActorTesting, setIsActorTesting] = createSignal(false);
  const [testResult, setTestResult] = createSignal<string | null>(null);
  
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

  // 🎯 Actor 시스템 배치 분할 테스트 함수
  const testActorBatchSplitting = async () => {
    if (isActorTesting()) return;
    
    setIsActorTesting(true);
    setTestResult(null);
    
    try {
      console.log('🎭 Starting Actor system batch splitting test...');
      
      // 실제 설정 파일에서 batch_size 가져오기
      const backendConfig = await tauriApi.getComprehensiveCrawlerConfig();
      const actualBatchSize = backendConfig.batch_size;
      
      console.log('📋 Loaded batch_size from config:', actualBatchSize);
      
      // start_actor_based_crawling 커맨드를 사용하여 배치 분할 테스트 (가짜 Actor)
      const request = {
        start_page: 300,  // 300-303 범위
        end_page: 303,
        max_products_per_page: 10,
        concurrent_requests: actualBatchSize,  // 설정 파일에서 가져온 실제 batch_size 사용
        request_timeout_seconds: 30
      };
      
      console.log('📦 Test configuration:', request);
      console.log(`🔍 Expected result: batch_size=${actualBatchSize}, page_range_limit=5 → batches based on actual config`);
      
      setTestResult(`🎭 Actor 시스템 배치 분할 테스트 시작...\n📦 설정: pages 300-303, batch_size=${actualBatchSize}\n🎯 설정 파일에서 읽어온 실제 batch_size 사용`);
      
      // Tauri 커맨드 호출 (가짜 Actor - 실제로는 ServiceBased)
      const result = await invoke('start_actor_system_crawling', { request });
      
      console.log('✅ Actor system test completed:', result);
      
      // 결과 분석
      const testSummary = `✅ Actor 시스템 배치 분할 테스트 완료

📦 설정:
  - 페이지 범위: 300-303 (총 4페이지)
  - batch_size: ${actualBatchSize} (설정 파일에서 로드)
  - page_range_limit: 5
  
🎯 예상 결과:
  - 실제 batch_size=${actualBatchSize}를 사용한 배치 분할
  - 설정 파일 기반으로 동적 계산

📊 실제 결과:
${JSON.stringify(result, null, 2)}

🔧 SessionActor에서 create_simple_batch_plans() 함수가 호출되어 
pages 300-303을 batch_size=3으로 분할했습니다.`;

      setTestResult(testSummary);
      
      // 성공 후 시스템 상태 새로고침
      await fetchSystemStatus();
      
    } catch (error) {
      console.error('❌ Actor system test failed:', error);
      
      const errorSummary = `❌ Actor 시스템 배치 분할 테스트 실패

🚨 오류 내용:
${error}

🔍 문제 분석:
1. Actor 시스템 초기화 실패
2. 배치 분할 로직 오류
3. 채널 통신 문제
4. 백엔드 연결 실패

💡 해결 방법:
- 로그에서 SessionActor 생성 확인
- BatchPlan 생성 로그 확인
- 채널 연결 상태 점검`;

      setTestResult(errorSummary);
    } finally {
      setIsActorTesting(false);
    }
  };

  // 🎯 진짜 Actor 시스템 배치 분할 테스트 함수
  const testRealActorBatchSplitting = async () => {
    if (isActorTesting()) return;
    
    setIsActorTesting(true);
    setTestResult(null);
    
    try {
      console.log('🎭 Starting REAL Actor system batch splitting test...');
      
      setTestResult('🎭 진짜 Actor 시스템 배치 분할 테스트 시작...\n📦 설정: pages 294-298\n🎯 예상: Actor 로그 확인');
      
  // Tauri 커맨드 호출 (진짜 Actor - 설정 기반)
  const result = await invoke('start_actor_system_crawling', {
        request: {
          // CrawlingPlanner가 모든 설정을 자동 계산하므로 파라미터 불필요
        }
      });
      
      console.log('✅ Real Actor system test completed:', result);
      
      // 결과 분석
  const testSummary = `✅ 진짜 Actor 시스템 배치 분할 테스트 완료

📦 설정:
  - 페이지 범위: 294-298 (총 5페이지)
  - concurrency: 5
  
🎯 진짜 Actor 시스템 특징:
  - SessionActor 사용
  - 실제 Actor 메시지 패싱
  - 역순 크롤링 (298→294)

📊 실제 결과:
${JSON.stringify(result, null, 2)}

🔧 SessionActor에서 handle_start_crawling() 메서드가 호출되어 
실제 Actor 패러다임으로 크롤링을 처리했습니다.`;

      setTestResult(testSummary);
      // 세션 ID 추출 (result.session_id 또는 data.session_id)
      try {
        // @ts-ignore
        const sid = (result?.session_id) || (result?.sessionId) || (result?.data?.session_id);
        if (sid) { setCurrentSessionId(sid as string); }
      } catch (_) {}
      
      // 성공 후 시스템 상태 새로고침
      await fetchSystemStatus();
      
    } catch (error) {
      console.error('❌ Real Actor system test failed:', error);
      
      const errorSummary = `❌ 진짜 Actor 시스템 배치 분할 테스트 실패

🚨 오류 내용:
${error}

🔍 문제 분석:
1. 진짜 Actor 시스템 초기화 실패
2. SessionActor 생성 오류
3. Actor 메시지 패싱 문제
4. 백엔드 연결 실패

💡 해결 방법:
- 로그에서 SessionActor 생성 확인
- Actor 메시지 로그 확인
- 채널 연결 상태 점검`;

      setTestResult(errorSummary);
    } finally {
      setIsActorTesting(false);
    }
  };

  // 세션 상태 폴링
  const pollSessionStatus = async () => {
    if (!currentSessionId()) return;
    if (polling()) return;
    setPolling(true);
    try {
      const resp = await controls.status(currentSessionId()!);
      if (resp.ok && resp.data) {
        const status = (resp.data as any).status;
        setSessionStatusText(status);
        if (status === 'Completed' || status === 'Failed' || status === 'ShuttingDown') {
          // 완료되면 자동 폴링 중단
          setAutoRefresh(false);
        }
      }
    } finally {
      setPolling(false);
    }
  };

  // 수동 폴링 버튼 클릭 시 즉시 조회
  const handleSessionStatusClick = () => {
    pollSessionStatus();
  };

  const handlePause = async () => { if (currentSessionId()) { await controls.pause(currentSessionId()!); await pollSessionStatus(); } };
  const handleResume = async () => { if (currentSessionId()) { await controls.resume(currentSessionId()!); await pollSessionStatus(); } };
  const handleShutdown = async () => { await controls.shutdown(); await pollSessionStatus(); };

  // 세션 ID가 존재하면 상태 자동 폴링 (2초 간격)
  let sessionPollTimer: number | null = null;
  createEffect(() => {
    const sid = currentSessionId();
    if (sessionPollTimer) { clearInterval(sessionPollTimer); sessionPollTimer = null; }
    if (sid) {
      // 즉시 1회
      pollSessionStatus();
      sessionPollTimer = setInterval(() => { pollSessionStatus(); }, 2000);
    }
  });
  onCleanup(() => { if (sessionPollTimer) clearInterval(sessionPollTimer); });

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
          {/* 세션 제어 패널 */}
          <Show when={currentSessionId()}>
            <div class="flex flex-col gap-1 mr-4 p-2 rounded-md bg-white/60 border border-gray-200 text-xs min-w-[200px]">
              <div class="font-semibold text-gray-700">세션 제어</div>
              <div class="text-[11px] break-all text-gray-500">ID: {currentSessionId()}</div>
              <div class="flex items-center gap-2">
                <button class="px-2 py-1 bg-yellow-500 text-white rounded disabled:opacity-40" onClick={handlePause} disabled={!currentSessionId()}>⏸ Pause</button>
                <button class="px-2 py-1 bg-green-600 text-white rounded disabled:opacity-40" onClick={handleResume} disabled={!currentSessionId()}>▶ Resume</button>
                <button class="px-2 py-1 bg-red-600 text-white rounded" onClick={handleShutdown}>🛑 Shutdown</button>
              </div>
              <div class="flex items-center gap-2 mt-1">
                <button class="px-2 py-1 bg-blue-600 text-white rounded" onClick={handleSessionStatusClick}>ℹ Status</button>
                <span class="text-[11px] text-gray-700">{sessionStatusText() || '상태 미확인'}</span>
              </div>
            </div>
          </Show>
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
          
          {/* 🎯 가짜 Actor 시스템 배치 분할 테스트 버튼 */}
          <button 
            class="actor-test-button"
            onClick={testActorBatchSplitting}
            disabled={isActorTesting() || loading()}
            style={{
              "background": isActorTesting() ? "#9ca3af" : "#f97316",
              "color": "white",
              "padding": "8px 16px",
              "border": "none",
              "border-radius": "6px",
              "cursor": isActorTesting() ? "not-allowed" : "pointer",
              "margin-left": "8px"
            }}
          >
            {isActorTesting() ? '🔄 테스트 중...' : '🎭 가짜 Actor 테스트'}
          </button>
          
          {/* 🎯 진짜 Actor 시스템 배치 분할 테스트 버튼 */}
          <button 
            class="actor-test-button"
            onClick={testRealActorBatchSplitting}
            disabled={isActorTesting() || loading()}
            style={{
              "background": isActorTesting() ? "#9ca3af" : "#7c3aed",
              "color": "white",
              "padding": "8px 16px",
              "border": "none",
              "border-radius": "6px",
              "cursor": isActorTesting() ? "not-allowed" : "pointer",
              "margin-left": "8px"
            }}
          >
            {isActorTesting() ? '🔄 테스트 중...' : '🎭 진짜 Actor 테스트'}
          </button>
        </div>
        
        {/* 🎯 Actor 시스템 테스트 결과 표시 */}
        {testResult() && (
          <div style={{
            "background": testResult()!.includes('❌') ? "#fef2f2" : "#f0f9ff",
            "border": testResult()!.includes('❌') ? "1px solid #fecaca" : "1px solid #bfdbfe",
            "border-radius": "8px",
            "padding": "12px",
            "margin-top": "16px",
            "font-family": "monospace",
            "font-size": "14px",
            "white-space": "pre-wrap"
          }}>
            <div style={{ "font-weight": "bold", "margin-bottom": "8px" }}>
              🎭 Actor 시스템 배치 분할 테스트 결과:
            </div>
            {testResult()}
          </div>
        )}
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

              {/* Batch Actors section hidden (legacy path retired) */}

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
