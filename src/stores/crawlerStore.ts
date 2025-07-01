/**
 * Crawler Store - 크롤링 전용 상태 관리
 * 
 * 이 스토어는 크롤링 관련 상태만을 담당하며, 백엔드의 실시간 이벤트와
 * 동기화되어 UI에 반응형 업데이트를 제공합니다.
 */

import { createStore } from 'solid-js/store';
import { createSignal, onCleanup } from 'solid-js';
import { tauriApi } from '../services/tauri-api';
import type {
  CrawlingProgress,
  CrawlingTaskStatus,
  CrawlingResult,
  BackendCrawlerConfig
} from '../types/crawling';

// 크롤러 상태 인터페이스
interface CrawlerState {
  // 현재 크롤링 진행 상황
  progress: CrawlingProgress | null;
  
  // 연결 상태
  isConnected: boolean;
  isInitialized: boolean;
  
  // 에러 상태
  lastError: string | null;
  errorHistory: Array<{
    id: string;
    message: string;
    timestamp: Date;
    recoverable: boolean;
  }>;
  
  // 작업 상태
  activeTasks: Map<string, CrawlingTaskStatus>;
  
  // 크롤링 결과
  lastResult: CrawlingResult | null;
  
  // 설정
  currentConfig: BackendCrawlerConfig | null;
}

// 초기 상태
const initialState: CrawlerState = {
  progress: null,
  isConnected: false,
  isInitialized: false,
  lastError: null,
  errorHistory: [],
  activeTasks: new Map(),
  lastResult: null,
  currentConfig: null,
};

// 반응형 상태 생성
const [crawlerState, setCrawlerState] = createStore<CrawlerState>(initialState);

// 이벤트 구독 관리
const [eventSubscriptions] = createSignal<(() => void)[]>([]);

/**
 * 크롤러 스토어 클래스
 */
class CrawlerStore {
  // =========================================================================
  // 상태 접근자 (Getters)
  // =========================================================================

  get state() {
    return crawlerState;
  }

  get progress() {
    return () => crawlerState.progress;
  }

  get status() {
    return () => crawlerState.progress?.status || 'Idle';
  }

  get currentStage() {
    return () => crawlerState.progress?.current_stage || 'Idle';
  }

  get isConnected() {
    return () => crawlerState.isConnected;
  }

  get isInitialized() {
    return () => crawlerState.isInitialized;
  }

  get lastError() {
    return () => crawlerState.lastError;
  }

  get errorHistory() {
    return () => crawlerState.errorHistory;
  }

  get activeTasks() {
    return () => Array.from(crawlerState.activeTasks.values());
  }

  get lastResult() {
    return () => crawlerState.lastResult;
  }

  get currentConfig() {
    return () => crawlerState.currentConfig;
  }

  // =========================================================================
  // 상태 편의 접근자
  // =========================================================================

  get isIdle() {
    return () => this.status() === 'Idle';
  }

  get isRunning() {
    return () => this.status() === 'Running';
  }

  get isPaused() {
    return () => this.status() === 'Paused';
  }

  get isCompleted() {
    return () => this.status() === 'Completed';
  }

  get hasError() {
    return () => this.status() === 'Error' || crawlerState.lastError !== null;
  }

  get progressPercentage() {
    return () => crawlerState.progress?.percentage || 0;
  }

  get canStart() {
    return () => this.isConnected() && (this.isIdle() || this.isCompleted() || this.hasError());
  }

  get canPause() {
    return () => this.isConnected() && this.isRunning();
  }

  get canResume() {
    return () => this.isConnected() && this.isPaused();
  }

  get canStop() {
    return () => this.isConnected() && (this.isRunning() || this.isPaused());
  }

  // =========================================================================
  // 상태 업데이트 메서드
  // =========================================================================

  setProgress(progress: CrawlingProgress) {
    setCrawlerState('progress', progress);
    setCrawlerState('lastError', null); // 진행 중이면 에러 클리어
  }

