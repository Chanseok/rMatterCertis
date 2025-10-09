# SolidJS 기반 Tauri 프로젝트 UI 구현 가이드

## 개요
이 문서는 현재 React + Electron 기반 Matter 인증 정보 수집기의 UI 구성과 기능 배치를 SolidJS 기반 Tauri 프로젝트에서 유사하게 구현하기 위한 실무적 가이드입니다.

## 현재 프로젝트 UI 구조 분석

### 전체 애플리케이션 구조
```
Matter Certification Crawler
├── 헤더 (Header)
├── 탭 네비게이션 (Tab Navigation)
│   ├── 설정 (Settings) ⚙️
│   ├── 상태 & 제어 (Status & Control) 📊  
│   ├── 로컬DB (LocalDB) 🗄️
│   └── 분석 (Analysis) 📈
└── 메인 컨텐츠 영역
```

### 디자인 시스템
- **색상 테마**: 각 탭별 고유 색상 (설정: emerald, 상태: blue, 로컬DB: purple, 분석: amber)
- **그라데이션 배경**: `bg-gradient-to-br from-slate-50 to-gray-100`
- **프랭클린 다이어리 스타일**: 탭은 노트북 스타일의 둥근 모서리 디자인
- **다크모드**: 전체 애플리케이션에서 지원

## SolidJS 구현 가이드

### 1. 프로젝트 구조

```
src/
├── components/
│   ├── layout/
│   │   ├── AppLayout.tsx
│   │   ├── Header.tsx
│   │   └── TabNavigation.tsx
│   ├── tabs/
│   │   ├── SettingsTab.tsx
│   │   ├── StatusTab.tsx
│   │   ├── LocalDBTab.tsx
│   │   └── AnalysisTab.tsx
│   ├── displays/
│   │   ├── StatusDisplay.tsx
│   │   ├── ProgressDisplay.tsx
│   │   └── MetricsDisplay.tsx
│   └── common/
│       ├── Button.tsx
│       ├── Input.tsx
│       └── ExpandableSection.tsx
├── stores/
│   ├── appStore.ts
│   ├── configStore.ts
│   └── dataStore.ts
└── types/
    └── index.ts
```

### 2. SolidJS Store 패턴 (MobX 대체)

```tsx
// stores/appStore.ts
import { createStore } from 'solid-js/store';

export interface TabConfig {
  id: string;
  label: string;
  icon: string;
  theme: {
    bg: string;
    border: string;
    text: string;
    accent: string;
  };
}

export interface AppState {
  activeTab: string;
  tabs: TabConfig[];
  isDarkMode: boolean;
  isLoading: boolean;
  expandedSections: Record<string, boolean>;
}

const [appState, setAppState] = createStore<AppState>({
  activeTab: 'status',
  tabs: [
    {
      id: 'settings',
      label: '설정',
      icon: '⚙️',
      theme: {
        bg: 'bg-emerald-50',
        border: 'border-emerald-200',
        text: 'text-emerald-700',
        accent: 'from-emerald-500 to-teal-500'
      }
    },
    {
      id: 'status',
      label: '상태 & 제어',
      icon: '📊',
      theme: {
        bg: 'bg-blue-50',
        border: 'border-blue-200',
        text: 'text-blue-700',
        accent: 'from-blue-500 to-indigo-500'
      }
    },
    {
      id: 'localDB',
      label: '로컬DB',
      icon: '🗄️',
      theme: {
        bg: 'bg-purple-50',
        border: 'border-purple-200',
        text: 'text-purple-700',
        accent: 'from-purple-500 to-violet-500'
      }
    },
    {
      id: 'analysis',
      label: '분석',
      icon: '📈',
      theme: {
        bg: 'bg-amber-50',
        border: 'border-amber-200',
        text: 'text-amber-700',
        accent: 'from-amber-500 to-orange-500'
      }
    }
  ],
  isDarkMode: false,
  isLoading: false,
  expandedSections: {}
});

export { appState, setAppState };
```

### 3. 탭 네비게이션 구현

