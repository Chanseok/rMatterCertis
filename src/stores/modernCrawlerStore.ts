/**
 * Modern Crawler Store - Phase 2 Implementation
 * 
 * 완전 자동화된 타입 시스템을 사용하여 백엔드와 1:1 동기화되는 크롤러 스토어
 * 더 이상 수동 타입 변환이나 매핑이 필요하지 않습니다.
 */

import { createStore } from 'solid-js/store';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { tauriApi } from '@/services/tauri-api';

// 자동 생성된 Rust 타입들만 사용
import type {
  CrawlingProgress,
  CrawlingResponse,
  StartCrawlingRequest,
  SystemStatePayload,
  ActorSystemStatus,
  Product,
  ProductDetail,
  ProductUrl,
  SiteStatus,
  DatabaseAnalysis,
  ProcessingStrategy
} from '@/types';

/**
 * 현대화된 크롤러 상태 인터페이스
 * 백엔드의 상태 기계와 완벽히 동기화됨
 */
interface ModernCrawlerState {
  // 실시간 시스템 상태 (삼중 채널 중 상태 채널)
  liveSystemState: SystemStatePayload | null;
  
  // 세션 진행 상태 (삼중 채널 중 진행 상태 채널)
  sessionProgress: CrawlingProgress | null;
  
  // 액터 시스템 상태
  actorSystemStatus: ActorSystemStatus | null;
  
  // 마지막 API 응답 (삼중 채널 중 결과 채널)
  lastApiResponse: CrawlingResponse | null;
  
  // 도메인 데이터 상태
  siteStatus: SiteStatus | null;
  databaseAnalysis: DatabaseAnalysis | null;
  processingStrategy: ProcessingStrategy | null;
  
  // 수집된 데이터 
  recentProducts: Product[];
  productDetails: ProductDetail[];
  collectedUrls: ProductUrl[];
  
  // 연결 및 오류 상태
  isConnected: boolean;
  lastError: string | null;
  
  // 현재 활성 세션 ID
  activeSessionId: string | null;
}

/**
 * 초기 상태 - 모든 상태는 null로 시작하여 백엔드 이벤트로 채워짐
 */
const initialState: ModernCrawlerState = {
  liveSystemState: null,
  sessionProgress: null,
  actorSystemStatus: null,
  lastApiResponse: null,
  siteStatus: null,
  databaseAnalysis: null,
  processingStrategy: null,
  recentProducts: [],
  productDetails: [],
  collectedUrls: [],
  isConnected: false,
  lastError: null,
  activeSessionId: null,
};

// 반응형 상태 생성
const [modernCrawlerState, setModernCrawlerState] = createStore<ModernCrawlerState>(initialState);

// 이벤트 리스너 관리
let eventUnsubscribers: UnlistenFn[] = [];

/**
 * 현대화된 크롤러 스토어 클래스
 */
class ModernCrawlerStore {
  
  /**
   * 스토어 초기화 및 이벤트 구독
   */
  async initialize(): Promise<void> {
    console.log('🚀 Initializing Modern Crawler Store with auto-generated types');
    
    try {
      await this.subscribeToBackendEvents();
      setModernCrawlerState('isConnected', true);
      console.log('✅ Modern Crawler Store initialized successfully');
    } catch (error) {
      console.error('❌ Failed to initialize Modern Crawler Store:', error);
      setModernCrawlerState('lastError', String(error));
    }
  }
  
  /**
   * 삼중 채널 이벤트 구독 (Phase 3 설계)
   */
  private async subscribeToBackendEvents(): Promise<void> {
    // 채널 1: 실시간 시스템 상태
    const systemStateUnsubscriber = await listen<SystemStatePayload>('event-system-state', (event) => {
      console.log('📊 System State Update:', event.payload);
      setModernCrawlerState('liveSystemState', event.payload);
    });
    
    // 채널 2: 세션 진행 상태
    const progressUnsubscriber = await listen<CrawlingProgress>('event-crawling-progress', (event) => {
      console.log('📈 Crawling Progress Update:', event.payload);
      setModernCrawlerState('sessionProgress', event.payload);
      
      // 세션 ID 업데이트
      if (event.payload.session_id) {
        setModernCrawlerState('activeSessionId', event.payload.session_id);
      }
    });
    
    // 채널 3: API 응답 결과
    const responseUnsubscriber = await listen<CrawlingResponse>('event-api-response', (event) => {
      console.log('📤 API Response:', event.payload);
      setModernCrawlerState('lastApiResponse', event.payload);
    });
    
    // 액터 시스템 상태 (선택적)
    const actorStatusUnsubscriber = await listen<ActorSystemStatus>('event-actor-status', (event) => {
      console.log('🎭 Actor System Update:', event.payload);
      setModernCrawlerState('actorSystemStatus', event.payload);
    });
    
    // 에러 이벤트 (unified actor-based)
    const errorUnsubscriber = await tauriApi.subscribeToErrorsUnified((payload) => {
      try {
        const message = typeof payload === 'string'
          ? payload
          : payload?.message || payload?.reason || payload?.error || JSON.stringify(payload);
        console.error('❌ Crawling Error (unified):', message, payload);
        setModernCrawlerState('lastError', String(message));
      } catch (e) {
        console.error('❌ Failed to handle unified error payload:', e);
        setModernCrawlerState('lastError', 'Unknown error');
      }
    });
    
    // 구독 해제 함수들 저장
    eventUnsubscribers = [
      systemStateUnsubscriber,
      progressUnsubscriber,
      responseUnsubscriber,
      actorStatusUnsubscriber,
  errorUnsubscriber
    ];
  }
  
