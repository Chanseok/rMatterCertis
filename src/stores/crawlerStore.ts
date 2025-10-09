/**
 * Crawler Store - 크롤링 전용 상태 관리
 * 
 * 이 스토어는 크롤링 관련 상태만을 담당하며, 백엔드의 실시간 이벤트와
 * 동기화되어 UI에 반응형 업데이트를 제공합니다.
 */

import { createStore } from 'solid-js/store';
import { createSignal, onCleanup } from 'solid-js';
import { tauriApi } from '../services/tauri-api';
import { apiAdapter, safeApiCall } from '../platform/tauri';
// types from '@/types' were unused and removed to satisfy noUnused
import type { CrawlingProgress, CrawlingTaskStatus, BackendCrawlerConfig, CrawlingStatusCheck } from '../types/crawling';
import { CrawlingStatus, CrawlingStage } from '../types/crawling';
import { DatabaseHealth } from '../types/crawling';
import type { 
  SessionStatusDto, 
  StartCrawlingDto
} from '../types/domain';

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
  
  // 크롤링 완료 요약 (이벤트 payload)
  lastResult: any | null;
  
  // 사이트 분석 결과 (탭 전환 시에도 유지)
  siteAnalysisResult: CrawlingStatusCheck | null;
  siteAnalysisTimestamp: Date | null;
  isAnalyzing: boolean;
  
  // 설정
  currentConfig: BackendCrawlerConfig | null;
  
  // 세션 관리 (domain/crawling-store.ts에서 통합)
  currentSessionId: string | null;
  activeSessions: SessionStatusDto[];
  sessionHistory: SessionStatusDto[];
  isStarting: boolean;
  isStopping: boolean;
  isPausing: boolean;
  isResuming: boolean;
  
  // 시간 추정을 위한 통계
  timeStats: {
    listPageDurations: number[]; // ListPage 크롤링 소요 시간들 (ms)
    detailDurations: number[];    // Detail 크롤링 소요 시간들 (ms)
    sessionStartTime: Date | null;
    lastProgressTime: number | null;  // 마지막 Progress 이벤트 시간 (timestamp)
    lastProgressCurrent: number | null;  // 마지막 Progress의 current 값
    lastProgressStage: string | null;  // 마지막 Progress의 stage
    
    // 세션 전체 통계 (배치가 아닌 전체 세션 기준)
    totalListPagesInSession: number | null;  // 이번 세션의 총 ListPage 수
    completedListPagesInSession: number;     // 완료된 ListPage 수
    totalDetailsInSession: number | null;    // 이번 세션의 총 Detail 아이템 수
    completedDetailsInSession: number;       // 완료된 Detail 아이템 수
    
    // 배치 정보
    currentBatchIndex: number | null;  // 현재 배치 번호 (1부터 시작)
    totalBatches: number | null;       // 총 배치 수
    batchId: string | null;            // 현재 배치 ID
  };
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
  siteAnalysisResult: null,
  siteAnalysisTimestamp: null,
  isAnalyzing: false,
  currentConfig: null,
  currentSessionId: null,
  activeSessions: [],
  sessionHistory: [],
  isStarting: false,
  isStopping: false,
  isPausing: false,
  isResuming: false,
  timeStats: {
    listPageDurations: [],
    detailDurations: [],
    sessionStartTime: null,
    lastProgressTime: null,
    lastProgressCurrent: null,
    lastProgressStage: null,
    totalListPagesInSession: null,
    completedListPagesInSession: 0,
    totalDetailsInSession: null,
    completedDetailsInSession: 0,
    currentBatchIndex: null,
    totalBatches: null,
    batchId: null,
  },
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
  return () => crawlerState.progress?.status ?? CrawlingStatus.Idle;
  }

  get currentStage() {
  return () => crawlerState.progress?.current_stage ?? CrawlingStage.Idle;
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

  get siteAnalysisResult() {
    return () => crawlerState.siteAnalysisResult;
  }

  get siteAnalysisTimestamp() {
    return () => crawlerState.siteAnalysisTimestamp;
  }

  get isAnalyzing() {
    return () => crawlerState.isAnalyzing;
  }

  get currentConfig() {
    return () => crawlerState.currentConfig;
  }

  // =========================================================================
  // 상태 편의 접근자
  // =========================================================================

  get isIdle() {
  return () => this.status() === CrawlingStatus.Idle;
  }

  get isRunning() {
  return () => this.status() === CrawlingStatus.Running;
  }

  get isPaused() {
  return () => this.status() === CrawlingStatus.Paused;
  }

  get isCompleted() {
  return () => this.status() === CrawlingStatus.Completed;
  }

  get hasError() {
  return () => this.status() === CrawlingStatus.Error || crawlerState.lastError !== null;
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

  setResult(result: any) {
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
  // 크롤링 제어 메서드 (통합된 세션 관리)
  // =========================================================================

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
    console.log('📡 Subscribing to unified actor events...');
    
    // 모든 액터 이벤트 구독 (variant 필터 없이)
    const unlisten = await tauriApi.subscribeToUnifiedActorEvents({
      onEvent: (payload) => this.handleActorEvent(payload),
    });
    
    eventSubscriptions()[0] = () => { 
      try { 
        unlisten(); 
      } catch {} 
    };
    console.log('✅ Subscribed to unified actor events.');
  }

  private handleActorEvent(payload: any): void {
    // Log all events for debugging
    console.log(`[CrawlerStore Actor Event] variant: ${payload.variant}`, payload);

    const variant = payload.variant;

    switch (variant) {
        case 'SessionStarted':
            setCrawlerState('progress', {
                status: CrawlingStatus.Running,
                current_stage: CrawlingStage.StatusCheck,
                percentage: 0,
                current: 0,
                total: payload.total_pages || 0,
                message: 'Session started...',
                new_items: 0,
                updated_items: 0,
                errors: 0,
                timestamp: payload.timestamp,
                current_step: 'Starting',
                elapsed_time: 0,
            });
            setCrawlerState('currentSessionId', payload.session_id);
            // 세션 시작 시간 기록
            setCrawlerState('timeStats', 'sessionStartTime', new Date());
            // 통계 초기화
            setCrawlerState('timeStats', 'listPageDurations', []);
            setCrawlerState('timeStats', 'detailDurations', []);
            setCrawlerState('timeStats', 'lastProgressTime', null);
            setCrawlerState('timeStats', 'lastProgressCurrent', null);
            setCrawlerState('timeStats', 'lastProgressStage', null);
            // 세션 전체 통계 초기화
            setCrawlerState('timeStats', 'totalListPagesInSession', null);
            setCrawlerState('timeStats', 'completedListPagesInSession', 0);
            setCrawlerState('timeStats', 'totalDetailsInSession', null);
            setCrawlerState('timeStats', 'completedDetailsInSession', 0);
            setCrawlerState('timeStats', 'currentBatchIndex', null);
            setCrawlerState('timeStats', 'totalBatches', null);
            setCrawlerState('timeStats', 'batchId', null);
            break;

      case 'StageStarted': {
        const normalizedStageType = this.normalizeStageType(payload.stage_type);
        const stageType = this.mapStageTypeToCrawlingStage(normalizedStageType);
        console.log(`[CrawlerStore] StageStarted event received:`, payload);
        
        // 세션 전체 통계 업데이트 - 첫 번째 Stage만 또는 누적
        if (normalizedStageType.includes('listpage')) {
          // 첫 StageStarted이거나 null이면 설정, 이후에는 누적
          setCrawlerState('timeStats', 'totalListPagesInSession', (prev) => {
            if (prev === null) {
              console.log(`[CrawlerStore] Setting initial totalListPagesInSession: ${payload.items_count}`);
              return payload.items_count;
            } else {
              const newTotal = prev + payload.items_count;
              console.log(`[CrawlerStore] Accumulating totalListPagesInSession: ${prev} + ${payload.items_count} = ${newTotal}`);
              return newTotal;
            }
          });
          // completedListPagesInSession은 누적되므로 리셋하지 않음
        } else if (normalizedStageType.includes('productdetail')) {
          // Detail도 마찬가지로 누적
          setCrawlerState('timeStats', 'totalDetailsInSession', (prev) => {
            if (prev === null) {
              console.log(`[CrawlerStore] Setting initial totalDetailsInSession: ${payload.items_count}`);
              return payload.items_count;
            } else {
              const newTotal = prev + payload.items_count;
              console.log(`[CrawlerStore] Accumulating totalDetailsInSession: ${prev} + ${payload.items_count} = ${newTotal}`);
              return newTotal;
            }
          });
          // completedDetailsInSession은 누적되므로 리셋하지 않음
        }
        
        setCrawlerState('progress', (prev) => {
          const switchingToDetail = normalizedStageType.includes('productdetail');
          const newProgress = {
            ...prev,
            status: CrawlingStatus.Running,
            current_stage: stageType,
            stage_started_at: new Date(),
            total: payload.items_count,
            current: 0, // Reset current on new stage
            item_type: payload.item_type,
            // 상세 단계 진입 시 진행률 0부터 다시 (이전 스테이지 잔류 방지)
            percentage: switchingToDetail ? 0 : (prev?.percentage ?? 0),
          };

          // ProductDetailCrawling 스테이지의 경우 total을 명시적으로 업데이트
          if (normalizedStageType.includes('productdetail')) {
            console.log(
              `[CrawlerStore] Updating total for ProductDetailCrawling: ${payload.items_count}`
            );
            newProgress.total = payload.items_count;
            newProgress.current = 0; // 여기서도 current를 리셋합니다.
          }
          
          console.log('[CrawlerStore] New progress state:', newProgress);
          return newProgress as CrawlingProgress;
        });
        break;
      }

    case 'TaskLifecycle': {
      // Derive stage heuristically from task kind
      const kind = payload.task_kind;
      let stage: CrawlingStage | null = null;
      if (kind === 'Page') stage = CrawlingStage.ProductList;
      if (kind === 'Product') stage = CrawlingStage.ProductDetails;
      if (stage) {
        setCrawlerState('progress', (prev: CrawlingProgress | null) => ({
          ...prev!,
          current_stage: stage!,
        }));
      }
      break;
    }

    case 'Progress':
       setCrawlerState('progress', (prev: CrawlingProgress | null) => {
        if (!prev) return prev as any;
        const isDetailStage = prev.current_stage === CrawlingStage.ProductDetails;
        // Detail 단계에서는 total을 StageStarted에서 설정한 값을 보호
        // current_step 값이 페이지 기준일 수 있으므로 아이템 기반 current는 StageItemCompleted 누적에 맡긴다.
        return {
          ...prev,
          status: CrawlingStatus.Running,
          percentage: isDetailStage ? prev.percentage : payload.percentage,
          current_step: payload.message,
          // ProductDetails 단계에서는 current/total을 덮어쓰지 않음
          current: isDetailStage ? prev.current : payload.current_step,
          total: isDetailStage ? prev.total : payload.total_steps,
        };
      });
      
      // Progress 이벤트마다 시간 추정 계산 (진행률 기반)
      const currentProgress = crawlerState.progress;
      if (currentProgress && currentProgress.status === 'Running') {
        const now = Date.now();
        const stats = crawlerState.timeStats;
        
        // 첫 Progress 이벤트거나 stage가 변경된 경우 초기화
        if (!stats.lastProgressTime || stats.lastProgressStage !== (currentProgress.current_stage || '')) {
          setCrawlerState('timeStats', 'lastProgressTime', now);
          setCrawlerState('timeStats', 'lastProgressCurrent', currentProgress.current || 0);
          setCrawlerState('timeStats', 'lastProgressStage', currentProgress.current_stage || '');
        } else {
          // 이전 Progress와 현재 Progress 사이의 시간 차이 계산
          const timeDiff = now - stats.lastProgressTime;
          const itemsDiff = (currentProgress.current || 0) - (stats.lastProgressCurrent || 0);
          
          if (itemsDiff > 0 && timeDiff > 0) {
            // 평균 시간 계산 (아이템당 소요 시간)
            const avgTimePerItem = timeDiff / itemsDiff;
            
            // stage에 따라 적절한 통계에 저장
            const stageType = (currentProgress.current_stage || '').toLowerCase();
            if (stageType.includes('listpage')) {
              setCrawlerState('timeStats', 'listPageDurations', (prev) => {
                const updated = [...prev, avgTimePerItem];
                return updated.slice(-50); // 최근 50개만 유지
              });
            } else if (stageType.includes('productdetail')) {
              setCrawlerState('timeStats', 'detailDurations', (prev) => {
                const updated = [...prev, avgTimePerItem];
                return updated.slice(-100); // 최근 100개만 유지
              });
            }
            
            // 시간 추정 계산
            const timeEstimates = this.calculateTimeEstimates(stageType);
            setCrawlerState('progress', 'time_estimates', timeEstimates);
            
            // 현재 값으로 업데이트
            setCrawlerState('timeStats', 'lastProgressTime', now);
            setCrawlerState('timeStats', 'lastProgressCurrent', currentProgress.current || 0);
          }
        }
      }
      break;

    case 'SessionCompleted':
      setCrawlerState('progress', (prev: CrawlingProgress | null) => ({
        ...prev!,
        status: CrawlingStatus.Completed,
        percentage: 100,
        message: 'Session completed successfully.',
      }));
            setCrawlerState('lastResult', payload.summary);
            break;
        
    case 'SessionFailed':
       setCrawlerState('progress', (prev: CrawlingProgress | null) => ({
        ...prev!,
        status: CrawlingStatus.Error,
       }));
            this.setError(payload.error || 'Session failed');
            break;

        // 실시간 개별 아이템 이벤트 처리
        case 'StageItemStarted':
          this.handleStageItemStarted(payload);
          break;
          
        case 'StageItemCompleted':
          this.handleStageItemCompleted(payload);
          break;
        
        case 'ListPageBatchStarted':
          // 배치 시작 정보 저장
          console.log('[CrawlerStore] ListPageBatchStarted:', payload);
          
          // 세션 전체 페이지 수 설정 (첫 배치에서 한 번만)
          if (payload.total_pages_in_session !== undefined) {
            setCrawlerState('timeStats', 'totalListPagesInSession', (prev) => {
              if (prev === null) {
                console.log(`[CrawlerStore] Setting totalListPagesInSession from first batch: ${payload.total_pages_in_session}`);
                return payload.total_pages_in_session;
              }
              return prev; // 이미 설정되었으면 유지
            });
          }
          
          // 배치 정보 업데이트
          if (payload.batch_index !== undefined) {
            setCrawlerState('timeStats', 'currentBatchIndex', payload.batch_index + 1); // 1-based index for display
          }
          if (payload.total_batches !== undefined) {
            setCrawlerState('timeStats', 'totalBatches', payload.total_batches);
          }
          if (payload.batch_id) {
            setCrawlerState('timeStats', 'batchId', payload.batch_id);
          }
          break;

        default:
            break;
    }
  }

  private handleStageItemStarted(payload: any): void {
    console.log('🟢 [StageItemStarted] 이벤트 수신:', payload);
    
    // 새로운 StageItemType 구조에 맞게 타입 추출
    const itemTypeDisplay = this.getItemTypeDisplay(payload.item_type);
    
    const activeItem = {
      item_id: payload.item_id,
      item_type: itemTypeDisplay,
      stage_type: payload.stage_type,
      started_at: payload.timestamp,
    };

    setCrawlerState('progress', (prev: CrawlingProgress | null) => ({
      ...prev!,
      active_items: [...(prev?.active_items || []), activeItem],
      message: `Processing ${itemTypeDisplay}: ${payload.item_id}`,
    }));
  }

  private handleStageItemCompleted(payload: any): void {
    console.log('✅ [CrawlerStore] StageItemCompleted received:', payload);
    
    // 새로운 StageItemType 구조에 맞게 타입 추출
    const itemTypeDisplay = this.getItemTypeDisplay(payload.item_type);
    
    const completedItem = {
      item_id: payload.item_id,
      item_type: itemTypeDisplay,
      stage_type: payload.stage_type,
      success: payload.success,
      duration_ms: payload.duration_ms,
      collected_count: payload.collected_count,
      error: payload.error,
      completed_at: payload.timestamp,
    };

    // 시간 통계 업데이트
    const normalizedStageType = this.normalizeStageType(payload.stage_type);
    if (payload.duration_ms && payload.success) {
      if (normalizedStageType.includes('listpage')) {
        setCrawlerState('timeStats', 'listPageDurations', (prev) => {
          const updated = [...prev, payload.duration_ms];
          // 최근 50개만 유지하여 평균을 계산
          return updated.slice(-50);
        });
        // 완료 카운트 증가 (페이지 하나 완료)
        setCrawlerState('timeStats', 'completedListPagesInSession', (prev) => prev + 1);
        
        // totalListPagesInSession은 ListPageBatchStarted에서 설정됨
        // 여기서는 null 체크만 하고 로그 출력
        if (crawlerState.timeStats.totalListPagesInSession === null) {
          console.warn('[CrawlerStore] totalListPagesInSession is null at StageItemCompleted - waiting for ListPageBatchStarted');
        }
      } else if (normalizedStageType.includes('productdetail')) {
        setCrawlerState('timeStats', 'detailDurations', (prev) => {
          const updated = [...prev, payload.duration_ms];
          // 최근 100개만 유지하여 평균을 계산
          return updated.slice(-100);
        });
        // 완료 카운트 증가 (Detail 하나 완료)
        setCrawlerState('timeStats', 'completedDetailsInSession', (prev) => prev + 1);
        
        // totalDetailsInSession은 StageStarted에서 설정됨
        if (crawlerState.timeStats.totalDetailsInSession === null) {
          console.warn('[CrawlerStore] totalDetailsInSession is null at StageItemCompleted - waiting for StageStarted');
        }
      }
    }

    setCrawlerState('progress', (prev: CrawlingProgress | null) => {
      if (!prev) return prev;

      // 활성 아이템에서 제거
      const updatedActiveItems = (prev.active_items || []).filter(
        item => item.item_id !== payload.item_id
      );

      // 최근 완료된 아이템에 추가 (최대 20개까지만 유지)
      const updatedRecentCompleted = [
        completedItem,
        ...(prev.recent_completed_items || []),
      ].slice(0, 20);

      const newCurrent = prev.current + (payload.success ? 1 : 0);
      const newErrors = prev.errors + (payload.success ? 0 : 1);
      
      // 시간 추정 계산
      const timeEstimates = this.calculateTimeEstimates(normalizedStageType);

      return {
        ...prev,
        active_items: updatedActiveItems,
        recent_completed_items: updatedRecentCompleted,
        current: newCurrent,
        errors: newErrors,
        new_items: prev.new_items + (payload.collected_count || 0),
        // 상세 단계에서는 아이템 개수 기반으로 percentage 재계산
        percentage: prev.current_stage === CrawlingStage.ProductDetails && prev.total > 0
          ? Math.min(100, Math.round((newCurrent / prev.total) * 100))
          : prev.percentage,
        time_estimates: timeEstimates,
      };
    });
  }

  // 시간 추정 계산
  private calculateTimeEstimates(
    stageType: string
  ) {
    const stats = crawlerState.timeStats;
    const now = new Date();
    
    console.log('[TimeEstimate] Calculating estimates...', {
      stageType,
      totalListPages: stats.totalListPagesInSession,
      completedListPages: stats.completedListPagesInSession,
      totalDetails: stats.totalDetailsInSession,
      completedDetails: stats.completedDetailsInSession,
      listPageDurationsCount: stats.listPageDurations.length,
      detailDurationsCount: stats.detailDurations.length,
    });
    
    // ListPage 통계 - 세션 전체 기준으로 계산
    let listPageStats;
    if (stageType.includes('listpage') && stats.listPageDurations.length > 0) {
      const avgTimePerPage = stats.listPageDurations.reduce((a, b) => a + b, 0) / stats.listPageDurations.length;
      const totalPages = stats.totalListPagesInSession || 0;
      const completedPages = stats.completedListPagesInSession;
      const remainingPages = totalPages - completedPages;
      
      listPageStats = {
        completed_pages: completedPages,
        total_pages: totalPages,
        remaining_pages: remainingPages,
        avg_time_per_page_ms: Math.round(avgTimePerPage),
        estimated_remaining_ms: Math.round(avgTimePerPage * remainingPages),
      };
    }
    
    // Detail 통계 - 세션 전체 기준으로 계산
    let detailStats;
    if (stageType.includes('productdetail') && stats.detailDurations.length > 0) {
      const avgTimePerItem = stats.detailDurations.reduce((a, b) => a + b, 0) / stats.detailDurations.length;
      const avgTimePer10Items = avgTimePerItem * 10;
      const totalDetails = stats.totalDetailsInSession || 0;
      const completedDetails = stats.completedDetailsInSession;
      const remainingItems = totalDetails - completedDetails;
      
      detailStats = {
        completed_products: completedDetails,
        total_products: totalDetails,
        remaining_products: remainingItems,
        avg_time_per_10_products_ms: Math.round(avgTimePer10Items),
        estimated_remaining_ms: Math.round(avgTimePerItem * remainingItems),
      };
    }
    
    // 전체 추정: ListPage와 Detail 양쪽 합산
    // 현재 스테이지에 따라 적절하게 계산
    let totalEstimatedMs = 0;
    
    if (stageType.includes('listpage')) {
      // ListPage 단계: ListPage 남은 시간 + 아직 시작하지 않은 Detail 전체 시간
      totalEstimatedMs = listPageStats?.estimated_remaining_ms || 0;
      
      // Detail 단계가 아직 시작하지 않았다면, 남은 ListPage들에서 나올 Detail들의 예상 시간 추가
      if (stats.detailDurations.length > 0 && listPageStats) {
        const avgDetailTime = stats.detailDurations.reduce((a, b) => a + b, 0) / stats.detailDurations.length;
        // 남은 페이지 * 페이지당 평균 제품 수 (약 12개 가정)
        const estimatedDetailsFromRemainingPages = listPageStats.remaining_pages * 12;
        totalEstimatedMs += avgDetailTime * estimatedDetailsFromRemainingPages;
      }
    } else if (stageType.includes('productdetail')) {
      // Detail 단계: 현재 Detail 남은 시간 + 아직 처리하지 않은 ListPage들의 Detail 시간
      totalEstimatedMs = detailStats?.estimated_remaining_ms || 0;
      
      console.log('[TimeEstimate] ProductDetail stage calculation:', {
        currentDetailRemaining: detailStats?.estimated_remaining_ms,
        totalListPages: stats.totalListPagesInSession,
        completedListPages: stats.completedListPagesInSession,
      });
      
      // 아직 처리하지 않은 ListPage들이 있다면 해당 페이지들의 예상 시간도 추가
      if (stats.listPageDurations.length > 0 && stats.totalListPagesInSession) {
        const completedListPages = stats.completedListPagesInSession;
        const totalListPages = stats.totalListPagesInSession;
        const remainingListPages = totalListPages - completedListPages;
        
        console.log('[TimeEstimate] Remaining ListPages calculation:', {
          remainingListPages,
          hasDetailDurations: stats.detailDurations.length > 0,
        });
        
        if (remainingListPages > 0) {
          // 남은 ListPage 크롤링 시간
          const avgListPageTime = stats.listPageDurations.reduce((a, b) => a + b, 0) / stats.listPageDurations.length;
          const listPageTimeToAdd = avgListPageTime * remainingListPages;
          totalEstimatedMs += listPageTimeToAdd;
          
          console.log('[TimeEstimate] Adding remaining ListPage time:', {
            avgListPageTime,
            remainingListPages,
            timeToAdd: listPageTimeToAdd,
          });
          
          // 남은 ListPage에서 나올 Detail 크롤링 시간
          if (stats.detailDurations.length > 0) {
            const avgDetailTime = stats.detailDurations.reduce((a, b) => a + b, 0) / stats.detailDurations.length;
            const estimatedDetailsFromRemainingPages = remainingListPages * 12; // 페이지당 평균 12개
            const detailTimeToAdd = avgDetailTime * estimatedDetailsFromRemainingPages;
            totalEstimatedMs += detailTimeToAdd;
            
            console.log('[TimeEstimate] Adding future Detail time:', {
              avgDetailTime,
              estimatedDetailsFromRemainingPages,
              timeToAdd: detailTimeToAdd,
            });
          }
        }
      }
      
      console.log('[TimeEstimate] Final total for ProductDetail stage:', totalEstimatedMs);
    }
    
    const estimatedCompletionTime = new Date(now.getTime() + totalEstimatedMs).toISOString();
    
    // 배치 정보 추가
    let batchInfo;
    if (stats.currentBatchIndex && stats.totalBatches && stats.batchId) {
      batchInfo = {
        current_batch: stats.currentBatchIndex,
        total_batches: stats.totalBatches,
        batch_id: stats.batchId,
      };
    }
    
    const result = {
      list_page_stats: listPageStats,
      detail_stats: detailStats,
      batch_info: batchInfo,
      total_estimated_remaining_ms: totalEstimatedMs,
      estimated_completion_time: estimatedCompletionTime,
    };
    
    return result;
  }

  // Normalize stage_type field that can be string or nested-enum object
  private normalizeStageType(stageTypeRaw: any): string {
    if (typeof stageTypeRaw === 'string') return stageTypeRaw.toLowerCase();
    if (stageTypeRaw && typeof stageTypeRaw === 'object') {
      const k = Object.keys(stageTypeRaw)[0];
      return (k || '').toLowerCase();
    }
    return '';
  }

  // Best-effort mapping from backend stage type to UI CrawlingStage enum
  private mapStageTypeToCrawlingStage(stageTypeLower: string): CrawlingStage {
    if (stageTypeLower.includes('listpage')) return CrawlingStage.ProductList;
    if (stageTypeLower.includes('productdetail')) return CrawlingStage.ProductDetails;
    if (stageTypeLower.includes('validation')) return CrawlingStage.DatabaseAnalysis;
    if (stageTypeLower.includes('database')) return CrawlingStage.Database;
    if (stageTypeLower.includes('saving') || stageTypeLower.includes('persist')) return CrawlingStage.DatabaseSave;
    return CrawlingStage.StatusCheck;
  }

  // StageItemType을 사용자 친화적 문자열로 변환
  private getItemTypeDisplay(itemType: any): string {
    if (typeof itemType === 'string') {
      return itemType;
    }
    
    if (typeof itemType === 'object' && itemType !== null) {
      if (itemType.Page) {
        return `Page ${itemType.Page.page_number}`;
      }
      if (itemType.Product) {
        return `Product (Page ${itemType.Product.page_number})`;
      }
      if (itemType.Url) {
        return `URL (${itemType.Url.url_type})`;
      }
      if (itemType.ProductUrls) {
        return `ProductUrls (${itemType.ProductUrls.urls.length} items)`;
      }
      if (itemType.ProductDetail) {
        return `ProductDetail (${itemType.ProductDetail.url})`;
      }
      if (itemType === 'SiteCheck') {
        return 'SiteCheck';
      }
    }
    
    return 'Unknown';
  }

  // Phase mapping removed

  // =========================================================================
  // 실시간 업데이트 및 자동 갱신 메서드
  // =========================================================================

  async startRealTimeUpdates(): Promise<void> {
    // This method is now handled by the unified subscribeToEvents
    console.log('🎧 Real-time updates are managed by the unified event bridge.');
  }

  stopAutoRefresh(): void {
    // 기존 구독 정리 로직
    console.log('🛑 Stopping auto refresh');
  }

  async refreshStatus(sessionId?: string): Promise<void> {
    if (!sessionId && !crawlerState.currentSessionId) return;
    
    const targetSessionId = sessionId || crawlerState.currentSessionId!;
    
    try {
      const result = await safeApiCall(() => apiAdapter.getCrawlingStatus(targetSessionId));
      
      if (result.error) {
        this.setError(result.error.message);
        return;
      }

      if (result.data) {
        // 세션 상태 업데이트
        setCrawlerState('currentSessionId', result.data.session_id);
      }
    } catch (error) {
      this.setError('Failed to refresh status');
    }
  }

  // =========================================================================
  // 사이트 분석 관리
  // =========================================================================

  /**
   * 사이트 종합 분석 실행
   */
  async performSiteAnalysis(): Promise<CrawlingStatusCheck | null> {
    try {
      console.log('🔍 사이트 종합 분석 시작...');
      
      setCrawlerState('isAnalyzing', true);
      setCrawlerState('lastError', null);
      
      const result = await tauriApi.checkSiteStatus();
      
      // Backend는 CrawlingResponse 구조로 반환하므로 data 필드에서 실제 데이터 추출
      if (result && result.success && result.data) {
        const analysisData = result.data;
        
        // Backend 응답을 Frontend가 기대하는 형식으로 변환
        const transformedResult: CrawlingStatusCheck = {
          database_status: {
            total_products: analysisData.database_analysis?.total_products || 0,
            last_updated: analysisData.database_analysis?.analyzed_at || new Date().toISOString(),
            last_crawl_time: analysisData.database_analysis?.analyzed_at,
            page_range: [
              analysisData.database_analysis?.max_page_id || 0, 
              (analysisData.database_analysis?.max_page_id || 0) + 10
            ] as [number, number],
            health: DatabaseHealth.Healthy,
            size_mb: 0 // TODO: 실제 DB 크기 계산
          },
          site_status: {
            is_accessible: (analysisData.site_analysis?.health_score || 0) > 0.5,
            response_time_ms: 0, // TODO: 실제 응답 시간 추가
            total_pages: analysisData.site_analysis?.total_pages || 0,
            estimated_products: analysisData.site_analysis?.estimated_products || 0,
            last_check_time: analysisData.site_analysis?.analyzed_at || new Date().toISOString(),
            health_score: analysisData.site_analysis?.health_score || 0,
            data_change_status: { Stable: { count: analysisData.site_analysis?.estimated_products || 0 } }
          },
          recommendation: {
            action: 'crawl' as const,
            priority: 'medium' as const,
            reason: `사이트: ${analysisData.site_analysis?.total_pages || 0}페이지, DB: ${analysisData.database_analysis?.total_products || 0}개 제품 저장됨`,
            suggested_range: [
              analysisData.range_preview?.start_page || 1, 
              analysisData.range_preview?.end_page || 10
            ] as [number, number],
            estimated_new_items: Math.max(0, (analysisData.site_analysis?.estimated_products || 0) - (analysisData.database_analysis?.total_products || 0)),
            efficiency_score: analysisData.site_analysis?.health_score || 0,
            next_steps: [`${analysisData.range_preview?.start_page || 1}페이지부터 크롤링 시작`]
          },
          sync_comparison: {
            database_count: analysisData.database_analysis?.total_products || 0,
            site_estimated_count: analysisData.site_analysis?.estimated_products || 0,
            sync_percentage: analysisData.database_analysis?.total_products && analysisData.site_analysis?.estimated_products 
              ? (analysisData.database_analysis.total_products / analysisData.site_analysis.estimated_products) * 100 
              : 0,
            last_sync_time: analysisData.database_analysis?.analyzed_at
          }
        };
        
        setCrawlerState('siteAnalysisResult', transformedResult);
        setCrawlerState('siteAnalysisTimestamp', new Date());
        
        console.log('✅ 사이트 분석 완료 및 변환:', transformedResult);
        console.log('📊 원본 Backend 데이터:', analysisData);
        
        return transformedResult;
      } else {
        console.error('❌ Backend 응답 구조가 예상과 다름:', result);
        setCrawlerState('lastError', 'Backend 응답 구조 오류');
        return null;
      }
      
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      console.error('❌ 사이트 분석 실패:', errorMessage);
      
      setCrawlerState('lastError', `사이트 분석 실패: ${errorMessage}`);
      return null;
      
    } finally {
      setCrawlerState('isAnalyzing', false);
    }
  }

  /**
   * 저장된 사이트 분석 결과 지우기
   */
  clearSiteAnalysis(): void {
    setCrawlerState('siteAnalysisResult', null);
    setCrawlerState('siteAnalysisTimestamp', null);
    console.log('🗑️ 사이트 분석 결과 삭제됨');
  }

  /**
   * 사이트 분석 결과가 유효한지 확인 (예: 1시간 이내)
   */
  isSiteAnalysisValid(maxAgeMinutes: number = 60): boolean {
    const timestamp = crawlerState.siteAnalysisTimestamp;
    if (!timestamp || !crawlerState.siteAnalysisResult) {
      return false;
    }
    
    const now = new Date();
    const ageMinutes = (now.getTime() - timestamp.getTime()) / (1000 * 60);
    return ageMinutes <= maxAgeMinutes;
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
        products_per_page: 12,
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
        products_per_page: 12,
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

  // =========================================================================
  // 세션 관리 메서드 (domain/crawling-store.ts에서 통합)
  // =========================================================================

  async startCrawling(dto: StartCrawlingDto): Promise<boolean> {
    setCrawlerState('isStarting', true);
    this.setError(null);

    try {
      const result = await safeApiCall(() => apiAdapter.startCrawling(dto));
      
      if (result.error) {
        this.setError(result.error.message);
        return false;
      }

      if (result.data) {
        setCrawlerState('currentSessionId', result.data.session_id);
        // 기존 progress 업데이트 로직 활용
        if (result.data.progress !== undefined) {
          this.setProgress({
            current: Math.floor(result.data.progress * 100),
            total: 100,
            percentage: result.data.progress,
            current_stage: CrawlingStage.ProductList,
            status: ((): CrawlingStatus => {
              const s = String(result.data.status || '').toLowerCase();
              if (s === 'running') return CrawlingStatus.Running;
              if (s === 'paused') return CrawlingStatus.Paused;
              if (s === 'completed') return CrawlingStatus.Completed;
              if (s === 'error') return CrawlingStatus.Error;
              if (s === 'cancelled') return CrawlingStatus.Cancelled;
              return CrawlingStatus.Idle;
            })(),
            new_items: 0,
            updated_items: 0,
            errors: 0,
            timestamp: result.data.last_updated || new Date().toISOString(),
            current_step: result.data.current_step,
            message: '',
            elapsed_time: 0,
          });
        }
        
        // 실시간 업데이트 시작
        this.startRealTimeUpdates().catch((error) => {
          console.error('Failed to start real-time updates:', error);
        });
        
        return true;
      }

      return false;
    } catch (error) {
      this.setError('Failed to start crawling');
      return false;
    } finally {
      setCrawlerState('isStarting', false);
    }
  }

  async stopCrawling(sessionId?: string): Promise<boolean> {
    const targetSessionId = sessionId || crawlerState.currentSessionId;
    if (!targetSessionId) return false;

    setCrawlerState('isStopping', true);
    this.setError(null);

    try {
  const result = await safeApiCall(() => apiAdapter.stopCrawling(targetSessionId));
      
      if (result.error) {
        this.setError(result.error.message);
        return false;
      }

      if (result.data) {
        setCrawlerState('currentSessionId', null);
        this.stopAutoRefresh();
        return true;
      }

      return false;
    } catch (error) {
      this.setError('Failed to stop crawling');
      return false;
    } finally {
      setCrawlerState('isStopping', false);
    }
  }

  async pauseCrawling(sessionId?: string): Promise<boolean> {
    const targetSessionId = sessionId || crawlerState.currentSessionId;
    if (!targetSessionId) return false;

    setCrawlerState('isPausing', true);
    this.setError(null);

    try {
  const result = await safeApiCall(() => apiAdapter.pauseCrawling(targetSessionId));
      
      if (result.error) {
        this.setError(result.error.message);
        return false;
      }

      return !!result.data;
    } catch (error) {
      this.setError('Failed to pause crawling');
      return false;
    } finally {
      setCrawlerState('isPausing', false);
    }
  }

  async resumeCrawling(sessionId?: string): Promise<boolean> {
    const targetSessionId = sessionId || crawlerState.currentSessionId;
    if (!targetSessionId) return false;

    setCrawlerState('isResuming', true);
    this.setError(null);

    try {
  const result = await safeApiCall(() => apiAdapter.resumeCrawling(targetSessionId));
      
      if (result.error) {
        this.setError(result.error.message);
        return false;
      }

      return !!result.data;
    } catch (error) {
      this.setError('Failed to resume crawling');
      return false;
    } finally {
      setCrawlerState('isResuming', false);
    }
  }

  async loadActiveSessions(): Promise<void> {
    this.setError(null);

    try {
      const result = await safeApiCall(() => apiAdapter.getActiveCrawlingSessions());
      
      if (result.error) {
        this.setError(result.error.message);
        return;
      }

      if (result.data) {
        setCrawlerState('activeSessions', result.data);
      }
    } catch (error) {
      this.setError('Failed to load active sessions');
    }
  }

  async loadSessionHistory(limit = 50): Promise<void> {
    this.setError(null);

    try {
      const result = await safeApiCall(() => apiAdapter.getCrawlingSessionHistory(limit));
      
      if (result.error) {
        this.setError(result.error.message);
        return;
      }

      if (result.data) {
        setCrawlerState('sessionHistory', result.data);
      }
    } catch (error) {
      this.setError('Failed to load session history');
    }
  }

  setCurrentSession(sessionId: string | null): void {
    setCrawlerState('currentSessionId', sessionId);
    if (sessionId) {
      this.refreshStatus(sessionId);
    } else {
      this.stopAutoRefresh();
    }
  }

  // 추가 getter 메서드들
  get currentSessionId() {
    return () => crawlerState.currentSessionId;
  }

  get activeSessions() {
    return () => crawlerState.activeSessions;
  }

  get sessionHistory() {
    return () => crawlerState.sessionHistory;
  }

  get isStarting() {
    return () => crawlerState.isStarting;
  }

  get isStopping() {
    return () => crawlerState.isStopping;
  }

  get isPausing() {
    return () => crawlerState.isPausing;
  }

  get isResuming() {
    return () => crawlerState.isResuming;
  }

  get isOperationPending() {
    return () => crawlerState.isStarting || crawlerState.isStopping || 
                 crawlerState.isPausing || crawlerState.isResuming;
  }
}

// 싱글톤 인스턴스 생성
export const crawlerStore = new CrawlerStore();

// 자동 정리 설정
onCleanup(() => {
  crawlerStore.cleanup();
});