```tsx
// components/layout/TabNavigation.tsx
import { For, createMemo } from 'solid-js';
import { appState, setAppState } from '../../stores/appStore';

export function TabNavigation() {
  const activeTabTheme = createMemo(() => 
    appState.tabs.find(tab => tab.id === appState.activeTab)?.theme
  );

  const handleTabClick = (tabId: string) => {
    setAppState('activeTab', tabId);
    
    // 탭 전환 애니메이션 효과
    const tabElement = document.querySelector(`[data-tab="${tabId}"]`);
    if (tabElement) {
      tabElement.classList.add('tab-focus-animation');
      setTimeout(() => {
        tabElement.classList.remove('tab-focus-animation');
      }, 2000);
    }
  };

  return (
    <div class="bg-white shadow-sm">
      <div class="px-6 pt-4">
        <div class="flex space-x-1">
          <For each={appState.tabs}>
            {(tab, index) => (
              <button
                data-tab={tab.id}
                onClick={() => handleTabClick(tab.id)}
                class={`
                  relative px-6 py-3 font-medium text-sm whitespace-nowrap
                  transition-all duration-200 ease-in-out rounded-t-lg
                  focus:outline-none
                  ${appState.activeTab === tab.id
                    ? `${tab.theme.bg} ${tab.theme.text} ${tab.theme.border} border-t border-l border-r border-b-0 shadow-md -mb-px z-10`
                    : 'bg-gray-50 text-gray-500 hover:text-gray-700 hover:bg-gray-100 border border-transparent hover:border-gray-200'
                  }
                  ${index() === 0 ? 'ml-0' : ''}
                `}
                style={{
                  'box-shadow': appState.activeTab === tab.id 
                    ? '0 -2px 8px rgba(0,0,0,0.04), 0 2px 4px rgba(0,0,0,0.02)' 
                    : 'none'
                }}
              >
                <span class="mr-2 text-base">{tab.icon}</span>
                <span class="font-semibold">{tab.label}</span>
                
                {/* 활성 탭에 그라데이션 언더라인 */}
                {appState.activeTab === tab.id && (
                  <div class={`absolute bottom-0 left-0 right-0 h-1 bg-gradient-to-r ${tab.theme.accent} rounded-b-lg`} />
                )}
              </button>
            )}
          </For>
        </div>
      </div>
    </div>
  );
}
```

### 4. 메인 레이아웃 구현

```tsx
// components/layout/AppLayout.tsx
import { JSX, createMemo } from 'solid-js';
import { appState } from '../../stores/appStore';
import { Header } from './Header';
import { TabNavigation } from './TabNavigation';

interface AppLayoutProps {
  children: JSX.Element;
}

export function AppLayout(props: AppLayoutProps) {
  const activeTabTheme = createMemo(() => 
    appState.tabs.find(tab => tab.id === appState.activeTab)?.theme
  );

  return (
    <div class="flex flex-col h-screen bg-gradient-to-br from-slate-50 to-gray-100">
      {/* 헤더 */}
      <Header />
      
      {/* 탭 네비게이션 */}
      <TabNavigation />
      
      {/* 메인 컨텐츠 */}
      <main class={`flex-1 ${activeTabTheme()?.bg || 'bg-gray-50'} transition-colors duration-200`}>
        <div class="px-6 py-6 h-full">
          {props.children}
        </div>
      </main>
    </div>
  );
}
```

## 탭별 상세 구현 가이드

### 1. 설정 탭 (Settings Tab)

#### 주요 기능 배치
- **크롤링 설정**: 페이지 범위, 동시 실행 수, 재시도 설정
- **배치 처리 설정**: 일괄 처리 옵션
- **로깅 설정**: 로그 레벨, 파일 저장 옵션
- **고급 설정**: 개발자 모드, 디버그 옵션

```tsx
// components/tabs/SettingsTab.tsx
import { createSignal, For } from 'solid-js';
import { ExpandableSection } from '../common/ExpandableSection';

export function SettingsTab() {
  const [isAdvancedExpanded, setIsAdvancedExpanded] = createSignal(false);
  const [isBatchExpanded, setIsBatchExpanded] = createSignal(true);

  return (
    <div class="space-y-6">
      {/* 기본 크롤링 설정 */}
      <ExpandableSection
        title="크롤링 설정"
        isExpanded={true}
        onToggle={() => {}}
        icon="⚙️"
      >
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-2">
              시작 페이지
            </label>
            <input 
              type="number" 
              class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-emerald-500"
              placeholder="1"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-2">
              종료 페이지
            </label>
            <input 
              type="number" 
              class="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-emerald-500"
              placeholder="100"
            />
          </div>
        </div>
      </ExpandableSection>

      {/* 배치 처리 설정 */}
      <ExpandableSection
        title="배치 처리 설정"
        isExpanded={isBatchExpanded()}
        onToggle={setIsBatchExpanded}
        icon="📦"
      >
        <div class="space-y-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-2">
              동시 실행 수
            </label>
            <select class="w-full px-3 py-2 border border-gray-300 rounded-md">
              <option value="6">6개 (기본값)</option>
              <option value="12">12개</option>
              <option value="24">24개</option>
            </select>
          </div>
        </div>
      </ExpandableSection>

      {/* 저장 버튼 */}
      <div class="flex justify-end">
        <button class="px-6 py-2 bg-emerald-600 text-white rounded-md hover:bg-emerald-700 transition-colors">
          설정 저장
        </button>
      </div>
    </div>
  );
}
```

