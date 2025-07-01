/**
 * Realtime Manager - 실시간 이벤트 관리
 * 
 * 이 모듈은 백엔드 이벤트와 프론트엔드 스토어 간의 동기화를 담당하며,
 * 실시간 업데이트의 중앙 허브 역할을 합니다.
 */

import { createSignal, onCleanup } from 'solid-js';
import { tauriApi } from './tauri-api';
import { crawlerStore } from '../stores/crawlerStore';
import { databaseStore } from '../stores/databaseStore';
import { uiStore } from '../stores/uiStore';
import type {
  CrawlingProgress,
  CrawlingTaskStatus,
  DatabaseStats,
  CrawlingResult
} from '../types/crawling';

// 연결 상태 타입
interface ConnectionState {
  isConnected: boolean;
  isInitializing: boolean;
  lastError: string | null;
  retryCount: number;
  maxRetries: number;
}

// 이벤트 통계
interface EventStats {
  totalEvents: number;
  progressEvents: number;
  taskEvents: number;
  errorEvents: number;
  dbEvents: number;
  lastEventTime: Date | null;
}

/**
 * 실시간 이벤트 관리자 클래스
 */
class RealtimeManager {
  private subscriptions: Map<string, () => void> = new Map();
  private connectionStateSignal = createSignal<ConnectionState>({
    isConnected: false,
    isInitializing: false,
    lastError: null,
    retryCount: 0,
    maxRetries: 3,
  });
  
  private eventStatsSignal = createSignal<EventStats>({
    totalEvents: 0,
    progressEvents: 0,
    taskEvents: 0,
    errorEvents: 0,
    dbEvents: 0,
    lastEventTime: null,
  });

  private initializationPromise: Promise<void> | null = null;
  private retryTimeout: number | null = null;

  // =========================================================================
  // 상태 접근자
  // =========================================================================

  get connectionState() {
    return this.connectionStateSignal[0];
  }

  private setConnectionState(newState: Partial<ConnectionState>) {
    const currentState = this.connectionStateSignal[0]();
    this.connectionStateSignal[1]({ ...currentState, ...newState });
  }

  get eventStats() {
    return this.eventStatsSignal[0];
  }

  private setEventStats(newStats: Partial<EventStats>) {
    const currentStats = this.eventStatsSignal[0]();
    this.eventStatsSignal[1]({ ...currentStats, ...newStats });
  }

  get isConnected() {
    return this.connectionState().isConnected;
  }

  get isInitializing() {
    return this.connectionState().isInitializing;
  }

  get lastError() {
    return this.connectionState().lastError;
  }

  get stats() {
    return this.eventStats();
  }

  get canRetry() {
    const state = this.connectionState();
    return !state.isInitializing && state.retryCount < state.maxRetries;
  }

  // =========================================================================
  // 초기화 및 연결 관리
  // =========================================================================

  /**
   * 실시간 매니저 초기화
   */
  async initialize(): Promise<void> {
    if (this.initializationPromise) {
      return this.initializationPromise;
    }

    this.initializationPromise = this._initialize();
    return this.initializationPromise;
  }

  private async _initialize(): Promise<void> {
    const state = this.connectionState();
    
    if (state.isInitializing) {
      console.log('⚠️ 실시간 매니저가 이미 초기화 중입니다');
      return;
    }

    this.setConnectionState({
      ...state,
      isInitializing: true,
      lastError: null,
    });

    try {
      console.log('🔧 실시간 이벤트 매니저 초기화 중...');

      // 스토어들 초기화
      await Promise.all([
        crawlerStore.initialize(),
        databaseStore.initialize(),
      ]);

      // 이벤트 구독 설정
      await this.subscribeToAllEvents();

      // 초기 상태 동기화
      await this.syncInitialState();

      this.setConnectionState({
        isConnected: true,
        isInitializing: false,
        lastError: null,
        retryCount: 0,
        maxRetries: 3,
      });

      uiStore.showSuccess('실시간 연결이 설정되었습니다', '연결 성공');
      console.log('✅ 실시간 이벤트 매니저 초기화 완료');

    } catch (error) {
      const errorMessage = `실시간 매니저 초기화 실패: ${error}`;
      console.error('❌ 실시간 매니저 초기화 실패:', error);

      this.setConnectionState({
        isConnected: false,
        isInitializing: false,
        lastError: errorMessage,
        retryCount: state.retryCount + 1,
        maxRetries: 3,
      });

      uiStore.showError(errorMessage, '연결 실패');

      // 자동 재시도
      this.scheduleRetry();
      throw error;
    }
  }

