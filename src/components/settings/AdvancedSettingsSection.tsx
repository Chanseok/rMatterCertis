/**
 * AdvancedSettingsSection - 고급 설정 카테고리별 섹션
 * 검증된 40개 설정을 6개 카테고리로 그룹핑
 */
import { Component, createSignal, Show } from "solid-js";
import { settingsState } from "../../stores/settingsStore";

// 카테고리 헤더 컴포넌트
const CategoryHeader: Component<{
  icon: string;
  title: string;
  count: number;
  dangerStats: { safe: number; caution: number; danger: number };
  expanded: boolean;
  onToggle: () => void;
}> = (props) => {
  return (
    <button
      type="button"
      class="flex items-center gap-3 w-full hover:bg-gray-50 rounded-lg p-3 transition-colors"
      onClick={props.onToggle}
    >
      <span class="text-xl">{props.icon}</span>
      <div class="flex-1 text-left">
        <div class="font-semibold text-gray-800">{props.title}</div>
        <div class="text-xs text-gray-500">
          설정 {props.count}개 •{" "}
          {props.dangerStats.safe > 0 && <span class="text-green-600">🟢 {props.dangerStats.safe}</span>}
          {props.dangerStats.safe > 0 && (props.dangerStats.caution > 0 || props.dangerStats.danger > 0) && " "}
          {props.dangerStats.caution > 0 && <span class="text-amber-600">🟡 {props.dangerStats.caution}</span>}
          {props.dangerStats.caution > 0 && props.dangerStats.danger > 0 && " "}
          {props.dangerStats.danger > 0 && <span class="text-red-600">🔴 {props.dangerStats.danger}</span>}
        </div>
      </div>
      <span class="text-gray-400">{props.expanded ? "▼" : "▶"}</span>
    </button>
  );
};