### 2. 상태 & 제어 탭 (Status & Control Tab)

#### 주요 기능 배치
- **크롤링 대시보드**: 진행률, 현재 단계, 시간 정보
- **제어 버튼들**: 시작/중지, 상태 체크, 수동 크롤링
- **사이트-로컬 비교**: 실시간 데이터 비교 정보
- **진행률 시각화**: 단계별 진행 상황, 동시 작업 시각화

```tsx
// components/tabs/StatusTab.tsx
import { createSignal, createMemo } from 'solid-js';
import { StatusDisplay } from '../displays/StatusDisplay';
import { ProgressDisplay } from '../displays/ProgressDisplay';
import { ExpandableSection } from '../common/ExpandableSection';

export function StatusTab() {
  const [isRunning, setIsRunning] = createSignal(false);
  const [progress, setProgress] = createSignal(0);
  const [currentStage, setCurrentStage] = createSignal<1 | 2 | 3>(1);

  const stageInfo = createMemo(() => {
    const stage = currentStage();
    const stages = {
      1: { text: '1단계: 목록 수집', color: 'bg-blue-100 text-blue-800' },
      2: { text: '2단계: 검증', color: 'bg-yellow-100 text-yellow-800' },
      3: { text: '3단계: 상세정보', color: 'bg-green-100 text-green-800' }
    };
    return stages[stage];
  });

  const handleStart = () => {
    setIsRunning(true);
    // Tauri 명령 호출
    // invoke('start_crawling', { config: ... });
  };

  const handleStop = () => {
    setIsRunning(false);
    // Tauri 명령 호출
    // invoke('stop_crawling');
  };

  return (
    <div class="space-y-6">
      {/* 현재 상태 표시 */}
      <div class="bg-white rounded-lg shadow-md p-6">
        <div class="flex items-center justify-between mb-4">
          <h3 class="text-lg font-semibold text-gray-900">크롤링 상태</h3>
          <span class={`px-3 py-1 rounded-full text-sm font-medium ${stageInfo().color}`}>
            {stageInfo().text}
          </span>
        </div>
        
        <StatusDisplay 
          isRunning={isRunning()}
          progress={progress()}
          stage={currentStage()}
        />
      </div>

      {/* 제어 버튼 */}
      <div class="bg-white rounded-lg shadow-md p-6">
        <h3 class="text-lg font-semibold text-gray-900 mb-4">크롤링 제어</h3>
        <div class="flex gap-4">
          <button
            onClick={handleStart}
            disabled={isRunning()}
            class="px-6 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {isRunning() ? '실행 중...' : '크롤링 시작'}
          </button>
          
          <button
            onClick={handleStop}
            disabled={!isRunning()}
            class="px-6 py-2 bg-red-600 text-white rounded-md hover:bg-red-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            중지
          </button>
          
          <button class="px-6 py-2 bg-gray-600 text-white rounded-md hover:bg-gray-700 transition-colors">
            상태 체크
          </button>
        </div>
      </div>

      {/* 사이트-로컬 비교 */}
      <ExpandableSection
        title="사이트-로컬 비교"
        isExpanded={true}
        onToggle={() => {}}
        icon="📊"
      >
        <div class="grid grid-cols-2 gap-4">
          <div class="text-center">
            <div class="text-2xl font-bold text-blue-600">1,234</div>
            <div class="text-sm text-gray-600">사이트 제품 수</div>
          </div>
          <div class="text-center">
            <div class="text-2xl font-bold text-purple-600">1,200</div>
            <div class="text-sm text-gray-600">로컬 DB 제품 수</div>
          </div>
        </div>
        
        {/* 진행률 바 */}
        <div class="mt-4">
          <div class="w-full bg-gray-200 rounded-full h-3">
            <div 
              class="h-full bg-gradient-to-r from-blue-500 to-purple-600 rounded-full transition-all duration-500"
              style={{ width: `${(1200/1234) * 100}%` }}
            />
          </div>
        </div>
      </ExpandableSection>

      {/* 동시 작업 시각화 */}
      {isRunning() && (
        <div class="bg-gradient-to-br from-blue-50 to-purple-50 rounded-lg p-4">
          <h4 class="text-md font-semibold text-blue-700 mb-2">동시 진행 작업</h4>
          <div class="grid grid-cols-8 gap-1">
            {Array.from({ length: 12 }, (_, i) => (
              <div 
                class={`w-8 h-8 rounded-full flex items-center justify-center text-xs font-bold
                  ${i < 8 ? 'bg-blue-400 text-white animate-pulse' : 'bg-gray-300 text-gray-500'}`}
              >
                {i < 8 ? '▶' : ''}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
```

### 3. 로컬DB 탭 (LocalDB Tab)

