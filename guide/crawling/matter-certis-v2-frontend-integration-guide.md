# Matter Certis v2 - Frontend Integration Guide: SolidJS + Tauri IPC

## 목차
1. [프론트엔드 아키텍처 설계](#프론트엔드-아키텍처-설계)
2. [옵션 1: UI 통합 구현](#옵션-1-ui-통합-구현)
3. [옵션 2: 고급 기능 구현](#옵션-2-고급-기능-구현)
4. [IPC 통신 계층](#ipc-통신-계층)
5. [상태 관리 시스템](#상태-관리-시스템)
6. [실시간 업데이트 시스템](#실시간-업데이트-시스템)

---

## 프론트엔드 아키텍처 설계

### 1.1 전체 구조 개요

```
Frontend (SolidJS)
├── stores/              # 상태 관리
│   ├── crawlerStore.ts  # 크롤링 상태
│   ├── databaseStore.ts # DB 상태
│   └── uiStore.ts       # UI 상태
├── components/          # 컴포넌트
│   ├── crawler/         # 크롤링 관련
│   ├── database/        # DB 관련
│   └── shared/          # 공통
├── services/            # 서비스 계층
│   ├── tauri-api.ts     # Tauri IPC
│   ├── websocket.ts     # 실시간 통신
│   └── types.ts         # 타입 정의
└── utils/               # 유틸리티
    ├── formatters.ts    # 데이터 포맷
    └── validators.ts    # 검증
```

### 1.2 타입 정의 시스템

```typescript
// src/services/types.ts
export interface CrawlingConfig {
  start_page: number;
  end_page: number;
  concurrency: number;
  delay_ms: number;
  auto_add_to_local_db: boolean;
  retry_max: number;
  page_timeout_ms: number;
}

export interface CrawlingProgress {
  current: number;
  total: number;
  percentage: number;
  current_stage: CrawlingStage;
  current_step: string;
  status: CrawlingStatus;
  message: string;
  remaining_time?: number;
  elapsed_time: number;
  new_items: number;
  updated_items: number;
  current_batch?: number;
  total_batches?: number;
  errors?: number;
}

export enum CrawlingStage {
  Idle = "Idle",
  TotalPages = "TotalPages", 
  ProductList = "ProductList",
  ProductDetail = "ProductDetail",
  Database = "Database",
}

export enum CrawlingStatus {
  Idle = "Idle",
  Running = "Running",
  Paused = "Paused",
  Completed = "Completed",
  Error = "Error",
}

export interface CrawlingTaskStatus {
  task_id: string;
  status: TaskStatus;
  message: string;
  timestamp: number;
  stage: CrawlingStage;
  details?: any;
}

export interface DatabaseStats {
  total_products: number;
  total_devices: number;
  last_updated: string;
  storage_size: string;
}

export interface CrawlingResult {
  total_processed: number;
  new_items: number;
  updated_items: number;
  errors: number;
  duration_ms: number;
  stages_completed: CrawlingStage[];
}
```

---

## 옵션 1: UI 통합 구현

### 2.1 실시간 진행 상황 표시 컴포넌트

```tsx
// src/components/crawler/CrawlingProgress.tsx
import { createSignal, createEffect, onCleanup } from 'solid-js';
import { tauriApi } from '../../services/tauri-api';
import type { CrawlingProgress } from '../../services/types';

export function CrawlingProgress() {
  const [progress, setProgress] = createSignal<CrawlingProgress | null>(null);
  const [isConnected, setIsConnected] = createSignal(false);

  let unsubscribe: (() => void) | null = null;

  createEffect(async () => {
    try {
      // Tauri 이벤트 리스너 등록
      unsubscribe = await tauriApi.subscribeToProgress((progressData) => {
        setProgress(progressData);
        setIsConnected(true);
      });
    } catch (error) {
      console.error('Failed to subscribe to progress:', error);
      setIsConnected(false);
    }
  });

  onCleanup(() => {
    if (unsubscribe) {
      unsubscribe();
    }
  });

  const getStageDisplayName = (stage: string) => {
    const stageNames = {
      'TotalPages': '총 페이지 수 확인',
      'ProductList': '제품 목록 수집',
      'ProductDetail': '제품 상세 정보 수집',
      'Database': '데이터베이스 저장',
    };
    return stageNames[stage] || stage;
  };

  const formatTime = (seconds: number) => {
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

  return (
    <div class="bg-white rounded-lg shadow-md p-6">
      <div class="flex items-center justify-between mb-4">
        <h3 class="text-lg font-semibold text-gray-800">크롤링 진행 상황</h3>
        <div class={`w-3 h-3 rounded-full ${isConnected() ? 'bg-green-500' : 'bg-red-500'}`}></div>
      </div>

      {progress() ? (
        <>
          {/* 전체 진행률 */}
          <div class="mb-6">
            <div class="flex justify-between items-center mb-2">
              <span class="text-sm font-medium text-gray-700">
                전체 진행률: {progress()!.percentage.toFixed(1)}%
              </span>
              <span class="text-sm text-gray-500">
                {progress()!.current} / {progress()!.total}
              </span>
            </div>
            <div class="w-full bg-gray-200 rounded-full h-3">
              <div 
                class="bg-blue-600 h-3 rounded-full transition-all duration-300 ease-out"
                style={`width: ${progress()!.percentage}%`}
              ></div>
            </div>
          </div>

          {/* 현재 단계 정보 */}
          <div class="grid grid-cols-2 gap-4 mb-4">
            <div class="bg-gray-50 p-3 rounded">
              <div class="text-sm text-gray-600">현재 단계</div>
              <div class="font-semibold text-gray-800">
                {getStageDisplayName(progress()!.current_stage)}
              </div>
            </div>
            <div class="bg-gray-50 p-3 rounded">
              <div class="text-sm text-gray-600">상태</div>
              <div class={`font-semibold ${
                progress()!.status === 'Running' ? 'text-blue-600' :
                progress()!.status === 'Completed' ? 'text-green-600' :
                progress()!.status === 'Error' ? 'text-red-600' : 'text-gray-600'
              }`}>
                {progress()!.status}
              </div>
            </div>
          </div>

          {/* 배치 정보 (있는 경우) */}
          {progress()!.current_batch && progress()!.total_batches && (
            <div class="bg-blue-50 p-3 rounded mb-4">
              <div class="text-sm text-blue-700">
                배치 진행: {progress()!.current_batch} / {progress()!.total_batches}
              </div>
            </div>
          )}

          {/* 통계 정보 */}
          <div class="grid grid-cols-3 gap-4 mb-4">
            <div class="text-center">
              <div class="text-2xl font-bold text-green-600">{progress()!.new_items}</div>
              <div class="text-sm text-gray-600">신규</div>
            </div>
            <div class="text-center">
              <div class="text-2xl font-bold text-blue-600">{progress()!.updated_items}</div>
              <div class="text-sm text-gray-600">업데이트</div>
            </div>
            <div class="text-center">
              <div class="text-2xl font-bold text-red-600">{progress()!.errors || 0}</div>
              <div class="text-sm text-gray-600">에러</div>
            </div>
          </div>

          {/* 시간 정보 */}
          <div class="flex justify-between text-sm text-gray-600">
            <span>소요 시간: {formatTime(progress()!.elapsed_time)}</span>
            {progress()!.remaining_time && (
              <span>예상 남은 시간: {formatTime(progress()!.remaining_time)}</span>
            )}
          </div>

          {/* 현재 메시지 */}
          <div class="mt-4 p-3 bg-gray-50 rounded">
            <div class="text-sm text-gray-700">{progress()!.message}</div>
          </div>
        </>
      ) : (
        <div class="text-center py-8 text-gray-500">
          크롤링이 시작되지 않았습니다.
        </div>
      )}
    </div>
  );
}
```

### 2.2 크롤링 제어 컴포넌트

```tsx
// src/components/crawler/CrawlingControls.tsx
import { createSignal, createEffect } from 'solid-js';
import { tauriApi } from '../../services/tauri-api';
import { crawlerStore } from '../../stores/crawlerStore';
import type { CrawlingConfig } from '../../services/types';

export function CrawlingControls() {
  const [config, setConfig] = createSignal<CrawlingConfig>({
    start_page: 1,
    end_page: 10,
    concurrency: 3,
    delay_ms: 1000,
    auto_add_to_local_db: true,
    retry_max: 3,
    page_timeout_ms: 30000,
  });

  const [isLoading, setIsLoading] = createSignal(false);

  const handleStartCrawling = async () => {
    setIsLoading(true);
    try {
      await tauriApi.startCrawling(config());
      crawlerStore.setStatus('Running');
    } catch (error) {
      console.error('Failed to start crawling:', error);
      alert(`크롤링 시작 실패: ${error}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handlePauseCrawling = async () => {
    setIsLoading(true);
    try {
      await tauriApi.pauseCrawling();
      crawlerStore.setStatus('Paused');
    } catch (error) {
      console.error('Failed to pause crawling:', error);
    } finally {
      setIsLoading(false);
    }
  };

  const handleStopCrawling = async () => {
    setIsLoading(true);
    try {
      await tauriApi.stopCrawling();
      crawlerStore.setStatus('Idle');
    } catch (error) {
      console.error('Failed to stop crawling:', error);
    } finally {
      setIsLoading(false);
    }
  };

  const handleResumeCrawling = async () => {
    setIsLoading(true);
    try {
      await tauriApi.resumeCrawling();
      crawlerStore.setStatus('Running');
    } catch (error) {
      console.error('Failed to resume crawling:', error);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div class="bg-white rounded-lg shadow-md p-6">
      <h3 class="text-lg font-semibold text-gray-800 mb-4">크롤링 설정 및 제어</h3>
      
      {/* 설정 폼 */}
      <div class="grid grid-cols-2 gap-4 mb-6">
        <div>
          <label class="block text-sm font-medium text-gray-700 mb-1">시작 페이지</label>
          <input
            type="number"
            min="1"
            value={config().start_page}
            onInput={(e) => setConfig(prev => ({ ...prev, start_page: parseInt(e.target.value) || 1 }))}
            class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
            disabled={crawlerStore.status() === 'Running'}
          />
        </div>
        <div>
          <label class="block text-sm font-medium text-gray-700 mb-1">종료 페이지</label>
          <input
            type="number"
            min="1"
            value={config().end_page}
            onInput={(e) => setConfig(prev => ({ ...prev, end_page: parseInt(e.target.value) || 10 }))}
            class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
            disabled={crawlerStore.status() === 'Running'}
          />
        </div>
        <div>
          <label class="block text-sm font-medium text-gray-700 mb-1">동시 처리 수</label>
          <input
            type="number"
            min="1"
            max="10"
            value={config().concurrency}
            onInput={(e) => setConfig(prev => ({ ...prev, concurrency: parseInt(e.target.value) || 3 }))}
            class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
            disabled={crawlerStore.status() === 'Running'}
          />
        </div>
        <div>
          <label class="block text-sm font-medium text-gray-700 mb-1">지연 시간 (ms)</label>
          <input
            type="number"
            min="100"
            step="100"
            value={config().delay_ms}
            onInput={(e) => setConfig(prev => ({ ...prev, delay_ms: parseInt(e.target.value) || 1000 }))}
            class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
            disabled={crawlerStore.status() === 'Running'}
          />
        </div>
      </div>

      {/* 옵션 체크박스 */}
      <div class="mb-6">
        <label class="flex items-center">
          <input
            type="checkbox"
            checked={config().auto_add_to_local_db}
            onChange={(e) => setConfig(prev => ({ ...prev, auto_add_to_local_db: e.target.checked }))}
            class="mr-2"
            disabled={crawlerStore.status() === 'Running'}
          />
          <span class="text-sm text-gray-700">자동으로 로컬 DB에 저장</span>
        </label>
      </div>

      {/* 제어 버튼 */}
      <div class="flex gap-3">
        {crawlerStore.status() === 'Idle' && (
          <button
            onClick={handleStartCrawling}
            disabled={isLoading()}
            class="flex-1 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-400 text-white font-medium py-2 px-4 rounded-md transition-colors"
          >
            {isLoading() ? '시작 중...' : '크롤링 시작'}
          </button>
        )}
        
        {crawlerStore.status() === 'Running' && (
          <>
            <button
              onClick={handlePauseCrawling}
              disabled={isLoading()}
              class="flex-1 bg-yellow-600 hover:bg-yellow-700 disabled:bg-gray-400 text-white font-medium py-2 px-4 rounded-md transition-colors"
            >
              {isLoading() ? '일시정지 중...' : '일시정지'}
            </button>
            <button
              onClick={handleStopCrawling}
              disabled={isLoading()}
              class="flex-1 bg-red-600 hover:bg-red-700 disabled:bg-gray-400 text-white font-medium py-2 px-4 rounded-md transition-colors"
            >
              {isLoading() ? '중단 중...' : '중단'}
            </button>
          </>
        )}
        
        {crawlerStore.status() === 'Paused' && (
          <>
            <button
              onClick={handleResumeCrawling}
              disabled={isLoading()}
              class="flex-1 bg-green-600 hover:bg-green-700 disabled:bg-gray-400 text-white font-medium py-2 px-4 rounded-md transition-colors"
            >
              {isLoading() ? '재시작 중...' : '재시작'}
            </button>
            <button
              onClick={handleStopCrawling}
              disabled={isLoading()}
              class="flex-1 bg-red-600 hover:bg-red-700 disabled:bg-gray-400 text-white font-medium py-2 px-4 rounded-md transition-colors"
            >
              {isLoading() ? '중단 중...' : '완전 중단'}
            </button>
          </>
        )}
      </div>
    </div>
  );
}
```

### 2.3 결과 요약 및 에러 리포트

```tsx
// src/components/crawler/CrawlingResults.tsx
import { createSignal, createEffect, For } from 'solid-js';
import { tauriApi } from '../../services/tauri-api';
import type { CrawlingResult, CrawlingTaskStatus } from '../../services/types';

export function CrawlingResults() {
  const [results, setResults] = createSignal<CrawlingResult | null>(null);
  const [errors, setErrors] = createSignal<CrawlingTaskStatus[]>([]);
  const [isLoading, setIsLoading] = createSignal(false);

  const loadResults = async () => {
    setIsLoading(true);
    try {
      const [resultData, errorData] = await Promise.all([
        tauriApi.getCrawlingResults(),
        tauriApi.getCrawlingErrors()
      ]);
      setResults(resultData);
      setErrors(errorData);
    } catch (error) {
      console.error('Failed to load results:', error);
    } finally {
      setIsLoading(false);
    }
  };

  createEffect(() => {
    loadResults();
  });

  const formatDuration = (ms: number) => {
    const seconds = Math.floor(ms / 1000);
    const minutes = Math.floor(seconds / 60);
    const hours = Math.floor(minutes / 60);
    
    if (hours > 0) {
      return `${hours}시간 ${minutes % 60}분`;
    } else if (minutes > 0) {
      return `${minutes}분 ${seconds % 60}초`;
    } else {
      return `${seconds}초`;
    }
  };

  const exportResults = async () => {
    try {
      await tauriApi.exportCrawlingResults();
      alert('결과가 성공적으로 내보내졌습니다.');
    } catch (error) {
      alert(`내보내기 실패: ${error}`);
    }
  };

  const clearErrors = async () => {
    try {
      await tauriApi.clearCrawlingErrors();
      setErrors([]);
    } catch (error) {
      console.error('Failed to clear errors:', error);
    }
  };

  return (
    <div class="space-y-6">
      {/* 결과 요약 */}
      <div class="bg-white rounded-lg shadow-md p-6">
        <div class="flex items-center justify-between mb-4">
          <h3 class="text-lg font-semibold text-gray-800">크롤링 결과 요약</h3>
          <button
            onClick={loadResults}
            disabled={isLoading()}
            class="px-4 py-2 text-sm bg-blue-600 hover:bg-blue-700 disabled:bg-gray-400 text-white rounded-md transition-colors"
          >
            {isLoading() ? '로딩 중...' : '새로고침'}
          </button>
        </div>

        {results() ? (
          <>
            <div class="grid grid-cols-2 md:grid-cols-4 gap-4 mb-6">
              <div class="text-center p-4 bg-gray-50 rounded">
                <div class="text-2xl font-bold text-blue-600">{results()!.total_processed}</div>
                <div class="text-sm text-gray-600">총 처리</div>
              </div>
              <div class="text-center p-4 bg-green-50 rounded">
                <div class="text-2xl font-bold text-green-600">{results()!.new_items}</div>
                <div class="text-sm text-gray-600">신규 항목</div>
              </div>
              <div class="text-center p-4 bg-yellow-50 rounded">
                <div class="text-2xl font-bold text-yellow-600">{results()!.updated_items}</div>
                <div class="text-sm text-gray-600">업데이트</div>
              </div>
              <div class="text-center p-4 bg-red-50 rounded">
                <div class="text-2xl font-bold text-red-600">{results()!.errors}</div>
                <div class="text-sm text-gray-600">에러</div>
              </div>
            </div>

            <div class="flex justify-between items-center mb-4">
              <div class="text-sm text-gray-600">
                총 소요 시간: {formatDuration(results()!.duration_ms)}
              </div>
              <div class="text-sm text-gray-600">
                완료된 단계: {results()!.stages_completed.join(', ')}
              </div>
            </div>

            <button
              onClick={exportResults}
              class="w-full bg-green-600 hover:bg-green-700 text-white font-medium py-2 px-4 rounded-md transition-colors"
            >
              결과 내보내기 (CSV)
            </button>
          </>
        ) : (
          <div class="text-center py-8 text-gray-500">
            크롤링 결과가 없습니다.
          </div>
        )}
      </div>

      {/* 에러 리포트 */}
      <div class="bg-white rounded-lg shadow-md p-6">
        <div class="flex items-center justify-between mb-4">
          <h3 class="text-lg font-semibold text-gray-800">
            에러 리포트 ({errors().length})
          </h3>
          {errors().length > 0 && (
            <button
              onClick={clearErrors}
              class="px-4 py-2 text-sm bg-red-600 hover:bg-red-700 text-white rounded-md transition-colors"
            >
              에러 지우기
            </button>
          )}
        </div>

        {errors().length > 0 ? (
          <div class="space-y-3 max-h-96 overflow-y-auto">
            <For each={errors()}>
              {(error) => (
                <div class="border border-red-200 bg-red-50 rounded-lg p-4">
                  <div class="flex justify-between items-start mb-2">
                    <div class="font-medium text-red-800">
                      {error.task_id}
                    </div>
                    <div class="text-xs text-red-600">
                      {new Date(error.timestamp).toLocaleString()}
                    </div>
                  </div>
                  <div class="text-sm text-red-700 mb-2">
                    단계: {error.stage}
                  </div>
                  <div class="text-sm text-red-600 bg-red-100 p-2 rounded">
                    {error.message}
                  </div>
                  {error.details && (
                    <details class="mt-2">
                      <summary class="text-xs text-red-600 cursor-pointer">상세 정보</summary>
                      <pre class="text-xs text-red-500 mt-1 whitespace-pre-wrap overflow-x-auto">
                        {JSON.stringify(error.details, null, 2)}
                      </pre>
                    </details>
                  )}
                </div>
              )}
            </For>
          </div>
        ) : (
          <div class="text-center py-8 text-gray-500">
            에러가 없습니다.
          </div>
        )}
      </div>
    </div>
  );
}
```

---

## 옵션 2: 고급 기능 구현

### 3.1 병렬 크롤링 모니터링

```tsx
// src/components/crawler/ParallelCrawlingMonitor.tsx
import { createSignal, createEffect, For } from 'solid-js';
import { tauriApi } from '../../services/tauri-api';

interface ParallelTask {
  id: string;
  url: string;
  status: 'pending' | 'running' | 'completed' | 'failed';
  start_time?: number;
  end_time?: number;
  error_message?: string;
  stage: string;
}

export function ParallelCrawlingMonitor() {
  const [tasks, setTasks] = createSignal<ParallelTask[]>([]);
  const [maxConcurrency, setMaxConcurrency] = createSignal(5);
  const [activeTasks, setActiveTasks] = createSignal(0);

  let eventUnsubscribe: (() => void) | null = null;

  createEffect(async () => {
    try {
      // 병렬 작업 상태 구독
      eventUnsubscribe = await tauriApi.subscribeToParallelTasks((taskUpdate) => {
        setTasks(prev => {
          const existing = prev.find(t => t.id === taskUpdate.id);
          if (existing) {
            return prev.map(t => t.id === taskUpdate.id ? { ...t, ...taskUpdate } : t);
          } else {
            return [...prev, taskUpdate];
          }
        });

        // 활성 작업 수 업데이트
        setActiveTasks(prev => {
          const runningCount = tasks().filter(t => t.status === 'running').length;
          return runningCount;
        });
      });
    } catch (error) {
      console.error('Failed to subscribe to parallel tasks:', error);
    }
  });

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'pending': return 'bg-gray-500';
      case 'running': return 'bg-blue-500';
      case 'completed': return 'bg-green-500';
      case 'failed': return 'bg-red-500';
      default: return 'bg-gray-500';
    }
  };

  const getStatusText = (status: string) => {
    switch (status) {
      case 'pending': return '대기';
      case 'running': return '실행 중';
      case 'completed': return '완료';
      case 'failed': return '실패';
      default: return '알 수 없음';
    }
  };

  const formatDuration = (startTime?: number, endTime?: number) => {
    if (!startTime) return '-';
    const end = endTime || Date.now();
    const duration = Math.floor((end - startTime) / 1000);
    return `${duration}초`;
  };

  return (
    <div class="bg-white rounded-lg shadow-md p-6">
      <div class="flex items-center justify-between mb-4">
        <h3 class="text-lg font-semibold text-gray-800">병렬 크롤링 모니터</h3>
        <div class="flex items-center space-x-4">
          <div class="text-sm text-gray-600">
            활성: {activeTasks()} / {maxConcurrency()}
          </div>
          <div class="w-12 h-2 bg-gray-200 rounded-full">
            <div 
              class="h-2 bg-blue-500 rounded-full transition-all"
              style={`width: ${(activeTasks() / maxConcurrency()) * 100}%`}
            ></div>
          </div>
        </div>
      </div>

      {/* 통계 */}
      <div class="grid grid-cols-4 gap-4 mb-6">
        <div class="text-center p-3 bg-gray-50 rounded">
          <div class="text-lg font-semibold text-gray-700">
            {tasks().filter(t => t.status === 'pending').length}
          </div>
          <div class="text-xs text-gray-500">대기</div>
        </div>
        <div class="text-center p-3 bg-blue-50 rounded">
          <div class="text-lg font-semibold text-blue-700">
            {tasks().filter(t => t.status === 'running').length}
          </div>
          <div class="text-xs text-blue-500">실행 중</div>
        </div>
        <div class="text-center p-3 bg-green-50 rounded">
          <div class="text-lg font-semibold text-green-700">
            {tasks().filter(t => t.status === 'completed').length}
          </div>
          <div class="text-xs text-green-500">완료</div>
        </div>
        <div class="text-center p-3 bg-red-50 rounded">
          <div class="text-lg font-semibold text-red-700">
            {tasks().filter(t => t.status === 'failed').length}
          </div>
          <div class="text-xs text-red-500">실패</div>
        </div>
      </div>

      {/* 작업 목록 */}
      <div class="max-h-96 overflow-y-auto">
        <For each={tasks().slice(0, 50)}>
          {(task) => (
            <div class="flex items-center justify-between p-3 border-b border-gray-100 hover:bg-gray-50">
              <div class="flex items-center space-x-3">
                <div class={`w-3 h-3 rounded-full ${getStatusColor(task.status)}`}></div>
                <div class="flex-1 min-w-0">
                  <div class="text-sm font-medium text-gray-900 truncate">
                    {task.url.length > 50 ? `${task.url.substring(0, 50)}...` : task.url}
                  </div>
                  <div class="text-xs text-gray-500">
                    {task.stage} | {getStatusText(task.status)}
                  </div>
                </div>
              </div>
              <div class="text-xs text-gray-500">
                {formatDuration(task.start_time, task.end_time)}
              </div>
            </div>
          )}
        </For>
        
        {tasks().length > 50 && (
          <div class="p-3 text-center text-sm text-gray-500">
            ... 그리고 {tasks().length - 50}개 더
          </div>
        )}
      </div>
    </div>
  );
}
```

### 3.2 데이터베이스 연동 컴포넌트

```tsx
// src/components/database/DatabaseIntegration.tsx
import { createSignal, createEffect } from 'solid-js';
import { tauriApi } from '../../services/tauri-api';
import type { DatabaseStats } from '../../services/types';

export function DatabaseIntegration() {
  const [stats, setStats] = createSignal<DatabaseStats | null>(null);
  const [isConnected, setIsConnected] = createSignal(false);
  const [isLoading, setIsLoading] = createSignal(false);

  const loadDatabaseStats = async () => {
    setIsLoading(true);
    try {
      const dbStats = await tauriApi.getDatabaseStats();
      setStats(dbStats);
      setIsConnected(true);
    } catch (error) {
      console.error('Failed to load database stats:', error);
      setIsConnected(false);
    } finally {
      setIsLoading(false);
    }
  };

  createEffect(() => {
    loadDatabaseStats();
    // 주기적 업데이트
    const interval = setInterval(loadDatabaseStats, 30000);
    return () => clearInterval(interval);
  });

  const handleBackupDatabase = async () => {
    setIsLoading(true);
    try {
      await tauriApi.backupDatabase();
      alert('데이터베이스 백업이 완료되었습니다.');
    } catch (error) {
      alert(`백업 실패: ${error}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleOptimizeDatabase = async () => {
    setIsLoading(true);
    try {
      await tauriApi.optimizeDatabase();
      alert('데이터베이스 최적화가 완료되었습니다.');
      await loadDatabaseStats(); // 통계 새로고침
    } catch (error) {
      alert(`최적화 실패: ${error}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleExportData = async (format: 'csv' | 'json' | 'excel') => {
    setIsLoading(true);
    try {
      await tauriApi.exportDatabaseData(format);
      alert(`${format.toUpperCase()} 형식으로 데이터가 내보내졌습니다.`);
    } catch (error) {
      alert(`내보내기 실패: ${error}`);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div class="bg-white rounded-lg shadow-md p-6">
      <div class="flex items-center justify-between mb-4">
        <h3 class="text-lg font-semibold text-gray-800">데이터베이스 연동</h3>
        <div class="flex items-center space-x-2">
          <div class={`w-3 h-3 rounded-full ${isConnected() ? 'bg-green-500' : 'bg-red-500'}`}></div>
          <span class="text-sm text-gray-600">
            {isConnected() ? '연결됨' : '연결 안됨'}
          </span>
        </div>
      </div>

      {stats() ? (
        <>
          {/* 데이터베이스 통계 */}
          <div class="grid grid-cols-2 md:grid-cols-3 gap-4 mb-6">
            <div class="text-center p-4 bg-blue-50 rounded">
              <div class="text-2xl font-bold text-blue-600">{stats()!.total_products.toLocaleString()}</div>
              <div class="text-sm text-blue-700">총 제품 수</div>
            </div>
            <div class="text-center p-4 bg-green-50 rounded">
              <div class="text-2xl font-bold text-green-600">{stats()!.total_devices.toLocaleString()}</div>
              <div class="text-sm text-green-700">총 디바이스 수</div>
            </div>
            <div class="text-center p-4 bg-yellow-50 rounded">
              <div class="text-lg font-bold text-yellow-600">{stats()!.storage_size}</div>
              <div class="text-sm text-yellow-700">저장소 크기</div>
            </div>
          </div>

          <div class="mb-6">
            <div class="text-sm text-gray-600">
              마지막 업데이트: {new Date(stats()!.last_updated).toLocaleString()}
            </div>
          </div>

          {/* 데이터베이스 작업 버튼 */}
          <div class="space-y-3">
            <div class="flex space-x-3">
              <button
                onClick={handleBackupDatabase}
                disabled={isLoading()}
                class="flex-1 bg-blue-600 hover:bg-blue-700 disabled:bg-gray-400 text-white font-medium py-2 px-4 rounded-md transition-colors"
              >
                백업 생성
              </button>
              <button
                onClick={handleOptimizeDatabase}
                disabled={isLoading()}
                class="flex-1 bg-green-600 hover:bg-green-700 disabled:bg-gray-400 text-white font-medium py-2 px-4 rounded-md transition-colors"
              >
                DB 최적화
              </button>
            </div>

            <div class="border-t pt-3">
              <div class="text-sm font-medium text-gray-700 mb-2">데이터 내보내기:</div>
              <div class="flex space-x-2">
                <button
                  onClick={() => handleExportData('csv')}
                  disabled={isLoading()}
                  class="flex-1 bg-gray-600 hover:bg-gray-700 disabled:bg-gray-400 text-white text-sm py-2 px-3 rounded-md transition-colors"
                >
                  CSV
                </button>
                <button
                  onClick={() => handleExportData('json')}
                  disabled={isLoading()}
                  class="flex-1 bg-gray-600 hover:bg-gray-700 disabled:bg-gray-400 text-white text-sm py-2 px-3 rounded-md transition-colors"
                >
                  JSON
                </button>
                <button
                  onClick={() => handleExportData('excel')}
                  disabled={isLoading()}
                  class="flex-1 bg-gray-600 hover:bg-gray-700 disabled:bg-gray-400 text-white text-sm py-2 px-3 rounded-md transition-colors"
                >
                  Excel
                </button>
              </div>
            </div>
          </div>
        </>
      ) : (
        <div class="text-center py-8">
          {isLoading() ? (
            <div class="text-gray-500">데이터베이스 상태를 확인하는 중...</div>
          ) : (
            <div class="text-red-500">데이터베이스에 연결할 수 없습니다.</div>
          )}
        </div>
      )}
    </div>
  );
}
```

---

## IPC 통신 계층

### 4.1 Tauri API 서비스

```typescript
// src/services/tauri-api.ts
import { invoke } from '@tauri-apps/api/tauri';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import type { 
  CrawlingConfig, 
  CrawlingProgress, 
  CrawlingResult,
  CrawlingTaskStatus,
  DatabaseStats 
} from './types';

class TauriApiService {
  // 크롤링 제어
  async startCrawling(config: CrawlingConfig): Promise<void> {
    return invoke('start_crawling', { config });
  }

  async pauseCrawling(): Promise<void> {
    return invoke('pause_crawling');
  }

  async resumeCrawling(): Promise<void> {
    return invoke('resume_crawling');
  }

  async stopCrawling(): Promise<void> {
    return invoke('stop_crawling');
  }

  // 상태 조회
  async getCrawlingStatus(): Promise<CrawlingProgress> {
    return invoke('get_crawling_status');
  }

  async getCrawlingResults(): Promise<CrawlingResult> {
    return invoke('get_crawling_results');
  }

  async getCrawlingErrors(): Promise<CrawlingTaskStatus[]> {
    return invoke('get_crawling_errors');
  }

  // 데이터베이스
  async getDatabaseStats(): Promise<DatabaseStats> {
    return invoke('get_database_stats');
  }

  async backupDatabase(): Promise<void> {
    return invoke('backup_database');
  }

  async optimizeDatabase(): Promise<void> {
    return invoke('optimize_database');
  }

  async exportDatabaseData(format: 'csv' | 'json' | 'excel'): Promise<void> {
    return invoke('export_database_data', { format });
  }

  async exportCrawlingResults(): Promise<void> {
    return invoke('export_crawling_results');
  }

  async clearCrawlingErrors(): Promise<void> {
    return invoke('clear_crawling_errors');
  }

  // 이벤트 구독
  async subscribeToProgress(callback: (progress: CrawlingProgress) => void): Promise<UnlistenFn> {
    return listen('crawling-progress', (event) => {
      callback(event.payload as CrawlingProgress);
    });
  }

  async subscribeToTaskStatus(callback: (status: CrawlingTaskStatus) => void): Promise<UnlistenFn> {
    return listen('crawling-task-status', (event) => {
      callback(event.payload as CrawlingTaskStatus);
    });
  }

  async subscribeToParallelTasks(callback: (task: any) => void): Promise<UnlistenFn> {
    return listen('parallel-task-update', (event) => {
      callback(event.payload);
    });
  }

  async subscribeToStageChange(callback: (stage: { stage: string; message: string }) => void): Promise<UnlistenFn> {
    return listen('crawling-stage-changed', (event) => {
      callback(event.payload as { stage: string; message: string });
    });
  }

  async subscribeToErrors(callback: (error: string) => void): Promise<UnlistenFn> {
    return listen('crawling-error', (event) => {
      callback(event.payload as string);
    });
  }
}

export const tauriApi = new TauriApiService();
```

### 4.2 백엔드 Tauri Commands

```rust
// src-tauri/src/commands.rs
use tauri::State;
use crate::crawler::{CrawlerEngine, CrawlingConfig};
use crate::database::DatabaseManager;
use crate::state::AppState;

#[tauri::command]
pub async fn start_crawling(
    config: CrawlingConfig,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let mut crawler = state.crawler.lock().await;
    
    // 이벤트 구독 설정
    let app_handle_clone = app_handle.clone();
    crawler.set_progress_callback(move |progress| {
        let _ = app_handle_clone.emit_all("crawling-progress", &progress);
    });

    let app_handle_clone = app_handle.clone();
    crawler.set_task_status_callback(move |status| {
        let _ = app_handle_clone.emit_all("crawling-task-status", &status);
    });

    crawler.start(config).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pause_crawling(state: State<'_, AppState>) -> Result<(), String> {
    let mut crawler = state.crawler.lock().await;
    crawler.pause().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn resume_crawling(state: State<'_, AppState>) -> Result<(), String> {
    let mut crawler = state.crawler.lock().await;
    crawler.resume().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_crawling(state: State<'_, AppState>) -> Result<(), String> {
    let mut crawler = state.crawler.lock().await;
    crawler.stop().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_crawling_status(state: State<'_, AppState>) -> Result<CrawlingProgress, String> {
    let crawler = state.crawler.lock().await;
    crawler.get_progress().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_database_stats(state: State<'_, AppState>) -> Result<DatabaseStats, String> {
    let db = state.database.lock().await;
    db.get_stats().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn backup_database(state: State<'_, AppState>) -> Result<(), String> {
    let db = state.database.lock().await;
    db.create_backup().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn optimize_database(state: State<'_, AppState>) -> Result<(), String> {
    let db = state.database.lock().await;
    db.optimize().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_database_data(
    format: String,
    state: State<'_, AppState>
) -> Result<(), String> {
    let db = state.database.lock().await;
    match format.as_str() {
        "csv" => db.export_to_csv().await,
        "json" => db.export_to_json().await,
        "excel" => db.export_to_excel().await,
        _ => Err("Unsupported format".to_string()),
    }.map_err(|e| e.to_string())
}
```

---

## 상태 관리 시스템

### 5.1 크롤러 상태 스토어

```typescript
// src/stores/crawlerStore.ts
import { createSignal, createEffect } from 'solid-js';
import { createStore } from 'solid-js/store';
import { tauriApi } from '../services/tauri-api';
import type { CrawlingProgress, CrawlingStatus } from '../services/types';

interface CrawlerState {
  status: CrawlingStatus;
  progress: CrawlingProgress | null;
  isConnected: boolean;
  lastError: string | null;
}

const [crawlerState, setCrawlerState] = createStore<CrawlerState>({
  status: 'Idle',
  progress: null,
  isConnected: false,
  lastError: null,
});

const [eventSubscriptions] = createSignal<(() => void)[]>([]);

class CrawlerStore {
  get state() {
    return crawlerState;
  }

  get status() {
    return () => crawlerState.status;
  }

  get progress() {
    return () => crawlerState.progress;
  }

  get isConnected() {
    return () => crawlerState.isConnected;
  }

  get lastError() {
    return () => crawlerState.lastError;
  }

  setStatus(status: CrawlingStatus) {
    setCrawlerState('status', status);
  }

  setProgress(progress: CrawlingProgress) {
    setCrawlerState('progress', progress);
    setCrawlerState('status', progress.status);
  }

  setConnected(connected: boolean) {
    setCrawlerState('isConnected', connected);
  }

  setError(error: string | null) {
    setCrawlerState('lastError', error);
  }

  async initialize() {
    try {
      // 초기 상태 로드
      const initialProgress = await tauriApi.getCrawlingStatus();
      this.setProgress(initialProgress);
      this.setConnected(true);

      // 이벤트 구독
      const progressUnsub = await tauriApi.subscribeToProgress((progress) => {
        this.setProgress(progress);
      });

      const errorUnsub = await tauriApi.subscribeToErrors((error) => {
        this.setError(error);
      });

      // 구독 해제 함수 저장
      eventSubscriptions().push(progressUnsub, errorUnsub);
      
    } catch (error) {
      console.error('Failed to initialize crawler store:', error);
      this.setConnected(false);
      this.setError(error.toString());
    }
  }

  cleanup() {
    // 모든 이벤트 구독 해제
    eventSubscriptions().forEach(unsub => unsub());
  }
}

export const crawlerStore = new CrawlerStore();
```

---

## 실시간 업데이트 시스템

### 6.1 실시간 이벤트 관리자

```typescript
// src/services/realtime-manager.ts
import { tauriApi } from './tauri-api';
import { crawlerStore } from '../stores/crawlerStore';
import type { CrawlingProgress, CrawlingTaskStatus } from './types';

class RealtimeManager {
  private subscriptions: (() => void)[] = [];
  private isInitialized = false;

  async initialize() {
    if (this.isInitialized) return;

    try {
      // 진행상황 업데이트 구독
      const progressUnsub = await tauriApi.subscribeToProgress((progress: CrawlingProgress) => {
        crawlerStore.setProgress(progress);
        
        // 사용자 정의 이벤트 발생
        window.dispatchEvent(new CustomEvent('crawler-progress-updated', {
          detail: progress
        }));
      });

      // 작업 상태 업데이트 구독
      const taskStatusUnsub = await tauriApi.subscribeToTaskStatus((status: CrawlingTaskStatus) => {
        // 작업별 상태 업데이트 처리
        window.dispatchEvent(new CustomEvent('crawler-task-updated', {
          detail: status
        }));
      });

      // 단계 변경 구독
      const stageChangeUnsub = await tauriApi.subscribeToStageChange((stage) => {
        window.dispatchEvent(new CustomEvent('crawler-stage-changed', {
          detail: stage
        }));
      });

      // 에러 구독
      const errorUnsub = await tauriApi.subscribeToErrors((error) => {
        crawlerStore.setError(error);
        
        // 에러 토스트 표시
        window.dispatchEvent(new CustomEvent('crawler-error', {
          detail: error
        }));
      });

      this.subscriptions = [progressUnsub, taskStatusUnsub, stageChangeUnsub, errorUnsub];
      this.isInitialized = true;

      console.log('Realtime manager initialized successfully');
    } catch (error) {
      console.error('Failed to initialize realtime manager:', error);
      throw error;
    }
  }

  cleanup() {
    this.subscriptions.forEach(unsub => unsub());
    this.subscriptions = [];
    this.isInitialized = false;
  }

  // 수동으로 상태 새로고침
  async refreshStatus() {
    try {
      const status = await tauriApi.getCrawlingStatus();
      crawlerStore.setProgress(status);
    } catch (error) {
      console.error('Failed to refresh status:', error);
      crawlerStore.setError(error.toString());
    }
  }
}

export const realtimeManager = new RealtimeManager();
```

### 6.2 메인 앱 컴포넌트 통합

```tsx
// src/App.tsx
import { createEffect, onCleanup } from 'solid-js';
import { crawlerStore } from './stores/crawlerStore';
import { realtimeManager } from './services/realtime-manager';
import { CrawlingControls } from './components/crawler/CrawlingControls';
import { CrawlingProgress } from './components/crawler/CrawlingProgress';
import { CrawlingResults } from './components/crawler/CrawlingResults';
import { ParallelCrawlingMonitor } from './components/crawler/ParallelCrawlingMonitor';
import { DatabaseIntegration } from './components/database/DatabaseIntegration';

export default function App() {
  // 앱 초기화
  createEffect(async () => {
    try {
      await crawlerStore.initialize();
      await realtimeManager.initialize();
    } catch (error) {
      console.error('App initialization failed:', error);
    }
  });

  // 정리
  onCleanup(() => {
    crawlerStore.cleanup();
    realtimeManager.cleanup();
  });

  return (
    <div class="min-h-screen bg-gray-100">
      <div class="container mx-auto px-4 py-8">
        <h1 class="text-3xl font-bold text-gray-900 mb-8">
          Matter Certis v2 - 배치 크롤링
        </h1>

        <div class="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-8">
          {/* 크롤링 제어 */}
          <CrawlingControls />
          
          {/* 진행 상황 */}
          <CrawlingProgress />
        </div>

        <div class="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-8">
          {/* 병렬 처리 모니터 */}
          <ParallelCrawlingMonitor />
          
          {/* 데이터베이스 연동 */}
          <DatabaseIntegration />
        </div>

        {/* 결과 및 에러 */}
        <CrawlingResults />
      </div>
    </div>
  );
}
```

이 가이드는 Rust/Tauri/SolidJS 기반의 Matter Certis v2 프로젝트에서 실시간 UI 통합과 고급 기능을 구현하기 위한 완전한 아키텍처를 제공합니다. 

**핵심 특징:**
- **실시간 양방향 통신**: Tauri 이벤트 시스템을 통한 백엔드-프론트엔드 통신
- **반응형 상태 관리**: SolidJS의 반응성을 활용한 효율적인 상태 업데이트
- **모듈형 컴포넌트**: 재사용 가능하고 독립적인 컴포넌트 설계
- **타입 안전성**: TypeScript를 통한 엄격한 타입 체크
- **확장 가능한 아키텍처**: 새로운 기능 추가가 용이한 구조
