import { createSignal, Show, For } from 'solid-js';
import { BackendCrawlerConfig, ConfigPreset, CONFIG_PRESETS } from '../../../types/crawling';
import { Button, Modal, Toast } from '../../ui';

// Default configuration for BackendCrawlerConfig
const DEFAULT_BACKEND_CONFIG: BackendCrawlerConfig = {
  // Core settings
  start_page: 1,
  end_page: 10,
  concurrency: 5,
  delay_ms: 500,
  
  // Advanced settings
  page_range_limit: 50,
  product_list_retry_count: 3,
  product_detail_retry_count: 3,
  products_per_page: 20,
  auto_add_to_local_db: true,
  auto_status_check: true,
  crawler_type: 'full',

  // Batch processing
  batch_size: 10,
  batch_delay_ms: 1000,
  enable_batch_processing: true,
  batch_retry_limit: 3,

  // URLs
  base_url: '',
  matter_filter_url: '',
  
  // Timeouts
  page_timeout_ms: 30000,
  product_detail_timeout_ms: 30000,
  
  // Concurrency & Performance
  initial_concurrency: 5,
  detail_concurrency: 5,
  retry_concurrency: 2,
  min_request_delay_ms: 500,
  max_request_delay_ms: 1000,
  retry_start: 1000,
  retry_max: 3,
  cache_ttl_ms: 3600000,
  
  // Browser settings
  headless_browser: true,
  max_concurrent_tasks: 5,
  request_delay: 500,
  custom_user_agent: undefined,
  
  // Logging
  logging: {
    level: 'info',
    enable_stack_trace: true,
    enable_timestamp: true,
    components: {
      crawler: 'info',
      parser: 'info',
      network: 'info',
      database: 'info'
    }
  }
};

// 임시: 실제 구현에서는 store/actions로 분리
const getDefaultConfig = () => ({ ...DEFAULT_BACKEND_CONFIG });

const Settings = () => {
  const [form, setForm] = createSignal<BackendCrawlerConfig>(getDefaultConfig());
  const [showModal, setShowModal] = createSignal(false);
  const [toast, setToast] = createSignal<{ type: 'success' | 'error'; message: string } | null>(null);

  // 프리셋 적용
  const applyPreset = (preset: ConfigPreset) => {
    setForm({ ...form(), ...preset.config });
    setToast({ type: 'success', message: `프리셋 "${preset.name}" 적용됨` });
  };

  // 저장 (실제 구현에서는 API/store 연동)
  const handleSave = () => {
    setShowModal(false);
    setToast({ type: 'success', message: '설정이 저장되었습니다.' });
  };

  // 리셋
  const handleReset = () => {
    setForm(getDefaultConfig());
    setToast({ type: 'success', message: '기본값으로 초기화되었습니다.' });
  };

  // 입력 핸들러
  const handleChange = (key: keyof BackendCrawlerConfig, value: any) => {
    setForm({ ...form(), [key]: value });
  };

  return (
    <div class="settings-section">
      <h2>크롤러 설정</h2>
      <div class="preset-buttons">
        <For each={CONFIG_PRESETS}>{preset => (
          <Button variant="outline" onClick={() => applyPreset(preset)}>{preset.name}</Button>
        )}</For>
        <Button variant="ghost" onClick={handleReset}>기본값</Button>
      </div>
      <form class="settings-form" onSubmit={e => { e.preventDefault(); handleSave(); }}>
        <div class="form-row">
          <label>페이지 범위 제한
            <input type="number" value={form().page_range_limit} min={1} max={1000} onInput={e => handleChange('page_range_limit', +e.currentTarget.value)} />
          </label>
          <label>페이지당 제품 수
            <input type="number" value={form().products_per_page} min={1} max={100} onInput={e => handleChange('products_per_page', +e.currentTarget.value)} />
          </label>
          <label>동시 요청 수
            <input type="number" value={form().initial_concurrency} min={1} max={64} onInput={e => handleChange('initial_concurrency', +e.currentTarget.value)} />
          </label>
        </div>
        <div class="form-row">
          <label>기본 URL
            <input type="url" value={form().base_url} onInput={e => handleChange('base_url', e.currentTarget.value)} />
          </label>
          <label>Matter 필터 URL
            <input type="url" value={form().matter_filter_url} onInput={e => handleChange('matter_filter_url', e.currentTarget.value)} />
          </label>
        </div>
        <div class="form-row">
          <label>크롤러 타입
            <select value={form().crawler_type} onChange={e => handleChange('crawler_type', e.currentTarget.value)}>
              <option value="full">full</option>
              <option value="quick">quick</option>
            </select>
          </label>
          <label>헤드리스 브라우저
            <input type="checkbox" checked={form().headless_browser} onChange={e => handleChange('headless_browser', e.currentTarget.checked)} />
          </label>
        </div>
        <div class="form-actions">
          <Button type="submit" variant="primary">저장</Button>
        </div>
      </form>
      <Show when={toast()}>
        <Toast type={toast()!.type} message={toast()!.message} duration={2000} onClose={() => setToast(null)} />
      </Show>
      <Modal open={showModal()} onClose={() => setShowModal(false)} title="설정 저장">
        <div>설정을 저장하시겠습니까?</div>
        <div class="form-actions">
          <Button onClick={handleSave}>확인</Button>
          <Button variant="ghost" onClick={() => setShowModal(false)}>취소</Button>
        </div>
      </Modal>
    </div>
  );
};

export default Settings;