#### 주요 기능 배치
- **데이터베이스 요약**: 총 제품 수, 마지막 업데이트 시간
- **제품 목록**: 페이지네이션과 검색 기능
- **데이터 관리**: 엑셀 내보내기, 데이터 삭제
- **필터링**: 제조사, 디바이스 타입별 필터

```tsx
// components/tabs/LocalDBTab.tsx
import { createSignal, createMemo, For } from 'solid-js';
import { createStore } from 'solid-js/store';

interface Product {
  id: number;
  manufacturer: string;
  model: string;
  deviceType: string;
  certificationDate: string;
  pageId: number;
}

export function LocalDBTab() {
  const [products, setProducts] = createStore<Product[]>([]);
  const [currentPage, setCurrentPage] = createSignal(1);
  const [searchQuery, setSearchQuery] = createSignal('');
  const [isLoading, setIsLoading] = createSignal(false);
  
  const itemsPerPage = 12;

  const filteredProducts = createMemo(() => {
    const query = searchQuery().toLowerCase();
    return products.filter(product => 
      product.manufacturer.toLowerCase().includes(query) ||
      product.model.toLowerCase().includes(query) ||
      product.deviceType.toLowerCase().includes(query)
    );
  });

  const paginatedProducts = createMemo(() => {
    const start = (currentPage() - 1) * itemsPerPage;
    return filteredProducts().slice(start, start + itemsPerPage);
  });

  const totalPages = createMemo(() => 
    Math.ceil(filteredProducts().length / itemsPerPage)
  );

  const handleExport = async () => {
    // Tauri 명령으로 엑셀 내보내기
    // await invoke('export_to_excel', { data: products });
  };

  return (
    <div class="space-y-6">
      {/* 데이터베이스 요약 */}
      <div class="bg-white rounded-lg shadow-md p-6">
        <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div class="text-center">
            <div class="text-3xl font-bold text-purple-600">{products.length.toLocaleString()}</div>
            <div class="text-sm text-gray-600">총 제품 수</div>
          </div>
          <div class="text-center">
            <div class="text-3xl font-bold text-blue-600">
              {new Set(products.map(p => p.manufacturer)).size}
            </div>
            <div class="text-sm text-gray-600">제조사 수</div>
          </div>
          <div class="text-center">
            <div class="text-3xl font-bold text-green-600">
              {new Set(products.map(p => p.deviceType)).size}
            </div>
            <div class="text-sm text-gray-600">디바이스 유형 수</div>
          </div>
        </div>
      </div>

      {/* 검색 및 필터 */}
      <div class="bg-white rounded-lg shadow-md p-6">
        <div class="flex flex-col sm:flex-row gap-4">
          <div class="flex-1">
            <input
              type="text"
              placeholder="제조사, 모델명, 디바이스 유형으로 검색..."
              value={searchQuery()}
              onInput={(e) => setSearchQuery(e.currentTarget.value)}
              class="w-full px-4 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-purple-500"
            />
          </div>
          <button
            onClick={handleExport}
            class="px-6 py-2 bg-purple-600 text-white rounded-md hover:bg-purple-700 transition-colors"
          >
            엑셀 내보내기
          </button>
        </div>
      </div>

      {/* 제품 목록 */}
      <div class="bg-white rounded-lg shadow-md">
        <div class="overflow-x-auto">
          <table class="w-full">
            <thead class="bg-gray-50">
              <tr>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                  제조사
                </th>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                  모델명
                </th>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                  디바이스 유형
                </th>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                  인증 날짜
                </th>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                  페이지 ID
                </th>
              </tr>
            </thead>
            <tbody class="bg-white divide-y divide-gray-200">
              <For each={paginatedProducts()}>
                {(product) => (
                  <tr class="hover:bg-gray-50">
                    <td class="px-6 py-4 whitespace-nowrap text-sm font-medium text-gray-900">
                      {product.manufacturer}
                    </td>
                    <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                      {product.model}
                    </td>
                    <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                      {product.deviceType}
                    </td>
                    <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                      {product.certificationDate}
                    </td>
                    <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
                      {product.pageId}
                    </td>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
        </div>

        {/* 페이지네이션 */}
        <div class="bg-white px-6 py-3 border-t border-gray-200">
          <div class="flex items-center justify-between">
            <div class="text-sm text-gray-700">
              총 {filteredProducts().length}개 중 {(currentPage() - 1) * itemsPerPage + 1}-{Math.min(currentPage() * itemsPerPage, filteredProducts().length)}개 표시
            </div>
            <div class="flex space-x-2">
              <button
                onClick={() => setCurrentPage(Math.max(1, currentPage() - 1))}
                disabled={currentPage() === 1}
                class="px-3 py-2 bg-gray-200 text-gray-700 rounded disabled:opacity-50"
              >
                이전
              </button>
              <span class="px-3 py-2 text-gray-700">
                {currentPage()} / {totalPages()}
              </span>
              <button
                onClick={() => setCurrentPage(Math.min(totalPages(), currentPage() + 1))}
                disabled={currentPage() === totalPages()}
                class="px-3 py-2 bg-gray-200 text-gray-700 rounded disabled:opacity-50"
              >
                다음
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
```

