/**
 * SettingsTab - 설정 탭 컴포넌트
 * settingsStore를 기반으로 한 실제 백엔드 연동 설정 UI
 */
import { Component, createSignal, onMount, For } from 'solid-js';
import { emit } from '@tauri-apps/api/event';
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
      // 설정 저장 성공 시, 전역 이벤트로 알림을 보냄
      try {
        await emit('settings-updated', { at: Date.now() });
      } catch (e) {
        console.warn('[SettingsTab] settings-updated emit failed', e);
      }
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
        
        select {
          padding: 8px 12px;
          border: 1px solid #d1d5db;
          border-radius: 6px;
          font-size: 14px;
          transition: border-color 0.2s;
          background: white;
        }
        
        select:focus {
          outline: none;
          border-color: #3b82f6;
          box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.1);
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
        
        {/* 크롤링 범위 설정 */}
        <fieldset>
          <legend>크롤링 범위 설정</legend>
          <div class="form-row">
            <label>페이지 범위 제한
              <input 
                type="number" 
                value={settingsState.getNestedValue('user.crawling.page_range_limit') || 20} 
                min={1} 
                max={100} 
                onInput={e => settingsState.updateNestedField('user.crawling.page_range_limit', +e.currentTarget.value)} 
              />
              <small>한 번의 크롤링에서 처리할 최대 페이지 수</small>
            </label>
            <label>크롤링 모드
              <select 
                value={settingsState.getNestedValue('user.crawling.crawling_mode') || 'incremental'} 
                onChange={e => settingsState.updateNestedField('user.crawling.crawling_mode', e.currentTarget.value)}
              >
                <option value="incremental">증분 업데이트 (기본)</option>
                <option value="gap_filling">누락 제품 보완</option>
                <option value="integrity_check">무결성 검증 (Binary Search)</option>
                <option value="full_rebuild">전체 재구축</option>
                <option value="custom_range">사용자 지정 범위</option>
              </select>
              <small>크롤링 전략: 증분(최신만), 누락보완(빈구멍채우기), 무결성검증(데이터손실탐지), 전체재구축</small>
            </label>
          </div>
          
          {/* 사용자 지정 범위 모드일 때만 표시 */}
          {settingsState.getNestedValue('user.crawling.crawling_mode') === 'custom_range' && (
            <div class="form-row">
              <label>사용자 지정 페이지 범위
                <input 
                  type="text" 
                  value={settingsState.getNestedValue('user.crawling.custom_page_ranges') || ''} 
                  placeholder="예: 1-10, 15, 20-25, 30"
                  onInput={e => settingsState.updateNestedField('user.crawling.custom_page_ranges', e.currentTarget.value)} 
                />
                <small>크롤링할 페이지를 지정하세요. 범위(1-10), 단일 페이지(15), 쉼표로 구분</small>
              </label>
            </div>
          )}
          
          <div class="form-row">
            <label>자동 범위 조정
              <input 
                type="checkbox" 
                checked={settingsState.getNestedValue('user.crawling.auto_adjust_range') || true} 
                onChange={e => settingsState.updateNestedField('user.crawling.auto_adjust_range', e.currentTarget.checked)} 
              />
              <small>시스템이 사이트 상태와 데이터 변화에 따라 범위를 자동 조정</small>
            </label>
            <label>데이터 검증 활성화
              <input 
                type="checkbox" 
                checked={settingsState.getNestedValue('user.crawling.enable_data_validation') || true} 
                onChange={e => settingsState.updateNestedField('user.crawling.enable_data_validation', e.currentTarget.checked)} 
              />
              <small>수집된 데이터의 유효성을 검증하여 품질 보장</small>
            </label>
          </div>
          <div class="form-row">
            <label>누락 제품 탐지 임계값
              <input 
                type="number" 
                value={settingsState.getNestedValue('user.crawling.gap_detection_threshold') || 5} 
                min={1} 
                max={50} 
                onInput={e => settingsState.updateNestedField('user.crawling.gap_detection_threshold', +e.currentTarget.value)} 
              />
              <small>연속으로 이 개수만큼 누락 시 gap으로 인식</small>
            </label>
            <label>Binary Search 최대 깊이
              <input 
                type="number" 
                value={settingsState.getNestedValue('user.crawling.binary_search_max_depth') || 10} 
                min={3} 
                max={20} 
                onInput={e => settingsState.updateNestedField('user.crawling.binary_search_max_depth', +e.currentTarget.value)} 
              />
              <small>무결성 검증 시 이진 탐색 최대 반복 횟수</small>
            </label>
          </div>
        </fieldset>

        {/* 기본 크롤링 설정 */}
        <fieldset>
          <legend>크롤링 설정</legend>
          <div class="form-row">
            <label>최대 페이지 수
              <input 
                type="number" 
                value={settingsState.getNestedValue('user.crawling.page_range_limit')} 
                min={1} 
                max={1000} 
                onInput={e => settingsState.updateNestedField('user.crawling.page_range_limit', +e.currentTarget.value)} 
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
            <label>목록 페이지 재시도 횟수
              <input 
                type="number" 
                value={settingsState.getNestedValue('user.crawling.product_list_retry_count')} 
                min={1} 
                max={30} 
                onInput={e => settingsState.updateNestedField('user.crawling.product_list_retry_count', +e.currentTarget.value)} 
              />
            </label>
            <label>상세 페이지 재시도 횟수
              <input 
                type="number" 
                value={settingsState.getNestedValue('user.crawling.product_detail_retry_count')} 
                min={1} 
                max={30} 
                onInput={e => settingsState.updateNestedField('user.crawling.product_detail_retry_count', +e.currentTarget.value)} 
              />
            </label>
            <label>오류 허용 임계값 (%)
              <input 
                type="number" 
                value={settingsState.getNestedValue('user.crawling.error_threshold_percent') || 10} 
                min={1} 
                max={50} 
                onInput={e => settingsState.updateNestedField('user.crawling.error_threshold_percent', +e.currentTarget.value)} 
              />
              <small>이 비율 이상 오류 발생 시 크롤링 중단</small>
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
