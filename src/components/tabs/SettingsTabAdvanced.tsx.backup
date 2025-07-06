/**
 * SettingsTab - 백엔드 연동된 확장 가능한 설정 탭
 */

import { Component, createEffect, onMount, Show } from 'solid-js';
import { ExpandableSection } from '../common/ExpandableSection';
import { ZoomControls } from '../common/ZoomControls';
import { settingsState } from '../../stores/settingsStore';

export const SettingsTab: Component = () => {
  
  // 컴포넌트 마운트 시 설정 로드
  onMount(async () => {
    await settingsState.loadSettings();
  });

  // 자동 저장 (설정 변경 후 3초 후)
  let saveTimeout: number;
  createEffect(() => {
    if (settingsState.isDirty) {
      clearTimeout(saveTimeout);
      saveTimeout = setTimeout(() => {
        settingsState.saveSettings();
      }, 3000);
    }
  });

  const handleSliderChange = (
    category: 'crawling' | 'logging' | 'batch',
    field: string,
    value: number
  ) => {
    if (category === 'crawling') {
      settingsState.updateCrawlingSettings({ [field]: value });
    } else if (category === 'batch') {
      settingsState.updateBatchSettings({ [field]: value });
    }
  };

  const handleCheckboxChange = (
    category: 'logging' | 'batch',
    field: string,
    checked: boolean
  ) => {
    if (category === 'logging') {
      settingsState.updateLoggingSettings({ [field]: checked });
    } else if (category === 'batch') {
      settingsState.updateBatchSettings({ [field]: checked });
    }
  };

  const handleSelectChange = (field: string, value: string) => {
    settingsState.updateLoggingSettings({ [field]: value });
  };

  return (
    <div style="padding: 24px; background: white; color: black; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;">
      {/* 헤더 */}
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 24px;">
        <div>
          <h2 style="margin: 0 0 8px 0; font-size: 24px; font-weight: 600; color: #1f2937;">⚙️ 설정</h2>
          <div style="display: flex; align-items: center; gap: 16px; font-size: 14px; color: #6b7280;">
            <Show when={settingsState.isLoading}>
              <span style="color: #3b82f6;">🔄 로딩 중...</span>
            </Show>
            <Show when={settingsState.isDirty}>
              <span style="color: #f59e0b;">● 저장되지 않은 변경사항</span>
            </Show>
            <Show when={settingsState.lastSaved}>
              <span>마지막 저장: {new Date(settingsState.lastSaved!).toLocaleTimeString()}</span>
            </Show>
          </div>
        </div>
        
        {/* 줌 컨트롤과 액션 버튼들 */}
        <div style="display: flex; align-items: center; gap: 12px;">
          <ZoomControls />
          <button
            onClick={() => settingsState.saveSettings()}
            disabled={!settingsState.isDirty || settingsState.isLoading}
            style={`padding: 8px 16px; background: ${settingsState.isDirty ? '#3b82f6' : '#9ca3af'}; color: white; border: none; border-radius: 6px; font-weight: 500; cursor: ${settingsState.isDirty ? 'pointer' : 'not-allowed'}; transition: background-color 0.2s;`}
          >
            💾 저장
          </button>
          <button
            onClick={() => settingsState.resetToDefaults()}
            style="padding: 8px 16px; background: #6b7280; color: white; border: none; border-radius: 6px; font-weight: 500; cursor: pointer; transition: background-color 0.2s;"
            onMouseOver={(e) => e.currentTarget.style.background = '#4b5563'}
            onMouseOut={(e) => e.currentTarget.style.background = '#6b7280'}
          >
            🔄 기본값 복원
          </button>
        </div>
      </div>

      {/* 기본 설정 섹션 */}
      <div style="margin-bottom: 24px;">
        <ExpandableSection
          title="기본 설정"
          icon="🔧"
          isExpanded={settingsState.expandedSections.basic}
          onToggle={() => settingsState.toggleSection('basic')}
        >
          <div style="space-y: 20px;">
            {/* 동시 다운로드 수 */}
            <div>
              <label style="display: block; margin-bottom: 8px; font-weight: 500; color: #374151;">
                동시 다운로드 수: {settingsState.settings.crawling.concurrent_downloads}
              </label>
              <input
                type="range"
                min="1"
                max="10"
                value={settingsState.settings.crawling.concurrent_downloads}
                onInput={(e) => handleSliderChange('crawling', 'concurrent_downloads', parseInt(e.currentTarget.value))}
                style="width: 100%; height: 6px; background: #ddd; border-radius: 3px; outline: none;"
              />
              <div style="display: flex; justify-content: space-between; font-size: 12px; color: #6b7280; margin-top: 4px;">
                <span>1</span>
                <span>10</span>
              </div>
            </div>

            {/* 요청 간 지연 */}
            <div style="margin-top: 20px;">
              <label style="display: block; margin-bottom: 8px; font-weight: 500; color: #374151;">
                요청 간 지연: {settingsState.settings.crawling.request_delay_ms}ms
              </label>
              <input
                type="range"
                min="0"
                max="5000"
                step="100"
                value={settingsState.settings.crawling.request_delay_ms}
                onInput={(e) => handleSliderChange('crawling', 'request_delay_ms', parseInt(e.currentTarget.value))}
                style="width: 100%; height: 6px; background: #ddd; border-radius: 3px; outline: none;"
              />
              <div style="display: flex; justify-content: space-between; font-size: 12px; color: #6b7280; margin-top: 4px;">
                <span>0ms</span>
                <span>5000ms</span>
              </div>
            </div>

            {/* 타임아웃 */}
            <div style="margin-top: 20px;">
              <label style="display: block; margin-bottom: 8px; font-weight: 500; color: #374151;">
                타임아웃: {settingsState.settings.crawling.timeout_seconds}초
              </label>
              <input
                type="range"
                min="10"
                max="120"
                value={settingsState.settings.crawling.timeout_seconds}
                onInput={(e) => handleSliderChange('crawling', 'timeout_seconds', parseInt(e.currentTarget.value))}
                style="width: 100%; height: 6px; background: #ddd; border-radius: 3px; outline: none;"
              />
              <div style="display: flex; justify-content: space-between; font-size: 12px; color: #6b7280; margin-top: 4px;">
                <span>10초</span>
                <span>120초</span>
              </div>
            </div>

            {/* 재시도 횟수 */}
            <div style="margin-top: 20px;">
              <label style="display: block; margin-bottom: 8px; font-weight: 500; color: #374151;">
                재시도 횟수: {settingsState.settings.crawling.retry_count}
              </label>
              <input
                type="range"
                min="0"
                max="10"
                value={settingsState.settings.crawling.retry_count}
                onInput={(e) => handleSliderChange('crawling', 'retry_count', parseInt(e.currentTarget.value))}
                style="width: 100%; height: 6px; background: #ddd; border-radius: 3px; outline: none;"
              />
              <div style="display: flex; justify-content: space-between; font-size: 12px; color: #6b7280; margin-top: 4px;">
                <span>0</span>
                <span>10</span>
              </div>
            </div>
          </div>
        </ExpandableSection>
      </div>

      {/* 로깅 설정 섹션 */}
      <div style="margin-bottom: 24px;">
        <ExpandableSection
          title="로깅 설정"
          icon="📝"
          isExpanded={settingsState.expandedSections.logging}
          onToggle={() => settingsState.toggleSection('logging')}
        >
          <div style="space-y: 16px;">
            {/* 로그 레벨 */}
            <div>
              <label style="display: block; margin-bottom: 8px; font-weight: 500; color: #374151;">로그 레벨</label>
              <select
                value={settingsState.settings.logging.level}
                onChange={(e) => handleSelectChange('level', e.currentTarget.value)}
                style="width: 100%; padding: 8px 12px; border: 1px solid #d1d5db; border-radius: 6px; background: white; font-size: 14px;"
              >
                <option value="DEBUG">DEBUG</option>
                <option value="INFO">INFO</option>
                <option value="WARN">WARN</option>
                <option value="ERROR">ERROR</option>
              </select>
            </div>

            {/* 터미널 출력 */}
            <div style="margin-top: 16px;">
              <label style="display: flex; align-items: center; font-weight: 500; color: #374151; cursor: pointer;">
                <input
                  type="checkbox"
                  checked={settingsState.settings.logging.terminal_output}
                  onChange={(e) => handleCheckboxChange('logging', 'terminal_output', e.currentTarget.checked)}
                  style="margin-right: 8px; width: 16px; height: 16px;"
                />
                터미널 출력
              </label>
            </div>

            {/* 파일 로깅 */}
            <div style="margin-top: 16px;">
              <label style="display: flex; align-items: center; font-weight: 500; color: #374151; cursor: pointer;">
                <input
                  type="checkbox"
                  checked={settingsState.settings.logging.file_logging}
                  onChange={(e) => handleCheckboxChange('logging', 'file_logging', e.currentTarget.checked)}
                  style="margin-right: 8px; width: 16px; height: 16px;"
                />
                파일 로깅
              </label>
            </div>

            {/* 자동 로그 정리 */}
            <div style="margin-top: 16px;">
              <label style="display: flex; align-items: center; font-weight: 500; color: #374151; cursor: pointer;">
                <input
                  type="checkbox"
                  checked={settingsState.settings.logging.auto_cleanup_logs}
                  onChange={(e) => handleCheckboxChange('logging', 'auto_cleanup_logs', e.currentTarget.checked)}
                  style="margin-right: 8px; width: 16px; height: 16px;"
                />
                자동 로그 정리
              </label>
            </div>
          </div>
        </ExpandableSection>
      </div>

      {/* 배치 처리 설정 섹션 */}
      <div style="margin-bottom: 24px;">
        <ExpandableSection
          title="배치 처리 설정"
          icon="📦"
          isExpanded={settingsState.expandedSections.batch}
          onToggle={() => settingsState.toggleSection('batch')}
        >
          <div style="space-y: 20px;">
            {/* 배치 크기 */}
            <div>
              <label style="display: block; margin-bottom: 8px; font-weight: 500; color: #374151;">
                배치 크기: {settingsState.settings.batch.batch_size}
              </label>
              <input
                type="range"
                min="10"
                max="200"
                step="10"
                value={settingsState.settings.batch.batch_size}
                onInput={(e) => handleSliderChange('batch', 'batch_size', parseInt(e.currentTarget.value))}
                style="width: 100%; height: 6px; background: #ddd; border-radius: 3px; outline: none;"
              />
              <div style="display: flex; justify-content: space-between; font-size: 12px; color: #6b7280; margin-top: 4px;">
                <span>10</span>
                <span>200</span>
              </div>
            </div>

            {/* 진행률 업데이트 간격 */}
            <div style="margin-top: 20px;">
              <label style="display: block; margin-bottom: 8px; font-weight: 500; color: #374151;">
                진행률 업데이트 간격: {settingsState.settings.batch.progress_interval_ms / 1000}초
              </label>
              <input
                type="range"
                min="500"
                max="10000"
                step="500"
                value={settingsState.settings.batch.progress_interval_ms}
                onInput={(e) => handleSliderChange('batch', 'progress_interval_ms', parseInt(e.currentTarget.value))}
                style="width: 100%; height: 6px; background: #ddd; border-radius: 3px; outline: none;"
              />
              <div style="display: flex; justify-content: space-between; font-size: 12px; color: #6b7280; margin-top: 4px;">
                <span>0.5초</span>
                <span>10초</span>
              </div>
            </div>

            {/* 자동 백업 */}
            <div style="margin-top: 16px;">
              <label style="display: flex; align-items: center; font-weight: 500; color: #374151; cursor: pointer;">
                <input
                  type="checkbox"
                  checked={settingsState.settings.batch.auto_backup}
                  onChange={(e) => handleCheckboxChange('batch', 'auto_backup', e.currentTarget.checked)}
                  style="margin-right: 8px; width: 16px; height: 16px;"
                />
                자동 백업
              </label>
            </div>

            {/* 배치 처리 활성화 */}
            <div style="margin-top: 16px;">
              <label style="display: flex; align-items: center; font-weight: 500; color: #374151; cursor: pointer;">
                <input
                  type="checkbox"
                  checked={settingsState.settings.batch.enable_batch_processing}
                  onChange={(e) => handleCheckboxChange('batch', 'enable_batch_processing', e.currentTarget.checked)}
                  style="margin-right: 8px; width: 16px; height: 16px;"
                />
                배치 처리 활성화
              </label>
            </div>
          </div>
        </ExpandableSection>
      </div>
    </div>
  );
};