### 4. ~~분석 탭 (Analysis Tab)~~ (⚠️ DEPRECATED)

> **⚠️ 이 섹션은 더 이상 사용되지 않습니다.**  
> 현재 UI는 **3개 탭 구조** (크롤링 엔진, 설정, 로컬DB)만 사용합니다.  
> 분석 기능은 **로컬DB 탭**으로 통합되었습니다.

<details>
<summary>📜 레거시 분석 탭 구현 참고 (클릭하여 펼치기)</summary>

#### 주요 기능 배치 (레거시)
- **분석 서브탭**: 제품 현황, 제조사 분석, 디바이스 유형 분석, 상호작용 분석, 데이터 테이블
- **날짜 범위 필터**: 슬라이더로 기간 선택
- **차트 시각화**: 파이차트, 바차트, 라인차트
- **통계 요약**: 핵심 지표들

```tsx
// components/tabs/AnalysisTab.tsx
import { createSignal, createMemo, For } from 'solid-js';
import { Chart } from '../common/Chart'; // 차트 라이브러리 래퍼

export function AnalysisTab() {
  const [activeSubTab, setActiveSubTab] = createSignal(0);
  const [dateRange, setDateRange] = createSignal({ start: new Date(), end: new Date() });

  const subTabs = [
    { id: 0, label: '제품 현황', icon: '📊', theme: { bg: 'bg-blue-50', text: 'text-blue-700', border: 'border-blue-200', accent: 'from-blue-500 to-indigo-500' } },
    { id: 1, label: '제조사 분석', icon: '🏭', theme: { bg: 'bg-emerald-50', text: 'text-emerald-700', border: 'border-emerald-200', accent: 'from-emerald-500 to-teal-500' } },
    { id: 2, label: '디바이스 유형 분석', icon: '📱', theme: { bg: 'bg-purple-50', text: 'text-purple-700', border: 'border-purple-200', accent: 'from-purple-500 to-violet-500' } },
    { id: 3, label: '상호작용 분석', icon: '🔄', theme: { bg: 'bg-rose-50', text: 'text-rose-700', border: 'border-rose-200', accent: 'from-rose-500 to-pink-500' } },
    { id: 4, label: '데이터 테이블', icon: '📋', theme: { bg: 'bg-orange-50', text: 'text-orange-700', border: 'border-orange-200', accent: 'from-orange-500 to-amber-500' } }
  ];

  const activeTabTheme = createMemo(() => 
    subTabs.find(tab => tab.id === activeSubTab())?.theme
  );

  return (
    <div class="space-y-6">
      {/* 통계 요약 카드 */}
      <div class="grid grid-cols-1 md:grid-cols-4 gap-6">
        <div class={`p-6 rounded-lg border-2 ${activeTabTheme()?.border || 'border-blue-200'} ${activeTabTheme()?.bg || 'bg-blue-50'}`}>
          <div class="text-sm text-gray-500 mb-1">총 제품 수</div>
          <div class={`text-2xl font-bold ${activeTabTheme()?.text || 'text-blue-700'}`}>
            1,234
          </div>
        </div>
        <div class={`p-6 rounded-lg border-2 ${activeTabTheme()?.border || 'border-blue-200'} ${activeTabTheme()?.bg || 'bg-blue-50'}`}>
          <div class="text-sm text-gray-500 mb-1">제조사 수</div>
          <div class={`text-2xl font-bold ${activeTabTheme()?.text || 'text-blue-700'}`}>
            89
          </div>
        </div>
        <div class={`p-6 rounded-lg border-2 ${activeTabTheme()?.border || 'border-blue-200'} ${activeTabTheme()?.bg || 'bg-blue-50'}`}>
          <div class="text-sm text-gray-500 mb-1">디바이스 유형 수</div>
          <div class={`text-2xl font-bold ${activeTabTheme()?.text || 'text-blue-700'}`}>
            45
          </div>
        </div>
        <div class={`p-6 rounded-lg border-2 ${activeTabTheme()?.border || 'border-blue-200'} ${activeTabTheme()?.bg || 'bg-blue-50'}`}>
          <div class="text-sm text-gray-500 mb-1">최근 업데이트</div>
          <div class={`text-2xl font-bold ${activeTabTheme()?.text || 'text-blue-700'}`}>
            오늘
          </div>
        </div>
      </div>

      {/* 분석 서브 탭 */}
      <div class="bg-white shadow-sm rounded-lg">
        <div class="px-6 pt-4">
          <div class="flex space-x-1">
            <For each={subTabs}>
              {(tab, index) => (
                <button
                  onClick={() => setActiveSubTab(tab.id)}
                  class={`
                    relative px-5 py-3 font-medium text-sm whitespace-nowrap
                    transition-all duration-200 ease-in-out rounded-t-lg
                    focus:outline-none
                    ${activeSubTab() === tab.id
                      ? `${tab.theme.bg} ${tab.theme.text} ${tab.theme.border} border-t border-l border-r border-b-0 shadow-md -mb-px z-10`
                      : 'bg-gray-50 text-gray-500 hover:text-gray-700 hover:bg-gray-100 border border-transparent hover:border-gray-200'
                    }
                    ${index() === 0 ? 'ml-0' : ''}
                  `}
                >
                  <span class="mr-2 text-base">{tab.icon}</span>
                  <span class="font-semibold">{tab.label}</span>
                  
                  {/* 활성 탭 강조 선 */}
                  {activeSubTab() === tab.id && (
                    <div class={`absolute bottom-0 left-4 right-4 h-0.5 bg-gradient-to-r ${tab.theme.accent} rounded-full`}></div>
                  )}
                </button>
              )}
            </For>
          </div>
        </div>
        
        <div class={`
          border rounded-b-lg shadow-sm p-6 relative
          ${activeTabTheme()?.bg || 'bg-blue-50'} ${activeTabTheme()?.border || 'border-blue-200'}
        `}>
          {/* 날짜 범위 슬라이더 */}
          <div class="mb-6 p-4 bg-white rounded-lg border">
            <h4 class="font-medium text-gray-800 mb-3">분석 기간 선택</h4>
            {/* 날짜 범위 슬라이더 구현 */}
          </div>

          {/* 탭별 컨텐츠 */}
          {activeSubTab() === 0 && (
            <div>
              <h3 class="text-lg font-medium mb-4 text-gray-800">제품 현황 개요</h3>
              <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
                <div class="h-80">
                  {/* 파이 차트 - 제조사별 분포 */}
                  <Chart type="pie" data={[]} title="제조사별 분포" />
                </div>
                <div class="h-80">
                  {/* 바 차트 - 월별 인증 현황 */}
                  <Chart type="bar" data={[]} title="월별 인증 현황" />
                </div>
              </div>
            </div>
          )}

          {activeSubTab() === 1 && (
            <div>
              <h3 class="text-lg font-medium mb-4 text-gray-800">제조사 분석</h3>
              <div class="space-y-4">
                {/* 상위 제조사 목록 */}
                <For each={Array.from({ length: 10 }, (_, i) => ({ name: `제조사 ${i+1}`, count: 100 - i*5 }))}>
                  {(manufacturer) => (
                    <div class="flex items-center justify-between p-3 bg-white rounded-lg border hover:shadow-md transition-shadow">
                      <div class="flex items-center space-x-3">
                        <div class="w-8 h-8 bg-emerald-100 rounded-full flex items-center justify-center">
                          <span class="text-emerald-600 font-semibold text-sm">
                            {manufacturer.name.charAt(manufacturer.name.length - 1)}
                          </span>
                        </div>
                        <span class="font-medium">{manufacturer.name}</span>
                      </div>
                      <div class="text-right">
                        <div class="font-semibold text-emerald-600">{manufacturer.count}</div>
                        <div class="text-xs text-gray-500">제품</div>
                      </div>
                    </div>
                  )}
                </For>
              </div>
            </div>
          )}

          {activeSubTab() === 2 && (
            <div>
              <h3 class="text-lg font-medium mb-4 text-gray-800">디바이스 유형 분석</h3>
              <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
                <div class="h-80">
                  {/* 디바이스 유형별 바 차트 */}
                  <Chart type="horizontalBar" data={[]} title="디바이스 유형별 분포" />
                </div>
                <div class="h-80">
                  {/* 시간별 트렌드 라인 차트 */}
                  <Chart type="line" data={[]} title="월별 인증 트렌드" />
                </div>
              </div>
            </div>
          )}

          {activeSubTab() === 3 && (
            <div>
              <h3 class="text-lg font-medium mb-4 text-gray-800">상호작용 분석</h3>
              <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
                <div>
                  <h4 class="font-medium text-gray-700 mb-3">네트워크 프로토콜 분포</h4>
                  <div class="h-64">
                    <Chart type="pie" data={[]} title="프로토콜별 분포" />
                  </div>
                </div>
                <div>
                  <h4 class="font-medium text-gray-700 mb-3">상호작용 복잡도</h4>
                  <div class="space-y-3">
                    {['WiFi', 'Thread', 'Zigbee', 'Bluetooth'].map((protocol) => (
                      <div class="flex items-center justify-between p-3 bg-white rounded border">
                        <span class="font-medium">{protocol}</span>
                        <div class="flex items-center space-x-2">
                          <div class="w-24 bg-gray-200 rounded-full h-2">
                            <div class="bg-rose-500 h-2 rounded-full" style={{ width: `${Math.random() * 100}%` }}></div>
                          </div>
                          <span class="text-sm text-gray-600">{Math.floor(Math.random() * 100)}%</span>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            </div>
          )}

          {activeSubTab() === 4 && (
            <div>
              <h3 class="text-lg font-medium mb-4 text-gray-800">상세 데이터 테이블</h3>
              {/* 여기에 상세 테이블 구현 */}
              <div class="bg-white rounded border overflow-hidden">
                <table class="w-full">
                  <thead class="bg-gray-50">
                    <tr>
                      <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">제조사</th>
                      <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">제품명</th>
                      <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">유형</th>
                      <th class="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase">인증일</th>
                    </tr>
                  </thead>
                  <tbody class="divide-y divide-gray-200">
                    {/* 테이블 데이터 */}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
```

