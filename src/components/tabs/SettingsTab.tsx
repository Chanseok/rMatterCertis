/**
 * SettingsTab - 설정 탭 컴포넌트
 * SolidJS-UI-Implementation-Guide.md를 기반으로 구현
 */

import { Component, createSignal, onMount, createEffect, For } from 'solid-js';
import { ExpandableSection } from '../common/ExpandableSection';
import { crawlerStore } from '../../stores/crawlerStore';
import { tauriApi } from '../../services/tauri-api';
import { loggingService } from '../../services/loggingService';

interface LoggingSettings {
  level: string;
  separate_frontend_backend: boolean;
  max_file_size_mb: number;
  max_files: number;
  auto_cleanup_logs: boolean;
  keep_only_latest: boolean;
  module_filters: Record<string, string>;
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
    const current = loggingSettings();
    const saved = savedLoggingSettings();
    const changed = JSON.stringify(current) !== JSON.stringify(saved);
    setHasUnsavedChanges(changed);
  });

  // 설정 로드 함수
  const loadSettings = async () => {
    try {
      const frontendConfig = await tauriApi.getFrontendConfig();
      
      if (frontendConfig?.user?.logging) {
        const settings = frontendConfig.user.logging;
        setSavedLoggingSettings(settings);
        setLoggingSettings(settings);
      }
      
      await loggingService.info('설정을 성공적으로 로드했습니다', 'SettingsTab');
    } catch (error) {
      console.error('Failed to load config:', error);
      setSaveStatus({ 
        type: 'error', 
        message: '설정 로드에 실패했습니다: ' + (error instanceof Error ? error.message : '알 수 없는 오류')
      });
      await loggingService.error(`설정 로드 실패: ${error}`, 'SettingsTab');
    }
  };

  // 설정 로드
  onMount(async () => {
    await loadSettings();
  });

  const handleSaveSettings = async () => {
    setIsSaving(true);
    setSaveStatus({ type: null, message: '' });
    
    try {
      // 로깅 설정 저장
      await tauriApi.updateLoggingSettings(loggingSettings());
      
      // 저장된 설정으로 업데이트
      setSavedLoggingSettings(loggingSettings());
      
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
          <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            <For each={loggingPresets}>
              {(preset) => (
                <div class={`p-4 rounded-lg shadow-sm border transition-colors ${
                  isPresetActive(preset) 
                    ? 'bg-emerald-50 dark:bg-emerald-900/20 border-emerald-300 dark:border-emerald-700' 
                    : 'bg-white dark:bg-gray-700 border-gray-200 dark:border-gray-600'
                }`}>
                  <div class="flex flex-col space-y-2">
                    <div class="flex items-center justify-between">
                      <h5 class={`font-medium ${
                        isPresetActive(preset) 
                          ? 'text-emerald-900 dark:text-emerald-100' 
                          : 'text-gray-900 dark:text-white'
                      }`}>
                        {preset.name}
                      </h5>
                      {isPresetActive(preset) && (
                        <div class="flex items-center space-x-1">
                          <svg class="w-4 h-4 text-emerald-600 dark:text-emerald-400" fill="currentColor" viewBox="0 0 20 20">
                            <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd" />
                          </svg>
                          <span class="text-xs font-medium text-emerald-600 dark:text-emerald-400">활성</span>
                        </div>
                      )}
                    </div>
                    <p class={`text-sm ${
                      isPresetActive(preset) 
                        ? 'text-emerald-700 dark:text-emerald-300' 
                        : 'text-gray-600 dark:text-gray-300'
                    }`}>
                      {preset.description}
                    </p>
                    <div class="flex flex-col text-xs text-gray-500 dark:text-gray-400 space-y-1">
                      <span>레벨: {preset.config.level.toUpperCase()}</span>
                      <span>파일 크기: {preset.config.max_file_size_mb}MB</span>
                      <span>분리: {preset.config.separate_frontend_backend ? '예' : '아니오'}</span>
                    </div>
                    <button 
                      onClick={() => applyLoggingPreset(preset)}
                      disabled={isPresetActive(preset)}
                      class={`mt-2 px-3 py-2 text-sm rounded-md transition-colors focus:outline-none focus:ring-2 ${
                        isPresetActive(preset)
                          ? 'bg-gray-300 dark:bg-gray-600 text-gray-500 dark:text-gray-400 cursor-not-allowed'
                          : 'bg-emerald-600 text-white hover:bg-emerald-700 focus:ring-emerald-500'
                      }`}
                    >
                      {isPresetActive(preset) ? '현재 활성' : '적용'}
                    </button>
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