  setConnected(connected: boolean) {
    setCrawlerState('isConnected', connected);
  }

  setError(error: string | null) {
    setCrawlerState('lastError', error);
    
    if (error) {
      // 에러 히스토리에 추가
      const errorEntry = {
        id: Date.now().toString(),
        message: error,
        timestamp: new Date(),
        recoverable: true, // 기본값
      };
      
      setCrawlerState('errorHistory', (prev) => [errorEntry, ...prev.slice(0, 9)]); // 최대 10개 유지
    }
  }

  updateTaskStatus(taskStatus: CrawlingTaskStatus) {
    setCrawlerState('activeTasks', (prev) => {
      const newMap = new Map(prev);
      newMap.set(taskStatus.task_id, taskStatus);
      return newMap;
    });
  }

  removeTask(taskId: string) {
    setCrawlerState('activeTasks', (prev) => {
      const newMap = new Map(prev);
      newMap.delete(taskId);
      return newMap;
    });
  }

  setResult(result: CrawlingResult) {
    setCrawlerState('lastResult', result);
  }

  setConfig(config: BackendCrawlerConfig) {
    setCrawlerState('currentConfig', config);
  }

  clearErrors() {
    setCrawlerState('lastError', null);
    setCrawlerState('errorHistory', []);
  }

  reset() {
    setCrawlerState(initialState);
  }

  // =========================================================================
  // 크롤링 제어 메서드
  // =========================================================================

  async startCrawling(config: BackendCrawlerConfig): Promise<string> {
    try {
      this.setConfig(config);
      this.clearErrors();
      
      const sessionId = await tauriApi.startCrawling(config);
      
      console.log('✅ 크롤링 세션 시작:', sessionId);
      return sessionId;
    } catch (error) {
      const errorMessage = `크롤링 시작 실패: ${error}`;
      this.setError(errorMessage);
      throw new Error(errorMessage);
    }
  }

  async pauseCrawling(): Promise<void> {
    try {
      await tauriApi.pauseCrawling();
      console.log('⏸️ 크롤링 일시정지됨');
    } catch (error) {
      const errorMessage = `크롤링 일시정지 실패: ${error}`;
      this.setError(errorMessage);
      throw new Error(errorMessage);
    }
  }

  async resumeCrawling(): Promise<void> {
    try {
      await tauriApi.resumeCrawling();
      console.log('▶️ 크롤링 재개됨');
    } catch (error) {
      const errorMessage = `크롤링 재개 실패: ${error}`;
      this.setError(errorMessage);
      throw new Error(errorMessage);
    }
  }

  async stopCrawling(): Promise<void> {
    try {
      await tauriApi.stopCrawling();
      console.log('⏹️ 크롤링 중단됨');
    } catch (error) {
      const errorMessage = `크롤링 중단 실패: ${error}`;
      this.setError(errorMessage);
      throw new Error(errorMessage);
    }
  }

  // =========================================================================
  // 초기화 및 정리
  // =========================================================================

  async initialize(): Promise<void> {
    if (crawlerState.isInitialized) {
      console.log('⚠️ 크롤러 스토어는 이미 초기화되었습니다');
      return;
    }

    try {
      console.log('🔧 크롤러 스토어 초기화 중...');
      
      // 초기 상태 로드
      await this.refreshStatus();
      
      // 기본 설정 로드
      await this.loadDefaultConfig();
      
      // 실시간 이벤트 구독
      await this.subscribeToEvents();
      
      setCrawlerState('isInitialized', true);
      setCrawlerState('isConnected', true);
      
      console.log('✅ 크롤러 스토어 초기화 완료');
    } catch (error) {
      console.error('❌ 크롤러 스토어 초기화 실패:', error);
      this.setError(`초기화 실패: ${error}`);
      setCrawlerState('isConnected', false);
    }
  }

