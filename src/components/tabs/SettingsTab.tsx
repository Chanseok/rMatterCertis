/**
 * SettingsTab - 설정 탭 컴포넌트
 * settingsStore를 기반으로 한 실제 백엔드 연동 설정 UI
 */
import { Component, createSignal, onMount, For } from 'solid-js';
import { settingsState } from '../../stores/settingsStore';
import { CONFIG_PRESETS } from '../../types/config';

export const SettingsTab: Component = () => {
  const [saveMessage, setSaveMessage] = createSignal<string>('');
  const [showMessage, setShowMessage] = createSignal(false);
  const [showModal, setShowModal] = createSignal(false);

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

  const applyPreset = (presetName: string) => {
    settingsState.applyPreset(presetName);
    setSaveMessage(`✅ 프리셋 "${presetName}" 적용됨`);
    setShowMessage(true);
    setTimeout(() => setShowMessage(false), 3000);
  };

  return (
    <div class="settings-container">
      <style>{`
        .settings-container {
          padding: 24px;
          background: white;
          color: black;
          font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
          max-width: 1200px;
          margin: 0 auto;
        }
        
        .settings-header {
          display: flex;
          justify-content: space-between;
          align-items: center;
          margin-bottom: 24px;
          padding-bottom: 16px;
          border-bottom: 1px solid #e5e7eb;
        }
        
        .settings-title {
          margin: 0;
          font-size: 24px;
          font-weight: 600;
          color: #1f2937;
        }
        
        .preset-buttons {
          display: flex;
          gap: 8px;
          margin-bottom: 24px;
        }
        
        .btn {
          padding: 8px 16px;
          border: 1px solid #d1d5db;
          border-radius: 6px;
          cursor: pointer;
          font-size: 14px;
          transition: all 0.2s;
          background: white;
          color: #374151;
        }
        
        .btn:hover {
          background: #f9fafb;
          border-color: #9ca3af;
        }
        
        .btn-primary {
          background: #3b82f6;
          color: white;
          border-color: #3b82f6;
        }
        
        .btn-primary:hover {
          background: #2563eb;
          border-color: #2563eb;
        }
        
        .btn-ghost {
          background: transparent;
          border-color: transparent;
          color: #6b7280;
        }
        
        .btn-ghost:hover {
          background: #f3f4f6;
          color: #374151;
        }
        
        .settings-form {
          display: flex;
          flex-direction: column;
          gap: 24px;
        }
        
        fieldset {
          border: 1px solid #e5e7eb;
          border-radius: 8px;
          padding: 16px;
          margin: 0;
        }
        
        legend {
          font-weight: 600;
          color: #1f2937;
          padding: 0 8px;
        }
        
        .form-row {
          display: grid;
          grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
          gap: 16px;
          margin-bottom: 12px;
        }
        
        .form-row:last-child {
          margin-bottom: 0;
        }
        
        label {
          display: flex;
          flex-direction: column;
          gap: 4px;
          font-weight: 500;
          color: #374151;
        }
        
        input[type="number"], input[type="text"] {
          padding: 8px 12px;
          border: 1px solid #d1d5db;
          border-radius: 6px;
          font-size: 14px;
          transition: border-color 0.2s;
        }
        
        input[type="number"]:focus, input[type="text"]:focus {
          outline: none;
          border-color: #3b82f6;
          box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.1);
        }
        
        input[type="checkbox"] {
          width: 16px;
          height: 16px;
          margin-top: 4px;
        }
        
        .form-actions {
          display: flex;
          gap: 12px;
          justify-content: flex-end;
          padding-top: 16px;
          border-top: 1px solid #e5e7eb;
        }
        
        .toast {
          position: fixed;
          top: 20px;
          right: 20px;
          padding: 12px 16px;
          background: #10b981;
          color: white;
          border-radius: 6px;
          font-weight: 500;
          z-index: 1000;
          box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);
        }
        
        .toast.error {
          background: #ef4444;
        }
        
        .modal {
          position: fixed;
          top: 0;
          left: 0;
          right: 0;
          bottom: 0;
          background: rgba(0, 0, 0, 0.5);
          display: flex;
          align-items: center;
          justify-content: center;
          z-index: 1000;
        }
        
        .modal-content {
          background: white;
          padding: 24px;
          border-radius: 8px;
          max-width: 600px;
          max-height: 80vh;
          overflow-y: auto;
          box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.1);
        }
        
        .config-preview {
          background: #f8fafc;
          border: 1px solid #e2e8f0;
          border-radius: 4px;
          padding: 12px;
          font-size: 12px;
          font-family: Monaco, 'Courier New', monospace;
          overflow-x: auto;
          margin: 16px 0;
        }
        
        .loading {
          opacity: 0.6;
          pointer-events: none;
        }
      `}</style>

      <div class="settings-header">
        <h2 class="settings-title">⚙️ 애플리케이션 설정</h2>
        <div style="display: flex; gap: 8px;">
          <button class="btn btn-ghost" onClick={handleReset}>
            기본값으로 초기화
          </button>
          <button class="btn btn-primary" onClick={handleSave} disabled={settingsState.isLoading}>
            {settingsState.isLoading ? '저장 중...' : '설정 저장'}
          </button>
        </div>
      </div>

      <div class="preset-buttons">
        <For each={CONFIG_PRESETS}>{preset => (
          <button class="btn" onClick={() => applyPreset(preset.name)}>
            {preset.name}
          </button>
        )}</For>
      </div>

      <form class={`settings-form ${settingsState.isLoading ? 'loading' : ''}`} onSubmit={e => { e.preventDefault(); handleSave(); }}>
        
        {/* 기본 크롤링 설정 */}
        <fieldset>
          <legend>크롤링 설정</legend>
          <div class="form-row">
            <label>최대 페이지 수
              <input 
                type="number" 
                value={settingsState.getNestedValue('user.max_pages')} 
                min={1} 
                max={1000} 
                onInput={e => settingsState.updateNestedField('user.max_pages', +e.currentTarget.value)} 
              />
            </label>
            <label>요청 지연 시간 (ms)
              <input 
                type="number" 
                value={settingsState.getNestedValue('user.request_delay_ms')} 
                min={100} 
                max={5000} 
                onInput={e => settingsState.updateNestedField('user.request_delay_ms', +e.currentTarget.value)} 
              />
            </label>
            <label>최대 동시 요청 수
              <input 
                type="number" 
                value={settingsState.getNestedValue('user.max_concurrent_requests')} 
                min={1} 
                max={50} 
                onInput={e => settingsState.updateNestedField('user.max_concurrent_requests', +e.currentTarget.value)} 
              />
            </label>
          </div>
        </fieldset>

        {/* 배치 처리 설정 */}
        <fieldset>
          <legend>배치 처리</legend>
          <div class="form-row">
            <label>배치 크기
              <input 
                type="number" 
                value={settingsState.getNestedValue('user.batch.batch_size')} 
                min={1} 
                max={100} 
                onInput={e => settingsState.updateNestedField('user.batch.batch_size', +e.currentTarget.value)} 
              />
            </label>
            <label>배치 지연 시간 (ms)
              <input 
                type="number" 
                value={settingsState.getNestedValue('user.batch.batch_delay_ms')} 
                min={100} 
                max={5000} 
                onInput={e => settingsState.updateNestedField('user.batch.batch_delay_ms', +e.currentTarget.value)} 
              />
            </label>
            <label>배치 처리 활성화
              <input 
                type="checkbox" 
                checked={settingsState.getNestedValue('user.batch.enable_batch_processing')} 
                onChange={e => settingsState.updateNestedField('user.batch.enable_batch_processing', e.currentTarget.checked)} 
              />
            </label>
          </div>
        </fieldset>

        {/* 상세 크롤링 설정 */}
        <fieldset>
          <legend>상세 설정</legend>
          <div class="form-row">
            <label>페이지 범위 제한
              <input 
                type="number" 
                value={settingsState.getNestedValue('user.crawling.page_range_limit')} 
                min={1} 
                max={100} 
                onInput={e => settingsState.updateNestedField('user.crawling.page_range_limit', +e.currentTarget.value)} 
              />
            </label>
            <label>목록 페이지 재시도 횟수
              <input 
                type="number" 
                value={settingsState.getNestedValue('user.crawling.product_list_retry_count')} 
                min={1} 
                max={10} 
                onInput={e => settingsState.updateNestedField('user.crawling.product_list_retry_count', +e.currentTarget.value)} 
              />
            </label>
            <label>상세 페이지 재시도 횟수
              <input 
                type="number" 
                value={settingsState.getNestedValue('user.crawling.product_detail_retry_count')} 
                min={1} 
                max={10} 
                onInput={e => settingsState.updateNestedField('user.crawling.product_detail_retry_count', +e.currentTarget.value)} 
              />
            </label>
          </div>
          <div class="form-row">
            <label>자동 DB 저장
              <input 
                type="checkbox" 
                checked={settingsState.getNestedValue('user.crawling.auto_add_to_local_db')} 
                onChange={e => settingsState.updateNestedField('user.crawling.auto_add_to_local_db', e.currentTarget.checked)} 
              />
            </label>
            <label>상세 로깅
              <input 
                type="checkbox" 
                checked={settingsState.getNestedValue('user.verbose_logging')} 
                onChange={e => settingsState.updateNestedField('user.verbose_logging', e.currentTarget.checked)} 
              />
            </label>
          </div>
        </fieldset>

        {/* 고급 설정 */}
        <fieldset>
          <legend>고급 설정</legend>
          <div class="form-row">
            <label>요청 타임아웃 (초)
              <input 
                type="number" 
                value={settingsState.getNestedValue('advanced.request_timeout_seconds')} 
                min={5} 
                max={120} 
                onInput={e => settingsState.updateNestedField('advanced.request_timeout_seconds', +e.currentTarget.value)} 
              />
            </label>
            <label>최대 검색 시도 횟수
              <input 
                type="number" 
                value={settingsState.getNestedValue('advanced.max_search_attempts')} 
                min={1} 
                max={20} 
                onInput={e => settingsState.updateNestedField('advanced.max_search_attempts', +e.currentTarget.value)} 
              />
            </label>
            <label>재시도 지연 시간 (ms)
              <input 
                type="number" 
                value={settingsState.getNestedValue('advanced.retry_delay_ms')} 
                min={500} 
                max={10000} 
                onInput={e => settingsState.updateNestedField('advanced.retry_delay_ms', +e.currentTarget.value)} 
              />
            </label>
          </div>
        </fieldset>

        <div class="form-actions">
          <button type="button" class="btn" onClick={() => setShowModal(true)}>
            미리보기
          </button>
          <button type="submit" class="btn btn-primary" disabled={settingsState.isLoading}>
            {settingsState.isLoading ? '저장 중...' : '설정 저장'}
          </button>
        </div>
      </form>

      {showMessage() && (
        <div class={`toast ${saveMessage().includes('❌') ? 'error' : ''}`}>
          {saveMessage()}
        </div>
      )}
      
      {showModal() && (
        <div class="modal" onClick={(e) => e.target === e.currentTarget && setShowModal(false)}>
          <div class="modal-content">
            <h3>설정 미리보기</h3>
            <pre class="config-preview">{JSON.stringify(settingsState.settings, null, 2)}</pre>
            <div class="form-actions">
              <button class="btn btn-primary" onClick={handleSave}>저장</button>
              <button class="btn btn-ghost" onClick={() => setShowModal(false)}>취소</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default SettingsTab;