</details>

---

## 현재 활성 탭 구조 (2025-10-09 기준)

### ✅ 1. 크롤링 엔진 탭 (CrawlingEngineTabSimple)

**위치**: `src/components/tabs/CrawlingEngineTabSimple.tsx`

**주요 기능**:
- 🔍 **사이트 상태 체크**: 실시간 페이지 카운트 감지
- 📋 **크롤링 플랜**: 전체/부분/수동 범위 선택
- 🚀 **실행 제어**: 시작/중지/일시정지/재개
- 📊 **Stage별 진행률**: 5단계 파이프라인 실시간 추적
- 📝 **Actor 이벤트 콘솔**: 백엔드 이벤트 로그 스트리밍
- 🧪 **DB 진단**: 좌표 정합성 검사 및 동기화

**구성**:
```tsx
// 패널 구조
<ControlPanel />           // 플랜 선택 및 실행 버튼
<SessionStatusCard />      // 현재 세션 상태
<StageStatsPanels />       // Stage 1-5 실시간 통계
<DiagnosticsPanel />       // DB 진단 및 동기화
<SyncPanel />              // 좌표 동기화 범위 지정
<ValidationPanel />        // 데이터 검증
<PersistPanel />           // DB 저장 통계
```

### ✅ 2. 설정 탭 (SettingsTab)

