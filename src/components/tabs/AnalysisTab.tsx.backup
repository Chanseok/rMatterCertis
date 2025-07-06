/**
 * AnalysisTab - 분석 탭 컴포넌트
 * SolidJS-UI-Implementation-Guide.md를 기반으로 구현
 */

import { Component, createSignal, createMemo, For, Show } from 'solid-js';

interface AnalysisSubTab {
  id: number;
  label: string;
  icon: string;
  theme: {
    bg: string;
    text: string;
    border: string;
    accent: string;
  };
}

export const AnalysisTab: Component = () => {
  const [activeSubTab, setActiveSubTab] = createSignal(0);

  const subTabs: AnalysisSubTab[] = [
    { 
      id: 0, 
      label: '제품 현황', 
      icon: '📊', 
      theme: { 
        bg: 'bg-blue-50', 
        text: 'text-blue-700', 
        border: 'border-blue-200', 
        accent: 'from-blue-500 to-indigo-500' 
      } 
    },
    { 
      id: 1, 
      label: '제조사 분석', 
      icon: '🏭', 
      theme: { 
        bg: 'bg-emerald-50', 
        text: 'text-emerald-700', 
        border: 'border-emerald-200', 
        accent: 'from-emerald-500 to-teal-500' 
      } 
    },
    { 
      id: 2, 
      label: '디바이스 유형 분석', 
      icon: '📱', 
      theme: { 
        bg: 'bg-purple-50', 
        text: 'text-purple-700', 
        border: 'border-purple-200', 
        accent: 'from-purple-500 to-violet-500' 
      } 
    },
    { 
      id: 3, 
      label: '상호작용 분석', 
      icon: '🔄', 
      theme: { 
        bg: 'bg-rose-50', 
        text: 'text-rose-700', 
        border: 'border-rose-200', 
        accent: 'from-rose-500 to-pink-500' 
      } 
    },
    { 
      id: 4, 
      label: '데이터 테이블', 
      icon: '📋', 
      theme: { 
        bg: 'bg-orange-50', 
        text: 'text-orange-700', 
        border: 'border-orange-200', 
        accent: 'from-orange-500 to-amber-500' 
      } 
    }
  ];

  const activeTabTheme = createMemo(() => 
    subTabs.find(tab => tab.id === activeSubTab())?.theme
  );

  // 임시 통계 데이터
  const stats = {
    totalProducts: 1234,
    totalManufacturers: 89,
    totalDeviceTypes: 45,
    lastUpdate: '오늘'
  };

  const manufacturerData = [
    { name: 'Samsung', count: 245, percentage: 19.9 },
    { name: 'LG', count: 189, percentage: 15.3 },
    { name: 'Apple', count: 156, percentage: 12.6 },
    { name: 'Google', count: 134, percentage: 10.9 },
    { name: 'Amazon', count: 98, percentage: 7.9 }
  ];

  const deviceTypeData = [
    { name: 'Smart Speaker', count: 312, percentage: 25.3 },
    { name: 'Light Bulb', count: 267, percentage: 21.6 },
    { name: 'Smart Switch', count: 234, percentage: 19.0 },
    { name: 'Sensor', count: 198, percentage: 16.0 },
    { name: 'Others', count: 223, percentage: 18.1 }
  ];

  return (
    <div class="space-y-6">
      {/* 통계 요약 카드 */}
      <div class="grid grid-cols-1 md:grid-cols-4 gap-6">
        <div class={`p-6 rounded-lg border-2 ${activeTabTheme()?.border || 'border-blue-200'} ${activeTabTheme()?.bg || 'bg-blue-50'} dark:${activeTabTheme()?.bg?.replace('50', '900/20') || 'bg-blue-900/20'} dark:border-blue-700`}>
          <div class="text-sm text-gray-500 dark:text-gray-400 mb-1">총 제품 수</div>
          <div class={`text-2xl font-bold ${activeTabTheme()?.text || 'text-blue-700'} dark:${activeTabTheme()?.text?.replace('700', '400') || 'text-blue-400'}`}>
            {stats.totalProducts.toLocaleString()}
          </div>
        </div>
        <div class={`p-6 rounded-lg border-2 ${activeTabTheme()?.border || 'border-blue-200'} ${activeTabTheme()?.bg || 'bg-blue-50'} dark:${activeTabTheme()?.bg?.replace('50', '900/20') || 'bg-blue-900/20'} dark:border-blue-700`}>
          <div class="text-sm text-gray-500 dark:text-gray-400 mb-1">제조사 수</div>
          <div class={`text-2xl font-bold ${activeTabTheme()?.text || 'text-blue-700'} dark:${activeTabTheme()?.text?.replace('700', '400') || 'text-blue-400'}`}>
            {stats.totalManufacturers}
          </div>
        </div>
        <div class={`p-6 rounded-lg border-2 ${activeTabTheme()?.border || 'border-blue-200'} ${activeTabTheme()?.bg || 'bg-blue-50'} dark:${activeTabTheme()?.bg?.replace('50', '900/20') || 'bg-blue-900/20'} dark:border-blue-700`}>
          <div class="text-sm text-gray-500 dark:text-gray-400 mb-1">디바이스 유형 수</div>
          <div class={`text-2xl font-bold ${activeTabTheme()?.text || 'text-blue-700'} dark:${activeTabTheme()?.text?.replace('700', '400') || 'text-blue-400'}`}>
            {stats.totalDeviceTypes}
          </div>
        </div>
        <div class={`p-6 rounded-lg border-2 ${activeTabTheme()?.border || 'border-blue-200'} ${activeTabTheme()?.bg || 'bg-blue-50'} dark:${activeTabTheme()?.bg?.replace('50', '900/20') || 'bg-blue-900/20'} dark:border-blue-700`}>
          <div class="text-sm text-gray-500 dark:text-gray-400 mb-1">최근 업데이트</div>
          <div class={`text-2xl font-bold ${activeTabTheme()?.text || 'text-blue-700'} dark:${activeTabTheme()?.text?.replace('700', '400') || 'text-blue-400'}`}>
            {stats.lastUpdate}
          </div>
        </div>
      </div>

      {/* 분석 서브 탭 */}
      <div class="bg-white dark:bg-gray-800 shadow-sm rounded-lg border border-gray-200 dark:border-gray-700">
        <div class="px-6 pt-4">
          <div class="flex flex-wrap gap-1">
            <For each={subTabs}>
              {(tab, index) => (
                <button
                  onClick={() => setActiveSubTab(tab.id)}
                  class={`
                    relative px-5 py-3 font-medium text-sm whitespace-nowrap rounded-t-lg transition-all duration-200
                    ${activeSubTab() === tab.id
                      ? `${tab.theme.bg} ${tab.theme.text} ${tab.theme.border} border-t border-l border-r border-b-0 shadow-md -mb-px z-10 dark:${tab.theme.bg.replace('50', '800')} dark:${tab.theme.text.replace('700', '300')}`
                      : 'bg-gray-50 text-gray-500 hover:text-gray-700 hover:bg-gray-100 border border-transparent hover:border-gray-200 dark:bg-gray-700 dark:text-gray-400 dark:hover:text-gray-300 dark:hover:bg-gray-600'
                    }
                    ${index() === 0 ? 'ml-0' : ''}
                  `}
                >
                  <span class="mr-2 text-base">{tab.icon}</span>
                  <span class="font-semibold">{tab.label}</span>
                  
                  {/* 활성 탭 강조 선 */}
                  {activeSubTab() === tab.id && (
                    <div class={`absolute bottom-0 left-0 right-0 h-1 bg-gradient-to-r ${tab.theme.accent} rounded-b-lg`} />
                  )}
                </button>
              )}
            </For>
          </div>
        </div>
        
        <div class={`
          border rounded-b-lg shadow-sm p-6 relative
          ${activeTabTheme()?.bg || 'bg-blue-50'} ${activeTabTheme()?.border || 'border-blue-200'}
          dark:${activeTabTheme()?.bg?.replace('50', '900/20') || 'bg-blue-900/20'} dark:border-blue-700
        `}>
          {/* 날짜 범위 슬라이더 */}
          <div class="mb-6 p-4 bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700">
            <h4 class="font-medium text-gray-800 dark:text-gray-200 mb-3">분석 기간 선택</h4>
            <div class="flex items-center space-x-4">
              <input 
                type="date" 
                class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md text-sm dark:bg-gray-700 dark:text-white"
                value="2024-01-01"
              />
              <span class="text-gray-500 dark:text-gray-400">~</span>
              <input 
                type="date" 
                class="px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md text-sm dark:bg-gray-700 dark:text-white"
                value={new Date().toISOString().split('T')[0]}
              />
            </div>
          </div>

          {/* 탭별 컨텐츠 */}
          <Show when={activeSubTab() === 0}>
            <div>
              <h3 class="text-lg font-medium mb-4 text-gray-800 dark:text-gray-200">제품 현황 개요</h3>
              <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
                <div class="bg-white dark:bg-gray-800 p-6 rounded-lg border border-gray-200 dark:border-gray-700">
                  <h4 class="text-md font-semibold text-gray-700 dark:text-gray-300 mb-4">제조사별 분포</h4>
                  <div class="space-y-3">
                    <For each={manufacturerData.slice(0, 5)}>
                      {(item) => (
                        <div class="flex items-center justify-between">
                          <span class="text-sm text-gray-600 dark:text-gray-400">{item.name}</span>
                          <div class="flex items-center space-x-2">
                            <div class="w-20 bg-gray-200 dark:bg-gray-700 rounded-full h-2">
                              <div 
                                class="bg-blue-600 h-2 rounded-full" 
                                style={{ width: `${item.percentage}%` }}
                              />
                            </div>
                            <span class="text-sm font-medium text-gray-700 dark:text-gray-300 w-12 text-right">
                              {item.count}
                            </span>
                          </div>
                        </div>
                      )}
                    </For>
                  </div>
                </div>
                <div class="bg-white dark:bg-gray-800 p-6 rounded-lg border border-gray-200 dark:border-gray-700">
                  <h4 class="text-md font-semibold text-gray-700 dark:text-gray-300 mb-4">디바이스 유형별 분포</h4>
                  <div class="space-y-3">
                    <For each={deviceTypeData}>
                      {(item) => (
                        <div class="flex items-center justify-between">
                          <span class="text-sm text-gray-600 dark:text-gray-400">{item.name}</span>
                          <div class="flex items-center space-x-2">
                            <div class="w-20 bg-gray-200 dark:bg-gray-700 rounded-full h-2">
                              <div 
                                class="bg-purple-600 h-2 rounded-full" 
                                style={{ width: `${item.percentage}%` }}
                              />
                            </div>
                            <span class="text-sm font-medium text-gray-700 dark:text-gray-300 w-12 text-right">
                              {item.count}
                            </span>
                          </div>
                        </div>
                      )}
                    </For>
                  </div>
                </div>
              </div>
            </div>
          </Show>

          <Show when={activeSubTab() === 1}>
            <div>
              <h3 class="text-lg font-medium mb-4 text-gray-800 dark:text-gray-200">제조사 분석</h3>
              <div class="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 p-6">
                <h4 class="text-md font-semibold text-gray-700 dark:text-gray-300 mb-4">상위 제조사 목록</h4>
                <div class="space-y-4">
                  <For each={manufacturerData}>
                    {(manufacturer, index) => (
                      <div class="flex items-center justify-between p-4 bg-emerald-50 dark:bg-emerald-900/20 rounded-lg border border-emerald-200 dark:border-emerald-700">
                        <div class="flex items-center space-x-4">
                          <div class="w-8 h-8 bg-emerald-600 text-white rounded-full flex items-center justify-center text-sm font-bold">
                            {index() + 1}
                          </div>
                          <div>
                            <div class="font-medium text-gray-900 dark:text-white">{manufacturer.name}</div>
                            <div class="text-sm text-gray-500 dark:text-gray-400">{manufacturer.percentage}% of total</div>
                          </div>
                        </div>
                        <div class="text-2xl font-bold text-emerald-600 dark:text-emerald-400">
                          {manufacturer.count}
                        </div>
                      </div>
                    )}
                  </For>
                </div>
              </div>
            </div>
          </Show>

          <Show when={activeSubTab() === 2}>
            <div>
              <h3 class="text-lg font-medium mb-4 text-gray-800 dark:text-gray-200">디바이스 유형 분석</h3>
              <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                <For each={deviceTypeData}>
                  {(deviceType) => (
                    <div class="bg-white dark:bg-gray-800 p-6 rounded-lg border border-gray-200 dark:border-gray-700">
                      <div class="flex items-center justify-between mb-4">
                        <h4 class="text-md font-semibold text-gray-700 dark:text-gray-300">{deviceType.name}</h4>
                        <span class="text-lg">📱</span>
                      </div>
                      <div class="text-3xl font-bold text-purple-600 dark:text-purple-400 mb-2">
                        {deviceType.count}
                      </div>
                      <div class="text-sm text-gray-500 dark:text-gray-400">
                        전체의 {deviceType.percentage}%
                      </div>
                      <div class="mt-3 w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2">
                        <div 
                          class="bg-purple-600 h-2 rounded-full transition-all duration-500" 
                          style={{ width: `${deviceType.percentage}%` }}
                        />
                      </div>
                    </div>
                  )}
                </For>
              </div>
            </div>
          </Show>

          <Show when={activeSubTab() === 3}>
            <div>
              <h3 class="text-lg font-medium mb-4 text-gray-800 dark:text-gray-200">상호작용 분석</h3>
              <div class="bg-white dark:bg-gray-800 p-6 rounded-lg border border-gray-200 dark:border-gray-700">
                <div class="text-center py-12">
                  <div class="text-6xl mb-4">🔄</div>
                  <h4 class="text-lg font-medium text-gray-700 dark:text-gray-300 mb-2">상호작용 분석</h4>
                  <p class="text-gray-500 dark:text-gray-400">
                    제조사와 디바이스 유형 간의 상관관계 분석 기능이 준비 중입니다.
                  </p>
                </div>
              </div>
            </div>
          </Show>

          <Show when={activeSubTab() === 4}>
            <div>
              <h3 class="text-lg font-medium mb-4 text-gray-800 dark:text-gray-200">데이터 테이블</h3>
              <div class="bg-white dark:bg-gray-800 p-6 rounded-lg border border-gray-200 dark:border-gray-700">
                <div class="text-center py-12">
                  <div class="text-6xl mb-4">📋</div>
                  <h4 class="text-lg font-medium text-gray-700 dark:text-gray-300 mb-2">상세 데이터 테이블</h4>
                  <p class="text-gray-500 dark:text-gray-400">
                    자세한 데이터는 로컬DB 탭에서 확인하실 수 있습니다.
                  </p>
                  <button class="mt-4 px-4 py-2 bg-orange-600 text-white rounded-md hover:bg-orange-700 transition-colors">
                    로컬DB 탭으로 이동
                  </button>
                </div>
              </div>
            </div>
          </Show>
        </div>
      </div>
    </div>
  );
};
