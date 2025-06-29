import { createSignal, Show, For } from 'solid-js';
import { CrawlerConfig, ConfigPreset, CONFIG_PRESETS, DEFAULT_CRAWLER_CONFIG } from '../../../types/crawling';
import { Button, Modal, Toast } from '../../ui';

// 임시: 실제 구현에서는 store/actions로 분리
const getDefaultConfig = () => ({ ...DEFAULT_CRAWLER_CONFIG });

const Settings = () => {
  const [form, setForm] = createSignal<CrawlerConfig>(getDefaultConfig());
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
  const handleChange = (key: keyof CrawlerConfig, value: any) => {
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
            <input type="number" value={form().pageRangeLimit} min={1} max={1000} onInput={e => handleChange('pageRangeLimit', +e.currentTarget.value)} />
          </label>
          <label>페이지당 제품 수
            <input type="number" value={form().productsPerPage} min={1} max={100} onInput={e => handleChange('productsPerPage', +e.currentTarget.value)} />
          </label>
          <label>동시 요청 수
            <input type="number" value={form().initialConcurrency} min={1} max={64} onInput={e => handleChange('initialConcurrency', +e.currentTarget.value)} />
          </label>
        </div>
        <div class="form-row">
          <label>기본 URL
            <input type="url" value={form().baseUrl} onInput={e => handleChange('baseUrl', e.currentTarget.value)} />
          </label>
          <label>Matter 필터 URL
            <input type="url" value={form().matterFilterUrl} onInput={e => handleChange('matterFilterUrl', e.currentTarget.value)} />
          </label>
        </div>
        <div class="form-row">
          <label>크롤러 타입
            <select value={form().crawlerType} onChange={e => handleChange('crawlerType', e.currentTarget.value)}>
              <option value="axios">axios</option>
              <option value="playwright">playwright</option>
            </select>
          </label>
          <label>헤드리스 브라우저
            <input type="checkbox" checked={form().headlessBrowser} onChange={e => handleChange('headlessBrowser', e.currentTarget.checked)} />
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