**위치**: `src/components/tabs/SettingsTab.tsx`

**주요 기능**:
- ⚙️ **크롤링 설정**: 동시성, 재시도, 타임아웃
- 🏭 **Vendor 관리**: 제조사 목록 동기화
- 📱 **Device Types 관리**: 디바이스 유형 동기화
- 📁 **경로 설정**: DB, 로그, 백업 디렉토리
- 📊 **로깅 설정**: 로그 레벨, 파일 크기 제한

**구성**:
```tsx
// 설정 그룹
<CrawlingSettings />       // 크롤링 파라미터
<VendorSettings />         // Vendor 관리
<DeviceTypeSettings />     // Device Type 관리
<PathSettings />           // 디렉토리 경로
<LoggingSettings />        // 로그 설정
```

### ✅ 3. 로컬DB 탭 (LocalDBTab)

**위치**: `src/components/tabs/LocalDBTab.tsx`

**주요 기능**:
- 📊 **통계 대시보드**: 제품 수, 페이지 수, 최근 업데이트
- 🔍 **제품 검색**: 키워드, 제조사, 디바이스 유형 필터
- 📤 **백업/복원**: Excel 전체 백업 및 복원
- 📁 **데이터 관리**: 범위 삭제, 전체 삭제
- 📈 **분석 도구**: 제조사별/유형별 통계 (분석 탭 통합)

**구성**:
```tsx
// 대시보드 섹션
<StatsSummary />           // 통계 요약 카드
<SearchPanel />            // 제품 검색 및 필터
<BackupRestorePanel />     // 백업/복원 도구
<DataManagementPanel />    // 데이터 삭제 도구
<AnalyticsPanel />         // 제조사/유형별 분석 (구 분석 탭)
```

### 📝 새 탭 추가 가이드

1. **탭 컴포넌트 생성**:
   ```tsx
   // src/components/tabs/MyNewTab.tsx
   export default function MyNewTab() {
     return <div>My New Tab Content</div>;
   }
   ```

2. **App.tsx에 라우팅 추가**:
   ```tsx
   // src/App.tsx
   import MyNewTab from './components/tabs/MyNewTab';
   
   const tabs = [
     { id: 'crawling', label: '크롤링 엔진', component: CrawlingEngineTabSimple },
     { id: 'settings', label: '설정', component: SettingsTab },
     { id: 'localdb', label: '로컬DB', component: LocalDBTab },
     { id: 'mynew', label: '새 탭', component: MyNewTab }, // 추가
   ];
   ```

