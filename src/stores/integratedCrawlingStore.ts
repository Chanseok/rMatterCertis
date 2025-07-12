/**
 * Integrated Crawling Store - 통합 크롤링 상태 관리
 * 
 * 이 스토어는 기존 상태 관리 시스템과 새로운 게임 스타일 시각화를 
 * 통합하여 일관된 상태 관리를 제공합니다.
 * 
 * v4.0 아키텍처 문서의 SystemStatePayload와 호환되도록 설계
 */

import { createStore } from 'solid-js/store';
import { onMount, onCleanup } from 'solid-js';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

// v4.0 아키텍처 호환 인터페이스
export interface SystemStatePayload {
  overallStatus: 'Idle' | 'Running' | 'Paused' | 'Stopping' | 'Completed' | 'Error';
  activeProfile: 'MaxPerformance' | 'Balanced' | 'EcoMode' | 'Custom';
  
  prediction: {
    estimatedCompletionISO: string;
    confidenceIntervalMinutes: [number, number];
    isAvailable: boolean;
  };

  progress: {
    totalTasks: number;
    completedTasks: number;
    percentage: number;
  };

  workerPools: WorkerPoolState[];

  resourceUsage: {
    cpuPercentage: number;
    memoryMb: number;
    memoryMaxMb: number;
  };

  errorCount: number;
  totalProductsSaved: number;
}

export interface WorkerPoolState {
  id: 'list_fetcher' | 'list_parser' | 'detail_fetcher' | 'detail_parser' | 'db_saver';
  name: string;
  activeWorkers: number;
  maxWorkers: number;
  queueDepth: number;
  queueCapacity: number;
  tasksPerMinute: number;
  avgTaskDurationMs: number;
  status: 'Idle' | 'Working' | 'Busy' | 'Error';
}

// 통합 크롤링 상태
export interface IntegratedCrawlingState {
  // 현재 상태
  isInitialized: boolean;
  
  // 백엔드 연동 상태
  isBackendConnected: boolean;
  lastBackendUpdate: string | null;
  
  // 시스템 상태 (v4.0 호환)
  systemState: SystemStatePayload | null;
  
  // 시뮬레이션 모드 (백엔드 연동 전 임시)
  simulationMode: boolean;
  
  // 뷰 모드 설정
  viewMode: 'classic' | 'city' | '3d' | 'metrics';
  
  // 사용자 제어 설정
  userPreferences: {
    autoRefreshInterval: number;
    showDetailedMetrics: boolean;
    enableAnimations: boolean;
    theme: 'light' | 'dark' | 'auto';
  };
}

// 초기 상태
const initialState: IntegratedCrawlingState = {
  isInitialized: false,
  isBackendConnected: false,
  lastBackendUpdate: null,
  systemState: null,
  simulationMode: true, // 기본적으로 시뮬레이션 모드로 시작
  viewMode: 'classic',
  userPreferences: {
    autoRefreshInterval: 1000,
    showDetailedMetrics: true,
    enableAnimations: true,
    theme: 'auto'
  }
};

// 스토어 생성
const [integratedState, setIntegratedState] = createStore<IntegratedCrawlingState>(initialState);

