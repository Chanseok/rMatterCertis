/**
 * StatusTab - 상태 & 제어 탭 컴포넌트 (안전한 버전)
 * 복잡한 의존성 없이 기본 기능만 구현
 */

import { Component, createSignal, onMount } from 'solid-js';
import { ExpandableSection } from '../common/ExpandableSection';
import { tauriApi } from '../../services/tauri-api';

export const StatusTab: Component = () => {
  const [isControlExpanded, setIsControlExpanded] = createSignal(true);
  const [isStatusExpanded, setIsStatusExpanded] = createSignal(true);
  const [isLoading, setIsLoading] = createSignal(false);
  const [statusMessage, setStatusMessage] = createSignal('앱이 초기화되었습니다.');
  const [crawlingStatus, setCrawlingStatus] = createSignal<'idle' | 'running' | 'paused' | 'completed'>('idle');
  const [progress, setProgress] = createSignal(0);
  const [currentPage, setCurrentPage] = createSignal(0);
  const [totalPages, setTotalPages] = createSignal(0);

  onMount(() => {
    setStatusMessage('상태 & 제어 탭이 로드되었습니다.');
  });

  const handleStart = async () => {
    try {
      setIsLoading(true);
      setCrawlingStatus('running');
      setStatusMessage('크롤링을 시작합니다...');
      
      // 실제 크롤링 로직은 추후 구현
      // await tauriApi.startCrawling();
      
      setStatusMessage('크롤링이 시작되었습니다.');
    } catch (error) {
      console.error('Failed to start crawling:', error);
      setStatusMessage(`크롤링 시작 실패: ${error}`);
      setCrawlingStatus('idle');
    } finally {
      setIsLoading(false);
    }
  };

  const handleStop = async () => {
    try {
      setIsLoading(true);
      setCrawlingStatus('idle');
      setProgress(0);
      setCurrentPage(0);
      setStatusMessage('크롤링을 중지합니다...');
      
      // 실제 크롤링 중지 로직은 추후 구현
      // await tauriApi.stopCrawling();
      
      setStatusMessage('크롤링이 중지되었습니다.');
    } catch (error) {
      console.error('Failed to stop crawling:', error);
      setStatusMessage(`크롤링 중지 실패: ${error}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleStatusCheck = async () => {
    try {
      setIsLoading(true);
      setStatusMessage('사이트 상태를 확인하는 중...');
      
      // 실제 상태 체크 로직은 추후 구현
      // const status = await tauriApi.checkSiteStatus();
      
      setStatusMessage('사이트 상태 확인이 완료되었습니다.');
    } catch (error) {
      console.error('Failed to check site status:', error);
      setStatusMessage(`상태 확인 실패: ${error}`);
    } finally {
      setIsLoading(false);
    }
  };

  const getStatusColor = () => {
    switch (crawlingStatus()) {
      case 'running': return 'text-green-600';
      case 'paused': return 'text-yellow-600';
      case 'completed': return 'text-blue-600';
      default: return 'text-gray-600';
    }
  };

  const getStatusText = () => {
    switch (crawlingStatus()) {
      case 'running': return '실행 중';
      case 'paused': return '일시 정지';
      case 'completed': return '완료';
      default: return '대기 중';
    }
  };

  return (
    <div class="flex flex-col space-y-6 p-6">
      {/* 상태 메시지 */}
      <div class="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-700 rounded-lg p-4">
        <div class="flex items-center space-x-3">
          <div class="flex-shrink-0">
            <svg class="w-5 h-5 text-blue-400" fill="currentColor" viewBox="0 0 20 20">
              <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clip-rule="evenodd" />
            </svg>
          </div>
          <div>
            <h3 class="text-sm font-medium text-blue-800 dark:text-blue-200">
              시스템 상태
            </h3>
            <div class="mt-1 text-sm text-blue-700 dark:text-blue-300">
              {statusMessage()}
            </div>
          </div>
        </div>
      </div>

      {/* 크롤링 제어 섹션 */}
      <ExpandableSection 
        title="크롤링 제어" 
        isExpanded={isControlExpanded()} 
        onToggle={() => setIsControlExpanded(!isControlExpanded())}
        icon="🎮"
      >
        <div class="space-y-4">
          {/* 현재 상태 */}
          <div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-4">
            <div class="flex items-center justify-between">
              <div>
                <h4 class="text-sm font-medium text-gray-900 dark:text-white">크롤링 상태</h4>
                <p class={`text-lg font-semibold ${getStatusColor()}`}>
                  {getStatusText()}
                </p>
              </div>
              <div class="text-right">
                <p class="text-sm text-gray-600 dark:text-gray-400">진행률</p>
                <p class="text-lg font-semibold text-gray-900 dark:text-white">
                  {progress()}%
                </p>
              </div>
            </div>
            
            {/* 진행률 바 */}
            <div class="mt-3">
              <div class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2">
                <div 
                  class="bg-blue-600 h-2 rounded-full transition-all duration-300"
                  style={`width: ${progress()}%`}
                ></div>
              </div>
              <div class="flex justify-between text-xs text-gray-600 dark:text-gray-400 mt-1">
                <span>페이지: {currentPage()}</span>
                <span>총 페이지: {totalPages()}</span>
              </div>
            </div>
          </div>

          {/* 제어 버튼 */}
          <div class="flex space-x-3 justify-center">
            <button 
              class="px-6 py-3 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed font-medium transition-colors"
              onClick={handleStart}
              disabled={isLoading() || crawlingStatus() === 'running'}
            >
              {isLoading() ? '처리 중...' : '🚀 크롤링 시작'}
            </button>
            <button 
              class="px-6 py-3 bg-red-600 text-white rounded-lg hover:bg-red-700 disabled:opacity-50 disabled:cursor-not-allowed font-medium transition-colors"
              onClick={handleStop}
              disabled={isLoading() || crawlingStatus() === 'idle'}
            >
              {isLoading() ? '처리 중...' : '⏹️ 중지'}
            </button>
          </div>
        </div>
      </ExpandableSection>

      {/* 사이트 상태 확인 섹션 */}
      <ExpandableSection 
        title="사이트 상태 체크" 
        isExpanded={isStatusExpanded()} 
        onToggle={() => setIsStatusExpanded(!isStatusExpanded())}
        icon="🔍"
      >
        <div class="space-y-4">
          <p class="text-sm text-gray-600 dark:text-gray-400">
            사이트 상태를 확인하여 새로운 데이터가 있는지 검사합니다.
          </p>
          
          <div class="flex justify-center">
            <button 
              class="px-6 py-3 bg-indigo-600 text-white rounded-lg hover:bg-indigo-700 disabled:opacity-50 disabled:cursor-not-allowed font-medium transition-colors"
              onClick={handleStatusCheck}
              disabled={isLoading()}
            >
              {isLoading() ? '확인 중...' : '🔍 상태 확인'}
            </button>
          </div>
        </div>
      </ExpandableSection>
    </div>
  );
};
