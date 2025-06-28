// Crawling store for managing crawling sessions and progress
import { createStore } from 'solid-js/store';
import { createSignal } from 'solid-js';
import type { 
  SessionStatusDto, 
  StartCrawlingDto,
  CrawlingStatus
} from '../../types/domain';
import { apiAdapter, safeApiCall } from '../../platform/tauri';

// ============================================================================
// Crawling Store State
// ============================================================================

interface CrawlingProgress {
  sessionId: string | null;
  status: CrawlingStatus;
  progress: number;
  currentStep: string;
  startedAt: string | null;
  lastUpdated: string | null;
  error: string | null;
}

interface CrawlingState {
  currentSession: CrawlingProgress | null;
  activeSessions: SessionStatusDto[];
  sessionHistory: SessionStatusDto[];
  loading: boolean;
  error: string | null;
  lastRefresh: Date | null;
}

interface CrawlingActions {
  // Session Management
  startCrawling: (dto: StartCrawlingDto) => Promise<boolean>;
  stopCrawling: (sessionId: string) => Promise<boolean>;
  pauseCrawling: (sessionId: string) => Promise<boolean>;
  resumeCrawling: (sessionId: string) => Promise<boolean>;
  
  // Status Updates
  refreshStatus: (sessionId?: string) => Promise<void>;
  loadActiveSessions: () => Promise<void>;
  loadSessionHistory: (limit?: number) => Promise<void>;
  
  // UI State
  clearError: () => void;
  setCurrentSession: (sessionId: string | null) => void;
}

// ============================================================================
// Store Creation
// ============================================================================

