/**
 * SettingsTab - 설정 탭 컴포넌트
 * SolidJS-UI-Implementation-Guide.md를 기반으로 구현
 */

import { Component, createSignal } from 'solid-js';
import { ExpandableSection } from '../common/ExpandableSection';
import { crawlerStore } from '../../stores/crawlerStore';

export const SettingsTab: Component = () => {
  const [isAdvancedExpanded, setIsAdvancedExpanded] = createSignal(false);
  const [isBatchExpanded, setIsBatchExpanded] = createSignal(true);

  const handleSaveSettings = () => {
    // 설정 저장 로직
    console.log('Settings saved');
  };

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
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              시작 페이지
            </label>
            <input 
              type="number" 
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md focus:outline-none focus:ring-2 focus:ring-emerald-500 dark:bg-gray-700 dark:text-white"
              placeholder="1"
              value={crawlerStore.state.currentConfig?.start_page || 1}
              onInput={(e) => console.log('Start page changed:', e.currentTarget.value)}
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              종료 페이지
            </label>
            <input 
              type="number" 
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md focus:outline-none focus:ring-2 focus:ring-emerald-500 dark:bg-gray-700 dark:text-white"
              placeholder="100"
              value={crawlerStore.state.currentConfig?.end_page || 100}
              onInput={(e) => console.log('End page changed:', e.currentTarget.value)}
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
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              동시 실행 수
            </label>
            <select 
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md focus:outline-none focus:ring-2 focus:ring-emerald-500 dark:bg-gray-700 dark:text-white"
              value={crawlerStore.state.currentConfig?.concurrency || 6}
              onChange={(e) => console.log('Concurrency changed:', e.currentTarget.value)}
            >
              <option value="6">6개 (기본값)</option>
              <option value="12">12개</option>
              <option value="24">24개</option>
            </select>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              재시도 횟수
            </label>
            <input 
              type="number" 
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md focus:outline-none focus:ring-2 focus:ring-emerald-500 dark:bg-gray-700 dark:text-white"
              placeholder="3"
              value={crawlerStore.state.currentConfig?.product_detail_retry_count || 3}
              onInput={(e) => console.log('Retry count changed:', e.currentTarget.value)}
            />
          </div>
        </div>
      </ExpandableSection>

      {/* 고급 설정 */}
      <ExpandableSection
        title="고급 설정"
        isExpanded={isAdvancedExpanded()}
        onToggle={setIsAdvancedExpanded}
        icon="🔧"
      >
        <div class="space-y-4">
          <div class="flex items-center space-x-2">
            <input 
              type="checkbox" 
              id="debugMode"
              class="rounded border-gray-300 text-emerald-600 shadow-sm focus:border-emerald-300 focus:ring focus:ring-emerald-200 focus:ring-opacity-50"
              checked={false}
              onChange={(e) => console.log('Debug mode changed:', e.currentTarget.checked)}
            />
            <label for="debugMode" class="text-sm font-medium text-gray-700 dark:text-gray-300">
              디버그 모드 활성화
            </label>
          </div>
          <div class="flex items-center space-x-2">
            <input 
              type="checkbox" 
              id="enableLogging"
              class="rounded border-gray-300 text-emerald-600 shadow-sm focus:border-emerald-300 focus:ring focus:ring-emerald-200 focus:ring-opacity-50"
              checked={false}
              onChange={(e) => console.log('Logging changed:', e.currentTarget.checked)}
            />
            <label for="enableLogging" class="text-sm font-medium text-gray-700 dark:text-gray-300">
              상세 로깅 활성화
            </label>
          </div>
        </div>
      </ExpandableSection>

      {/* 저장 버튼 */}
      <div class="flex justify-end">
        <button 
          onClick={handleSaveSettings}
          class="px-6 py-2 bg-emerald-600 text-white rounded-md hover:bg-emerald-700 transition-colors focus:outline-none focus:ring-2 focus:ring-emerald-500 focus:ring-offset-2"
        >
          설정 저장
        </button>
      </div>
    </div>
  );
};
