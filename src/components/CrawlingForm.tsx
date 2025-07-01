import { createSignal, For } from "solid-js";
import { crawlerStore } from '../stores/crawlerStore';
import { uiStore } from '../stores/uiStore';
import type { CrawlingConfig } from "../types/crawling";

interface CrawlingFormProps {
  onSuccess: () => void;
  onCancel: () => void;
}

export function CrawlingForm(props: CrawlingFormProps) {
  const [startPage, setStartPage] = createSignal(1);
  const [endPage, setEndPage] = createSignal(100);
  const [concurrency, setConcurrency] = createSignal(3);
  const [delayMs, setDelayMs] = createSignal(1000);
  // 아래 변수들은 현재 미사용되지만 향후 기능 확장을 위해 유지합니다
  const [autoAddToDb] = createSignal(true);
  const [retryMax] = createSignal(3);
  const [pageTimeout] = createSignal(30000);
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const presetConfigs = [
    {
      name: "CSA-IoT 인증 사이트 (기본)",
      startPage: 1,
      endPage: 100,
      concurrency: 3,
    },
    {
      name: "CSA-IoT 인증 사이트 (전체)",
      startPage: 1,
      endPage: 500,
      concurrency: 5,
    },
    {
      name: "CSA-IoT 제품 목록 (빠른 수집)",
      startPage: 1,
      endPage: 200,
      concurrency: 8,
    },
  ];

  const applyPreset = (preset: typeof presetConfigs[0]) => {
    setStartPage(preset.startPage);
    setEndPage(preset.endPage);
    setConcurrency(preset.concurrency);
  };

  const handleSubmit = async (e: Event) => {
    e.preventDefault();
    
    if (startPage() <= 0 || endPage() <= 0 || startPage() > endPage()) {
      setError("페이지 범위를 올바르게 설정해주세요.");
      return;
    }

    try {
      setLoading(true);
      setError(null);

      const config: CrawlingConfig = {
        start_page: startPage(),
        end_page: endPage(),
        concurrency: concurrency(),
        delay_ms: delayMs(),
        auto_add_to_local_db: autoAddToDb(),
        retry_max: retryMax(),
        page_timeout_ms: pageTimeout(),
      };

      await crawlerStore.startCrawling(config);
      uiStore.showSuccess('크롤링이 시작되었습니다', '시작 성공');
      props.onSuccess();
    } catch (err) {
      console.error("Failed to start crawling:", err);
      const errorMessage = err instanceof Error ? err.message : "크롤링 시작에 실패했습니다.";
      setError(errorMessage);
      uiStore.showError(errorMessage, '크롤링 시작 실패');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div class="crawling-form-overlay">
      <div class="crawling-form">
        <div class="form-header">
          <h2>새 크롤링 세션 시작</h2>
          <button class="close-btn" onClick={props.onCancel}>×</button>
        </div>

        <form onSubmit={handleSubmit}>
          {/* Preset Configurations */}
          <div class="form-section">
            <label>빠른 설정:</label>
            <div class="preset-buttons">
              <For each={presetConfigs}>
                {(preset) => (
                  <button
                    type="button"
                    class="inline-flex items-center justify-center px-4 py-2 text-sm font-medium rounded-lg border border-gray-300 dark:border-gray-600 bg-white/80 dark:bg-gray-800/80 hover:bg-gray-50 dark:hover:bg-gray-700/80 text-gray-700 dark:text-gray-200 transition-all duration-200"
                    onClick={() => applyPreset(preset)}
                  >
                    {preset.name}
                  </button>
                )}
              </For>
            </div>
          </div>

          {/* Start Page */}
          <div class="form-group">
            <label for="startPage">시작 페이지 *</label>
            <input
              id="startPage"
              type="number"
              min="1"
              value={startPage()}
              onInput={(e) => setStartPage(parseInt(e.currentTarget.value) || 1)}
              placeholder="1"
              required
            />
          </div>

          {/* End Page */}
          <div class="form-group">
            <label for="endPage">종료 페이지 *</label>
            <input
              id="endPage"
              type="number"
              min="1"
              max="10000"
              value={endPage()}
              onInput={(e) => setEndPage(parseInt(e.currentTarget.value) || 100)}
              placeholder="100"
              required
            />
            <small>수집할 페이지의 범위를 지정하세요.</small>
          </div>

          {/* Concurrency */}
          <div class="form-group">
            <label for="concurrency">동시 처리 수</label>
            <input
              id="concurrency"
              type="number"
              min="1"
              max="20"
              value={concurrency()}
              onInput={(e) => setConcurrency(parseInt(e.currentTarget.value) || 3)}
            />
          </div>
          <div class="form-group">
            <label for="concurrentRequests">동시 요청 수</label>
            <input
              id="concurrentRequests"
              type="number"
              min="1"
              max="10"
              value={concurrency()}
              onInput={(e) => setConcurrency(parseInt(e.currentTarget.value) || 3)}
            />
            <small>너무 높게 설정하면 서버에 부하를 줄 수 있습니다.</small>
          </div>

          {/* Delay */}
          <div class="form-group">
            <label for="delayMs">요청 간 지연 시간 (ms)</label>
            <input
              id="delayMs"
              type="number"
              min="0"
              max="10000"
              value={delayMs()}
              onInput={(e) => setDelayMs(parseInt(e.currentTarget.value) || 1000)}
            />
            <small>요청 사이의 지연 시간입니다. 예의를 지켜주세요.</small>
          </div>

          {error() && (
            <div class="error-message">
              ❌ {error()}
            </div>
          )}

          <div class="form-actions">
            <button
              type="button"
              class="inline-flex items-center justify-center px-6 py-3 text-sm font-semibold rounded-xl bg-gradient-to-r from-gray-600 to-gray-700 hover:from-gray-700 hover:to-gray-800 text-white transition-all duration-300 disabled:opacity-50 disabled:cursor-not-allowed"
              onClick={props.onCancel}
              disabled={loading()}
            >
              취소
            </button>
            <button
              type="submit"
              class="btn-primary"
              disabled={loading()}
            >
              {loading() ? "시작 중..." : "크롤링 시작"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
