import { createSignal, For } from "solid-js";
import { CrawlingService } from "../services/crawlingService";
import type { StartCrawlingRequest } from "../types/crawling";

interface CrawlingFormProps {
  onSuccess: (sessionId: string) => void;
  onCancel: () => void;
}

export function CrawlingForm(props: CrawlingFormProps) {
  const [startUrl, setStartUrl] = createSignal("https://certification.csa-iot.org");
  const [targetDomains, setTargetDomains] = createSignal("certification.csa-iot.org");
  const [maxPages, setMaxPages] = createSignal(100);
  const [concurrentRequests, setConcurrentRequests] = createSignal(3);
  const [delayMs, setDelayMs] = createSignal(1000);
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const presetConfigs = [
    {
      name: "CSA-IoT 인증 사이트 (기본)",
      startUrl: "https://certification.csa-iot.org",
      domains: "certification.csa-iot.org",
      maxPages: 100,
    },
    {
      name: "CSA-IoT 인증 사이트 (전체)",
      startUrl: "https://certification.csa-iot.org",
      domains: "certification.csa-iot.org",
      maxPages: 500,
    },
    {
      name: "CSA-IoT 제품 목록",
      startUrl: "https://certification.csa-iot.org/products",
      domains: "certification.csa-iot.org",
      maxPages: 200,
    },
  ];

  const applyPreset = (preset: typeof presetConfigs[0]) => {
    setStartUrl(preset.startUrl);
    setTargetDomains(preset.domains);
    setMaxPages(preset.maxPages);
  };

  const handleSubmit = async (e: Event) => {
    e.preventDefault();
    
    if (!startUrl().trim()) {
      setError("시작 URL을 입력해주세요.");
      return;
    }

    try {
      setLoading(true);
      setError(null);

      const request: StartCrawlingRequest = {
        start_url: startUrl().trim(),
        target_domains: targetDomains()
          .split(",")
          .map(domain => domain.trim())
          .filter(domain => domain.length > 0),
        max_pages: maxPages(),
        concurrent_requests: concurrentRequests(),
        delay_ms: delayMs(),
      };

      const sessionId = await CrawlingService.startCrawling(request);
      props.onSuccess(sessionId);
    } catch (err) {
      console.error("Failed to start crawling:", err);
      setError(err instanceof Error ? err.message : "크롤링 시작에 실패했습니다.");
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
                    class="btn btn-outline"
                    onClick={() => applyPreset(preset)}
                  >
                    {preset.name}
                  </button>
                )}
              </For>
            </div>
          </div>

          {/* Start URL */}
          <div class="form-group">
            <label for="startUrl">시작 URL *</label>
            <input
              id="startUrl"
              type="url"
              value={startUrl()}
              onInput={(e) => setStartUrl(e.currentTarget.value)}
              placeholder="https://example.com"
              required
            />
          </div>

          {/* Target Domains */}
          <div class="form-group">
            <label for="targetDomains">대상 도메인</label>
            <input
              id="targetDomains"
              type="text"
              value={targetDomains()}
              onInput={(e) => setTargetDomains(e.currentTarget.value)}
              placeholder="example.com, subdomain.example.com"
            />
            <small>여러 도메인은 쉼표로 구분하세요. 비워두면 모든 도메인을 허용합니다.</small>
          </div>

          {/* Max Pages */}
          <div class="form-group">
            <label for="maxPages">최대 페이지 수</label>
            <input
              id="maxPages"
              type="number"
              min="1"
              max="10000"
              value={maxPages()}
              onInput={(e) => setMaxPages(parseInt(e.currentTarget.value) || 100)}
            />
          </div>

          {/* Concurrent Requests */}
          <div class="form-group">
            <label for="concurrentRequests">동시 요청 수</label>
            <input
              id="concurrentRequests"
              type="number"
              min="1"
              max="10"
              value={concurrentRequests()}
              onInput={(e) => setConcurrentRequests(parseInt(e.currentTarget.value) || 3)}
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
              class="btn btn-secondary"
              onClick={props.onCancel}
              disabled={loading()}
            >
              취소
            </button>
            <button
              type="submit"
              class="btn btn-primary"
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
