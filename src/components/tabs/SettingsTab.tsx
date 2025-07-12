/**
 * SettingsTab - 실제 기능이 있는 설정 탭 컴포넌트
 */

import { Component, createSignal, onMount, createEffect, For } from 'solid-js';
import { ExpandableSection } from '../common/ExpandableSection';
import { tauriApi } from '../../services/tauri-api';
import { loggingService } from '../../services/loggingService';
import { CrawlingStatusCheck } from '../../types/crawling';

interface LoggingSettings {
  level: string;
  separate_frontend_backend: boolean;
  max_file_size_mb: number;
  max_files: number;
  auto_cleanup_logs: boolean;
  keep_only_latest: boolean;
  module_filters: Record<string, string>;
}

interface BatchSettings {
  batch_size: number;
  batch_delay_ms: number;
  enable_batch_processing: boolean;
  batch_retry_limit: number;
}

interface CrawlingSettings {
  page_range_limit: number;
  product_list_retry_count: number;
  product_detail_retry_count: number;
  auto_add_to_local_db: boolean;
}

interface PerformanceSettings {
  max_concurrent_requests: number;
  request_delay_ms: number;
  max_pages: number;
}

interface SaveStatus {
  type: 'success' | 'error' | 'info' | null;
  message: string;
}

export const SettingsTab: Component = () => {
  const [isAdvancedExpanded, setIsAdvancedExpanded] = createSignal(false);
  const [isBatchExpanded, setIsBatchExpanded] = createSignal(true);
  const [isLoggingExpanded, setIsLoggingExpanded] = createSignal(true);
  const [isPresetExpanded, setIsPresetExpanded] = createSignal(true);
  
  // 현재 저장된 설정 (서버에서 로드된 원본)
  const [savedLoggingSettings, setSavedLoggingSettings] = createSignal<LoggingSettings>({
    level: 'info',
    separate_frontend_backend: false,
    max_file_size_mb: 10,
    max_files: 5,
    auto_cleanup_logs: true,
    keep_only_latest: false,
    module_filters: {
      'sqlx': 'warn',
      'reqwest': 'info',
      'hyper': 'warn',
      'tokio': 'info',
      'tauri': 'info',
      'wry': 'warn',
      'matter_certis_v2': 'info'
    }
  });
  
  // 현재 UI에서 편집 중인 설정
  const [loggingSettings, setLoggingSettings] = createSignal<LoggingSettings>({
    level: 'info',
    separate_frontend_backend: false,
    max_file_size_mb: 10,
    max_files: 5,
    auto_cleanup_logs: true,
    keep_only_latest: false,
    module_filters: {
      'sqlx': 'warn',
      'reqwest': 'info',
      'hyper': 'warn',
      'tokio': 'info',
      'tauri': 'info',
      'wry': 'warn',
      'matter_certis_v2': 'info'
    }
  });
  
  // 현재 저장된 배치 설정
  const [savedBatchSettings, setSavedBatchSettings] = createSignal<BatchSettings>({
    batch_size: 5,
    batch_delay_ms: 2000,
    enable_batch_processing: true,
    batch_retry_limit: 3
  });
  
  // 현재 UI에서 편집 중인 배치 설정
  const [batchSettings, setBatchSettings] = createSignal<BatchSettings>({
    batch_size: 5,
    batch_delay_ms: 2000,
    enable_batch_processing: true,
    batch_retry_limit: 3
  });
  
  // 현재 저장된 크롤링 설정
  const [savedCrawlingSettings, setSavedCrawlingSettings] = createSignal<CrawlingSettings>({
    page_range_limit: 6,
    product_list_retry_count: 3,
    product_detail_retry_count: 3,
    auto_add_to_local_db: true
  });
  
  // 현재 UI에서 편집 중인 크롤링 설정
  const [crawlingSettings, setCrawlingSettings] = createSignal<CrawlingSettings>({
    page_range_limit: 6,
    product_list_retry_count: 3,
    product_detail_retry_count: 3,
    auto_add_to_local_db: true
  });
  
  // 현재 저장된 성능 설정
  const [savedPerformanceSettings, setSavedPerformanceSettings] = createSignal<PerformanceSettings>({
    max_concurrent_requests: 12,
    request_delay_ms: 800,
    max_pages: 10
  });
  
  // 현재 UI에서 편집 중인 성능 설정
  const [performanceSettings, setPerformanceSettings] = createSignal<PerformanceSettings>({
    max_concurrent_requests: 12,
    request_delay_ms: 800,
    max_pages: 10
  });
  
  // 상태 체크 관련 signals
  const [statusCheck, setStatusCheck] = createSignal<CrawlingStatusCheck | null>(null);
  const [isCheckingStatus, setIsCheckingStatus] = createSignal(false);
  const [statusCheckError, setStatusCheckError] = createSignal<string>('');
  
  const [logCleanupResult, setLogCleanupResult] = createSignal<string>('');
  const [isCleaningLogs, setIsCleaningLogs] = createSignal(false);
  const [saveStatus, setSaveStatus] = createSignal<SaveStatus>({ type: null, message: '' });
  const [isSaving, setIsSaving] = createSignal(false);
  const [hasUnsavedChanges, setHasUnsavedChanges] = createSignal(false);

  // 로그 프리셋 정의
  const loggingPresets = [
    {
      name: '기본 로그',
      description: '일반적인 개발용 로그 설정',
      config: {
        level: 'info' as const,
        separate_frontend_backend: false,
        max_file_size_mb: 10,
        max_files: 5,
        auto_cleanup_logs: true,
        keep_only_latest: false,
        module_filters: {
          'sqlx': 'warn',
          'reqwest': 'info',
          'hyper': 'warn',
          'tokio': 'info',
          'tauri': 'info',
          'wry': 'warn',
          'matter_certis_v2': 'info'
        }
      }
    },
    {
      name: 'HTTP 로그',
      description: 'HTTP 요청/응답 상세 로그 (sqlx 로그는 최소화)',
      config: {
        level: 'debug' as const,
        separate_frontend_backend: true,
        max_file_size_mb: 20,
        max_files: 10,
        auto_cleanup_logs: true,
        keep_only_latest: false,
        module_filters: {
          'sqlx': 'error',      // HTTP 디버깅 시 sqlx 로그 억제
          'reqwest': 'debug',   // HTTP 클라이언트 상세 로그
          'hyper': 'info',      // HTTP 서버 로그
          'tokio': 'warn',      // 비동기 런타임 로그 최소화
          'tauri': 'info',
          'wry': 'warn',
          'matter_certis_v2': 'debug'
        }
      }
    },
    {
      name: '데이터베이스 로그',
      description: 'SQL 쿼리 및 데이터베이스 상세 로그',
      config: {
        level: 'trace' as const,
        separate_frontend_backend: true,
        max_file_size_mb: 50,
        max_files: 15,
        auto_cleanup_logs: false,
        keep_only_latest: false,
        module_filters: {
          'sqlx': 'debug',      // SQL 쿼리 상세 로그
          'reqwest': 'info',
          'hyper': 'warn',
          'tokio': 'info',
          'tauri': 'info',
          'wry': 'warn',
          'matter_certis_v2': 'trace'
        }
      }
    },
    {
      name: '프로덕션 로그',
      description: '최소한의 로그만 기록 (성능 최적화)',
      config: {
        level: 'warn' as const,
        separate_frontend_backend: false,
        max_file_size_mb: 5,
        max_files: 3,
        auto_cleanup_logs: true,
        keep_only_latest: true,
        module_filters: {
          'sqlx': 'error',      // 프로덕션에서는 에러만
          'reqwest': 'warn',
          'hyper': 'error',
          'tokio': 'error',
          'tauri': 'warn',
          'wry': 'error',
          'matter_certis_v2': 'warn'
        }
      }
    },
    {
      name: '풀 디버그',
      description: '모든 컴포넌트의 상세 로그 (문제 해결용)',
      config: {
        level: 'trace' as const,
        separate_frontend_backend: true,
        max_file_size_mb: 100,
        max_files: 20,
        auto_cleanup_logs: false,
        keep_only_latest: false,
        module_filters: {
          'sqlx': 'trace',      // 모든 모듈 최대 상세도
          'reqwest': 'trace',
          'hyper': 'debug',
          'tokio': 'debug',
          'tauri': 'debug',
          'wry': 'debug',
          'matter_certis_v2': 'trace'
        }
      }
    }
  ];

  // 변경사항 감지
  createEffect(() => {
    const currentLogging = loggingSettings();
    const savedLogging = savedLoggingSettings();
    const currentBatch = batchSettings();
    const savedBatch = savedBatchSettings();
    const currentCrawling = crawlingSettings();
    const savedCrawling = savedCrawlingSettings();
    
    const loggingChanged = JSON.stringify(currentLogging) !== JSON.stringify(savedLogging);
    const batchChanged = JSON.stringify(currentBatch) !== JSON.stringify(savedBatch);
    const crawlingChanged = JSON.stringify(currentCrawling) !== JSON.stringify(savedCrawling);
    
    setHasUnsavedChanges(loggingChanged || batchChanged || crawlingChanged);
  });

  // 설정 로드 함수
  const loadSettings = async () => {
    try {
      console.log('🔄 SettingsTab: 설정 파일에서 현재 값들을 로드 중...');
      const frontendConfig = await tauriApi.getFrontendConfig();
      console.log('✅ SettingsTab: 설정 로드 완료:', frontendConfig);
      
      if (frontendConfig?.user?.logging) {
        const settings = frontendConfig.user.logging;
        console.log('📋 로깅 설정 적용:', settings);
        setSavedLoggingSettings(settings);
        setLoggingSettings(settings);
      }
      
      if (frontendConfig?.user?.batch) {
        const batchConfig = frontendConfig.user.batch;
        console.log('📋 배치 설정 적용:', batchConfig);
        setSavedBatchSettings(batchConfig);
        setBatchSettings(batchConfig);
      }
      
      if (frontendConfig?.user?.crawling) {
        const crawlingConfig = frontendConfig.user.crawling;
        console.log('📋 크롤링 설정 적용:', crawlingConfig);
        setSavedCrawlingSettings(crawlingConfig);
        setCrawlingSettings(crawlingConfig);
      }

      // 전체 설정 상태 로깅
      console.log('💾 현재 적용된 설정 상태:');
      console.log('- 로깅:', loggingSettings());
      console.log('- 배치:', batchSettings());
      console.log('- 크롤링:', crawlingSettings());
      
      await loggingService.info('설정을 성공적으로 로드했습니다', 'SettingsTab');
    } catch (error) {
      console.error('❌ SettingsTab: 설정 로드 실패:', error);
      setSaveStatus({ 
        type: 'error', 
        message: '설정 로드에 실패했습니다: ' + (error instanceof Error ? error.message : '알 수 없는 오류')
      });
      await loggingService.error(`설정 로드 실패: ${error}`, 'SettingsTab');
    }
  };

  // 설정 로드
  onMount(async () => {
    console.log('SettingsTab: onMount 시작');
    try {
      await loadSettings();
      console.log('SettingsTab: 설정 로드 완료');
    } catch (error) {
      console.error('SettingsTab: onMount 에러:', error);
    }
  });

  const handleSaveSettings = async () => {
    setIsSaving(true);
    setSaveStatus({ type: null, message: '' });
    
    try {
      // 로깅 설정 저장
      await tauriApi.updateLoggingSettings(loggingSettings());
      
      // 배치 설정 저장
      await tauriApi.updateBatchSettings(batchSettings());
      
      // 크롤링 설정 저장
      await tauriApi.updateCrawlingSettings(crawlingSettings());
      
      // 저장된 설정으로 업데이트
      setSavedLoggingSettings(loggingSettings());
      setSavedBatchSettings(batchSettings());
      setSavedCrawlingSettings(crawlingSettings());
      
      setSaveStatus({ 
        type: 'success', 
        message: '설정이 성공적으로 저장되었습니다!' 
      });
      
      await loggingService.info('설정 저장 완료', 'SettingsTab');
      
      // 3초 후 메시지 자동 제거
      setTimeout(() => {
        setSaveStatus({ type: null, message: '' });
      }, 3000);
      
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : '알 수 없는 오류';
      setSaveStatus({ 
        type: 'error', 
        message: `설정 저장에 실패했습니다: ${errorMessage}` 
      });
      await loggingService.error(`설정 저장 실패: ${errorMessage}`, 'SettingsTab');
    } finally {
      setIsSaving(false);
    }
  };

  const handleLogCleanup = async () => {
    setIsCleaningLogs(true);
    setLogCleanupResult('');
    try {
      const result = await loggingService.cleanupLogs();
      setLogCleanupResult(result);
      await loggingService.info('로그 파일 정리 완료', 'SettingsTab');
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : '알 수 없는 오류';
      setLogCleanupResult(`정리 실패: ${errorMessage}`);
      await loggingService.error(`로그 파일 정리 실패: ${errorMessage}`, 'SettingsTab');
    } finally {
      setIsCleaningLogs(false);
    }
  };

  const resetToSaved = () => {
    setLoggingSettings(savedLoggingSettings());
    setSaveStatus({ type: 'info', message: '변경사항이 취소되었습니다.' });
    setTimeout(() => {
      setSaveStatus({ type: null, message: '' });
    }, 2000);
  };

  // 상태 체크 함수
  const handleStatusCheck = async () => {
    setIsCheckingStatus(true);
    setStatusCheckError('');
    setStatusCheck(null);
    
    try {
      await loggingService.info('크롤링 상태 체크 시작', 'SettingsTab');
      const result = await tauriApi.getCrawlingStatusCheck();
      setStatusCheck(result);
      await loggingService.info(`상태 체크 완료: ${result.recommendation.reason}`, 'SettingsTab');
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : '알 수 없는 오류';
      setStatusCheckError(`상태 체크 실패: ${errorMessage}`);
      await loggingService.error(`상태 체크 실패: ${errorMessage}`, 'SettingsTab');
    } finally {
      setIsCheckingStatus(false);
    }
  };

  // 추천 설정 적용 함수
  const applyRecommendedSettings = () => {
    const check = statusCheck();
    if (check && check.recommendation.suggested_range) {
      const [startPage, endPage] = check.recommendation.suggested_range;
      setCrawlingSettings(prev => ({
        ...prev,
        page_range_limit: endPage - startPage + 1
      }));
      
      setSaveStatus({ 
        type: 'info', 
        message: `추천 설정이 적용되었습니다 (페이지 ${startPage}-${endPage})` 
      });
      
      setTimeout(() => {
        setSaveStatus({ type: null, message: '' });
      }, 3000);
    }
  };

  // 프리셋 적용 함수
  const applyLoggingPreset = (preset: typeof loggingPresets[0]) => {
    setLoggingSettings(preset.config);
    setSaveStatus({ 
      type: 'info', 
      message: `${preset.name} 프리셋이 적용되었습니다. 설정을 저장해주세요.` 
    });
    
    // 3초 후 메시지 자동 제거
    setTimeout(() => {
      setSaveStatus({ type: null, message: '' });
    }, 3000);
  };

  // 현재 설정이 특정 프리셋과 일치하는지 확인
  const isPresetActive = (preset: typeof loggingPresets[0]) => {
    const current = loggingSettings();
    return JSON.stringify(current) === JSON.stringify(preset.config);
  };

  return (
    <div class="space-y-6">
      {/* 상태 메시지 */}
      {saveStatus().type && (
        <div class={`px-4 py-3 rounded-md ${
          saveStatus().type === 'success' ? 'bg-green-100 border border-green-400 text-green-700' :
          saveStatus().type === 'error' ? 'bg-red-100 border border-red-400 text-red-700' :
          'bg-blue-100 border border-blue-400 text-blue-700'
        }`}>
          <div class="flex">
            <div class="flex-shrink-0">
              {saveStatus().type === 'success' && (
                <svg class="h-5 w-5 text-green-400" viewBox="0 0 20 20" fill="currentColor">
                  <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd" />
                </svg>
              )}
              {saveStatus().type === 'error' && (
                <svg class="h-5 w-5 text-red-400" viewBox="0 0 20 20" fill="currentColor">
                  <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clip-rule="evenodd" />
                </svg>
              )}
              {saveStatus().type === 'info' && (
                <svg class="h-5 w-5 text-blue-400" viewBox="0 0 20 20" fill="currentColor">
                  <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clip-rule="evenodd" />
                </svg>
              )}
            </div>
            <div class="ml-3">
              <p class="text-sm font-medium">
                {saveStatus().message}
              </p>
            </div>
          </div>
        </div>
      )}

      {/* 변경사항 알림 */}
      {hasUnsavedChanges() && (
        <div class="px-4 py-3 bg-yellow-50 border border-yellow-200 rounded-md">
          <div class="flex">
            <div class="flex-shrink-0">
              <svg class="h-5 w-5 text-yellow-400" viewBox="0 0 20 20" fill="currentColor">
                <path fill-rule="evenodd" d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z" clip-rule="evenodd" />
              </svg>
            </div>
            <div class="ml-3">
              <p class="text-sm text-yellow-700">
                <strong>저장되지 않은 변경사항이 있습니다.</strong> 변경사항을 저장하거나 취소하세요.
              </p>
            </div>
            <div class="ml-auto pl-3">
              <div class="flex space-x-2">
                <button
                  onClick={resetToSaved}
                  class="text-yellow-700 hover:text-yellow-900 text-sm underline"
                >
                  취소
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* 설정 관리 버튼들 */}
      <div class="flex justify-between items-center">
        <h2 class="text-xl font-semibold text-gray-900 dark:text-white">⚙️ 설정 관리</h2>
        <div class="flex space-x-3">
          <button
            onClick={loadSettings}
            class="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 transition-colors"
          >
            🔄 설정 다시 로드
          </button>
          <button
            onClick={handleSaveSettings}
            disabled={isSaving() || !hasUnsavedChanges()}
            class={`px-4 py-2 rounded-md focus:outline-none focus:ring-2 transition-colors ${
              isSaving() || !hasUnsavedChanges()
                ? 'bg-gray-400 text-gray-200 cursor-not-allowed'
                : 'bg-emerald-600 text-white hover:bg-emerald-700 focus:ring-emerald-500'
            }`}
          >
            {isSaving() ? '💾 저장 중...' : '💾 설정 저장'}
          </button>
        </div>
      </div>

      {/* 성능 설정 - 새로 추가 */}
      <ExpandableSection
        title="성능 설정"
        isExpanded={true}
        onToggle={() => {}}
        icon="⚡"
      >
        <div class="bg-yellow-50 border border-yellow-200 rounded-md p-4 mb-4">
          <div class="flex items-center">
            <div class="flex-shrink-0">
              <svg class="h-5 w-5 text-yellow-400" viewBox="0 0 20 20" fill="currentColor">
                <path fill-rule="evenodd" d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z" clip-rule="evenodd" />
              </svg>
            </div>
            <div class="ml-3">
              <h3 class="text-sm font-medium text-yellow-800">
                중요: 성능 설정은 크롤링 속도와 서버 부하에 직접 영향을 미칩니다
              </h3>
              <div class="mt-2 text-sm text-yellow-700">
                <p>• 병렬 처리 수가 높을수록 빠르지만 서버 부하가 증가합니다</p>
                <p>• 요청 간격이 짧을수록 빠르지만 차단될 위험이 높아집니다</p>
                <p>• 현재 설정: 12개 동시 처리, 800ms 간격 (권장)</p>
              </div>
            </div>
          </div>
        </div>
        
        <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              🔥 동시 병렬 처리 수
            </label>
            <input 
              type="number" 
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md focus:outline-none focus:ring-2 focus:ring-emerald-500 dark:bg-gray-700 dark:text-white"
              placeholder="12"
              min="1"
              max="24"
              value={performanceSettings().max_concurrent_requests}
              onInput={(e) => setPerformanceSettings(prev => ({
                ...prev,
                max_concurrent_requests: parseInt(e.currentTarget.value) || 12
              }))}
            />
            <p class="text-xs text-gray-500 mt-1">1-24 권장: 12</p>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              ⏱️ 요청 간격 (ms)
            </label>
            <input 
              type="number" 
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md focus:outline-none focus:ring-2 focus:ring-emerald-500 dark:bg-gray-700 dark:text-white"
              placeholder="800"
              min="200"
              max="3000"
              value={performanceSettings().request_delay_ms}
              onInput={(e) => setPerformanceSettings(prev => ({
                ...prev,
                request_delay_ms: parseInt(e.currentTarget.value) || 800
              }))}
            />
            <p class="text-xs text-gray-500 mt-1">200-3000ms 권장: 800ms</p>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              📄 전체 최대 페이지 수
            </label>
            <input 
              type="number" 
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md focus:outline-none focus:ring-2 focus:ring-emerald-500 dark:bg-gray-700 dark:text-white"
              placeholder="10"
              min="1"
              max="100"
              value={performanceSettings().max_pages}
              onInput={(e) => setPerformanceSettings(prev => ({
                ...prev,
                max_pages: parseInt(e.currentTarget.value) || 10
              }))}
            />
            <p class="text-xs text-gray-500 mt-1">1-100 권장: 10</p>
          </div>
        </div>
        
        <div class="mt-4 p-3 bg-blue-50 border border-blue-200 rounded-md">
          <h4 class="text-sm font-medium text-blue-800 mb-2">💡 예상 성능</h4>
          <div class="text-sm text-blue-700 grid grid-cols-1 md:grid-cols-2 gap-2">
            <p>• 페이지 수집 속도: ~{Math.round(performanceSettings().max_concurrent_requests / (performanceSettings().request_delay_ms / 1000))}페이지/초</p>
            <p>• 예상 완료 시간: ~{Math.ceil(performanceSettings().max_pages / performanceSettings().max_concurrent_requests)}분</p>
          </div>
        </div>
      </ExpandableSection>

      {/* 기본 크롤링 설정 */}
      <ExpandableSection
        title="크롤링 설정"
        isExpanded={true}
        onToggle={() => {}}
        icon="⚙️"
      >
        <div class="bg-emerald-50 border border-emerald-200 rounded-md p-4 mb-4">
          <div class="flex items-center">
            <div class="flex-shrink-0">
              <svg class="h-5 w-5 text-emerald-400" viewBox="0 0 20 20" fill="currentColor">
                <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clip-rule="evenodd" />
              </svg>
            </div>
            <div class="ml-3">
              <h3 class="text-sm font-medium text-emerald-800">
                🎯 페이지 범위 제한: 실제 크롤링에서 처리할 최대 페이지 수
              </h3>
              <div class="mt-2 text-sm text-emerald-700">
                <p>• <strong>시스템이 임의로 변경하지 않습니다</strong> - 사용자 설정을 엄격히 준수</p>
                <p>• 빈 데이터베이스라도 이 값을 초과하지 않습니다</p>
                <p>• 현재 설정: 최대 {crawlingSettings().page_range_limit}페이지만 크롤링</p>
              </div>
            </div>
          </div>
        </div>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              🎯 페이지 범위 제한 (핵심 설정)
            </label>
            <input 
              type="number" 
              class="w-full px-3 py-2 border-2 border-emerald-300 dark:border-emerald-600 rounded-md focus:outline-none focus:ring-2 focus:ring-emerald-500 dark:bg-gray-700 dark:text-white font-bold"
              placeholder="6"
              min="1"
              max="50"
              value={crawlingSettings().page_range_limit}
              onInput={(e) => setCrawlingSettings(prev => ({
                ...prev,
                page_range_limit: parseInt(e.currentTarget.value) || 6
              }))}
            />
            <p class="text-xs text-emerald-600 mt-1 font-medium">⚠️ 이 값이 실제 크롤링 페이지 수를 결정합니다</p>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              제품 목록 재시도 횟수
            </label>
            <input 
              type="number" 
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md focus:outline-none focus:ring-2 focus:ring-emerald-500 dark:bg-gray-700 dark:text-white"
              placeholder="3"
              value={crawlingSettings().product_list_retry_count}
              onInput={(e) => setCrawlingSettings(prev => ({
                ...prev,
                product_list_retry_count: parseInt(e.currentTarget.value) || 3
              }))}
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              제품 상세 재시도 횟수
            </label>
            <input 
              type="number" 
              class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md focus:outline-none focus:ring-2 focus:ring-emerald-500 dark:bg-gray-700 dark:text-white"
              placeholder="3"
              value={crawlingSettings().product_detail_retry_count}
              onInput={(e) => setCrawlingSettings(prev => ({
                ...prev,
                product_detail_retry_count: parseInt(e.currentTarget.value) || 3
              }))}
            />
          </div>
          <div class="flex items-center space-x-3">
            <input
              type="checkbox"
              id="auto-add-local-db"
              class="h-4 w-4 text-emerald-600 focus:ring-emerald-500 border-gray-300 rounded"
              checked={crawlingSettings().auto_add_to_local_db}
              onChange={(e) => setCrawlingSettings(prev => ({
                ...prev,
                auto_add_to_local_db: e.currentTarget.checked
              }))}
            />
            <label for="auto-add-local-db" class="text-sm font-medium text-gray-700 dark:text-gray-300">
              자동으로 로컬 DB에 추가
            </label>
          </div>
        </div>
        
        {/* 상태 체크 섹션 */}
        <div class="mt-6 p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg border border-blue-200 dark:border-blue-800">
          <div class="flex items-center justify-between mb-4">
            <h4 class="text-lg font-medium text-blue-900 dark:text-blue-100">
              🔍 크롤링 상태 체크
            </h4>
            <button
              onClick={handleStatusCheck}
              disabled={isCheckingStatus()}
              class="px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 text-white rounded-md transition-colors duration-200 flex items-center space-x-2"
            >
              {isCheckingStatus() ? (
                <>
                  <div class="animate-spin w-4 h-4 border-2 border-white border-t-transparent rounded-full"></div>
                  <span>분석 중...</span>
                </>
              ) : (
                <>
                  <span>🔍</span>
                  <span>상태 분석</span>
                </>
              )}
            </button>
          </div>
          
          <p class="text-sm text-blue-700 dark:text-blue-300 mb-4">
            로컬 데이터베이스와 사이트 상태를 분석하여 최적의 크롤링 범위를 추천합니다.
          </p>
          
          {statusCheckError() && (
            <div class="mb-4 p-3 bg-red-100 dark:bg-red-900/30 border border-red-300 dark:border-red-700 rounded-md">
              <p class="text-sm text-red-700 dark:text-red-300">{statusCheckError()}</p>
            </div>
          )}
          
          {statusCheck() && (
            <div class="space-y-4">
              <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div class="bg-white dark:bg-gray-800 p-3 rounded-md border">
                  <h5 class="font-medium text-gray-900 dark:text-gray-100 mb-2">📊 로컬 DB 상태</h5>
                  <p class="text-sm text-gray-600 dark:text-gray-400">
                    제품 수: <span class="font-mono">{statusCheck()!.database_status.total_products.toLocaleString()}</span>개
                  </p>
                  <p class="text-sm text-gray-600 dark:text-gray-400">
                    페이지 범위: <span class="font-mono">{statusCheck()!.database_status.page_range[0]}-{statusCheck()!.database_status.page_range[1]}</span>
                  </p>
                  <p class="text-sm text-gray-600 dark:text-gray-400">
                    상태: <span class="font-mono">{statusCheck()!.database_status.health}</span>
                  </p>
                  {statusCheck()!.database_status.last_crawl_time && (
                    <p class="text-sm text-gray-600 dark:text-gray-400">
                      마지막 크롤링: <span class="font-mono">{new Date(statusCheck()!.database_status.last_crawl_time!).toLocaleDateString()}</span>
                    </p>
                  )}
                </div>
                
                <div class="bg-white dark:bg-gray-800 p-3 rounded-md border">
                  <h5 class="font-medium text-gray-900 dark:text-gray-100 mb-2">🌐 사이트 상태</h5>
                  <p class="text-sm text-gray-600 dark:text-gray-400">
                    접근 가능: <span class={`font-mono ${statusCheck()!.site_status.is_accessible ? 'text-green-600' : 'text-red-600'}`}>
                      {statusCheck()!.site_status.is_accessible ? '✅ 정상' : '❌ 불가'}
                    </span>
                  </p>
                  <p class="text-sm text-gray-600 dark:text-gray-400">
                    최대 페이지: <span class="font-mono">{statusCheck()!.site_status.total_pages}</span>
                  </p>
                  <p class="text-sm text-gray-600 dark:text-gray-400">
                    예상 제품 수: <span class="font-mono">{statusCheck()!.site_status.estimated_products.toLocaleString()}</span>개
                  </p>
                  <p class="text-sm text-gray-600 dark:text-gray-400">
                    건강도: <span class="font-mono">{(statusCheck()!.site_status.health_score * 100).toFixed(1)}%</span>
                  </p>
                </div>
              </div>
              
              <div class="bg-green-50 dark:bg-green-900/20 p-4 rounded-md border border-green-200 dark:border-green-800">
                <div class="flex items-start justify-between">
                  <div class="flex-1">
                    <h5 class="font-medium text-green-900 dark:text-green-100 mb-2">💡 추천 설정</h5>
                    <p class="text-sm text-green-700 dark:text-green-300 mb-2">
                      추천 페이지 범위: <span class="font-mono font-bold">
                        {statusCheck()!.recommendation.suggested_range?.[0] || 1}-{statusCheck()!.recommendation.suggested_range?.[1] || 50}
                      </span>
                    </p>
                    <p class="text-sm text-green-700 dark:text-green-300 mb-2">
                      예상 신규 제품: <span class="font-mono font-bold">{statusCheck()!.recommendation.estimated_new_items.toLocaleString()}</span>개
                    </p>
                    <p class="text-sm text-green-700 dark:text-green-300 mb-2">
                      효율성 점수: <span class="font-mono font-bold">{(statusCheck()!.recommendation.efficiency_score * 100).toFixed(1)}%</span>
                    </p>
                    <p class="text-sm text-green-700 dark:text-green-300 mb-2">
                      동기화율: <span class="font-mono font-bold">{statusCheck()!.sync_comparison.sync_percentage.toFixed(1)}%</span>
                    </p>
                    <p class="text-sm text-green-600 dark:text-green-400 italic">
                      {statusCheck()!.recommendation.reason}
                    </p>
                  </div>
                  <button
                    onClick={applyRecommendedSettings}
                    class="ml-4 px-3 py-2 bg-green-600 hover:bg-green-700 text-white text-sm rounded-md transition-colors duration-200"
                  >
                    적용
                  </button>
                </div>
              </div>
            </div>
          )}
        </div>
      </ExpandableSection>

      {/* 배치 처리 설정 */}
      <ExpandableSection
        title="배치 처리 설정"
        isExpanded={isBatchExpanded()}
        onToggle={setIsBatchExpanded}
        icon="📦"
      >
        <div class="space-y-6">
          {/* 배치 처리 사용 체크박스 */}
          <div class="flex items-center space-x-3">
            <input
              type="checkbox"
              id="enable-batch-processing"
              class="h-4 w-4 text-emerald-600 focus:ring-emerald-500 border-gray-300 rounded"
              checked={batchSettings().enable_batch_processing}
              onChange={(e) => setBatchSettings(prev => ({
                ...prev,
                enable_batch_processing: e.currentTarget.checked
              }))}
            />
            <label for="enable-batch-processing" class="text-sm font-medium text-gray-700 dark:text-gray-300">
              배치 처리 사용
            </label>
          </div>

          {/* 배치 크기 슬라이더 (5-100) */}
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              배치 크기 ({batchSettings().batch_size})
            </label>
            <div class="flex items-center space-x-4">
              <span class="text-sm text-gray-500">5</span>
              <input
                type="range"
                min="5"
                max="100"
                step="5"
                class="flex-1 h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer"
                value={batchSettings().batch_size}
                onInput={(e) => setBatchSettings(prev => ({
                  ...prev,
                  batch_size: parseInt(e.currentTarget.value)
                }))}
              />
              <span class="text-sm text-gray-500">100</span>
            </div>
            <p class="text-xs text-gray-500 mt-1">
              한 번에 처리할 배치의 수량을 결정합니다.
            </p>
          </div>

          {/* 배치 간 지연시간 슬라이더 (1000-10000ms) */}
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              배치 간 지연시간 ({batchSettings().batch_delay_ms}ms)
            </label>
            <div class="flex items-center space-x-4">
              <span class="text-sm text-gray-500">1000</span>
              <input
                type="range"
                min="1000"
                max="10000"
                step="100"
                class="flex-1 h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer"
                value={batchSettings().batch_delay_ms}
                onInput={(e) => setBatchSettings(prev => ({
                  ...prev,
                  batch_delay_ms: parseInt(e.currentTarget.value)
                }))}
              />
              <span class="text-sm text-gray-500">10000</span>
            </div>
            <p class="text-xs text-gray-500 mt-1">
              다음 배치를 처리하기 시작하는 시간입니다. 장기 업적수 리스크를 사용화시간으로 중재합니다.
            </p>
          </div>

          {/* 배치 재시도 횟수 슬라이더 (1-10) */}
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
              배치 재시도 횟수 ({batchSettings().batch_retry_limit})
            </label>
            <div class="flex items-center space-x-4">
              <span class="text-sm text-gray-500">1</span>
              <input
                type="range"
                min="1"
                max="10"
                step="1"
                class="flex-1 h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer"
                value={batchSettings().batch_retry_limit}
                onInput={(e) => setBatchSettings(prev => ({
                  ...prev,
                  batch_retry_limit: parseInt(e.currentTarget.value)
                }))}
              />
              <span class="text-sm text-gray-500">10</span>
            </div>
            <p class="text-xs text-gray-500 mt-1">
              배치 처리 실패 시 최대 재시도 횟수입니다. 네트워크 불안정 등의 상황을 대비합니다.
            </p>
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

      {/* 로깅 설정 */}
      <ExpandableSection
        title="로깅 설정"
        isExpanded={isLoggingExpanded()}
        onToggle={() => setIsLoggingExpanded(!isLoggingExpanded())}
        icon="📋"
      >
        <div class="space-y-6">
          {/* 기본 로깅 설정 */}
          <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
            <div>
              <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                로그 레벨
              </label>
              <select 
                class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md focus:outline-none focus:ring-2 focus:ring-emerald-500 dark:bg-gray-700 dark:text-white"
                value={loggingSettings().level}
                onChange={(e) => {
                  setLoggingSettings(prev => ({ ...prev, level: e.currentTarget.value }));
                }}
              >
                <option value="error">Error</option>
                <option value="warn">Warning</option>
                <option value="info">Info</option>
                <option value="debug">Debug</option>
                <option value="trace">Trace</option>
              </select>
            </div>
            
            <div>
              <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                최대 파일 크기 (MB)
              </label>
              <input 
                type="number" 
                min="1"
                max="100"
                class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md focus:outline-none focus:ring-2 focus:ring-emerald-500 dark:bg-gray-700 dark:text-white"
                placeholder="10"
                value={loggingSettings().max_file_size_mb}
                onInput={(e) => {
                  const value = parseInt(e.currentTarget.value) || 10;
                  setLoggingSettings(prev => ({ ...prev, max_file_size_mb: value }));
                }}
              />
            </div>
            
            <div>
              <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                최대 파일 수 (1-10)
              </label>
              <input 
                type="number" 
                min="1"
                max="10"
                class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md focus:outline-none focus:ring-2 focus:ring-emerald-500 dark:bg-gray-700 dark:text-white"
                placeholder="5"
                value={loggingSettings().max_files}
                onInput={(e) => {
                  const value = parseInt(e.currentTarget.value) || 5;
                  setLoggingSettings(prev => ({ ...prev, max_files: value }));
                }}
              />
            </div>
          </div>
          
          {/* 로깅 옵션 */}
          <div class="space-y-3">
            <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300">로깅 옵션</h4>
            
            <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
              <div class="flex items-center space-x-2">
                <input 
                  type="checkbox" 
                  id="separateLogs"
                  class="rounded border-gray-300 text-emerald-600 shadow-sm focus:border-emerald-300 focus:ring focus:ring-emerald-200 focus:ring-opacity-50"
                  checked={loggingSettings().separate_frontend_backend}
                  onChange={(e) => {
                    setLoggingSettings(prev => ({ ...prev, separate_frontend_backend: e.currentTarget.checked }));
                  }}
                />
                <label for="separateLogs" class="text-sm text-gray-700 dark:text-gray-300">
                  프론트엔드/백엔드 로그 분리
                </label>
              </div>
              
              <div class="flex items-center space-x-2">
                <input 
                  type="checkbox" 
                  id="keepOnlyLatest"
                  class="rounded border-gray-300 text-emerald-600 shadow-sm focus:border-emerald-300 focus:ring focus:ring-emerald-200 focus:ring-opacity-50"
                  checked={loggingSettings().keep_only_latest}
                  onChange={(e) => {
                    setLoggingSettings(prev => ({ ...prev, keep_only_latest: e.currentTarget.checked }));
                  }}
                />
                <label for="keepOnlyLatest" class="text-sm text-gray-700 dark:text-gray-300">
                  최신 로그 파일만 유지
                </label>
              </div>
              
              <div class="flex items-center space-x-2">
                <input 
                  type="checkbox" 
                  id="autoCleanup"
                  class="rounded border-gray-300 text-emerald-600 shadow-sm focus:border-emerald-300 focus:ring focus:ring-emerald-200 focus:ring-opacity-50"
                  checked={loggingSettings().auto_cleanup_logs}
                  onChange={(e) => {
                    setLoggingSettings(prev => ({ ...prev, auto_cleanup_logs: e.currentTarget.checked }));
                  }}
                />
                <label for="autoCleanup" class="text-sm text-gray-700 dark:text-gray-300">
                  시작 시 자동 로그 정리
                </label>
              </div>
              
              <div class="flex items-center space-x-2">
                <input 
                  type="checkbox" 
                  id="fileOutput"
                  class="rounded border-gray-300 text-emerald-600 shadow-sm focus:border-emerald-300 focus:ring focus:ring-emerald-200 focus:ring-opacity-50"
                  checked={true}
                  disabled
                />
                <label for="fileOutput" class="text-sm text-gray-500 dark:text-gray-400">
                  파일 출력 활성화 (항상 켜짐)
                </label>
              </div>
            </div>
          </div>
          
          {/* 로그 파일 관리 */}
          <div class="space-y-3">
            <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300">로그 파일 관리</h4>
            
            <div class="flex items-center space-x-3">
              <button 
                onClick={handleLogCleanup}
                disabled={isCleaningLogs()}
                class="px-4 py-2 bg-red-600 text-white rounded-md hover:bg-red-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors focus:outline-none focus:ring-2 focus:ring-red-500 focus:ring-offset-2"
              >
                {isCleaningLogs() ? '정리 중...' : '로그 파일 정리'}
              </button>
              
              {logCleanupResult() && (
                <div class={`px-3 py-1 rounded text-sm ${
                  logCleanupResult().includes('실패') || logCleanupResult().includes('오류')
                    ? 'bg-red-100 text-red-700 border border-red-200'
                    : 'bg-green-100 text-green-700 border border-green-200'
                }`}>
                  {logCleanupResult()}
                </div>
              )}
            </div>
          </div>
          
          {/* 안내 메시지 */}
          <div class="p-4 bg-blue-50 border border-blue-200 rounded-lg">
            <div class="flex">
              <div class="flex-shrink-0">
                <svg class="h-5 w-5 text-blue-400" viewBox="0 0 20 20" fill="currentColor">
                  <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clip-rule="evenodd" />
                </svg>
              </div>
              <div class="ml-3">
                <p class="text-sm text-blue-700">
                  <strong>로깅 설정 안내:</strong><br/>
                  • 로그 레벨이 높을수록 더 자세한 정보가 기록됩니다<br/>
                  • 통합 로그: 하나의 파일에 모든 로그 기록<br/>
                  • 분리 로그: 프론트엔드와 백엔드 로그를 별도 파일에 기록<br/>
                  • 설정 변경은 즉시 적용됩니다
                </p>
              </div>
            </div>
          </div>
        </div>
      </ExpandableSection>

      {/* 로그 프리셋 */}
      <ExpandableSection
        title="로그 프리셋"
        isExpanded={isPresetExpanded()}
        onToggle={() => setIsPresetExpanded(!isPresetExpanded())}
        icon="📂"
      >
        <div class="space-y-4">
          <div class="flex items-center justify-between">
            <p class="text-sm text-gray-500 dark:text-gray-400">
              용도에 맞는 로그 설정을 빠르게 적용할 수 있습니다.
            </p>
            {(() => {
              const activePreset = loggingPresets.find(preset => isPresetActive(preset));
              return activePreset ? (
                <div class="flex items-center space-x-2 px-3 py-1 bg-emerald-100 dark:bg-emerald-900/30 rounded-full">
                  <svg class="w-4 h-4 text-emerald-600 dark:text-emerald-400" fill="currentColor" viewBox="0 0 20 20">
                    <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd" />
                  </svg>
                  <span class="text-sm font-medium text-emerald-700 dark:text-emerald-300">
                    현재: {activePreset.name}
                  </span>
                </div>
              ) : (
                <div class="flex items-center space-x-2 px-3 py-1 bg-gray-100 dark:bg-gray-700 rounded-full">
                  <svg class="w-4 h-4 text-gray-500" fill="currentColor" viewBox="0 0 20 20">
                    <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clip-rule="evenodd" />
                  </svg>
                  <span class="text-sm font-medium text-gray-600 dark:text-gray-400">
                    사용자 정의 설정
                  </span>
                </div>
              );
            })()}
          </div>
          <div class="grid grid-cols-2 lg:grid-cols-4 gap-3">
            <For each={loggingPresets}>
              {(preset) => (
                <div 
                  onClick={() => !isPresetActive(preset) && applyLoggingPreset(preset)}
                  class={`relative p-3 rounded-lg border-2 transition-all cursor-pointer hover:shadow-md ${
                    isPresetActive(preset) 
                      ? 'bg-emerald-50 dark:bg-emerald-900/20 border-emerald-400 dark:border-emerald-500 shadow-sm' 
                      : 'bg-white dark:bg-gray-700 border-gray-200 dark:border-gray-600 hover:border-emerald-300 dark:hover:border-emerald-600'
                  } ${isPresetActive(preset) ? 'cursor-default' : 'hover:scale-105'}`}
                >
                  {/* 활성 상태 표시 */}
                  {isPresetActive(preset) && (
                    <div class="absolute -top-1 -right-1 w-6 h-6 bg-emerald-500 rounded-full flex items-center justify-center">
                      <svg class="w-3 h-3 text-white" fill="currentColor" viewBox="0 0 20 20">
                        <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
                      </svg>
                    </div>
                  )}
                  
                  <div class="flex flex-col space-y-2">
                    {/* 프리셋 이름 */}
                    <h5 class={`font-semibold text-sm leading-tight ${
                      isPresetActive(preset) 
                        ? 'text-emerald-900 dark:text-emerald-100' 
                        : 'text-gray-900 dark:text-white'
                    }`}>
                      {preset.name}
                    </h5>
                    
                    {/* 설명 */}
                    <p class={`text-xs leading-snug ${
                      isPresetActive(preset) 
                        ? 'text-emerald-700 dark:text-emerald-300' 
                        : 'text-gray-600 dark:text-gray-300'
                    }`}>
                      {preset.description}
                    </p>
                    
                    {/* 핵심 정보 */}
                    <div class="flex flex-wrap gap-1">
                      <span class={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${
                        isPresetActive(preset)
                          ? 'bg-emerald-100 dark:bg-emerald-800 text-emerald-800 dark:text-emerald-200'
                          : 'bg-gray-100 dark:bg-gray-600 text-gray-800 dark:text-gray-200'
                      }`}>
                        {preset.config.level.toUpperCase()}
                      </span>
                      <span class={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${
                        isPresetActive(preset)
                          ? 'bg-emerald-100 dark:bg-emerald-800 text-emerald-800 dark:text-emerald-200'
                          : 'bg-gray-100 dark:bg-gray-600 text-gray-800 dark:text-gray-200'
                      }`}>
                        {preset.config.max_file_size_mb}MB
                      </span>
                    </div>
                    
                    {/* 선택 상태 */}
                    <div class={`text-center text-xs font-medium mt-1 ${
                      isPresetActive(preset)
                        ? 'text-emerald-600 dark:text-emerald-400'
                        : 'text-gray-500 dark:text-gray-400'
                    }`}>
                      {isPresetActive(preset) ? '✓ 현재 활성' : '클릭하여 적용'}
                    </div>
                  </div>
                </div>
              )}
            </For>
          </div>
        </div>
      </ExpandableSection>

      {/* 저장 버튼 */}
      <div class="flex justify-between items-center p-4 bg-gray-50 dark:bg-gray-800 rounded-lg">
        <div class="text-sm text-gray-600 dark:text-gray-400">
          {hasUnsavedChanges() ? (
            <span class="text-orange-600 dark:text-orange-400 font-medium">
              • 저장되지 않은 변경사항이 있습니다
            </span>
          ) : (
            <span class="text-green-600 dark:text-green-400">
              • 모든 변경사항이 저장됨
            </span>
          )}
        </div>
        
        <div class="flex space-x-3">
          {hasUnsavedChanges() && (
            <button 
              onClick={resetToSaved}
              class="px-4 py-2 text-gray-600 dark:text-gray-400 border border-gray-300 dark:border-gray-600 rounded-md hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors focus:outline-none focus:ring-2 focus:ring-gray-500 focus:ring-offset-2"
            >
              변경사항 취소
            </button>
          )}
          
          <button 
            onClick={handleSaveSettings}
            disabled={isSaving() || !hasUnsavedChanges()}
            class={`px-6 py-2 rounded-md transition-colors focus:outline-none focus:ring-2 focus:ring-emerald-500 focus:ring-offset-2 ${
              isSaving() || !hasUnsavedChanges()
                ? 'bg-gray-400 text-gray-200 cursor-not-allowed'
                : 'bg-emerald-600 text-white hover:bg-emerald-700'
            }`}
          >
            {isSaving() ? '저장 중...' : '설정 저장'}
          </button>
        </div>
      </div>
    </div>
  );
};