// 위험도 배지 컴포넌트
const DangerBadge: Component<{ level: "safe" | "caution" | "danger" }> = (props) => {
  const styles = {
    safe: "bg-green-100 text-green-700 border-green-300",
    caution: "bg-amber-100 text-amber-700 border-amber-300",
    danger: "bg-red-100 text-red-700 border-red-300",
  };
  const labels = {
    safe: "🟢 안전",
    caution: "⚠️ 주의",
    danger: "⚠️ 위험",
  };

  return (
    <span class={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium border ${styles[props.level]}`}>
      {labels[props.level]}
    </span>
  );
};

export const AdvancedSettingsSection: Component = () => {
  // 카테고리별 접기/펼치기
  const [showLogging, setShowLogging] = createSignal(false);
  const [showRetry, setShowRetry] = createSignal(false);
  const [showNetwork, setShowNetwork] = createSignal(false);
  const [showDatabase, setShowDatabase] = createSignal(false);
  const [showTiming, setShowTiming] = createSignal(false);
  const [showEngine, setShowEngine] = createSignal(false);

  return (
    <div class="space-y-4">
      {/* 📝 로깅 설정 (6개) */}
      <div class="bg-white/80 rounded-xl border border-gray-200 overflow-hidden">
        <CategoryHeader
          icon="📝"
          title="로깅 설정"
          count={6}
          dangerStats={{ safe: 3, caution: 3, danger: 0 }}
          expanded={showLogging()}
          onToggle={() => setShowLogging(!showLogging())}
        />
        <Show when={showLogging()}>
          <div class="p-4 space-y-4 bg-gray-50/50">
            {/* file_naming_strategy */}
            <div class="p-3 bg-amber-50 border border-amber-200 rounded-lg">
              <div class="flex items-start gap-2 mb-2">
                <DangerBadge level="caution" />
                <div class="flex-1">
                  <label class="text-sm font-medium text-gray-800">파일 이름 전략</label>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: unified (단일) 또는 daily (일별)
                  </div>
                </div>
              </div>
              <input
                type="text"
                class="w-full px-3 py-2 rounded-md bg-white border border-amber-300 focus:outline-none focus:ring-2 focus:ring-amber-400"
                value={settingsState.getNestedValue("user.logging.file_naming_strategy") || "unified"}
                onInput={(e) => settingsState.updateNestedField("user.logging.file_naming_strategy", e.currentTarget.value)}
                placeholder="unified | daily | timestamp"
              />
            </div>

            {/* max_files */}
            <div class="p-3 bg-amber-50 border border-amber-200 rounded-lg">
              <div class="flex items-start gap-2 mb-2">
                <DangerBadge level="caution" />
                <div class="flex-1">
                  <label class="text-sm font-medium text-gray-800">로그 보관 최대 개수</label>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: 5-10개
                  </div>
                </div>
              </div>
              <input
                type="number"
                class="w-32 px-3 py-2 rounded-md bg-white border border-amber-300 focus:outline-none focus:ring-2 focus:ring-amber-400"
                value={settingsState.getNestedValue("user.logging.max_files") || 5}
                min={1}
                max={50}
                onInput={(e) => settingsState.updateNestedField("user.logging.max_files", +e.currentTarget.value)}
              />
            </div>

            {/* auto_cleanup_logs */}
            <div class="p-3 bg-green-50 border border-green-200 rounded-lg">
              <label class="flex items-center gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  class="w-5 h-5 rounded border-gray-300 text-blue-600 focus:ring-2 focus:ring-blue-300"
                  checked={settingsState.getNestedValue("user.logging.auto_cleanup_logs") || false}
                  onChange={(e) => settingsState.updateNestedField("user.logging.auto_cleanup_logs", e.currentTarget.checked)}
                />
                <div class="flex-1">
                  <div class="flex items-center gap-2">
                    <DangerBadge level="safe" />
                    <span class="text-sm font-medium text-gray-800">종료 시 로그 자동 정리</span>
                  </div>
                  <div class="text-xs text-gray-500 mt-1">💡 권장: true (디스크 공간 관리)</div>
                </div>
              </label>
            </div>

            {/* keep_only_latest */}
            <div class="p-3 bg-green-50 border border-green-200 rounded-lg">
              <label class="flex items-center gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  class="w-5 h-5 rounded border-gray-300 text-blue-600 focus:ring-2 focus:ring-blue-300"
                  checked={settingsState.getNestedValue("user.logging.keep_only_latest") || false}
                  onChange={(e) => settingsState.updateNestedField("user.logging.keep_only_latest", e.currentTarget.checked)}
                />
                <div class="flex-1">
                  <div class="flex items-center gap-2">
                    <DangerBadge level="safe" />
                    <span class="text-sm font-medium text-gray-800">최신 로그만 유지</span>
                  </div>
                  <div class="text-xs text-gray-500 mt-1">💡 권장: false (여러 세션 로그 보관)</div>
                </div>
              </label>
            </div>

            {/* concise_startup */}
            <div class="p-3 bg-green-50 border border-green-200 rounded-lg">
              <label class="flex items-center gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  class="w-5 h-5 rounded border-gray-300 text-blue-600 focus:ring-2 focus:ring-blue-300"
                  checked={settingsState.getNestedValue("user.logging.concise_startup") ?? true}
                  onChange={(e) => settingsState.updateNestedField("user.logging.concise_startup", e.currentTarget.checked)}
                />
                <div class="flex-1">
                  <div class="flex items-center gap-2">
                    <DangerBadge level="safe" />
                    <span class="text-sm font-medium text-gray-800">간결한 시작 로그</span>
                  </div>
                  <div class="text-xs text-gray-500 mt-1">💡 권장: true (불필요한 로그 감소)</div>
                </div>
              </label>
            </div>

            {/* separate_frontend_backend */}
            <div class="p-3 bg-amber-50 border border-amber-200 rounded-lg">
              <label class="flex items-center gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  class="w-5 h-5 rounded border-gray-300 text-blue-600 focus:ring-2 focus:ring-blue-300"
                  checked={settingsState.getNestedValue("user.logging.separate_frontend_backend") ?? false}
                  onChange={(e) => settingsState.updateNestedField("user.logging.separate_frontend_backend", e.currentTarget.checked)}
                />
                <div class="flex-1">
                  <div class="flex items-center gap-2">
                    <DangerBadge level="caution" />
                    <span class="text-sm font-medium text-gray-800">프론트/백엔드 로그 분리</span>
                  </div>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: true (문제 추적 시 유용)
                    <br />
                    ⚠️ 주의: 로그 파일 2배 증가
                  </div>
                </div>
              </label>
            </div>
          </div>
        </Show>
      </div>

      {/* 🔄 재시도 정책 (4개) - 전체 위험 */}
      <div class="bg-white/80 rounded-xl border border-red-200 overflow-hidden">
        <CategoryHeader
          icon="🔄"
          title="재시도 정책"
          count={4}
          dangerStats={{ safe: 0, caution: 0, danger: 4 }}
          expanded={showRetry()}
          onToggle={() => setShowRetry(!showRetry())}
        />
        <Show when={showRetry()}>
          <div class="p-4 space-y-4 bg-red-50/30">
            <div class="p-3 bg-red-100 border border-red-300 rounded-lg">
              <div class="text-sm font-semibold text-red-800 mb-2">
                ⚠️⚠️⚠️ 전체 위험 구역 - 신중한 조정 필요!
              </div>
              <div class="text-xs text-red-700">
                과도한 재시도는 시스템 과부하, 크롤링 시간 증가, 자원 낭비를 유발합니다.
                <br />
                권장: 모든 재시도를 3-5회로 제한
              </div>
            </div>

            {/* batch_retry_limit */}
            <div class="p-3 bg-red-50 border border-red-300 rounded-lg">
              <div class="flex items-start gap-2 mb-2">
                <DangerBadge level="danger" />
                <div class="flex-1">
                  <label class="text-sm font-medium text-gray-800">배치 재시도 횟수</label>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: 3-5회
                    <br />
                    ⚠️ 위험: 5회 초과는 시스템 과부하
                  </div>
                </div>
              </div>
              <input
                type="number"
                class="w-32 px-3 py-2 rounded-md bg-white border border-red-300 focus:outline-none focus:ring-2 focus:ring-red-400"
                value={settingsState.getNestedValue("user.batch.batch_retry_limit") || 3}
                min={0}
                max={50}
                onInput={(e) => settingsState.updateNestedField("user.batch.batch_retry_limit", +e.currentTarget.value)}
              />
            </div>

            {/* product_list_retry_count */}
            <div class="p-3 bg-red-50 border border-red-300 rounded-lg">
              <div class="flex items-start gap-2 mb-2">
                <DangerBadge level="danger" />
                <div class="flex-1">
                  <label class="text-sm font-medium text-gray-800">목록 페이지 재시도 횟수</label>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: 3-5회 (기본값 9는 과도함!)
                    <br />
                    ⚠️ 위험: 높은 값은 느린 응답 시간
                  </div>
                </div>
              </div>
              <input
                type="number"
                class="w-32 px-3 py-2 rounded-md bg-white border border-red-300 focus:outline-none focus:ring-2 focus:ring-red-400"
                value={settingsState.getNestedValue("user.crawling.product_list_retry_count") || 9}
                min={0}
                max={50}
                onInput={(e) => settingsState.updateNestedField("user.crawling.product_list_retry_count", +e.currentTarget.value)}
              />
            </div>

            {/* product_detail_retry_count */}
            <div class="p-3 bg-red-50 border border-red-300 rounded-lg">
              <div class="flex items-start gap-2 mb-2">
                <DangerBadge level="danger" />
                <div class="flex-1">
                  <label class="text-sm font-medium text-gray-800">상세 페이지 재시도 횟수</label>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: 3-5회 (기본값 9는 과도함!)
                    <br />
                    ⚠️ 위험: 높은 값은 전체 시간 증가
                  </div>
                </div>
              </div>
              <input
                type="number"
                class="w-32 px-3 py-2 rounded-md bg-white border border-red-300 focus:outline-none focus:ring-2 focus:ring-red-400"
                value={settingsState.getNestedValue("user.crawling.product_detail_retry_count") || 9}
                min={0}
                max={50}
                onInput={(e) => settingsState.updateNestedField("user.crawling.product_detail_retry_count", +e.currentTarget.value)}
              />
            </div>

            {/* max_retries */}
            <div class="p-3 bg-red-50 border border-red-300 rounded-lg">
              <div class="flex items-start gap-2 mb-2">
                <DangerBadge level="danger" />
                <div class="flex-1">
                  <label class="text-sm font-medium text-gray-800">HTTP 요청 재시도 (워커)</label>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: 3-5회
                    <br />
                    ⚠️ 위험: 과도한 재시도는 자원 낭비
                  </div>
                </div>
              </div>
              <input
                type="number"
                class="w-32 px-3 py-2 rounded-md bg-white border border-red-300 focus:outline-none focus:ring-2 focus:ring-red-400"
                value={settingsState.getNestedValue("user.crawling.workers.max_retries") || 5}
                min={0}
                max={100}
                onInput={(e) => settingsState.updateNestedField("user.crawling.workers.max_retries", +e.currentTarget.value)}
              />
            </div>
          </div>
        </Show>
      </div>

      {/* 🌐 네트워크 및 워커 (5개) */}
      <div class="bg-white/80 rounded-xl border border-gray-200 overflow-hidden">
        <CategoryHeader
          icon="🌐"
          title="네트워크 및 워커"
          count={5}
          dangerStats={{ safe: 2, caution: 2, danger: 1 }}
          expanded={showNetwork()}
          onToggle={() => setShowNetwork(!showNetwork())}
        />
        <Show when={showNetwork()}>
          <div class="p-4 space-y-4 bg-gray-50/50">
            {/* user_agent */}
            <div class="p-3 bg-green-50 border border-green-200 rounded-lg">
              <div class="flex items-start gap-2 mb-2">
                <DangerBadge level="safe" />
                <div class="flex-1">
                  <label class="text-sm font-medium text-gray-800">User-Agent</label>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: 실제 브라우저와 유사한 값
                  </div>
                </div>
              </div>
              <input
                type="text"
                class="w-full px-3 py-2 rounded-md bg-white border border-green-300 focus:outline-none focus:ring-2 focus:ring-green-400 text-sm"
                value={settingsState.getNestedValue("user.crawling.workers.user_agent") ?? ""}
                onInput={(e) => settingsState.updateNestedField("user.crawling.workers.user_agent", e.currentTarget.value)}
                placeholder="Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)..."
              />
            </div>

            {/* user_agent_sync */}
            <div class="p-3 bg-green-50 border border-green-200 rounded-lg">
              <div class="flex items-start gap-2 mb-2">
                <DangerBadge level="safe" />
                <div class="flex-1">
                  <label class="text-sm font-medium text-gray-800">동기화 전용 User-Agent (선택)</label>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: 기본값과 다른 값 (추적 용이)
                  </div>
                </div>
              </div>
              <input
                type="text"
                class="w-full px-3 py-2 rounded-md bg-white border border-green-300 focus:outline-none focus:ring-2 focus:ring-green-400 text-sm"
                value={settingsState.getNestedValue("user.crawling.workers.user_agent_sync") ?? ""}
                onInput={(e) => settingsState.updateNestedField("user.crawling.workers.user_agent_sync", e.currentTarget.value)}
                placeholder="특정 동기화 작업용 (비워두면 기본값 사용)"
              />
            </div>

            {/* follow_redirects */}
            <div class="p-3 bg-amber-50 border border-amber-200 rounded-lg">
              <label class="flex items-center gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  class="w-5 h-5 rounded border-gray-300 text-blue-600 focus:ring-2 focus:ring-blue-300"
                  checked={settingsState.getNestedValue("user.crawling.workers.follow_redirects") ?? true}
                  onChange={(e) => settingsState.updateNestedField("user.crawling.workers.follow_redirects", e.currentTarget.checked)}
                />
                <div class="flex-1">
                  <div class="flex items-center gap-2">
                    <DangerBadge level="caution" />
                    <span class="text-sm font-medium text-gray-800">HTTP 리다이렉트 자동 추적</span>
                  </div>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: true (대부분의 경우)
                    <br />
                    ⚠️ 주의: 무한 리다이렉트 가능성
                  </div>
                </div>
              </label>
            </div>

            {/* respect_robots_txt */}
            <div class="p-3 bg-amber-50 border border-amber-200 rounded-lg">
              <label class="flex items-center gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  class="w-5 h-5 rounded border-gray-300 text-blue-600 focus:ring-2 focus:ring-blue-300"
                  checked={settingsState.getNestedValue("user.crawling.workers.respect_robots_txt") ?? false}
                  onChange={(e) => settingsState.updateNestedField("user.crawling.workers.respect_robots_txt", e.currentTarget.checked)}
                />
                <div class="flex-1">
                  <div class="flex items-center gap-2">
                    <DangerBadge level="caution" />
                    <span class="text-sm font-medium text-gray-800">robots.txt 준수</span>
                  </div>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: false (내부 테스트), true (공개 크롤링)
                    <br />
                    ⚠️ 주의: true 시 일부 페이지 크롤링 불가
                  </div>
                </div>
              </label>
            </div>

            {/* db_batch_size */}
            <div class="p-3 bg-red-50 border border-red-300 rounded-lg">
              <div class="flex items-start gap-2 mb-2">
                <DangerBadge level="danger" />
                <div class="flex-1">
                  <label class="text-sm font-medium text-gray-800">DB 배치 크기 (레코드 수)</label>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: 100-500개
                    <br />
                    ⚠️ 위험: 1000+ 메모리 부족, 10 이하는 비효율
                  </div>
                </div>
              </div>
              <input
                type="number"
                class="w-32 px-3 py-2 rounded-md bg-white border border-red-300 focus:outline-none focus:ring-2 focus:ring-red-400"
                value={settingsState.getNestedValue("user.crawling.workers.db_batch_size") || 100}
                min={1}
                max={10000}
                onInput={(e) => settingsState.updateNestedField("user.crawling.workers.db_batch_size", +e.currentTarget.value)}
              />
            </div>
          </div>
        </Show>
      </div>

      {/* 💾 데이터베이스 (1개) - 매우 중요 */}
      <div class="bg-white/80 rounded-xl border border-red-200 overflow-hidden">
        <CategoryHeader
          icon="💾"
          title="데이터베이스"
          count={1}
          dangerStats={{ safe: 0, caution: 0, danger: 1 }}
          expanded={showDatabase()}
          onToggle={() => setShowDatabase(!showDatabase())}
        />
        <Show when={showDatabase()}>
          <div class="p-4 space-y-4 bg-red-50/30">
            <div class="p-3 bg-red-100 border border-red-300 rounded-lg">
              <div class="text-sm font-semibold text-red-800 mb-2">
                ⚠️⚠️⚠️ 매우 중요한 설정!
              </div>
              <div class="text-xs text-red-700">
                SQLite는 동시성 제한이 있습니다. 너무 높은 값은 DB 락 경합을 유발합니다.
                <br />
                <strong>권장 조합:</strong>
                <br />
                • 저사양: db_batch_size=100, db_max_concurrency=3
                <br />
                • 권장: db_batch_size=200, db_max_concurrency=5
                <br />
                • 고사양: db_batch_size=500, db_max_concurrency=8
              </div>
            </div>

            {/* db_max_concurrency */}
            <div class="p-3 bg-red-50 border border-red-300 rounded-lg">
              <div class="flex items-start gap-2 mb-2">
                <DangerBadge level="danger" />
                <div class="flex-1">
                  <label class="text-sm font-medium text-gray-800">DB 최대 동시 실행 수</label>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: 3-10개
                    <br />
                    ⚠️ 위험: 20+ 매우 위험 (DB 락 경합)
                  </div>
                </div>
              </div>
              <input
                type="number"
                class="w-32 px-3 py-2 rounded-md bg-white border border-red-300 focus:outline-none focus:ring-2 focus:ring-red-400"
                value={settingsState.getNestedValue("user.crawling.workers.db_max_concurrency") || 5}
                min={1}
                max={10000}
                onInput={(e) => settingsState.updateNestedField("user.crawling.workers.db_max_concurrency", +e.currentTarget.value)}
              />
            </div>
          </div>
        </Show>
      </div>

      {/* ⏱️ 타이밍 및 스케줄링 (5개) */}
      <div class="bg-white/80 rounded-xl border border-gray-200 overflow-hidden">
        <CategoryHeader
          icon="⏱️"
          title="타이밍 및 스케줄링"
          count={5}
          dangerStats={{ safe: 1, caution: 2, danger: 2 }}
          expanded={showTiming()}
          onToggle={() => setShowTiming(!showTiming())}
        />
        <Show when={showTiming()}>
          <div class="p-4 space-y-4 bg-gray-50/50">
            {/* scheduler_interval_ms */}
            <div class="p-3 bg-amber-50 border border-amber-200 rounded-lg">
              <div class="flex items-start gap-2 mb-2">
                <DangerBadge level="caution" />
                <div class="flex-1">
                  <label class="text-sm font-medium text-gray-800">스케줄러 실행 간격 (ms)</label>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: 100-1000ms
                    <br />
                    ⚠️ 주의: 너무 짧으면 CPU 과부하
                  </div>
                </div>
              </div>
              <input
                type="number"
                class="w-32 px-3 py-2 rounded-md bg-white border border-amber-300 focus:outline-none focus:ring-2 focus:ring-amber-400"
                value={settingsState.getNestedValue("user.crawling.timing.scheduler_interval_ms") || 100}
                min={10}
                max={600000}
                onInput={(e) => settingsState.updateNestedField("user.crawling.timing.scheduler_interval_ms", +e.currentTarget.value)}
              />
            </div>

            {/* shutdown_timeout_seconds */}
            <div class="p-3 bg-amber-50 border border-amber-200 rounded-lg">
              <div class="flex items-start gap-2 mb-2">
                <DangerBadge level="caution" />
                <div class="flex-1">
                  <label class="text-sm font-medium text-gray-800">종료 타임아웃 (초)</label>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: 10-30초
                    <br />
                    ⚠️ 주의: 너무 짧으면 데이터 손실 가능
                  </div>
                </div>
              </div>
              <input
                type="number"
                class="w-32 px-3 py-2 rounded-md bg-white border border-amber-300 focus:outline-none focus:ring-2 focus:ring-amber-400"
                value={settingsState.getNestedValue("user.crawling.timing.shutdown_timeout_seconds") || 30}
                min={1}
                max={3600}
                onInput={(e) => settingsState.updateNestedField("user.crawling.timing.shutdown_timeout_seconds", +e.currentTarget.value)}
              />
            </div>

            {/* stats_interval_seconds */}
            <div class="p-3 bg-green-50 border border-green-200 rounded-lg">
              <div class="flex items-start gap-2 mb-2">
                <DangerBadge level="safe" />
                <div class="flex-1">
                  <label class="text-sm font-medium text-gray-800">통계 출력 간격 (초)</label>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: 5-30초
                  </div>
                </div>
              </div>
              <input
                type="number"
                class="w-32 px-3 py-2 rounded-md bg-white border border-green-300 focus:outline-none focus:ring-2 focus:ring-green-400"
                value={settingsState.getNestedValue("user.crawling.timing.stats_interval_seconds") || 10}
                min={1}
                max={3600}
                onInput={(e) => settingsState.updateNestedField("user.crawling.timing.stats_interval_seconds", +e.currentTarget.value)}
              />
            </div>

            {/* retry_delay_ms */}
            <div class="p-3 bg-red-50 border border-red-300 rounded-lg">
              <div class="flex items-start gap-2 mb-2">
                <DangerBadge level="danger" />
                <div class="flex-1">
                  <label class="text-sm font-medium text-gray-800">재시도 대기 시간 (ms)</label>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: 1000-3000ms
                    <br />
                    ⚠️ 위험: 너무 짧으면 서버 부하, 너무 길면 시간 증가
                  </div>
                </div>
              </div>
              <input
                type="number"
                class="w-32 px-3 py-2 rounded-md bg-white border border-red-300 focus:outline-none focus:ring-2 focus:ring-red-400"
                value={settingsState.getNestedValue("user.crawling.timing.retry_delay_ms") || 2000}
                min={0}
                max={600000}
                onInput={(e) => settingsState.updateNestedField("user.crawling.timing.retry_delay_ms", +e.currentTarget.value)}
              />
            </div>

            {/* operation_timeout_seconds */}
            <div class="p-3 bg-red-50 border border-red-300 rounded-lg">
              <div class="flex items-start gap-2 mb-2">
                <DangerBadge level="danger" />
                <div class="flex-1">
                  <label class="text-sm font-medium text-gray-800">전체 작업 타임아웃 (초)</label>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: 60-300초
                    <br />
                    ⚠️ 위험: 너무 짧으면 작업 중단
                  </div>
                </div>
              </div>
              <input
                type="number"
                class="w-32 px-3 py-2 rounded-md bg-white border border-red-300 focus:outline-none focus:ring-2 focus:ring-red-400"
                value={settingsState.getNestedValue("user.crawling.timing.operation_timeout_seconds") || 300}
                min={1}
                max={3600}
                onInput={(e) => settingsState.updateNestedField("user.crawling.timing.operation_timeout_seconds", +e.currentTarget.value)}
              />
            </div>
          </div>
        </Show>
      </div>

      {/* ⚙️ 엔진 내부 (8개) - 고급 사용자 전용 */}
      <div class="bg-white/80 rounded-xl border border-red-200 overflow-hidden">
        <CategoryHeader
          icon="⚙️"
          title="엔진 내부 (고급)"
          count={8}
          dangerStats={{ safe: 0, caution: 3, danger: 5 }}
          expanded={showEngine()}
          onToggle={() => setShowEngine(!showEngine())}
        />
        <Show when={showEngine()}>
          <div class="p-4 space-y-4 bg-red-50/20">
            <div class="p-3 bg-orange-100 border border-orange-300 rounded-lg">
              <div class="text-sm font-semibold text-orange-800 mb-2">
                ⚠️ 고급 사용자 전용 - 이해 없이 변경 금지!
              </div>
              <div class="text-xs text-orange-700">
                이 설정들은 크롤링 엔진의 내부 동작을 제어합니다.
                잘못된 값은 크롤링 완전 실패를 유발할 수 있습니다.
              </div>
            </div>

            {/* last_page_search_start */}
            <div class="p-3 bg-red-50 border border-red-300 rounded-lg">
              <div class="flex items-start gap-2 mb-2">
                <DangerBadge level="danger" />
                <div class="flex-1">
                  <label class="text-sm font-medium text-gray-800">마지막 페이지 탐색 시작점</label>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: 100-500 (사이트마다 다름)
                    <br />
                    ⚠️ 위험: 잘못된 값은 불필요한 요청 증가
                  </div>
                </div>
              </div>
              <input
                type="number"
                class="w-32 px-3 py-2 rounded-md bg-white border border-red-300 focus:outline-none focus:ring-2 focus:ring-red-400"
                value={settingsState.getNestedValue("advanced.last_page_search_start") || 100}
                min={1}
                max={20000}
                onInput={(e) => settingsState.updateNestedField("advanced.last_page_search_start", +e.currentTarget.value)}
              />
            </div>

            {/* max_search_attempts */}
            <div class="p-3 bg-amber-50 border border-amber-200 rounded-lg">
              <div class="flex items-start gap-2 mb-2">
                <DangerBadge level="caution" />
                <div class="flex-1">
                  <label class="text-sm font-medium text-gray-800">최대 탐색 시도 횟수</label>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: 5-10회
                    <br />
                    ⚠️ 주의: 너무 많으면 시간 낭비
                  </div>
                </div>
              </div>
              <input
                type="number"
                class="w-32 px-3 py-2 rounded-md bg-white border border-amber-300 focus:outline-none focus:ring-2 focus:ring-amber-400"
                value={settingsState.getNestedValue("advanced.max_search_attempts") || 10}
                min={1}
                max={100}
                onInput={(e) => settingsState.updateNestedField("advanced.max_search_attempts", +e.currentTarget.value)}
              />
            </div>

            {/* request_timeout_seconds */}
            <div class="p-3 bg-red-50 border border-red-300 rounded-lg">
              <div class="flex items-start gap-2 mb-2">
                <DangerBadge level="danger" />
                <div class="flex-1">
                  <label class="text-sm font-medium text-gray-800">고급 요청 타임아웃 (초)</label>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: 30-60초
                    <br />
                    ⚠️ 위험: operation_timeout_seconds와 중복 가능
                  </div>
                </div>
              </div>
              <input
                type="number"
                class="w-32 px-3 py-2 rounded-md bg-white border border-red-300 focus:outline-none focus:ring-2 focus:ring-red-400"
                value={settingsState.getNestedValue("advanced.request_timeout_seconds") || 60}
                min={1}
                max={600}
                onInput={(e) => settingsState.updateNestedField("advanced.request_timeout_seconds", +e.currentTarget.value)}
              />
            </div>

            {/* product_selectors */}
            <div class="p-3 bg-red-50 border border-red-300 rounded-lg">
              <div class="flex items-start gap-2 mb-2">
                <DangerBadge level="danger" />
                <div class="flex-1">
                  <label class="text-sm font-medium text-gray-800">상품 CSS 선택자 (쉼표 구분)</label>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: 사이트 구조에 맞는 정확한 선택자
                    <br />
                    ⚠️ <strong class="text-red-700">매우 위험:</strong> 잘못된 선택자는 크롤링 완전 실패!
                  </div>
                </div>
              </div>
              <input
                type="text"
                class="w-full px-3 py-2 rounded-md bg-white border border-red-300 focus:outline-none focus:ring-2 focus:ring-red-400 text-sm font-mono"
                value={(settingsState.getNestedValue("advanced.product_selectors") || []).join(", ")}
                onInput={(e) =>
                  settingsState.updateNestedField(
                    "advanced.product_selectors",
                    e.currentTarget.value.split(",").map((s) => s.trim()).filter(Boolean)
                  )
                }
                placeholder=".product-item, .product-card, article.product"
              />
            </div>

            {/* failure_threshold */}
            <div class="p-3 bg-red-50 border border-red-300 rounded-lg">
              <div class="flex items-start gap-2 mb-2">
                <DangerBadge level="danger" />
                <div class="flex-1">
                  <label class="text-sm font-medium text-gray-800">연속 실패 허용 임계값</label>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: 5-10회
                    <br />
                    ⚠️ 위험: 너무 낮으면 조기 종료, 너무 높으면 무의미한 재시도
                  </div>
                </div>
              </div>
              <input
                type="number"
                class="w-32 px-3 py-2 rounded-md bg-white border border-red-300 focus:outline-none focus:ring-2 focus:ring-red-400"
                value={settingsState.getNestedValue("advanced.failure_policy.failure_threshold") ?? 5}
                min={0}
                max={100}
                onInput={(e) => settingsState.updateNestedField("advanced.failure_policy.failure_threshold", +e.currentTarget.value)}
              />
            </div>

            {/* removal_grace_secs */}
            <div class="p-3 bg-amber-50 border border-amber-200 rounded-lg">
              <div class="flex items-start gap-2 mb-2">
                <DangerBadge level="caution" />
                <div class="flex-1">
                  <label class="text-sm font-medium text-gray-800">실패 세션 정리 유예 (초)</label>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: 10-30초
                    <br />
                    ⚠️ 주의: 너무 짧으면 복구 기회 없음
                  </div>
                </div>
              </div>
              <input
                type="number"
                class="w-32 px-3 py-2 rounded-md bg-white border border-amber-300 focus:outline-none focus:ring-2 focus:ring-amber-400"
                value={settingsState.getNestedValue("advanced.failure_policy.removal_grace_secs") ?? 10}
                min={0}
                max={3600}
                onInput={(e) => settingsState.updateNestedField("advanced.failure_policy.removal_grace_secs", +e.currentTarget.value)}
              />
            </div>

            {/* override_config_limit */}
            <div class="p-3 bg-amber-50 border border-amber-200 rounded-lg">
              <label class="flex items-center gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  class="w-5 h-5 rounded border-gray-300 text-blue-600 focus:ring-2 focus:ring-blue-300"
                  checked={settingsState.getNestedValue("user.crawling.intelligent_mode.override_config_limit") ?? false}
                  onChange={(e) => settingsState.updateNestedField("user.crawling.intelligent_mode.override_config_limit", e.currentTarget.checked)}
                />
                <div class="flex-1">
                  <div class="flex items-center gap-2">
                    <DangerBadge level="caution" />
                    <span class="text-sm font-medium text-gray-800">지능형 모드 설정 오버라이드 허용</span>
                  </div>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: true (지능형 계산 신뢰 시)
                    <br />
                    ⚠️ 주의: 예상보다 많은 페이지 크롤링 가능
                  </div>
                </div>
              </label>
            </div>

            {/* max_range_limit */}
            <div class="p-3 bg-red-50 border border-red-300 rounded-lg">
              <div class="flex items-start gap-2 mb-2">
                <DangerBadge level="danger" />
                <div class="flex-1">
                  <label class="text-sm font-medium text-gray-800">지능형 모드 최대 페이지 범위</label>
                  <div class="text-xs text-gray-500 mt-1">
                    💡 권장: 500-1000
                    <br />
                    ⚠️ 위험: 너무 크면 과도한 크롤링
                  </div>
                </div>
              </div>
              <input
                type="number"
                class="w-32 px-3 py-2 rounded-md bg-white border border-red-300 focus:outline-none focus:ring-2 focus:ring-red-400"
                value={settingsState.getNestedValue("user.crawling.intelligent_mode.max_range_limit") ?? 1000}
                min={1}
                max={10000}
                onInput={(e) => settingsState.updateNestedField("user.crawling.intelligent_mode.max_range_limit", +e.currentTarget.value)}
              />
            </div>
          </div>
        </Show>
      </div>
    </div>
  );
};