3. **탭 아이콘 및 스타일 설정**:
   ```tsx
   const tabIcons = {
     crawling: '🚀',
     settings: '⚙️',
     localdb: '💾',
     mynew: '✨', // 추가
   };
   ```

---

## Tauri 통합 가이드

### 1. Rust 백엔드 명령 정의

```rust
// src-tauri/src/main.rs
use tauri::command;

#[command]
async fn start_crawling(config: CrawlingConfig) -> Result<String, String> {
    // 크롤링 시작 로직
    Ok("Crawling started".to_string())
}

#[command]
async fn stop_crawling() -> Result<String, String> {
    // 크롤링 중지 로직
    Ok("Crawling stopped".to_string())
}

#[command]
async fn get_products(page: u32, limit: u32) -> Result<Vec<Product>, String> {
    // 데이터베이스에서 제품 조회
    Ok(vec![])
}

#[command]
async fn export_to_excel(data: Vec<Product>) -> Result<String, String> {
    // 엑셀 파일 생성 및 저장
    Ok("Excel exported".to_string())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            start_crawling,
            stop_crawling,
            get_products,
            export_to_excel
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 2. 프론트엔드에서 Tauri 명령 호출

```tsx
// utils/tauri.ts
import { invoke } from '@tauri-apps/api/tauri';

export async function startCrawling(config: any) {
  return await invoke('start_crawling', { config });
}

export async function stopCrawling() {
  return await invoke('stop_crawling');
}

export async function getProducts(page: number, limit: number) {
  return await invoke('get_products', { page, limit });
}

export async function exportToExcel(data: any[]) {
  return await invoke('export_to_excel', { data });
}
```

### 3. 실시간 이벤트 처리

```tsx
// stores/eventStore.ts
import { listen } from '@tauri-apps/api/event';
import { createSignal } from 'solid-js';

const [crawlingProgress, setCrawlingProgress] = createSignal(0);
const [crawlingStatus, setCrawlingStatus] = createSignal('idle');

// 백엔드에서 보내는 이벤트 수신
listen('crawling-progress', (event) => {
  setCrawlingProgress(event.payload.progress);
});

listen('crawling-status', (event) => {
  setCrawlingStatus(event.payload.status);
});

export { crawlingProgress, crawlingStatus };
```

## 스타일링 최적화

### 1. Tailwind CSS 설정

```javascript
// tailwind.config.js
module.exports = {
  content: ['./src/**/*.{js,jsx,ts,tsx}'],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        emerald: { /* 설정 탭 색상 */ },
        blue: { /* 상태 탭 색상 */ },
        purple: { /* 로컬DB 탭 색상 */ },
        amber: { /* 분석 탭 색상 */ }
      },
      animation: {
        'tab-focus': 'focusRingFadeOut 2s ease-out forwards',
        'pulse-slow': 'pulse 3s ease-in-out infinite',
        'sand-fall': 'sandFall 2s linear infinite'
      }
    }
  }
};
```

### 2. CSS 애니메이션 정의

```css
/* src/styles/animations.css */
@keyframes focusRingFadeOut {
  0% {
    box-shadow: 0 0 0 2px rgba(251, 146, 60, 0.75);
  }
  100% {
    box-shadow: 0 0 0 2px rgba(251, 146, 60, 0);
  }
}

@keyframes sandFall {
  0% { transform: translateY(-100%); }
  100% { transform: translateY(100%); }
}

.tab-focus-animation {
  animation: focusRingFadeOut 2s ease-out forwards;
}
```

## 성능 최적화 팁

### 1. SolidJS 최적화
- `createMemo`를 사용한 계산값 캐싱
- `createSignal`로 반응형 상태 관리
- `batch`를 사용한 업데이트 배치 처리

### 2. Tauri 최적화
- 백엔드 작업을 별도 스레드에서 실행
- 큰 데이터는 스트리밍으로 전송
- 필요한 경우에만 프론트엔드에 이벤트 발송

### 3. 메모리 관리
- 리스트 가상화로 대량 데이터 처리
- 불필요한 상태 정리
- 이벤트 리스너 적절한 정리

이 가이드를 따라 구현하면 현재 React + Electron 프로젝트의 UI 구성과 기능 배치를 SolidJS + Tauri 환경에서 유사하게 재현할 수 있습니다. 각 탭의 고유한 색상 테마와 기능적 배치를 그대로 유지하면서도 SolidJS의 성능상 이점과 Tauri의 가벼운 번들 크기를 활용할 수 있습니다.