  private async subscribeToEvents(): Promise<void> {
    const subscriptions: (() => void)[] = [];

    try {
      // 진행 상황 이벤트 구독
      const progressUnsub = await tauriApi.subscribeToProgress((progress) => {
        this.setProgress(progress);
      });
      subscriptions.push(progressUnsub);

      // 작업 상태 이벤트 구독
      const taskUnsub = await tauriApi.subscribeToTaskStatus((taskStatus) => {
        this.updateTaskStatus(taskStatus);
      });
      subscriptions.push(taskUnsub);

      // 스테이지 변경 이벤트 구독
      const stageUnsub = await tauriApi.subscribeToStageChange((data) => {
        console.log(`🔄 스테이지 변경: ${data.from} → ${data.to} (${data.message})`);
      });
      subscriptions.push(stageUnsub);

      // 에러 이벤트 구독
      const errorUnsub = await tauriApi.subscribeToErrors((error) => {
        console.error('❌ 크롤링 에러:', error);
        this.setError(error.message);
      });
      subscriptions.push(errorUnsub);

      // 완료 이벤트 구독
      const completedUnsub = await tauriApi.subscribeToCompletion((result) => {
        console.log('🎉 크롤링 완료:', result);
        this.setResult(result);
      });
      subscriptions.push(completedUnsub);

      // 구독 목록 저장
      eventSubscriptions()[0] = () => {
        subscriptions.forEach(unsub => unsub());
      };

      console.log('📡 실시간 이벤트 구독 완료');
    } catch (error) {
      console.error('❌ 이벤트 구독 실패:', error);
      throw error;
    }
  }

  async refreshStatus(): Promise<void> {
    try {
      const status = await tauriApi.getCrawlingStatus();
      this.setProgress(status);
    } catch (error) {
      console.warn('⚠️ 상태 새로고침 실패:', error);
      // 초기화 시에는 에러로 처리하지 않음
    }
  }

  cleanup(): void {
    console.log('🧹 크롤러 스토어 정리 중...');
    
    // 이벤트 구독 해제
    const unsubs = eventSubscriptions();
    unsubs.forEach(unsub => unsub?.());
    
    // Tauri API 정리
    tauriApi.cleanup();
    
    // 상태 초기화
    this.reset();
    
    console.log('✅ 크롤러 스토어 정리 완료');
  }

  // =========================================================================
  // 설정 관련 메서드
  // =========================================================================