  /**
   * 연결 재시도
   */
  async retry(): Promise<void> {
    if (!this.canRetry) {
      throw new Error('재시도 한도를 초과했습니다');
    }

    this.initializationPromise = null;
    return this.initialize();
  }

  /**
   * 자동 재시도 스케줄링
   */
  private scheduleRetry(): void {
    const state = this.connectionState();
    
    if (!this.canRetry) {
      return;
    }

    const delay = Math.min(1000 * Math.pow(2, state.retryCount), 10000); // 지수 백오프, 최대 10초
    
    console.log(`🔄 ${delay}ms 후 재시도 예정... (${state.retryCount + 1}/${state.maxRetries})`);
    
    this.retryTimeout = window.setTimeout(() => {
      this.retry().catch(console.error);
    }, delay);
  }

  // =========================================================================
  // 이벤트 구독 관리
  // =========================================================================

  /**
   * 모든 이벤트 구독 설정
   */
  private async subscribeToAllEvents(): Promise<void> {
    const subscriptions = await Promise.all([
      this.subscribeToProgressEvents(),
      this.subscribeToTaskEvents(),
      this.subscribeToStageEvents(),
      this.subscribeToErrorEvents(),
      this.subscribeToDatabaseEvents(),
      this.subscribeToCompletionEvents(),
    ]);

    // 구독 해제 함수들 저장
    subscriptions.forEach((unsub, index) => {
      this.subscriptions.set(`subscription_${index}`, unsub);
    });

    console.log('📡 모든 실시간 이벤트 구독 완료');
  }

  /**
   * 크롤링 진행 상황 이벤트 구독
   */
  private async subscribeToProgressEvents(): Promise<() => void> {
    return tauriApi.subscribeToProgress((progress: CrawlingProgress) => {
      console.log('📊 진행 상황 업데이트:', progress);
      
      crawlerStore.setProgress(progress);
      this.updateEventStats('progress');
    });
  }

  /**
   * 작업 상태 이벤트 구독
   */
  private async subscribeToTaskEvents(): Promise<() => void> {
    return tauriApi.subscribeToTaskStatus((taskStatus: CrawlingTaskStatus) => {
      console.log('📋 작업 상태 업데이트:', taskStatus);
      
      crawlerStore.updateTaskStatus(taskStatus);
      this.updateEventStats('task');
    });
  }

  /**
   * 스테이지 변경 이벤트 구독
   */
  private async subscribeToStageEvents(): Promise<() => void> {
    return tauriApi.subscribeToStageChange((data) => {
      console.log(`🔄 스테이지 변경: ${data.from} → ${data.to}`);
      
      uiStore.showInfo(`${data.message}`, '단계 변경');
      this.updateEventStats('stage');
    });
  }

  /**
   * 에러 이벤트 구독
   */
  private async subscribeToErrorEvents(): Promise<() => void> {
    return tauriApi.subscribeToErrors((error) => {
      console.error('❌ 크롤링 에러:', error);
      
      crawlerStore.setError(error.message);
      
      if (error.recoverable) {
        uiStore.showWarning(error.message, '복구 가능한 오류');
      } else {
        uiStore.showError(error.message, '치명적 오류');
      }
      
      this.updateEventStats('error');
    });
  }

  /**
   * 데이터베이스 이벤트 구독
   */
  private async subscribeToDatabaseEvents(): Promise<() => void> {
    return tauriApi.subscribeToDatabaseUpdates((stats: DatabaseStats) => {
      console.log('🗄️ 데이터베이스 통계 업데이트:', stats);
      
      databaseStore.setStats(stats);
      this.updateEventStats('database');
    });
  }

