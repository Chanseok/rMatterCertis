import { createSignal, createEffect, For, Show } from "solid-js";
import { CrawlingService } from "../services/crawlingService";
import type { CrawlingStats, CrawlingSessionState } from "../types/crawling";

interface CrawlingDashboardProps {
  onStartCrawling: () => void;
}

export function CrawlingDashboard(props: CrawlingDashboardProps) {
  const [stats, setStats] = createSignal<CrawlingStats | null>(null);
  const [activeSessions, setActiveSessions] = createSignal<CrawlingSessionState[]>([]);
  const [sessionHistory, setSessionHistory] = createSignal<CrawlingSessionState[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  // Auto-refresh every 5 seconds
  let refreshInterval: number;

  const loadDashboardData = async () => {
    try {
      setLoading(true);
      setError(null);
      
      const [statsData, activeData, historyData] = await Promise.all([
        CrawlingService.getCrawlingStats(),
        CrawlingService.getActiveSessions(),
        CrawlingService.getSessionHistory()
      ]);
      
      setStats(statsData);
      setActiveSessions(activeData);
      setSessionHistory(historyData.slice(0, 10)); // Latest 10 sessions
    } catch (err) {
      console.error("Failed to load dashboard data:", err);
      setError(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setLoading(false);
    }
  };

  const startAutoRefresh = () => {
    refreshInterval = setInterval(loadDashboardData, 5000);
  };

  const stopAutoRefresh = () => {
    if (refreshInterval) {
      clearInterval(refreshInterval);
    }
  };

  createEffect(() => {
    loadDashboardData();
    startAutoRefresh();
    
    // Cleanup on unmount
    return () => stopAutoRefresh();
  });

  const handleStopSession = async (sessionId: string) => {
    try {
      await CrawlingService.stopCrawling(sessionId);
      await loadDashboardData(); // Refresh data
    } catch (err) {
      console.error("Failed to stop session:", err);
      setError(`Failed to stop session: ${err}`);
    }
  };

  const handlePauseSession = async (sessionId: string) => {
    try {
      await CrawlingService.pauseCrawling(sessionId);
      await loadDashboardData(); // Refresh data
    } catch (err) {
      console.error("Failed to pause session:", err);
      setError(`Failed to pause session: ${err}`);
    }
  };

  const handleResumeSession = async (sessionId: string) => {
    try {
      await CrawlingService.resumeCrawling(sessionId);
      await loadDashboardData(); // Refresh data
    } catch (err) {
      console.error("Failed to resume session:", err);
      setError(`Failed to resume session: ${err}`);
    }
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case "Running": return "#4CAF50";
      case "Paused": return "#FF9800";
      case "Stopped": return "#F44336";
      case "Completed": return "#2196F3";
      case "Failed": return "#F44336";
      default: return "#9E9E9E";
    }
  };

  const formatProgress = (current: number, max: number) => {
    const percentage = max > 0 ? Math.round((current / max) * 100) : 0;
    return `${current}/${max} (${percentage}%)`;
  };

  return (
    <div class="crawling-dashboard">
      <div class="dashboard-header">
        <h2>크롤링 대시보드</h2>
        <div class="dashboard-actions">
          <button 
            class="btn btn-primary" 
            onClick={props.onStartCrawling}
          >
            새 크롤링 시작
          </button>
          <button 
            class="btn btn-secondary" 
            onClick={loadDashboardData}
            disabled={loading()}
          >
            {loading() ? "새로고침 중..." : "새로고침"}
          </button>
        </div>
      </div>

      <Show when={error()}>
        <div class="error-message">
          ❌ {error()}
        </div>
      </Show>

      {/* Statistics Cards */}
      <Show when={stats()}>
        <div class="stats-grid">
          <div class="stat-card">
            <h3>전체 세션</h3>
            <div class="stat-value">{stats()?.total_sessions || 0}</div>
          </div>
          <div class="stat-card">
            <h3>활성 세션</h3>
            <div class="stat-value">{stats()?.active_sessions || 0}</div>
          </div>
          <div class="stat-card">
            <h3>완료된 세션</h3>
            <div class="stat-value">{stats()?.completed_sessions || 0}</div>
          </div>
          <div class="stat-card">
            <h3>크롤링된 페이지</h3>
            <div class="stat-value">{stats()?.total_pages_crawled || 0}</div>
          </div>
          <div class="stat-card">
            <h3>평균 성공률</h3>
            <div class="stat-value">{Math.round((stats()?.average_success_rate || 0) * 100)}%</div>
          </div>
        </div>
      </Show>

      {/* Active Sessions */}
      <div class="dashboard-section">
        <h3>활성 크롤링 세션</h3>
        <Show 
          when={activeSessions().length > 0} 
          fallback={<p class="no-data">활성 세션이 없습니다.</p>}
        >
          <div class="sessions-grid">
            <For each={activeSessions()}>
              {(session) => (
                <div class="session-card active">
                  <div class="session-header">
                    <span 
                      class="status-badge" 
                      style={{ "background-color": getStatusColor(session.status) }}
                    >
                      {session.status}
                    </span>
                    <span class="session-id">#{session.session_id.slice(0, 8)}</span>
                  </div>
                  
                  <div class="session-info">
                    <p><strong>단계:</strong> {session.stage}</p>
                    <p><strong>진행률:</strong> {formatProgress(session.pages_crawled, session.max_pages)}</p>
                    <Show when={session.current_url}>
                      <p><strong>현재 URL:</strong> 
                        <span class="current-url">{session.current_url}</span>
                      </p>
                    </Show>
                    <p><strong>시작 시간:</strong> {new Date(session.start_time).toLocaleString()}</p>
                    <Show when={session.errors.length > 0}>
                      <p><strong>오류:</strong> <span class="error-count">{session.errors.length}개</span></p>
                    </Show>
                  </div>

                  <div class="session-actions">
                    <Show when={session.status === "Running"}>
                      <button 
                        class="btn btn-warning btn-sm"
                        onClick={() => handlePauseSession(session.session_id)}
                      >
                        일시정지
                      </button>
                    </Show>
                    <Show when={session.status === "Paused"}>
                      <button 
                        class="btn btn-success btn-sm"
                        onClick={() => handleResumeSession(session.session_id)}
                      >
                        재개
                      </button>
                    </Show>
                    <button 
                      class="btn btn-danger btn-sm"
                      onClick={() => handleStopSession(session.session_id)}
                    >
                      중지
                    </button>
                  </div>
                </div>
              )}
            </For>
          </div>
        </Show>
      </div>

      {/* Session History */}
      <div class="dashboard-section">
        <h3>최근 세션 기록</h3>
        <Show 
          when={sessionHistory().length > 0} 
          fallback={<p class="no-data">세션 기록이 없습니다.</p>}
        >
          <div class="history-table">
            <table>
              <thead>
                <tr>
                  <th>세션 ID</th>
                  <th>상태</th>
                  <th>단계</th>
                  <th>진행률</th>
                  <th>시작 시간</th>
                  <th>오류</th>
                </tr>
              </thead>
              <tbody>
                <For each={sessionHistory()}>
                  {(session) => (
                    <tr>
                      <td>#{session.session_id.slice(0, 8)}</td>
                      <td>
                        <span 
                          class="status-badge" 
                          style={{ "background-color": getStatusColor(session.status) }}
                        >
                          {session.status}
                        </span>
                      </td>
                      <td>{session.stage}</td>
                      <td>{formatProgress(session.pages_crawled, session.max_pages)}</td>
                      <td>{new Date(session.start_time).toLocaleString()}</td>
                      <td>{session.errors.length}</td>
                    </tr>
                  )}
                </For>
              </tbody>
            </table>
          </div>
        </Show>
      </div>
    </div>
  );
}