  /**
   * 백엔드에서 기본 크롤링 설정을 로드합니다.
   * 이 메서드는 초기화 단계에서 호출되어 기본 설정값을 가져옵니다.
   */
  async loadDefaultConfig(): Promise<BackendCrawlerConfig> {
    try {
      console.log('🔄 기본 크롤링 설정 로드 중...');
      const defaultConfig = await tauriApi.getDefaultCrawlingConfig();
      
      // 백엔드에서 받은 설정을 프론트엔드 설정 타입으로 변환
      // 필요한 경우 이곳에서 형식 변환을 수행
      
      // 기본 로깅 설정 추가 (백엔드에서 제공되지 않는 경우)
      if (!defaultConfig.logging) {
        defaultConfig.logging = {
          level: 'info',
          enable_stack_trace: true,
          enable_timestamp: true,
          components: {
            crawler: 'info',
            parser: 'info',
            network: 'info',
            database: 'info'
          }
        };
      }
      
      const backendConfig: BackendCrawlerConfig = {
        // Core settings
        start_page: 1,
        end_page: defaultConfig.max_pages || 10,
        concurrency: defaultConfig.max_concurrent_requests || 5,
        delay_ms: defaultConfig.request_delay_ms || 500,
        
        // Advanced settings
        page_range_limit: defaultConfig.advanced?.max_search_attempts || 10,
        product_list_retry_count: defaultConfig.advanced?.retry_attempts || 3,
        product_detail_retry_count: defaultConfig.advanced?.retry_attempts || 3,
        products_per_page: 20,
        auto_add_to_local_db: true,
        auto_status_check: true,
        crawler_type: 'full',

        // Batch processing
        batch_size: 10,
        batch_delay_ms: 1000,
        enable_batch_processing: true,
        batch_retry_limit: 3,

        // URLs
        base_url: defaultConfig.base_url || '',
        matter_filter_url: defaultConfig.matter_filter_url || '',
        
        // Timeouts
        page_timeout_ms: (defaultConfig.advanced?.request_timeout_seconds || 30) * 1000,
        product_detail_timeout_ms: (defaultConfig.advanced?.request_timeout_seconds || 30) * 1000,
        
        // Concurrency & Performance
        initial_concurrency: defaultConfig.max_concurrent_requests || 5,
        detail_concurrency: defaultConfig.max_concurrent_requests || 5,
        retry_concurrency: Math.max(1, (defaultConfig.max_concurrent_requests || 5) / 2),
        min_request_delay_ms: defaultConfig.request_delay_ms || 500,
        max_request_delay_ms: (defaultConfig.request_delay_ms || 500) * 2,
        retry_start: defaultConfig.advanced?.retry_delay_ms || 1000,
        retry_max: defaultConfig.advanced?.retry_attempts || 3,
        cache_ttl_ms: 3600000, // 1시간
        
        // Browser settings
        headless_browser: true,
        max_concurrent_tasks: defaultConfig.max_concurrent_requests || 5,
        request_delay: defaultConfig.request_delay_ms || 500,
        custom_user_agent: undefined,
        
        // Logging
        logging: {
          level: defaultConfig.verbose_logging ? 'debug' : 'info',
          enable_stack_trace: true,
          enable_timestamp: true,
          components: {
            crawler: defaultConfig.verbose_logging ? 'debug' : 'info',
            parser: defaultConfig.verbose_logging ? 'debug' : 'info',
            network: defaultConfig.verbose_logging ? 'debug' : 'info',
            database: defaultConfig.verbose_logging ? 'debug' : 'info'
          }
        }
      };
      
      // 현재 설정으로 설정
      this.setConfig(backendConfig);
      
      console.log('✅ 기본 크롤링 설정 로드 완료:', backendConfig);
      return backendConfig;
    } catch (error) {
      const errorMessage = `기본 설정 로드 실패: ${error}`;
      this.setError(errorMessage);
      console.error('❌', errorMessage);
      
      // 기본 설정 실패시 하드코딩된 기본값 사용
      const fallbackConfig: BackendCrawlerConfig = {
        start_page: 1,
        end_page: 10,
        concurrency: 5,
        delay_ms: 500,
        page_range_limit: 10,
        product_list_retry_count: 3,
        product_detail_retry_count: 3,
        products_per_page: 20,
        auto_add_to_local_db: true,
        auto_status_check: true,
        crawler_type: 'full',
        batch_size: 10,
        batch_delay_ms: 1000,
        enable_batch_processing: true,
        batch_retry_limit: 3,
        base_url: '',
        matter_filter_url: '',
        page_timeout_ms: 30000,
        product_detail_timeout_ms: 30000,
        initial_concurrency: 5,
        detail_concurrency: 5,
        retry_concurrency: 2,
        min_request_delay_ms: 500,
        max_request_delay_ms: 1000,
        retry_start: 1000,
        retry_max: 3,
        cache_ttl_ms: 3600000,
        headless_browser: true,
        max_concurrent_tasks: 5,
        request_delay: 500,
        logging: {
          level: 'info',
          enable_stack_trace: true,
          enable_timestamp: true,
          components: {
            crawler: 'info',
            parser: 'info',
            network: 'info',
            database: 'info'
          }
        }
      };
      
      this.setConfig(fallbackConfig);
      return fallbackConfig;
    }
  }
}

// 싱글톤 인스턴스 생성
export const crawlerStore = new CrawlerStore();

// 자동 정리 설정
onCleanup(() => {
  crawlerStore.cleanup();
});