  /**
   * 크롤링 시작 (타입 안전한 API 호출)
   */
  async startCrawling(request: StartCrawlingRequest): Promise<void> {
    try {
      console.log('🚀 Starting crawling with request:', request);
      const { invoke } = await import('@tauri-apps/api/core');
      // Map legacy AdvancedCrawlingConfig into ActorCrawlingRequest overrides
      const cfg = request.config;
      const actorReq = {
        site_url: null as string | null,
        start_page: cfg.start_page || null,
        end_page: cfg.end_page || null,
        page_count: null as number | null,
        concurrency: cfg.concurrency || null,
        batch_size: cfg.batch_size || null,
        delay_ms: cfg.delay_ms || null,
        mode: 'AdvancedEngine'
      };
      const response = await invoke<CrawlingResponse>('start_actor_system_crawling', { request: actorReq });
      console.log('✅ Crawling started (actor):', response);
      setModernCrawlerState('lastApiResponse', response);
    } catch (error) {
      console.error('❌ Failed to start crawling:', error);
      setModernCrawlerState('lastError', String(error));
      throw error;
    }
  }
  
  /**
   * 사이트 상태 체크
   */
  async checkSiteStatus(): Promise<void> {
    try {
      console.log('🔍 Checking site status...');
      
      const { invoke } = await import('@tauri-apps/api/core');
      const response = await invoke<CrawlingResponse>('check_advanced_site_status');
      
      console.log('✅ Site status checked:', response);
      setModernCrawlerState('lastApiResponse', response);
      
    } catch (error) {
      console.error('❌ Failed to check site status:', error);
      setModernCrawlerState('lastError', String(error));
      throw error;
    }
  }
  
  /**
   * 현재 상태 접근자들
   */
  get state(): ModernCrawlerState {
    return modernCrawlerState;
  }
  
  get isRunning(): boolean {
    return this.state.liveSystemState?.is_running ?? false;
  }
  
  get isHealthy(): boolean {
  const s = this.state.liveSystemState;
  if (!s) return false;
  // Some backends may not include is_healthy; treat missing as true for now
  // and read it if present (using a safe narrowing without breaking TS types)
  return (('is_healthy' in (s as any)) ? (s as any).is_healthy : true) as boolean;
  }
  
  get progressPercentage(): number {
    const progress = this.state.sessionProgress?.overall_progress;
    if (!progress) return 0;
    
    return progress.total > 0 ? (progress.completed / progress.total) * 100 : 0;
  }
  
  get currentStage(): string {
    const stages = this.state.sessionProgress?.stage_progress;
    if (!stages || stages.length === 0) return 'Idle';
    
    // 현재 진행 중인 스테이지 찾기
  const activeStage = stages.find(stage => stage.status === 'Running');
  // Prefer modern field if present, keep legacy fallback for compatibility
  const current = (activeStage as any)?.stage_type ?? activeStage?.stage_name;
  const last = (stages[stages.length - 1] as any)?.stage_type ?? stages[stages.length - 1]?.stage_name;
  return current ?? last ?? 'Unknown';
  }
  
  /**
   * 정리 및 구독 해제
   */
  cleanup(): void {
    console.log('🧹 Cleaning up Modern Crawler Store');
    
    eventUnsubscribers.forEach(unsubscribe => {
      try {
        unsubscribe();
      } catch (error) {
        console.warn('Warning: Failed to unsubscribe:', error);
      }
    });
    
    eventUnsubscribers = [];
    setModernCrawlerState(initialState);
  }
}

// 싱글톤 인스턴스
const modernCrawlerStore = new ModernCrawlerStore();

// SolidJS와의 통합을 위한 반응형 내보내기
export { modernCrawlerState, setModernCrawlerState, modernCrawlerStore };

// 정리 함수는 컴포넌트에서 onCleanup으로 호출
export const cleanupModernCrawlerStore = () => modernCrawlerStore.cleanup();

// 개발자 도구용 디버깅
if (import.meta.env.DEV) {
  (window as any).modernCrawlerStore = modernCrawlerStore;
  (window as any).modernCrawlerState = modernCrawlerState;
}