export function createCrawlingStore() {
  // Reactive state
  const [state, setState] = createStore<CrawlingState>({
    currentSession: null,
    activeSessions: [],
    sessionHistory: [],
    loading: false,
    error: null,
    lastRefresh: null,
  });

  // Loading signals for specific operations
  const [isStarting, setIsStarting] = createSignal(false);
  const [isStopping, setIsStopping] = createSignal(false);
  const [isPausing, setIsPausing] = createSignal(false);
  const [isResuming, setIsResuming] = createSignal(false);

  // Auto-refresh interval signal
  const [refreshInterval, setRefreshInterval] = createSignal<number | null>(null);

  // ========================================================================
  // Helper Functions
  // ========================================================================

  const setLoading = (loading: boolean) => {
    setState('loading', loading);
  };

  const setError = (error: string | null) => {
    setState('error', error);
  };

  const updateCurrentSession = (sessionData: SessionStatusDto) => {
    const progress: CrawlingProgress = {
      sessionId: sessionData.session_id,
      status: sessionData.status as CrawlingStatus,
      progress: sessionData.progress,
      currentStep: sessionData.current_step,
      startedAt: sessionData.started_at,
      lastUpdated: sessionData.last_updated,
      error: null,
    };

    setState('currentSession', progress);
    setState('lastRefresh', new Date());
  };

  // ========================================================================
  // Session Management
  // ========================================================================

  const startCrawling = async (dto: StartCrawlingDto): Promise<boolean> => {
    setIsStarting(true);
    setError(null);

    try {
      const result = await safeApiCall(() => apiAdapter.startCrawling(dto));
      
      if (result.error) {
        setError(result.error.message);
        return false;
      }

      if (result.data) {
        updateCurrentSession(result.data);
        
        // Start auto-refresh for active session
        startAutoRefresh();
        
        return true;
      }

      return false;
    } catch (error) {
      setError('Failed to start crawling');
      return false;
    } finally {
      setIsStarting(false);
    }
  };

  const stopCrawling = async (sessionId: string): Promise<boolean> => {
    setIsStopping(true);
    setError(null);

    try {
      const result = await safeApiCall(() => apiAdapter.stopCrawling(sessionId));
      
      if (result.error) {
        setError(result.error.message);
        return false;
      }

      if (result.data) {
        updateCurrentSession(result.data);
        
        // Stop auto-refresh when session ends
        stopAutoRefresh();
        
        return true;
      }

      return false;
    } catch (error) {
      setError('Failed to stop crawling');
      return false;
    } finally {
      setIsStopping(false);
    }
  };

  const pauseCrawling = async (sessionId: string): Promise<boolean> => {
    setIsPausing(true);
    setError(null);

    try {
      const result = await safeApiCall(() => apiAdapter.pauseCrawling(sessionId));
      
      if (result.error) {
        setError(result.error.message);
        return false;
      }

      if (result.data) {
        updateCurrentSession(result.data);
        return true;
      }

      return false;
    } catch (error) {
      setError('Failed to pause crawling');
      return false;
    } finally {
      setIsPausing(false);
    }
  };

  const resumeCrawling = async (sessionId: string): Promise<boolean> => {
    setIsResuming(true);
    setError(null);

    try {
      const result = await safeApiCall(() => apiAdapter.resumeCrawling(sessionId));
      
      if (result.error) {
        setError(result.error.message);
        return false;
      }

      if (result.data) {
        updateCurrentSession(result.data);
        return true;
      }

      return false;
    } catch (error) {
      setError('Failed to resume crawling');
      return false;
    } finally {
      setIsResuming(false);
    }
  };

  // ========================================================================
  // Status Updates
  // ========================================================================

  const refreshStatus = async (sessionId?: string): Promise<void> => {
    if (!sessionId && !state.currentSession?.sessionId) return;

    const targetSessionId = sessionId || state.currentSession!.sessionId!;
    
    try {
      const result = await safeApiCall(() => apiAdapter.getCrawlingStatus(targetSessionId));
      
      if (result.error) {
        setError(result.error.message);
        return;
      }

      if (result.data) {
        updateCurrentSession(result.data);
      }
    } catch (error) {
      setError('Failed to refresh status');
    }
  };

  const loadActiveSessions = async (): Promise<void> => {
    setLoading(true);
    setError(null);

    try {
      const result = await safeApiCall(() => apiAdapter.getActiveCrawlingSessions());
      
      if (result.error) {
        setError(result.error.message);
        return;
      }

      if (result.data) {
        setState('activeSessions', result.data);
        setState('lastRefresh', new Date());
      }
    } catch (error) {
      setError('Failed to load active sessions');
    } finally {
      setLoading(false);
    }
  };

  const loadSessionHistory = async (limit = 50): Promise<void> => {
    setLoading(true);
    setError(null);

    try {
      const result = await safeApiCall(() => apiAdapter.getCrawlingSessionHistory(limit));
      
      if (result.error) {
        setError(result.error.message);
        return;
      }

      if (result.data) {
        setState('sessionHistory', result.data);
        setState('lastRefresh', new Date());
      }
    } catch (error) {
      setError('Failed to load session history');
    } finally {
      setLoading(false);
    }
  };

  // ========================================================================
  // Auto-refresh Management
  // ========================================================================

  const startAutoRefresh = () => {
    // Clear existing interval
    stopAutoRefresh();
    
    // Start new interval (refresh every 2 seconds during active crawling)
    const intervalId = setInterval(() => {
      if (state.currentSession?.sessionId) {
        refreshStatus(state.currentSession.sessionId);
      }
    }, 2000);
    
    setRefreshInterval(intervalId as unknown as number);
  };

  const stopAutoRefresh = () => {
    const intervalId = refreshInterval();
    if (intervalId) {
      clearInterval(intervalId);
      setRefreshInterval(null);
    }
  };

  // ========================================================================
  // UI State Management
  // ========================================================================

  const clearError = () => {
    setError(null);
  };

  const setCurrentSession = (sessionId: string | null) => {
    if (sessionId) {
      refreshStatus(sessionId);
    } else {
      setState('currentSession', null);
      stopAutoRefresh();
    }
  };

  // ========================================================================
  // Return Store API
  // ========================================================================

  const actions: CrawlingActions = {
    startCrawling,
    stopCrawling,
    pauseCrawling,
    resumeCrawling,
    refreshStatus,
    loadActiveSessions,
    loadSessionHistory,
    clearError,
    setCurrentSession,
  };

  return {
    // State (read-only)
    state,
    
    // Loading indicators
    isStarting,
    isStopping,
    isPausing,
    isResuming,
    
    // Actions
    ...actions,
    
    // Auto-refresh control
    startAutoRefresh,
    stopAutoRefresh,
    
    // Computed getters
    get isActive() {
      return state.currentSession?.status === 'running';
    },
    
    get isPaused() {
      return state.currentSession?.status === 'paused';
    },
    
    get isCompleted() {
      return state.currentSession?.status === 'completed';
    },
    
    get hasError() {
      return !!state.error || state.currentSession?.status === 'error';
    },
    
    get progressPercentage() {
      return state.currentSession?.progress || 0;
    },
    
    get isOperationPending() {
      return isStarting() || isStopping() || isPausing() || isResuming();
    },
    
    get canStart() {
      return !state.currentSession || ['completed', 'error'].includes(state.currentSession.status);
    },
    
    get canStop() {
      return state.currentSession && ['running', 'paused'].includes(state.currentSession.status);
    },
    
    get canPause() {
      return state.currentSession?.status === 'running';
    },
    
    get canResume() {
      return state.currentSession?.status === 'paused';
    },
  };
}

// ============================================================================
// Store Type Export
// ============================================================================

export type CrawlingStore = ReturnType<typeof createCrawlingStore>;
