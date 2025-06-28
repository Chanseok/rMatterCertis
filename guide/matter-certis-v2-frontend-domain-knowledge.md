# Matter Certis v2 - 프론트엔드 도메인 지식 가이드

> 이 문서는 기존 React/Electron 구현체에서 추출한 핵심 UI 도메인 지식을 SolidJS 개발자가 Tauri 프론트엔드로 재구현할 수 있도록 정리한 가이드입니다.

## 📋 목차

1. [UI 아키텍처 개요](#ui-아키텍처-개요)
2. [핵심 UI 상태 모델](#핵심-ui-상태-모델)
3. [뷰모델 패턴](#뷰모델-패턴)
4. [컴포넌트 구조](#컴포넌트-구조)
5. [상태 관리 시스템](#상태-관리-시스템)
6. [플랫폼 API 추상화](#플랫폼-api-추상화)
7. [UI 테마 시스템](#ui-테마-시스템)
8. [이벤트 처리 패턴](#이벤트-처리-패턴)
9. [진행률 표시 시스템](#진행률-표시-시스템)
10. [SolidJS 구현 가이드](#solidjs-구현-가이드)

---

## UI 아키텍처 개요

### MVVM 기반 Clean Architecture

```
src/
├── components/           # UI 컴포넌트 (View Layer)
│   ├── common/          # 재사용 가능한 공통 컴포넌트
│   ├── displays/        # 단일 책임 디스플레이 컴포넌트
│   ├── tabs/            # 탭별 페이지 컴포넌트
│   ├── debug/           # 개발 전용 디버깅 컴포넌트
│   └── layout/          # 레이아웃 컴포넌트
├── viewmodels/          # 뷰모델 (ViewModel Layer)
├── stores/              # 도메인 스토어 (Model Layer)
│   └── domain/         # 도메인별 상태 관리
├── hooks/               # React Hooks (SolidJS에서는 createSignal)
├── services/            # UI 서비스 레이어
├── platform/            # 플랫폼 추상화
└── utils/               # UI 유틸리티
```

### 핵심 설계 원칙

1. **단일 책임 원칙**: 각 컴포넌트는 하나의 UI 기능만 담당
2. **관심사 분리**: 비즈니스 로직은 뷰모델에, UI 로직은 컴포넌트에
3. **반응형 상태 관리**: Store → ViewModel → Component 단방향 데이터 흐름
4. **플랫폼 독립성**: Tauri/Electron 무관한 추상화된 API

---

## 핵심 UI 상태 모델

### UIStore (UI 도메인 스토어)

```typescript
// SolidJS 구현 시 createStore와 createSignal 조합 사용
interface UIPreferences {
  theme: 'light' | 'dark' | 'system';
  sidebarCollapsed: boolean;
  autoRefresh: boolean;
  pageSize: number;
  showAdvancedOptions: boolean;
}

interface ViewState {
  // 섹션 확장 상태
  dbSectionExpanded: boolean;
  productsSectionExpanded: boolean;
  logsSectionExpanded: boolean;
  settingsSectionExpanded: boolean;
  
  // 모달 상태
  deleteModalVisible: boolean;
  settingsModalVisible: boolean;
  exportModalVisible: boolean;
  
  // 로딩 상태
  isRefreshing: boolean;
  isExporting: boolean;
}

interface SearchFilterState {
  searchQuery: string;
  filterBy: string;
  sortBy: string;
  sortOrder: 'asc' | 'desc';
  currentPage: number;
}

// SolidJS 구현 예시
function createUIStore() {
  const [preferences, setPreferences] = createStore<UIPreferences>({
    theme: 'system',
    sidebarCollapsed: false,
    autoRefresh: false,
    pageSize: 100,
    showAdvancedOptions: false
  });

  const [viewState, setViewState] = createStore<ViewState>({
    dbSectionExpanded: true,
    productsSectionExpanded: true,
    logsSectionExpanded: true,
    settingsSectionExpanded: true,
    deleteModalVisible: false,
    settingsModalVisible: false,
    exportModalVisible: false,
    isRefreshing: false,
    isExporting: false
  });

  const [searchFilter, setSearchFilter] = createStore<SearchFilterState>({
    searchQuery: '',
    filterBy: 'all',
    sortBy: 'pageId',
    sortOrder: 'desc',
    currentPage: 1
  });

  return {
    preferences,
    setPreferences,
    viewState,
    setViewState,
    searchFilter,
    setSearchFilter
  };
}
```

### CrawlingProgress (크롤링 진행 상태)

```typescript
interface CrawlingProgress {
  status: 'idle' | 'running' | 'paused' | 'completed' | 'error' | 'stopping';
  currentStage: 1 | 2 | 3; // 1: 목록수집, 2: 검증, 3: 상세수집
  currentPage: number;
  totalPages: number;
  processedItems: number;
  totalItems: number;
  percentage: number;
  elapsedTime: number;
  estimatedRemainingTime?: number;
  message?: string;
  
  // 단계별 세부 정보
  stageDetails: {
    stage1: StageProgress; // 제품 목록 수집
    stage2: StageProgress; // 중복 검증
    stage3: StageProgress; // 상세 정보 수집
  };
  
  // 실시간 메트릭
  metrics: {
    itemsPerSecond: number;
    averagePageLoadTime: number;
    successRate: number;
    errorCount: number;
  };
}

interface StageProgress {
  status: 'pending' | 'running' | 'completed' | 'error';
  startTime?: number;
  endTime?: number;
  processedItems: number;
  totalItems: number;
  percentage: number;
}
```

---

## 뷰모델 패턴

### CrawlingWorkflowViewModel

```typescript
// SolidJS 구현을 위한 뷰모델 패턴
class CrawlingWorkflowViewModel {
  // Signals for reactive state
  private readonly [crawlingProgress, setCrawlingProgress] = createSignal<CrawlingProgress>();
  private readonly [isRunning, setIsRunning] = createSignal(false);
  private readonly [error, setError] = createSignal<string | null>(null);

  // Platform API reference
  constructor(private platformAPI: IPlatformAPI) {
    this.initializeEventHandlers();
  }

  // 이벤트 핸들러 초기화
  private initializeEventHandlers() {
    // 진행 상황 업데이트 구독
    this.platformAPI.subscribeToEvent('progress', (progress) => {
      setCrawlingProgress(progress);
    });

    // 크롤링 완료 이벤트 구독
    this.platformAPI.subscribeToEvent('crawling:completed', (result) => {
      setIsRunning(false);
      // UI 업데이트 로직
    });

    // 에러 이벤트 구독
    this.platformAPI.subscribeToEvent('crawling:error', (error) => {
      setError(error.message);
      setIsRunning(false);
    });
  }

  // 크롤링 시작
  async startCrawling(config: CrawlerConfig): Promise<void> {
    try {
      setIsRunning(true);
      setError(null);
      await this.platformAPI.invokeMethod('startCrawling', config);
    } catch (error) {
      setError(error.message);
      setIsRunning(false);
    }
  }

  // 크롤링 중지
  async stopCrawling(): Promise<void> {
    try {
      await this.platformAPI.invokeMethod('stopCrawling');
    } catch (error) {
      setError(error.message);
    }
  }

  // Getters for reactive access
  get progress() { return crawlingProgress(); }
  get running() { return isRunning(); }
  get errorMessage() { return error(); }
}
```

### UIStateViewModel

```typescript
class UIStateViewModel {
  private readonly [activeTab, setActiveTab] = createSignal('status');
  private readonly [loadingMessage, setLoadingMessage] = createSignal('');
  private readonly [isLoading, setIsLoading] = createSignal(false);

  // 탭 전환 핸들러
  handleTabChange(tab: string) {
    setActiveTab(tab);
    
    // 탭별 추가 로직
    switch (tab) {
      case 'localDB':
        this.refreshDatabaseData();
        break;
      case 'analysis':
        this.loadAnalysisData();
        break;
    }
  }

  // 로딩 상태 관리
  showLoading(message: string) {
    setLoadingMessage(message);
    setIsLoading(true);
  }

  hideLoading() {
    setIsLoading(false);
    setLoadingMessage('');
  }

  get currentTab() { return activeTab(); }
  get loading() { return isLoading(); }
  get loadingText() { return loadingMessage(); }
}
```

---

## 컴포넌트 구조

### 1. 레이아웃 컴포넌트

#### AppLayout (메인 레이아웃)

```tsx
// SolidJS 구현 예시
interface AppLayoutProps {
  activeTab: string;
  onTabChange: (tab: string) => void;
  children: JSX.Element;
}

const AppLayout: Component<AppLayoutProps> = (props) => {
  const tabs = [
    { 
      id: 'settings', 
      label: '설정', 
      icon: '⚙️',
      theme: {
        bg: 'bg-emerald-50',
        border: 'border-emerald-200',
        text: 'text-emerald-700'
      }
    },
    { 
      id: 'status', 
      label: '상태 & 제어', 
      icon: '📊',
      theme: {
        bg: 'bg-blue-50',
        border: 'border-blue-200',
        text: 'text-blue-700'
      }
    },
    { 
      id: 'localDB', 
      label: '로컬DB',
      icon: '🗄️',
      theme: {
        bg: 'bg-purple-50',
        border: 'border-purple-200',
        text: 'text-purple-700'
      }
    },
    { 
      id: 'analysis', 
      label: '분석', 
      icon: '📈',
      theme: {
        bg: 'bg-amber-50',
        border: 'border-amber-200',
        text: 'text-amber-700'
      }
    }
  ];

  return (
    <div class="flex flex-col h-screen bg-gradient-to-br from-slate-50 to-gray-100">
      {/* 헤더 */}
      <header class="bg-white shadow-sm border-b border-gray-200">
        <div class="px-6 py-4">
          <h1 class="text-2xl font-bold text-gray-900">
            Matter Certification Crawler
          </h1>
        </div>
      </header>

      {/* 탭 네비게이션 */}
      <div class="bg-white shadow-sm">
        <div class="px-6 pt-4">
          <div class="flex space-x-1">
            <For each={tabs}>
              {(tab) => (
                <button
                  onClick={() => props.onTabChange(tab.id)}
                  class={`
                    relative px-6 py-3 font-medium text-sm
                    transition-all duration-200 ease-in-out rounded-t-lg
                    ${props.activeTab === tab.id
                      ? `${tab.theme.bg} ${tab.theme.text} ${tab.theme.border} border-t border-l border-r`
                      : 'bg-gray-50 text-gray-500 hover:text-gray-700'
                    }
                  `}
                >
                  <span class="mr-2 text-base">{tab.icon}</span>
                  <span class="font-semibold">{tab.label}</span>
                </button>
              )}
            </For>
          </div>
        </div>
      </div>

      {/* 메인 컨텐츠 */}
      <main class="flex-1 overflow-auto">
        {props.children}
      </main>
    </div>
  );
};
```

### 2. 공통 컴포넌트

#### Button 컴포넌트

```tsx
type ButtonVariant = 'primary' | 'secondary' | 'outline' | 'ghost';
type ButtonSize = 'sm' | 'md' | 'lg';

interface ButtonProps {
  variant?: ButtonVariant;
  size?: ButtonSize;
  loading?: boolean;
  disabled?: boolean;
  leftIcon?: JSX.Element;
  rightIcon?: JSX.Element;
  onClick?: () => void;
  children: JSX.Element;
}

const Button: Component<ButtonProps> = (props) => {
  const buttonClasses = () => {
    const base = 'inline-flex items-center justify-center font-medium rounded-md transition-colors focus:outline-none focus:ring-2 focus:ring-offset-2';
    
    const variants = {
      primary: 'bg-blue-600 text-white hover:bg-blue-700 focus:ring-blue-500',
      secondary: 'bg-gray-200 text-gray-900 hover:bg-gray-300 focus:ring-gray-500',
      outline: 'border border-gray-300 bg-white text-gray-700 hover:bg-gray-50 focus:ring-gray-500',
      ghost: 'text-gray-700 hover:bg-gray-100 focus:ring-gray-500'
    };

    const sizes = {
      sm: 'px-3 py-1.5 text-sm',
      md: 'px-4 py-2 text-base',
      lg: 'px-6 py-3 text-lg'
    };

    return `${base} ${variants[props.variant || 'primary']} ${sizes[props.size || 'md']}`;
  };

  return (
    <button
      class={buttonClasses()}
      disabled={props.disabled || props.loading}
      onClick={props.onClick}
    >
      {props.loading && (
        <svg class="animate-spin -ml-1 mr-2 h-4 w-4" fill="none" viewBox="0 0 24 24">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"/>
          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"/>
        </svg>
      )}
      {!props.loading && props.leftIcon && <span class="mr-2">{props.leftIcon}</span>}
      {props.children}
      {!props.loading && props.rightIcon && <span class="ml-2">{props.rightIcon}</span>}
    </button>
  );
};
```

#### ProgressIndicator 컴포넌트

```tsx
interface ProgressIndicatorProps {
  value: number;
  max?: number;
  size?: 'sm' | 'md' | 'lg';
  variant?: 'default' | 'success' | 'warning' | 'danger';
  showLabel?: boolean;
  label?: string;
  animated?: boolean;
}

const ProgressIndicator: Component<ProgressIndicatorProps> = (props) => {
  const percentage = () => Math.min(Math.max((props.value / (props.max || 100)) * 100, 0), 100);
  const displayLabel = () => props.label || `${Math.round(percentage())}%`;

  const sizeClasses = {
    sm: 'h-2',
    md: 'h-3',
    lg: 'h-4'
  };

  const variantClasses = {
    default: 'bg-blue-500',
    success: 'bg-green-500',
    warning: 'bg-yellow-500',
    danger: 'bg-red-500'
  };

  return (
    <div class="w-full">
      {props.showLabel !== false && (
        <div class="flex justify-between items-center mb-2">
          <span class="text-sm font-medium text-gray-700">{displayLabel()}</span>
          <span class="text-sm text-gray-500">{props.value}/{props.max || 100}</span>
        </div>
      )}
      
      <div class={`w-full bg-gray-200 rounded-full ${sizeClasses[props.size || 'md']}`}>
        <div
          class={`
            ${variantClasses[props.variant || 'default']} 
            ${sizeClasses[props.size || 'md']} 
            rounded-full transition-all duration-300 ease-out
            ${props.animated ? 'transition-all duration-300' : ''}
          `}
          style={`width: ${percentage()}%`}
        />
      </div>
    </div>
  );
};
```

### 3. 디스플레이 컴포넌트 (단일 책임)

#### CrawlingMetricsDisplay

```tsx
interface CrawlingMetricsDisplayProps {
  progress: CrawlingProgress;
  animatedValues: AnimatedValues;
  animatedDigits: AnimatedDigits;
}

const CrawlingMetricsDisplay: Component<CrawlingMetricsDisplayProps> = (props) => {
  const isPurpleTheme = () => 
    props.progress.currentStage === 1 || 
    props.progress.currentStage === 3 || 
    props.progress.status === 'completed';

  const renderMetricItem = (label: string, value: any, unit: string = '', isAnimated: boolean = false) => (
    <div class={`
      ${isPurpleTheme() 
        ? 'bg-gradient-to-br from-purple-50 to-indigo-50 border-purple-200' 
        : 'bg-gray-50 border-gray-200'
      } 
      border rounded-lg p-4 transition-all duration-300
    `}>
      <div class="flex items-center justify-between">
        <span class={`text-sm font-medium ${isPurpleTheme() ? 'text-purple-600' : 'text-gray-600'}`}>
          {label}
        </span>
        <span class={`text-lg font-bold ${isPurpleTheme() ? 'text-purple-800' : 'text-gray-800'}`}>
          {isAnimated ? 
            <AnimatedCounter target={value} duration={1000} /> : 
            value
          }{unit}
        </span>
      </div>
    </div>
  );

  return (
    <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
      {renderMetricItem('현재 페이지', props.progress.currentPage, '', true)}
      {renderMetricItem('전체 페이지', props.progress.totalPages, '', false)}
      {renderMetricItem('처리된 항목', props.progress.processedItems, '', true)}
      {renderMetricItem('진행률', Math.round(props.progress.percentage), '%', true)}
    </div>
  );
};
```

#### TimeEstimationDisplay

```tsx
interface TimeEstimationDisplayProps {
  elapsedTimeSeconds: number;
  remainingTimeSeconds?: number;
  confidence?: number;
  isRunning: boolean;
  showConfidenceIndicator?: boolean;
  compact?: boolean;
}

const TimeEstimationDisplay: Component<TimeEstimationDisplayProps> = (props) => {
  const formatTime = (seconds: number): string => {
    if (seconds <= 0) return '--:--';
    
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = Math.floor(seconds % 60);
    
    if (hours > 0) {
      return `${hours}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
    }
    return `${minutes}:${secs.toString().padStart(2, '0')}`;
  };

  const getConfidenceColor = (confidence: number): string => {
    if (confidence >= 0.8) return 'text-green-600';
    if (confidence >= 0.6) return 'text-yellow-600';
    return 'text-red-600';
  };

  return (
    <div class={`${props.compact ? 'flex items-center space-x-4' : 'space-y-3'}`}>
      {/* 경과 시간 */}
      <div class="flex items-center space-x-2">
        <span class="text-sm text-gray-600">경과:</span>
        <span class="font-mono text-lg font-semibold text-blue-600">
          {formatTime(props.elapsedTimeSeconds)}
        </span>
      </div>

      {/* 예상 남은 시간 */}
      {props.remainingTimeSeconds !== undefined && (
        <div class="flex items-center space-x-2">
          <span class="text-sm text-gray-600">남은 시간:</span>
          <span class="font-mono text-lg font-semibold text-orange-600">
            {formatTime(props.remainingTimeSeconds)}
          </span>
          {props.showConfidenceIndicator && props.confidence !== undefined && (
            <span class={`text-xs ${getConfidenceColor(props.confidence)}`}>
              ({Math.round(props.confidence * 100)}% 신뢰도)
            </span>
          )}
        </div>
      )}

      {/* 진행 상태 표시 */}
      {props.isRunning && (
        <div class="flex items-center space-x-2">
          <div class="w-2 h-2 bg-green-500 rounded-full animate-pulse"></div>
          <span class="text-sm text-green-600">실행 중</span>
        </div>
      )}
    </div>
  );
};
```

---

## 상태 관리 시스템

### SolidJS Store 패턴

```typescript
// 전역 스토어 생성
export function createGlobalStores() {
  // UI 상태 스토어
  const uiStore = createUIStore();
  
  // 크롤링 상태 스토어
  const [crawlingState, setCrawlingState] = createStore<CrawlingProgress>({
    status: 'idle',
    currentStage: 1,
    currentPage: 0,
    totalPages: 0,
    processedItems: 0,
    totalItems: 0,
    percentage: 0,
    elapsedTime: 0,
    message: '',
    stageDetails: {
      stage1: { status: 'pending', processedItems: 0, totalItems: 0, percentage: 0 },
      stage2: { status: 'pending', processedItems: 0, totalItems: 0, percentage: 0 },
      stage3: { status: 'pending', processedItems: 0, totalItems: 0, percentage: 0 }
    },
    metrics: {
      itemsPerSecond: 0,
      averagePageLoadTime: 0,
      successRate: 0,
      errorCount: 0
    }
  });

  // 데이터베이스 상태 스토어
  const [databaseState, setDatabaseState] = createStore({
    products: [] as MatterProduct[],
    summary: null as DatabaseSummary | null,
    isLoading: false,
    lastUpdated: null as Date | null
  });

  // 로그 상태 스토어
  const [logState, setLogState] = createStore({
    entries: [] as LogEntry[],
    filters: {
      level: 'all',
      component: 'all',
      timeRange: 'all'
    } as LogFilterState
  });

  return {
    uiStore,
    crawlingState,
    setCrawlingState,
    databaseState,
    setDatabaseState,
    logState,
    setLogState
  };
}

// Context로 제공
const StoreContext = createContext<ReturnType<typeof createGlobalStores>>();

export const StoreProvider: Component<{ children: JSX.Element }> = (props) => {
  const stores = createGlobalStores();
  
  return (
    <StoreContext.Provider value={stores}>
      {props.children}
    </StoreContext.Provider>
  );
};

export const useStores = () => {
  const context = useContext(StoreContext);
  if (!context) {
    throw new Error('useStores must be used within StoreProvider');
  }
  return context;
};
```

### 반응형 상태 업데이트

```typescript
// 이벤트 기반 상태 업데이트
export function setupEventHandlers(stores: ReturnType<typeof createGlobalStores>, platformAPI: IPlatformAPI) {
  // 크롤링 진행 상황 업데이트
  platformAPI.subscribeToEvent('progress', (progress) => {
    stores.setCrawlingState(progress);
  });

  // 데이터베이스 변경 이벤트
  platformAPI.subscribeToEvent('database:updated', async () => {
    stores.setDatabaseState('isLoading', true);
    try {
      const products = await platformAPI.invokeMethod('getProducts');
      const summary = await platformAPI.invokeMethod('getDatabaseSummary');
      
      stores.setDatabaseState({
        products,
        summary,
        isLoading: false,
        lastUpdated: new Date()
      });
    } catch (error) {
      stores.setDatabaseState('isLoading', false);
      console.error('Failed to update database state:', error);
    }
  });

  // 로그 이벤트
  platformAPI.subscribeToEvent('log', (logEntry) => {
    stores.setLogState('entries', (entries) => [logEntry, ...entries.slice(0, 999)]);
  });
}
```

---

## 플랫폼 API 추상화

### Tauri API 어댑터

```typescript
import { invoke } from '@tauri-apps/api/tauri';
import { listen } from '@tauri-apps/api/event';

export class TauriApiAdapter implements IPlatformAPI {
  private eventUnsubscribers = new Map<string, () => void>();

  subscribeToEvent<K extends keyof EventPayloadMapping>(
    eventName: K,
    callback: (data: EventPayloadMapping[K]) => void
  ): UnsubscribeFunction {
    const unsubscribe = listen(eventName, (event) => {
      callback(event.payload as EventPayloadMapping[K]);
    });

    this.eventUnsubscribers.set(eventName, unsubscribe);
    
    return () => {
      const unsub = this.eventUnsubscribers.get(eventName);
      if (unsub) {
        unsub();
        this.eventUnsubscribers.delete(eventName);
      }
    };
  }

  async invokeMethod<K extends keyof MethodParamsMapping, R = MethodReturnMapping[K]>(
    methodName: K,
    params?: MethodParamsMapping[K]
  ): Promise<R> {
    try {
      return await invoke(methodName, params || {}) as R;
    } catch (error) {
      console.error(`Failed to invoke ${methodName}:`, error);
      throw error;
    }
  }
}

// 플랫폼 API 초기화
export function initializePlatformAPI(): IPlatformAPI {
  return new TauriApiAdapter();
}
```

### 메서드 및 이벤트 타입 정의

```typescript
// Tauri 백엔드와의 통신 인터페이스
export interface MethodParamsMapping {
  // 크롤링 관련
  'start_crawling': { config: CrawlerConfig };
  'stop_crawling': {};
  'pause_crawling': {};
  'resume_crawling': {};
  
  // 설정 관리
  'get_config': {};
  'update_config': { config: Partial<CrawlerConfig> };
  'reset_config': {};
  
  // 데이터베이스 관련
  'get_products': { page?: number; limit?: number; search?: string };
  'get_product_by_id': { id: string };
  'delete_products': { ids: string[] };
  'export_products': { format: 'json' | 'csv' | 'xlsx'; filters?: any };
  'get_database_summary': {};
  
  // 분석 관련
  'get_analytics_data': { timeRange?: string };
  'get_vendor_statistics': {};
}

export interface MethodReturnMapping {
  'start_crawling': { success: boolean; sessionId: string };
  'stop_crawling': { success: boolean };
  'pause_crawling': { success: boolean };
  'resume_crawling': { success: boolean };
  
  'get_config': CrawlerConfig;
  'update_config': { success: boolean };
  'reset_config': { success: boolean };
  
  'get_products': { products: MatterProduct[]; total: number };
  'get_product_by_id': MatterProduct | null;
  'delete_products': { success: boolean; deletedCount: number };
  'export_products': { success: boolean; filePath: string };
  'get_database_summary': DatabaseSummary;
  
  'get_analytics_data': AnalyticsData;
  'get_vendor_statistics': VendorStatistics[];
}

export interface EventPayloadMapping {
  // 크롤링 이벤트
  'crawling:progress': CrawlingProgress;
  'crawling:completed': { success: boolean; summary: CrawlingSummary };
  'crawling:error': { error: string; stage: number };
  'crawling:stage_changed': { stage: number; message: string };
  
  // 데이터베이스 이벤트
  'database:updated': { type: 'insert' | 'update' | 'delete'; count: number };
  'database:error': { error: string };
  
  // 로그 이벤트
  'log': LogEntry;
  
  // 시스템 이벤트
  'system:memory_warning': { usage: number };
  'system:performance_stats': PerformanceStats;
}
```

---

## UI 테마 시스템

### 테마 설정 및 CSS 변수

```css
/* theme.css - CSS Custom Properties 활용 */
:root {
  /* Light Theme */
  --color-primary: #3b82f6;
  --color-primary-hover: #2563eb;
  --color-secondary: #6b7280;
  --color-success: #10b981;
  --color-warning: #f59e0b;
  --color-error: #ef4444;
  
  --background-primary: #ffffff;
  --background-secondary: #f9fafb;
  --background-tertiary: #f3f4f6;
  
  --text-primary: #111827;
  --text-secondary: #6b7280;
  --text-muted: #9ca3af;
  
  --border-color: #e5e7eb;
  --border-color-hover: #d1d5db;
  
  --shadow-sm: 0 1px 2px 0 rgb(0 0 0 / 0.05);
  --shadow-md: 0 4px 6px -1px rgb(0 0 0 / 0.1);
  --shadow-lg: 0 10px 15px -3px rgb(0 0 0 / 0.1);
}

[data-theme="dark"] {
  --color-primary: #60a5fa;
  --color-primary-hover: #3b82f6;
  
  --background-primary: #1f2937;
  --background-secondary: #111827;
  --background-tertiary: #0f172a;
  
  --text-primary: #f9fafb;
  --text-secondary: #d1d5db;
  --text-muted: #9ca3af;
  
  --border-color: #374151;
  --border-color-hover: #4b5563;
}
```

### 테마 관리 Hook

```typescript
export function createThemeManager() {
  const [theme, setTheme] = createSignal<'light' | 'dark' | 'system'>('system');

  const effectiveTheme = createMemo(() => {
    if (theme() === 'system') {
      return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    }
    return theme();
  });

  createEffect(() => {
    document.documentElement.setAttribute('data-theme', effectiveTheme());
  });

  const toggleTheme = () => {
    setTheme(current => {
      switch (current) {
        case 'light': return 'dark';
        case 'dark': return 'system';
        case 'system': return 'light';
        default: return 'light';
      }
    });
  };

  return {
    theme: theme,
    effectiveTheme: effectiveTheme,
    setTheme,
    toggleTheme
  };
}
```

---

## 이벤트 처리 패턴

### 사용자 상호작용 처리

```typescript
// 버튼 클릭 이벤트 처리 패턴
const CrawlingControls: Component = () => {
  const { crawlingState } = useStores();
  const platformAPI = usePlatformAPI();
  
  const [isStarting, setIsStarting] = createSignal(false);
  const [isStopping, setIsStopping] = createSignal(false);

  const handleStartCrawling = async () => {
    if (isStarting()) return;
    
    try {
      setIsStarting(true);
      
      const config = await platformAPI.invokeMethod('get_config');
      await platformAPI.invokeMethod('start_crawling', { config });
      
      // UI 피드백
      showNotification('크롤링을 시작했습니다.', 'success');
    } catch (error) {
      showNotification(`크롤링 시작 실패: ${error.message}`, 'error');
    } finally {
      setIsStarting(false);
    }
  };

  const handleStopCrawling = async () => {
    if (isStopping()) return;
    
    try {
      setIsStopping(true);
      await platformAPI.invokeMethod('stop_crawling');
    } catch (error) {
      showNotification(`크롤링 중지 실패: ${error.message}`, 'error');
    } finally {
      setIsStopping(false);
    }
  };

  return (
    <div class="flex space-x-4">
      <Button
        variant="primary"
        loading={isStarting()}
        disabled={crawlingState.status === 'running' || isStarting()}
        onClick={handleStartCrawling}
      >
        {isStarting() ? '시작 중...' : '크롤링 시작'}
      </Button>
      
      <Button
        variant="secondary"
        loading={isStopping()}
        disabled={crawlingState.status !== 'running' || isStopping()}
        onClick={handleStopCrawling}
      >
        {isStopping() ? '중지 중...' : '크롤링 중지'}
      </Button>
    </div>
  );
};
```

### 폼 상태 관리

```typescript
// 설정 폼 상태 관리 패턴
export function createConfigForm(initialConfig: CrawlerConfig) {
  const [config, setConfig] = createStore<CrawlerConfig>(initialConfig);
  const [isDirty, setIsDirty] = createSignal(false);
  const [isSaving, setIsSaving] = createSignal(false);
  const [errors, setErrors] = createStore<Record<string, string>>({});

  const updateField = <K extends keyof CrawlerConfig>(
    field: K, 
    value: CrawlerConfig[K]
  ) => {
    setConfig(field, value);
    setIsDirty(true);
    
    // 필드별 검증
    validateField(field, value);
  };

  const validateField = (field: string, value: any) => {
    const newErrors = { ...errors };
    
    switch (field) {
      case 'page_range_limit':
        if (value <= 0) {
          newErrors[field] = '페이지 제한은 1 이상이어야 합니다.';
        } else {
          delete newErrors[field];
        }
        break;
      case 'products_per_page':
        if (value < 1 || value > 100) {
          newErrors[field] = '페이지당 제품 수는 1-100 사이여야 합니다.';
        } else {
          delete newErrors[field];
        }
        break;
    }
    
    setErrors(newErrors);
  };

  const isValid = createMemo(() => Object.keys(errors).length === 0);

  const save = async (platformAPI: IPlatformAPI) => {
    if (!isValid()) return false;
    
    try {
      setIsSaving(true);
      await platformAPI.invokeMethod('update_config', { config });
      setIsDirty(false);
      return true;
    } catch (error) {
      console.error('Failed to save config:', error);
      return false;
    } finally {
      setIsSaving(false);
    }
  };

  return {
    config,
    updateField,
    isDirty,
    isSaving,
    errors,
    isValid,
    save
  };
}
```

---

## 진행률 표시 시스템

### 실시간 진행률 업데이트

```typescript
// 진행률 애니메이션 컴포넌트
const AnimatedProgress: Component<{
  current: number;
  total: number;
  duration?: number;
}> = (props) => {
  const [displayValue, setDisplayValue] = createSignal(0);
  
  // 부드러운 애니메이션을 위한 easing 함수
  const easeOutCubic = (t: number): number => 1 - Math.pow(1 - t, 3);
  
  createEffect(() => {
    const targetPercentage = (props.current / props.total) * 100;
    const startValue = displayValue();
    const difference = targetPercentage - startValue;
    const duration = props.duration || 1000;
    
    if (Math.abs(difference) < 0.1) return;
    
    let startTime: number;
    
    const animate = (currentTime: number) => {
      if (!startTime) startTime = currentTime;
      
      const elapsed = currentTime - startTime;
      const progress = Math.min(elapsed / duration, 1);
      const easedProgress = easeOutCubic(progress);
      
      const currentValue = startValue + (difference * easedProgress);
      setDisplayValue(currentValue);
      
      if (progress < 1) {
        requestAnimationFrame(animate);
      }
    };
    
    requestAnimationFrame(animate);
  });

  return (
    <div class="relative">
      <div class="flex justify-between text-sm font-medium mb-1">
        <span>진행률</span>
        <span>{Math.round(displayValue())}%</span>
      </div>
      <div class="w-full bg-gray-200 rounded-full h-2">
        <div
          class="bg-gradient-to-r from-blue-500 to-purple-500 h-2 rounded-full transition-all duration-300"
          style={`width: ${displayValue()}%`}
        />
      </div>
      <div class="flex justify-between text-xs text-gray-600 mt-1">
        <span>{props.current.toLocaleString()}</span>
        <span>{props.total.toLocaleString()}</span>
      </div>
    </div>
  );
};
```

### 단계별 진행 표시

```typescript
const StageProgressIndicator: Component<{
  stages: StageProgress[];
  currentStage: number;
}> = (props) => {
  const getStageIcon = (stage: number, status: StageProgress['status']) => {
    switch (status) {
      case 'completed': return '✅';
      case 'running': return '🔄';
      case 'error': return '❌';
      default: return '⏳';
    }
  };

  const getStageColor = (stage: number, status: StageProgress['status']) => {
    if (stage < props.currentStage) return 'text-green-600 bg-green-50';
    if (stage === props.currentStage) return 'text-blue-600 bg-blue-50';
    return 'text-gray-400 bg-gray-50';
  };

  return (
    <div class="flex items-center space-x-4">
      <For each={props.stages}>
        {(stage, index) => (
          <>
            <div class={`
              flex items-center space-x-2 px-3 py-2 rounded-lg
              ${getStageColor(index() + 1, stage.status)}
            `}>
              <span class="text-lg">{getStageIcon(index() + 1, stage.status)}</span>
              <div class="flex flex-col">
                <span class="text-sm font-medium">
                  단계 {index() + 1}
                </span>
                <span class="text-xs">
                  {Math.round(stage.percentage)}%
                </span>
              </div>
            </div>
            
            {index() < props.stages.length - 1 && (
              <div class={`
                w-8 h-px
                ${index() + 1 < props.currentStage ? 'bg-green-400' : 'bg-gray-300'}
              `} />
            )}
          </>
        )}
      </For>
    </div>
  );
};
```

---

## SolidJS 구현 가이드

### 프로젝트 구조

```
src/
├── app.tsx                 # 메인 앱 컴포넌트
├── index.tsx              # 진입점
├── components/            # UI 컴포넌트
│   ├── common/           # 재사용 가능한 컴포넌트
│   ├── displays/         # 단일 책임 디스플레이 컴포넌트
│   ├── layout/           # 레이아웃 컴포넌트
│   └── tabs/             # 탭별 페이지 컴포넌트
├── stores/               # 상태 관리
│   ├── ui.ts            # UI 상태 스토어
│   ├── crawling.ts      # 크롤링 상태 스토어
│   └── database.ts      # 데이터베이스 상태 스토어
├── platform/             # Tauri API 추상화
│   ├── api.ts           # 플랫폼 API 인터페이스
│   └── tauri.ts         # Tauri 구현체
├── utils/                # 유틸리티 함수
└── types/                # TypeScript 타입 정의
```

### 메인 앱 구현

```tsx
// app.tsx
import { Component, createEffect, onCleanup } from 'solid-js';
import { StoreProvider } from './stores';
import { PlatformAPIProvider } from './platform';
import { AppLayout } from './components/layout/AppLayout';
import { StatusTab } from './components/tabs/StatusTab';
import { SettingsTab } from './components/tabs/SettingsTab';
import { LocalDBTab } from './components/tabs/LocalDBTab';
import { AnalysisTab } from './components/tabs/AnalysisTab';

const App: Component = () => {
  return (
    <PlatformAPIProvider>
      <StoreProvider>
        <AppRouter />
      </StoreProvider>
    </PlatformAPIProvider>
  );
};

const AppRouter: Component = () => {
  const { uiStore } = useStores();
  
  const renderTabContent = () => {
    switch (uiStore.activeTab) {
      case 'settings': return <SettingsTab />;
      case 'status': return <StatusTab />;
      case 'localDB': return <LocalDBTab />;
      case 'analysis': return <AnalysisTab />;
      default: return <StatusTab />;
    }
  };

  return (
    <AppLayout
      activeTab={uiStore.activeTab}
      onTabChange={(tab) => uiStore.setActiveTab(tab)}
    >
      {renderTabContent()}
    </AppLayout>
  );
};

export default App;
```

### Tauri 통합

```typescript
// platform/tauri.ts
import { invoke } from '@tauri-apps/api/tauri';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

export class TauriPlatformAPI implements IPlatformAPI {
  private unlisteners: Map<string, UnlistenFn> = new Map();

  async subscribeToEvent<K extends keyof EventPayloadMapping>(
    eventName: K,
    callback: (data: EventPayloadMapping[K]) => void
  ): Promise<UnsubscribeFunction> {
    const unlisten = await listen(eventName, (event) => {
      callback(event.payload as EventPayloadMapping[K]);
    });

    this.unlisteners.set(eventName, unlisten);

    return () => {
      const unlistenFn = this.unlisteners.get(eventName);
      if (unlistenFn) {
        unlistenFn();
        this.unlisteners.delete(eventName);
      }
    };
  }

  async invokeMethod<K extends keyof MethodParamsMapping, R = MethodReturnMapping[K]>(
    methodName: K,
    params?: MethodParamsMapping[K]
  ): Promise<R> {
    try {
      // Tauri는 camelCase를 snake_case로 변환
      const tauriMethodName = methodName.replace(/([A-Z])/g, '_$1').toLowerCase();
      return await invoke(tauriMethodName, params || {}) as R;
    } catch (error) {
      console.error(`Failed to invoke ${methodName}:`, error);
      throw new Error(`Platform API call failed: ${error}`);
    }
  }

  dispose() {
    this.unlisteners.forEach(unlisten => unlisten());
    this.unlisteners.clear();
  }
}
```

### 성능 최적화

```typescript
// 메모이제이션과 배치 업데이트 패턴
export function createOptimizedTable<T>(initialData: T[]) {
  const [data, setData] = createSignal<T[]>(initialData);
  const [filters, setFilters] = createStore({
    search: '',
    sortBy: '',
    sortOrder: 'asc' as 'asc' | 'desc'
  });

  // 필터링과 정렬을 메모화
  const filteredData = createMemo(() => {
    let result = data();
    
    // 검색 필터
    if (filters.search) {
      const searchLower = filters.search.toLowerCase();
      result = result.filter(item => 
        Object.values(item as any).some(value => 
          String(value).toLowerCase().includes(searchLower)
        )
      );
    }
    
    // 정렬
    if (filters.sortBy) {
      result = [...result].sort((a, b) => {
        const aVal = (a as any)[filters.sortBy];
        const bVal = (b as any)[filters.sortBy];
        
        if (aVal < bVal) return filters.sortOrder === 'asc' ? -1 : 1;
        if (aVal > bVal) return filters.sortOrder === 'asc' ? 1 : -1;
        return 0;
      });
    }
    
    return result;
  });

  // 가상 스크롤링을 위한 청크 관리
  const [visibleRange, setVisibleRange] = createSignal({ start: 0, end: 50 });
  
  const visibleData = createMemo(() => {
    const range = visibleRange();
    return filteredData().slice(range.start, range.end);
  });

  return {
    data: visibleData,
    filters,
    setFilters,
    setData,
    setVisibleRange,
    totalCount: () => filteredData().length
  };
}
```

### 에러 경계 및 로딩 상태

```tsx
// 에러 경계 컴포넌트
export const ErrorBoundary: Component<{ children: JSX.Element }> = (props) => {
  const [error, setError] = createSignal<Error | null>(null);

  onCleanup(() => {
    setError(null);
  });

  return (
    <Show
      when={!error()}
      fallback={
        <div class="flex items-center justify-center min-h-screen">
          <div class="text-center">
            <h2 class="text-2xl font-bold text-red-600 mb-4">
              오류가 발생했습니다
            </h2>
            <p class="text-gray-600 mb-4">
              {error()?.message || '알 수 없는 오류'}
            </p>
            <Button onClick={() => setError(null)}>
              다시 시도
            </Button>
          </div>
        </div>
      }
    >
      {props.children}
    </Show>
  );
};

// 서스펜스와 로딩 상태
export const LoadingBoundary: Component<{ children: JSX.Element }> = (props) => {
  return (
    <Suspense
      fallback={
        <div class="flex items-center justify-center p-8">
          <div class="text-center">
            <div class="animate-spin rounded-full h-8 w-8 border-2 border-blue-500 border-t-transparent mx-auto mb-4"></div>
            <p class="text-gray-600">로딩 중...</p>
          </div>
        </div>
      }
    >
      {props.children}
    </Suspense>
  );
};
```

---

## 마이그레이션 체크리스트

### React → SolidJS 변환 매핑

| React | SolidJS | 비고 |
|-------|---------|------|
| `useState` | `createSignal` | 반응형 상태 |
| `useEffect` | `createEffect` | 부수 효과 |
| `useMemo` | `createMemo` | 메모화된 값 |
| `useContext` | `useContext` | 컨텍스트 사용 |
| `React.memo` | `createMemo` 또는 컴포넌트 분할 | 성능 최적화 |
| JSX fragments `<>` | JSX fragments `<>` | 동일 |
| Conditional rendering | `<Show>`, `<Switch>` | SolidJS 전용 컴포넌트 |
| List rendering | `<For>`, `<Index>` | SolidJS 전용 컴포넌트 |

### 주요 구현 우선순위

1. **1단계: 기본 설정**
   - Tauri + SolidJS 프로젝트 설정
   - 플랫폼 API 추상화 구현
   - 기본 라우팅 및 레이아웃

2. **2단계: 핵심 기능**
   - 상태 관리 시스템 구축
   - 크롤링 제어 UI
   - 진행률 표시 시스템

3. **3단계: 고급 기능**
   - 데이터 테이블 및 검색
   - 설정 관리 UI
   - 분석 및 차트

4. **4단계: 최적화**
   - 성능 최적화
   - 접근성 개선
   - 테스트 작성

이 문서는 기존 React/Electron 구현체의 핵심 UI 패턴과 상태 관리 로직을 SolidJS/Tauri 환경으로 변환하기 위한 완전한 가이드를 제공합니다. 각 섹션의 코드 예제는 실제 구현 가능한 형태로 작성되었으며, 현대적인 프론트엔드 개발 패턴을 반영했습니다.