  /**
   * 완료 이벤트 구독
   */
  private async subscribeToCompletionEvents(): Promise<() => void> {
    return tauriApi.subscribeToCompletion((result: CrawlingResult) => {
      console.log('🎉 크롤링 완료:', result);
      
      crawlerStore.setResult(result);
      
      uiStore.showSuccess(
        `총 ${result.total_processed}개 항목 처리 완료 (신규: ${result.new_items}, 업데이트: ${result.updated_items})`,
        '크롤링 완료'
      );
      
      this.updateEventStats('completion');
    });
  }

  // =========================================================================
  // 상태 동기화
  // =========================================================================

  /**
   * 초기 상태 동기화
   */
  private async syncInitialState(): Promise<void> {
    try {
      console.log('🔄 초기 상태 동기화 중...');
      
      // 크롤링 상태와 DB 통계를 병렬로 로드
      await Promise.allSettled([
        crawlerStore.refreshStatus(),
        databaseStore.refreshStats(),
      ]);
      
      console.log('✅ 초기 상태 동기화 완료');
    } catch (error) {
      console.warn('⚠️ 초기 상태 동기화 중 일부 실패:', error);
      // 전체 초기화를 실패시키지는 않음
    }
  }

  /**
   * 수동 상태 새로고침
   */
  async refreshAllStates(): Promise<void> {
    if (!this.isConnected) {
      throw new Error('연결되지 않은 상태에서는 새로고침할 수 없습니다');
    }

    try {
      uiStore.showInfo('상태를 새로고침하는 중...', '새로고침');
      
      await Promise.all([
        crawlerStore.refreshStatus(),
        databaseStore.refreshStats(),
      ]);
      
      uiStore.showSuccess('상태가 성공적으로 새로고침되었습니다', '새로고침 완료');
    } catch (error) {
      const errorMessage = `상태 새로고침 실패: ${error}`;
      uiStore.showError(errorMessage, '새로고침 실패');
      throw new Error(errorMessage);
    }
  }

  // =========================================================================
  // 이벤트 통계 관리
  // =========================================================================

  private updateEventStats(eventType: 'progress' | 'task' | 'stage' | 'error' | 'database' | 'completion'): void {
    const current = this.eventStats();
    
    this.setEventStats({
      totalEvents: current.totalEvents + 1,
      progressEvents: current.progressEvents + (eventType === 'progress' ? 1 : 0),
      taskEvents: current.taskEvents + (eventType === 'task' ? 1 : 0),
      errorEvents: current.errorEvents + (eventType === 'error' ? 1 : 0),
      dbEvents: current.dbEvents + (eventType === 'database' ? 1 : 0),
      lastEventTime: new Date(),
    });
  }

  /**
   * 이벤트 통계 초기화
   */
  resetEventStats(): void {
    this.setEventStats({
      totalEvents: 0,
      progressEvents: 0,
      taskEvents: 0,
      errorEvents: 0,
      dbEvents: 0,
      lastEventTime: null,
    });
  }

  // =========================================================================
  // 정리 및 해제
  // =========================================================================

  /**
   * 모든 구독 해제 및 정리
   */
  cleanup(): void {
    console.log('🧹 실시간 매니저 정리 중...');

    // 재시도 타이머 정리
    if (this.retryTimeout) {
      clearTimeout(this.retryTimeout);
      this.retryTimeout = null;
    }

    // 모든 이벤트 구독 해제
    for (const [key, unsubscribe] of this.subscriptions) {
      try {
        unsubscribe();
        console.log(`📡 ${key} 구독 해제됨`);
      } catch (error) {
        console.warn(`⚠️ ${key} 구독 해제 실패:`, error);
      }
    }
    this.subscriptions.clear();

    // Tauri API 정리
    tauriApi.cleanup();

    // 스토어 정리
    crawlerStore.cleanup();
    databaseStore.cleanup();

    // 상태 초기화
    this.setConnectionState({
      isConnected: false,
      isInitializing: false,
      lastError: null,
      retryCount: 0,
      maxRetries: 3,
    });

    this.resetEventStats();
    this.initializationPromise = null;

    console.log('✅ 실시간 매니저 정리 완료');
  }
}

// 싱글톤 인스턴스 생성
export const realtimeManager = new RealtimeManager();

// 자동 정리 설정
onCleanup(() => {
  realtimeManager.cleanup();
});
