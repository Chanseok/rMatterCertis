
/**
 * SettingsTab - 설정 탭 컴포넌트
 * settingsStore를 기반으로 한 실제 백엔드 연동 설정 UI
 */
import { Component, createSignal, onMount, Show } from 'solid-js';
import { settingsState } from '../../stores/settingsStore';

export const SettingsTab: Component = () => {
  const [saveMessage, setSaveMessage] = createSignal<string>('');
  const [showMessage, setShowMessage] = createSignal(false);

  onMount(async () => {
    console.log('⚙️ SettingsTab 컴포넌트 로드됨');
    await settingsState.loadSettings();
  });

  const handleSave = async () => {
    try {
      await settingsState.saveSettings();
      setSaveMessage('✅ 설정이 저장되었습니다');
      setShowMessage(true);
      setTimeout(() => setShowMessage(false), 3000);
    } catch (error) {
      setSaveMessage('❌ 설정 저장에 실패했습니다');
      setShowMessage(true);
      setTimeout(() => setShowMessage(false), 3000);
    }
  };

  const handleReset = async () => {
    if (confirm('모든 설정을 기본값으로 초기화하시겠습니까?')) {
      await settingsState.resetToDefaults();
      setSaveMessage('✅ 기본값으로 초기화되었습니다');
      setShowMessage(true);
      setTimeout(() => setShowMessage(false), 3000);
    }
  };

  return (
    <div style="padding: 24px; background: white; color: black; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 24px;">
        <h2 style="margin: 0; font-size: 24px; font-weight: 600; color: #1f2937;">⚙️ 설정</h2>
        <div style="display: flex; gap: 8px;">
          <button
            onClick={handleReset}
            style="padding: 8px 16px; background: #6b7280; color: white; border: none; border-radius: 6px; cursor: pointer; font-size: 14px; transition: background-color 0.2s;"
            onMouseOver={(e) => (e.currentTarget.style.background = '#4b5563')}
            onMouseOut={(e) => (e.currentTarget.style.background = '#6b7280')}
          >
            기본값으로 초기화
          </button>
          <button
            onClick={handleSave}
            disabled={!settingsState.isDirty}
            style={`padding: 8px 16px; background: ${settingsState.isDirty ? '#10b981' : '#d1d5db'}; color: white; border: none; border-radius: 6px; cursor: ${settingsState.isDirty ? 'pointer' : 'not-allowed'}; font-size: 14px; transition: background-color 0.2s;`}
            onMouseOver={(e) => settingsState.isDirty && (e.currentTarget.style.background = '#059669')}
            onMouseOut={(e) => settingsState.isDirty && (e.currentTarget.style.background = '#10b981')}
          >
            {settingsState.isLoading ? '저장 중...' : '설정 저장'}
          </button>
        </div>
      </div>

      <Show when={showMessage()}>
        <div style="background: #f0f9ff; border: 1px solid #0ea5e9; border-radius: 8px; padding: 12px; margin-bottom: 16px; color: #0c4a6e;">
          {saveMessage()}
        </div>
      </Show>

      {/* 크롤링 설정 */}
      <div style="background: #f9fafb; border: 1px solid #e5e7eb; border-radius: 8px; padding: 16px; margin-bottom: 16px;">
        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px;">
          <h3 style="margin: 0; font-size: 18px; font-weight: 500; color: #374151;">🚀 크롤링 설정</h3>
          <button
            onClick={() => settingsState.toggleSection('basic')}
            style="background: none; border: none; color: #6b7280; cursor: pointer; font-size: 14px;"
          >
            {settingsState.expandedSections.basic ? '▼' : '▶'}
          </button>
        </div>
        <Show when={settingsState.expandedSections.basic}>
          <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 16px;">
            <div>
              <label style="display: block; font-weight: 500; margin-bottom: 4px; color: #374151;">동시 다운로드 수</label>
              <input
                type="number"
                value={settingsState.settings.crawling.concurrent_downloads}
                min={1}
                max={20}
                onInput={(e) => settingsState.updateCrawlingSettings({ concurrent_downloads: +e.currentTarget.value })}
                style="width: 100%; padding: 8px; border: 1px solid #d1d5db; border-radius: 4px; font-size: 14px;"
              />
            </div>
            <div>
              <label style="display: block; font-weight: 500; margin-bottom: 4px; color: #374151;">요청 간 지연 시간 (ms)</label>
              <input
                type="number"
                value={settingsState.settings.crawling.request_delay_ms}
                min={0}
                max={10000}
                onInput={(e) => settingsState.updateCrawlingSettings({ request_delay_ms: +e.currentTarget.value })}
                style="width: 100%; padding: 8px; border: 1px solid #d1d5db; border-radius: 4px; font-size: 14px;"
              />
            </div>
            <div>
              <label style="display: block; font-weight: 500; margin-bottom: 4px; color: #374151;">타임아웃 (초)</label>
              <input
                type="number"
                value={settingsState.settings.crawling.timeout_seconds}
                min={5}
                max={300}
                onInput={(e) => settingsState.updateCrawlingSettings({ timeout_seconds: +e.currentTarget.value })}
                style="width: 100%; padding: 8px; border: 1px solid #d1d5db; border-radius: 4px; font-size: 14px;"
              />
            </div>
            <div>
              <label style="display: block; font-weight: 500; margin-bottom: 4px; color: #374151;">재시도 횟수</label>
              <input
                type="number"
                value={settingsState.settings.crawling.retry_count}
                min={0}
                max={10}
                onInput={(e) => settingsState.updateCrawlingSettings({ retry_count: +e.currentTarget.value })}
                style="width: 100%; padding: 8px; border: 1px solid #d1d5db; border-radius: 4px; font-size: 14px;"
              />
            </div>
          </div>
        </Show>
      </div>

      {/* 배치 처리 설정 */}
      <div style="background: #f9fafb; border: 1px solid #e5e7eb; border-radius: 8px; padding: 16px; margin-bottom: 16px;">
        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px;">
          <h3 style="margin: 0; font-size: 18px; font-weight: 500; color: #374151;">📦 배치 처리 설정</h3>
          <button
            onClick={() => settingsState.toggleSection('batch')}
            style="background: none; border: none; color: #6b7280; cursor: pointer; font-size: 14px;"
          >
            {settingsState.expandedSections.batch ? '▼' : '▶'}
          </button>
        </div>
        <Show when={settingsState.expandedSections.batch}>
          <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 16px;">
            <div>
              <label style="display: block; font-weight: 500; margin-bottom: 4px; color: #374151;">배치 크기</label>
              <input
                type="number"
                value={settingsState.settings.batch.batch_size}
                min={1}
                max={1000}
                onInput={(e) => settingsState.updateBatchSettings({ batch_size: +e.currentTarget.value })}
                style="width: 100%; padding: 8px; border: 1px solid #d1d5db; border-radius: 4px; font-size: 14px;"
              />
            </div>
            <div>
              <label style="display: block; font-weight: 500; margin-bottom: 4px; color: #374151;">진행률 업데이트 간격 (ms)</label>
              <input
                type="number"
                value={settingsState.settings.batch.progress_interval_ms}
                min={100}
                max={10000}
                onInput={(e) => settingsState.updateBatchSettings({ progress_interval_ms: +e.currentTarget.value })}
                style="width: 100%; padding: 8px; border: 1px solid #d1d5db; border-radius: 4px; font-size: 14px;"
              />
            </div>
            <div>
              <label style="display: block; font-weight: 500; margin-bottom: 4px; color: #374151;">배치 간 지연 시간 (ms)</label>
              <input
                type="number"
                value={settingsState.settings.batch.batch_delay_ms}
                min={0}
                max={5000}
                onInput={(e) => settingsState.updateBatchSettings({ batch_delay_ms: +e.currentTarget.value })}
                style="width: 100%; padding: 8px; border: 1px solid #d1d5db; border-radius: 4px; font-size: 14px;"
              />
            </div>
            <div style="display: flex; align-items: center; gap: 8px;">
              <input
                type="checkbox"
                checked={settingsState.settings.batch.auto_backup}
                onChange={(e) => settingsState.updateBatchSettings({ auto_backup: e.currentTarget.checked })}
                style="width: 16px; height: 16px;"
              />
              <label style="font-weight: 500; color: #374151;">자동 백업</label>
            </div>
            <div style="display: flex; align-items: center; gap: 8px;">
              <input
                type="checkbox"
                checked={settingsState.settings.batch.enable_batch_processing}
                onChange={(e) => settingsState.updateBatchSettings({ enable_batch_processing: e.currentTarget.checked })}
                style="width: 16px; height: 16px;"
              />
              <label style="font-weight: 500; color: #374151;">배치 처리 활성화</label>
            </div>
          </div>
        </Show>
      </div>

      {/* 로깅 설정 */}
      <div style="background: #f9fafb; border: 1px solid #e5e7eb; border-radius: 8px; padding: 16px;">
        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px;">
          <h3 style="margin: 0; font-size: 18px; font-weight: 500; color: #374151;">📝 로깅 설정</h3>
          <button
            onClick={() => settingsState.toggleSection('logging')}
            style="background: none; border: none; color: #6b7280; cursor: pointer; font-size: 14px;"
          >
            {settingsState.expandedSections.logging ? '▼' : '▶'}
          </button>
        </div>
        <Show when={settingsState.expandedSections.logging}>
          <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 16px;">
            <div>
              <label style="display: block; font-weight: 500; margin-bottom: 4px; color: #374151;">로그 레벨</label>
              <select
                value={settingsState.settings.logging.level}
                onChange={(e) => settingsState.updateLoggingSettings({ level: e.currentTarget.value as any })}
                style="width: 100%; padding: 8px; border: 1px solid #d1d5db; border-radius: 4px; font-size: 14px;"
              >
                <option value="DEBUG">DEBUG</option>
                <option value="INFO">INFO</option>
                <option value="WARN">WARN</option>
                <option value="ERROR">ERROR</option>
              </select>
            </div>
            <div>
              <label style="display: block; font-weight: 500; margin-bottom: 4px; color: #374151;">최대 파일 크기 (MB)</label>
              <input
                type="number"
                value={settingsState.settings.logging.max_file_size_mb}
                min={1}
                max={100}
                onInput={(e) => settingsState.updateLoggingSettings({ max_file_size_mb: +e.currentTarget.value })}
                style="width: 100%; padding: 8px; border: 1px solid #d1d5db; border-radius: 4px; font-size: 14px;"
              />
            </div>
            <div>
              <label style="display: block; font-weight: 500; margin-bottom: 4px; color: #374151;">최대 파일 수</label>
              <input
                type="number"
                value={settingsState.settings.logging.max_files}
                min={1}
                max={20}
                onInput={(e) => settingsState.updateLoggingSettings({ max_files: +e.currentTarget.value })}
                style="width: 100%; padding: 8px; border: 1px solid #d1d5db; border-radius: 4px; font-size: 14px;"
              />
            </div>
            <div style="display: flex; align-items: center; gap: 8px;">
              <input
                type="checkbox"
                checked={settingsState.settings.logging.terminal_output}
                onChange={(e) => settingsState.updateLoggingSettings({ terminal_output: e.currentTarget.checked })}
                style="width: 16px; height: 16px;"
              />
              <label style="font-weight: 500; color: #374151;">터미널 출력</label>
            </div>
            <div style="display: flex; align-items: center; gap: 8px;">
              <input
                type="checkbox"
                checked={settingsState.settings.logging.file_logging}
                onChange={(e) => settingsState.updateLoggingSettings({ file_logging: e.currentTarget.checked })}
                style="width: 16px; height: 16px;"
              />
              <label style="font-weight: 500; color: #374151;">파일 로깅</label>
            </div>
            <div style="display: flex; align-items: center; gap: 8px;">
              <input
                type="checkbox"
                checked={settingsState.settings.logging.auto_cleanup_logs}
                onChange={(e) => settingsState.updateLoggingSettings({ auto_cleanup_logs: e.currentTarget.checked })}
                style="width: 16px; height: 16px;"
              />
              <label style="font-weight: 500; color: #374151;">자동 로그 정리</label>
            </div>
          </div>
        </Show>
      </div>
    </div>
  );
};

export default SettingsTab;

