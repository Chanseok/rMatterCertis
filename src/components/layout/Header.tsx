/**
 * Header - 애플리케이션 헤더 컴포넌트
 * SolidJS-UI-Implementation-Guide.md를 기반으로 구현
 */

import { Component } from 'solid-js';

export const Header: Component = () => {
  return (
    <header class="bg-white dark:bg-gray-800 shadow-sm border-b border-gray-200 dark:border-gray-700">
      <div class="px-6 py-4">
        <div class="flex items-center justify-between">
          {/* 로고 및 타이틀 */}
          <div class="flex items-center space-x-3">
            <div class="w-8 h-8 bg-gradient-to-br from-blue-500 to-purple-600 rounded-lg flex items-center justify-center">
              <span class="text-white font-bold text-sm">MC</span>
            </div>
            <div>
              <h1 class="text-xl font-bold text-gray-900 dark:text-white">
                Matter Certification Crawler
              </h1>
              <p class="text-sm text-gray-500 dark:text-gray-400">
                인증 정보 수집 및 관리 시스템
              </p>
            </div>
          </div>

          {/* 헤더 액션 버튼들 제거 - 불필요한 UI 요소 */}
        </div>
      </div>
    </header>
  );
};