// 시뮬레이션 데이터 생성기
const createSimulationData = (): SystemStatePayload => {
  const now = new Date();
  const completionTime = new Date(now.getTime() + 30 * 60 * 1000); // 30분 후

  return {
    overallStatus: 'Running',
    activeProfile: 'Balanced',
    prediction: {
      estimatedCompletionISO: completionTime.toISOString(),
      confidenceIntervalMinutes: [25, 35],
      isAvailable: true
    },
    progress: {
      totalTasks: 1000,
      completedTasks: Math.floor(Math.random() * 800) + 200,
      percentage: Math.floor(Math.random() * 60) + 20
    },
    workerPools: [
      {
        id: 'list_fetcher',
        name: '목록 수집기',
        activeWorkers: 3,
        maxWorkers: 5,
        queueDepth: Math.floor(Math.random() * 20) + 5,
        queueCapacity: 50,
        tasksPerMinute: Math.floor(Math.random() * 10) + 15,
        avgTaskDurationMs: Math.floor(Math.random() * 1000) + 2000,
        status: 'Working'
      },
      {
        id: 'list_parser',
        name: '목록 파서',
        activeWorkers: 2,
        maxWorkers: 4,
        queueDepth: Math.floor(Math.random() * 15) + 3,
        queueCapacity: 30,
        tasksPerMinute: Math.floor(Math.random() * 8) + 12,
        avgTaskDurationMs: Math.floor(Math.random() * 800) + 1500,
        status: 'Working'
      },
      {
        id: 'detail_fetcher',
        name: '상세 수집기',
        activeWorkers: 4,
        maxWorkers: 8,
        queueDepth: Math.floor(Math.random() * 30) + 10,
        queueCapacity: 100,
        tasksPerMinute: Math.floor(Math.random() * 15) + 20,
        avgTaskDurationMs: Math.floor(Math.random() * 1500) + 3000,
        status: 'Busy'
      },
      {
        id: 'detail_parser',
        name: '상세 파서',
        activeWorkers: 3,
        maxWorkers: 6,
        queueDepth: Math.floor(Math.random() * 25) + 8,
        queueCapacity: 80,
        tasksPerMinute: Math.floor(Math.random() * 12) + 18,
        avgTaskDurationMs: Math.floor(Math.random() * 1200) + 2500,
        status: 'Working'
      },
      {
        id: 'db_saver',
        name: 'DB 저장기',
        activeWorkers: 2,
        maxWorkers: 3,
        queueDepth: Math.floor(Math.random() * 10) + 2,
        queueCapacity: 25,
        tasksPerMinute: Math.floor(Math.random() * 8) + 10,
        avgTaskDurationMs: Math.floor(Math.random() * 800) + 1000,
        status: 'Working'
      }
    ],
    resourceUsage: {
      cpuPercentage: Math.floor(Math.random() * 40) + 30,
      memoryMb: Math.floor(Math.random() * 1000) + 2000,
      memoryMaxMb: 8192
    },
    errorCount: Math.floor(Math.random() * 5),
    totalProductsSaved: Math.floor(Math.random() * 5000) + 10000
  };
};

