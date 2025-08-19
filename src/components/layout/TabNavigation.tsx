/**
 * TabNavigation - 탭 네비게이션 컴포넌트
 * SolidJS-UI-Implementation-Guide.md를 기반으로 구현
 */

import { For, Component, createSignal } from 'solid-js';
import { tabState, setActiveTab } from '../../stores/tabStore';
import { windowState } from '../../stores/windowStore';
import { tauriApi } from '../../services/tauri-api';

export const TabNavigation: Component = () => {
  const [isQuickCheckRunning, setIsQuickCheckRunning] = createSignal(false);
  const [isSiteAnalysisRunning, setIsSiteAnalysisRunning] = createSignal(false);

  const handleTabClick = (tabId: string) => {
    setActiveTab(tabId);
    // 마지막 활성 탭을 windowState에 저장
    windowState.setLastActiveTab(tabId);
  };

  const runQuickStatusCheck = async () => {
    try {
      setIsQuickCheckRunning(true);
      console.log('� 빠른 상태 체크 시작 (실시간 모니터링)...');
      
      // 상태 탭으로 이동
      setActiveTab('status');
      windowState.setLastActiveTab('status');
      
      // 잠시 후 상태 체크 실행 (UI가 로드될 시간을 줌)
      setTimeout(async () => {
        try {
          const result = await tauriApi.getCrawlingStatusCheck();
          console.log('✅ 빠른 상태 체크 완료:', result);
        } catch (error) {
          console.error('❌ 빠른 상태 체크 실패:', error);
        } finally {
          setIsQuickCheckRunning(false);
        }
      }, 100);
    } catch (error) {
      console.error('❌ 빠른 상태 체크 오류:', error);
      setIsQuickCheckRunning(false);
    }
  };

  const runSiteAnalysis = async () => {
    try {
      setIsSiteAnalysisRunning(true);
      console.log('🔍 사이트 종합 분석 시작 (사전 조사)...');
      
      // 상태 탭으로 이동
      setActiveTab('status');
      windowState.setLastActiveTab('status');
      
      // 잠시 후 사이트 분석 실행
      setTimeout(async () => {
        try {
          const result = await tauriApi.checkSiteStatus();
          console.log('✅ 사이트 분석 완료:', result);
        } catch (error) {
          console.error('❌ 사이트 분석 실패:', error);
        } finally {
          setIsSiteAnalysisRunning(false);
        }
      }, 100);
    } catch (error) {
      console.error('❌ 사이트 분석 오류:', error);
      setIsSiteAnalysisRunning(false);
    }
  };

  return (
    <div class="bg-white dark:bg-gray-800 shadow-sm">
      <div class="px-6 pt-4">
        <div class="flex items-center justify-between">
          {/* 탭 버튼들 */}
          <div class="flex space-x-1">
            <For each={tabState.tabs}>
              {(tab, index) => (
                <button
                  data-tab={tab.id}
                  onClick={() => handleTabClick(tab.id)}
                  class={`
                    relative px-6 py-3 font-medium text-sm whitespace-nowrap
                    transition-all duration-200 ease-in-out rounded-t-lg
                    focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500
                    ${tabState.activeTab === tab.id
                      ? `${tab.theme.bg} ${tab.theme.text} ${tab.theme.border} border-t border-l border-r border-b-0 shadow-md -mb-px z-10 dark:${tab.theme.bg.replace('50', '900')} dark:${tab.theme.text.replace('700', '300')}`
                      : 'bg-gray-50 text-gray-500 hover:text-gray-700 hover:bg-gray-100 border border-transparent hover:border-gray-200 dark:bg-gray-700 dark:text-gray-400 dark:hover:text-gray-300 dark:hover:bg-gray-600'
                    }
                    ${index() === 0 ? 'ml-0' : ''}
                  `}
                  style={{
                    'box-shadow': tabState.activeTab === tab.id 
                      ? '0 -2px 8px rgba(0,0,0,0.04), 0 2px 4px rgba(0,0,0,0.02)' 
                      : 'none'
                  }}
                >
                  <span class="mr-2 text-base">{tab.icon}</span>
                  <span class="font-semibold">{tab.label}</span>
                  
                  {/* 활성 탭에 그라데이션 언더라인 */}
                  {tabState.activeTab === tab.id && (
                    <div class={`absolute bottom-0 left-0 right-0 h-1 bg-gradient-to-r ${tab.theme.accent} rounded-b-lg`} />
                  )}
                </button>
              )}
            </For>
          </div>

          {/* 빠른 액세스 버튼들 */}
          <div class="flex items-center gap-2">
            {/* 사이트 종합 분석 버튼 */}
            <button
              onClick={runSiteAnalysis}
              disabled={isSiteAnalysisRunning() || isQuickCheckRunning()}
              class={`
                px-3 py-2 rounded-lg font-medium text-xs transition-all duration-200
                ${isSiteAnalysisRunning() || isQuickCheckRunning()
                  ? 'bg-gray-300 text-gray-500 cursor-not-allowed' 
                  : 'bg-green-500 hover:bg-green-600 text-white shadow-md hover:shadow-lg'
                }
                focus:outline-none focus:ring-2 focus:ring-green-500 focus:ring-offset-2
              `}
              title="실제 사이트를 분석하여 페이지 구조와 예상 제품 수를 파악합니다 (사전 조사)"
            >
              <span class="mr-1">{isSiteAnalysisRunning() ? '🔄' : '🔍'}</span>
              {isSiteAnalysisRunning() ? '분석 중...' : '사이트 분석'}
            </button>

            {/* 빠른 상태 체크 버튼 */}
            <button
              onClick={runQuickStatusCheck}
              disabled={isQuickCheckRunning() || isSiteAnalysisRunning()}
              class={`
                px-3 py-2 rounded-lg font-medium text-xs transition-all duration-200
                ${isQuickCheckRunning() || isSiteAnalysisRunning()
                  ? 'bg-gray-300 text-gray-500 cursor-not-allowed' 
                  : 'bg-blue-500 hover:bg-blue-600 text-white shadow-md hover:shadow-lg'
                }
                focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2
              `}
              title="메모리에서 현재 크롤링 진행 상황을 빠르게 조회합니다 (실시간 모니터링)"
            >
              <span class="mr-1">{isQuickCheckRunning() ? '🔄' : '�'}</span>
              {isQuickCheckRunning() ? '조회 중...' : '상태 조회'}
            </button>

            <div class="h-6 w-px bg-gray-300 dark:bg-gray-600"></div>

            {/* 현재 시간 표시 */}
            <div class="text-xs text-gray-500 dark:text-gray-400">
              {new Date().toLocaleTimeString('ko-KR', { hour: '2-digit', minute: '2-digit' })}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