// 액션 함수들
export const integratedActions = {
  // 초기화
  initialize: async () => {
    console.log('🔄 Integrated Crawling Store 초기화 중...');
    
    // 백엔드 연결 시도
    try {
      // TODO: 실제 백엔드 연결 로직 구현
      // const isConnected = await checkBackendConnection();
      const isConnected = false; // 임시로 false
      
      setIntegratedState({
        isInitialized: true,
        isBackendConnected: isConnected,
        simulationMode: !isConnected
      });
      
      if (!isConnected) {
        console.log('🎮 백엔드 연결 실패, 시뮬레이션 모드로 전환');
        integratedActions.startSimulation();
      } else {
        console.log('🔗 백엔드 연결 성공, 실시간 데이터 모드');
        integratedActions.startRealTimeUpdates();
      }
    } catch (error) {
      console.error('❌ 초기화 실패:', error);
      setIntegratedState({
        isInitialized: true,
        isBackendConnected: false,
        simulationMode: true
      });
      integratedActions.startSimulation();
    }
  },

  // 시뮬레이션 시작
  startSimulation: () => {
    console.log('🎮 시뮬레이션 모드 시작');
    
    const updateSimulation = () => {
      if (integratedState.simulationMode) {
        const simulationData = createSimulationData();
        setIntegratedState('systemState', simulationData);
        setIntegratedState('lastBackendUpdate', new Date().toISOString());
      }
    };
    
    // 즉시 한 번 실행
    updateSimulation();
    
    // 1초마다 업데이트
    const interval = setInterval(updateSimulation, 1000);
    
    // 정리 함수 등록
    onCleanup(() => {
      clearInterval(interval);
    });
  },

  // 실시간 업데이트 시작 (백엔드 연동)
  startRealTimeUpdates: async () => {
    console.log('🔗 실시간 업데이트 시작');
    
    try {
      // v4.0 아키텍처에 따른 이벤트 리스너 등록
      const unlisten = await listen<SystemStatePayload>('crawling-system-update', (event) => {
        setIntegratedState({
          systemState: event.payload,
          lastBackendUpdate: new Date().toISOString()
        });
      });
      
      // 정리 함수 등록
      onCleanup(() => {
        unlisten();
      });
    } catch (error) {
      console.error('❌ 실시간 업데이트 시작 실패:', error);
      // 폴백으로 시뮬레이션 모드로 전환
      setIntegratedState('simulationMode', true);
      integratedActions.startSimulation();
    }
  },

  // 뷰 모드 변경
  setViewMode: (mode: IntegratedCrawlingState['viewMode']) => {
    console.log(`🎨 뷰 모드 변경: ${mode}`);
    setIntegratedState('viewMode', mode);
    
    // 사용자 설정 저장
    localStorage.setItem('crawling-view-mode', mode);
  },

  // 사용자 설정 업데이트
  updateUserPreferences: (preferences: Partial<IntegratedCrawlingState['userPreferences']>) => {
    setIntegratedState('userPreferences', preferences);
    
    // 로컬 스토리지에 저장
    localStorage.setItem('crawling-user-preferences', JSON.stringify({
      ...integratedState.userPreferences,
      ...preferences
    }));
  },

  // 백엔드 연결 재시도
  reconnectBackend: async () => {
    console.log('🔄 백엔드 재연결 시도...');
    
    try {
      // TODO: 실제 백엔드 연결 로직
      const isConnected = false; // 임시
      
      if (isConnected) {
        setIntegratedState({
          isBackendConnected: true,
          simulationMode: false
        });
        integratedActions.startRealTimeUpdates();
      } else {
        throw new Error('연결 실패');
      }
    } catch (error) {
      console.error('❌ 백엔드 재연결 실패:', error);
    }
  },

  // 시뮬레이션 모드 토글
  toggleSimulationMode: () => {
    const newMode = !integratedState.simulationMode;
    setIntegratedState('simulationMode', newMode);
    
    if (newMode) {
      integratedActions.startSimulation();
    } else {
      integratedActions.startRealTimeUpdates();
    }
  },

  // 백엔드 연결 관리
  async connectToBackend(): Promise<boolean> {
    try {
      console.log('🔌 백엔드 연결 시도 중...');
      
      // Tauri API 서비스 연결 테스트
      const tauriApi = await import('../services/tauri-api');
      
      // 백엔드 상태 확인
      const isConnected = await tauriApi.tauriApi.checkBackendConnection();
      
      setIntegratedState('isBackendConnected', isConnected);
      setIntegratedState('lastBackendUpdate', new Date().toISOString());
      
      if (isConnected) {
        console.log('✅ 백엔드 연결 성공');
        // 실시간 이벤트 리스너 설정
        await integratedActions.setupRealTimeListeners();
      } else {
        console.log('❌ 백엔드 연결 실패 - 시뮬레이션 모드로 전환');
        setIntegratedState('simulationMode', true);
      }
      
      return isConnected;
    } catch (error) {
      console.error('❌ 백엔드 연결 오류:', error);
      setIntegratedState('isBackendConnected', false);
      setIntegratedState('simulationMode', true);
      return false;
    }
  },

  // 실시간 이벤트 리스너 설정
  async setupRealTimeListeners(): Promise<void> {
    try {
      console.log('🔄 실시간 이벤트 리스너 설정 중...');
      
      // 시스템 상태 업데이트 리스너
      await listen<SystemStatePayload>('system_state_update', (event) => {
        console.log('📡 시스템 상태 업데이트 수신:', event.payload);
        setIntegratedState('systemState', event.payload);
        setIntegratedState('lastBackendUpdate', new Date().toISOString());
      });

      // 진행률 업데이트 리스너
      await listen<any>('crawling_progress_update', (event) => {
        console.log('📊 진행률 업데이트 수신:', event.payload);
        if (integratedState.systemState) {
          setIntegratedState('systemState', 'progress', event.payload);
        }
      });

      // 에러 이벤트 리스너
      await listen<any>('crawling_error', (event) => {
        console.error('❌ 크롤링 에러 수신:', event.payload);
        if (integratedState.systemState) {
          setIntegratedState('systemState', 'errorCount', 
            (integratedState.systemState.errorCount || 0) + 1);
        }
      });

      console.log('✅ 실시간 이벤트 리스너 설정 완료');
    } catch (error) {
      console.error('❌ 실시간 이벤트 리스너 설정 실패:', error);
    }
  },

  // 백엔드 연결 해제
  async disconnectFromBackend(): Promise<void> {
    try {
      console.log('🔌 백엔드 연결 해제 중...');
      
      // 이벤트 리스너 정리
      // TODO: 실제 언리스너 함수들 저장 및 정리
      
      setIntegratedState('isBackendConnected', false);
      setIntegratedState('lastBackendUpdate', null);
      setIntegratedState('systemState', null);
      
      console.log('✅ 백엔드 연결 해제 완료');
    } catch (error) {
      console.error('❌ 백엔드 연결 해제 실패:', error);
    }
  },

  // 백엔드 연결 테스트 (실제 Tauri 명령어 사용)
  async testBackendConnection(): Promise<boolean> {
    try {
      console.log('🔄 백엔드 연결 테스트 중...');
      setIntegratedState('isBackendConnected', false);
      
      // 실제 백엔드 명령어들을 순차적으로 테스트
      const testCommands = [
        { name: 'get_local_db_stats', description: '로컬 DB 통계' },
        { name: 'get_comprehensive_crawler_config', description: '크롤러 설정' },
        { name: 'get_frontend_config', description: '프론트엔드 설정' }
      ];
      
      let successCount = 0;
      for (const cmd of testCommands) {
        try {
          console.log(`🔍 테스트 중: ${cmd.description}...`);
          await invoke(cmd.name);
          successCount++;
          console.log(`✅ ${cmd.description} 테스트 성공`);
        } catch (error) {
          console.warn(`⚠️ ${cmd.description} 테스트 실패:`, error);
        }
      }
      
      const isConnected = successCount >= 2; // 최소 2개 명령어 성공 시 연결된 것으로 간주
      setIntegratedState('isBackendConnected', isConnected);
      setIntegratedState('simulationMode', !isConnected);
      
      if (isConnected) {
        console.log(`✅ 백엔드 연결 성공 (${successCount}/${testCommands.length} 명령어 성공)`);
        // 실제 연결 시 실시간 리스너 설정
        await this.setupRealTimeListeners();
      } else {
        console.log(`❌ 백엔드 연결 실패 (${successCount}/${testCommands.length} 명령어 성공)`);
      }
      
      return isConnected;
    } catch (error) {
      console.error('❌ 백엔드 연결 테스트 오류:', error);
      setIntegratedState('isBackendConnected', false);
      setIntegratedState('simulationMode', true);
      return false;
    }
  },
};

// 컴포넌트에서 사용할 수 있도록 export
export { integratedState, setIntegratedState };

// 자동 초기화 훅
export const useIntegratedCrawlingStore = () => {
  onMount(() => {
    if (!integratedState.isInitialized) {
      integratedActions.initialize();
    }
  });
  
  return {
    state: integratedState,
    actions: integratedActions
  };
};
