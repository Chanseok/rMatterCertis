/**
 * SettingsTab - 설정 탭 컴포넌트
 * settingsStore를 기반으로 한 실제 백엔드 연동 설정 UI
 */
import { Component, createSignal, onMount, For, Show } from "solid-js";
import { emit } from "@tauri-apps/api/event";
import { settingsState } from "../../stores/settingsStore";
import { CONFIG_PRESETS } from "../../types/config";

export const SettingsTab: Component = () => {
  const [saveMessage, setSaveMessage] = createSignal<string>("");
  const [showMessage, setShowMessage] = createSignal(false);
  const [showModal, setShowModal] = createSignal(false);
  const [showAdvanced, setShowAdvanced] = createSignal(false);
  const [showAppManaged, setShowAppManaged] = createSignal(false);

  onMount(async () => {
    console.log("⚙️ SettingsTab 컴포넌트 로드됨");
    await settingsState.loadSettings();
  });

  const handleSave = async () => {
    try {
      await settingsState.saveSettings();
      setSaveMessage("✅ 설정이 저장되었습니다");
      setShowMessage(true);
      // 설정 저장 성공 시, 전역 이벤트로 알림을 보냄
      try {
        await emit("settings-updated", { at: Date.now() });
      } catch (e) {
        console.warn("[SettingsTab] settings-updated emit failed", e);
      }
      setTimeout(() => setShowMessage(false), 3000);
    } catch (error) {
      setSaveMessage("❌ 설정 저장에 실패했습니다");
      setShowMessage(true);
      setTimeout(() => setShowMessage(false), 3000);
    }
  };

  const handleReset = async () => {
    if (confirm("모든 설정을 기본값으로 초기화하시겠습니까?")) {
      await settingsState.resetToDefaults();
      setSaveMessage("✅ 기본값으로 초기화되었습니다");
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
    <div class="min-h-screen bg-gradient-to-br from-slate-50 via-gray-50 to-blue-50 p-6">
      <div class="w-full max-w-7xl mx-auto space-y-6">
        {/* Header Card */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6 flex items-center justify-between">
          <h2 class="text-2xl md:text-3xl font-bold text-gray-800 flex items-center gap-2">
            <span>⚙️</span>
            <span class="bg-gradient-to-r from-blue-600 via-indigo-600 to-blue-700 bg-clip-text text-transparent">
              애플리케이션 설정
            </span>
          </h2>
          <div class="flex gap-2">
            <button
              class="px-4 py-2 rounded-lg text-gray-700 bg-white border border-gray-200 hover:bg-gray-50 transition-colors duration-200"
              onClick={handleReset}
            >
              기본값으로 초기화
            </button>
            <button
              class={`px-4 py-2 rounded-lg text-white shadow transition-all duration-200 ${
                settingsState.isLoading
                  ? "bg-gray-400 cursor-not-allowed"
                  : "bg-gradient-to-r from-blue-600 to-indigo-600 hover:from-blue-700 hover:to-indigo-700"
              }`}
              onClick={handleSave}
              disabled={settingsState.isLoading}
            >
              {settingsState.isLoading ? "저장 중..." : "설정 저장"}
            </button>
          </div>
        </div>

        {/* Presets */}
        <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-4">
          <div class="flex items-center justify-between mb-2">
            <h3 class="text-lg font-semibold text-gray-800">프리셋</h3>
            <button
              type="button"
              class="text-sm text-blue-600 hover:text-blue-700 transition-colors duration-200"
              onClick={() => setShowAdvanced((p) => !p)}
            >
              {showAdvanced() ? "▲ 고급 설정 숨기기" : "▼ 고급 설정 보기"}
            </button>
          </div>
          <div class="flex flex-wrap gap-2">
            <For each={CONFIG_PRESETS}>
              {(preset) => (
                <button
                  class={`px-3 py-1.5 rounded-lg border transition-all duration-200 text-sm ${
                    settingsState.currentPreset === preset.name
                      ? 'border-blue-500 bg-blue-50 text-blue-700 font-semibold shadow-sm'
                      : 'border-gray-200 bg-white hover:bg-gray-50 text-gray-700'
                  }`}
                  onClick={() => applyPreset(preset.name)}
                >
                  {settingsState.currentPreset === preset.name && '✓ '}
                  {preset.name}
                </button>
              )}
            </For>
          </div>
        </div>

        {/* Settings Form (간소화된 핵심 설정 + 고급 설정 토글) */}
        <form
          class={`space-y-6 ${
            settingsState.isLoading ? "opacity-60 pointer-events-none" : ""
          }`}
          onSubmit={(e) => {
            e.preventDefault();
            handleSave();
          }}
        >
          {/* 핵심 설정 */}
          <fieldset class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
            <legend class="px-2 text-lg font-semibold text-gray-800">
              핵심 설정
            </legend>
            {/* 처리량 · 동시성 */}
            <div class="mt-4">
              <div class="flex items-center gap-2 mb-2">
                <span class="w-1.5 h-4 rounded bg-indigo-300"></span>
                <div class="text-sm font-semibold text-gray-800">
                  처리량 · 동시성
                </div>
              </div>
              <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
                <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-gray-200 bg-white/80 hover:bg-white transition p-3">
                  <div class="flex-1">
                    <div class="text-sm font-medium text-gray-800">
                      최대 동시 요청
                    </div>
                    <div class="text-xs text-gray-500">
                      동시에 보낼 수 있는 요청 개수
                    </div>
                  </div>
                  <input
                    type="number"
                    class="text-lg px-3 py-2 w-24 text-center rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300"
                    value={settingsState.getNestedValue(
                      "user.max_concurrent_requests"
                    )}
                    min={1}
                    max={100}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.max_concurrent_requests",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
                <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-gray-200 bg-white/80 hover:bg-white transition p-3">
                  <div class="flex-1">
                    <div class="text-sm font-medium text-gray-800">
                      목록 동시 처리
                    </div>
                    <div class="text-xs text-gray-500">
                      리스트 페이지 처리 동시성
                    </div>
                  </div>
                  <input
                    type="number"
                    class="text-lg px-3 py-2 w-24 text-center rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300"
                    value={settingsState.getNestedValue(
                      "user.crawling.workers.list_page_max_concurrent"
                    )}
                    min={1}
                    max={1000}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.crawling.workers.list_page_max_concurrent",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
                <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-gray-200 bg-white/80 hover:bg-white transition p-3">
                  <div class="flex-1">
                    <div class="text-sm font-medium text-gray-800">
                      상세 동시 처리
                    </div>
                    <div class="text-xs text-gray-500">
                      상세 페이지 처리 동시성
                    </div>
                  </div>
                  <input
                    type="number"
                    class="text-lg px-3 py-2 w-24 text-center rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300"
                    value={settingsState.getNestedValue(
                      "user.crawling.workers.product_detail_max_concurrent"
                    )}
                    min={1}
                    max={1000}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.crawling.workers.product_detail_max_concurrent",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
              </div>
            </div>

            {/* 지연 · 템포 */}
            <div class="mt-6">
              <div class="flex items-center gap-2 mb-2">
                <span class="w-1.5 h-4 rounded bg-indigo-300"></span>
                <div class="text-sm font-semibold text-gray-800">
                  지연(ms)· 템포
                </div>
              </div>
              <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
                <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-gray-200 bg-white/80 hover:bg-white transition p-3">
                  <div class="flex-1">
                    <div class="text-sm font-medium text-gray-800">
                      요청 지연
                    </div>
                    <div class="text-xs text-gray-500">
                      서버 부하 방지를 위한 요청 간 간격
                    </div>
                  </div>
                  <input
                    type="number"
                    class="text-lg px-3 py-2 w-24 text-center rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300"
                    value={settingsState.getNestedValue(
                      "user.request_delay_ms"
                    )}
                    min={100}
                    max={10000}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.request_delay_ms",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
                <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-gray-200 bg-white/80 hover:bg-white transition p-3">
                  <div class="flex-1">
                    <div class="text-sm font-medium text-gray-800">
                      배치 지연
                    </div>
                    <div class="text-xs text-gray-500">배치 간 대기 시간</div>
                  </div>
                  <input
                    type="number"
                    class="text-lg px-3 py-2 w-24 text-center rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300"
                    value={settingsState.getNestedValue(
                      "user.batch.batch_delay_ms"
                    )}
                    min={0}
                    max={60000}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.batch.batch_delay_ms",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
                <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-gray-200 bg-white/80 hover:bg-white transition p-3">
                  <div class="flex-1">
                    <div class="text-sm font-medium text-gray-800">
                      상세 로그 모드
                    </div>
                  </div>
                  <input
                    type="checkbox"
                    class="w-4 h-4"
                    checked={settingsState.getNestedValue(
                      "user.verbose_logging"
                    )}
                    onChange={(e) =>
                      settingsState.updateNestedField(
                        "user.verbose_logging",
                        e.currentTarget.checked
                      )
                    }
                  />
                </label>
              </div>
            </div>

            {/* 범위 · 크기 */}
            <div class="mt-6">
              <div class="flex items-center gap-2 mb-2">
                <span class="w-1.5 h-4 rounded bg-indigo-300"></span>
                <div class="text-sm font-semibold text-gray-800">
                  범위 · 크기
                </div>
              </div>
              <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
                <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-gray-200 bg-white/80 hover:bg-white transition p-3">
                  <div class="flex-1">
                    <div class="text-sm font-medium text-gray-800">
                      페이지 범위
                    </div>
                    <div class="text-xs text-gray-500">
                      크롤링할 최대 페이지 수
                    </div>
                  </div>
                  <input
                    type="number"
                    class="text-lg px-3 py-2 w-24 text-center rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300"
                    value={settingsState.getNestedValue(
                      "user.crawling.page_range_limit"
                    )}
                    min={1}
                    max={10000}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.crawling.page_range_limit",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
                <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-gray-200 bg-white/80 hover:bg-white transition p-3">
                  <div class="flex-1">
                    <div class="text-sm font-medium text-gray-800">
                      배치 크기
                    </div>
                    <div class="text-xs text-gray-500">
                      한 번에 저장할 레코드 수
                    </div>
                  </div>
                  <input
                    type="number"
                    class="text-lg px-3 py-2 w-24 text-center rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300"
                    value={settingsState.getNestedValue(
                      "user.batch.batch_size"
                    )}
                    min={1}
                    max={1000}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.batch.batch_size",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
                <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-gray-200 bg-white/80 hover:bg-white transition p-3">
                  <div class="flex-1">
                    <div class="text-sm font-medium text-gray-800">
                      로컬DB 자동 추가
                    </div>
                  </div>
                  <input
                    type="checkbox"
                    class="w-4 h-4"
                    checked={settingsState.getNestedValue(
                      "user.crawling.auto_add_to_local_db"
                    )}
                    onChange={(e) =>
                      settingsState.updateNestedField(
                        "user.crawling.auto_add_to_local_db",
                        e.currentTarget.checked
                      )
                    }
                  />
                </label>
              </div>
            </div>

            {/* 지능형 모드: 요청에 따라 UI에서 제거됨 */}

            {/* 워커(병렬 처리) */}
            <div class="mt-6">
              <div class="flex items-center gap-2 mb-2">
                <span class="w-1.5 h-4 rounded bg-indigo-300"></span>
                <div class="text-sm font-semibold text-gray-800">
                  워커(병렬 처리)
                </div>
              </div>
              <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-gray-200 bg-white/80 hover:bg-white transition p-3">
                  <div class="flex-1">
                    <div class="text-sm font-medium text-gray-800">
                      요청 타임아웃(초)
                    </div>
                    <div class="text-xs text-gray-500">
                      HTTP 요청 최대 대기 시간
                    </div>
                  </div>
                  <input
                    type="number"
                    class="text-lg px-3 py-2 w-24 text-center rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300"
                    value={settingsState.getNestedValue(
                      "user.crawling.workers.request_timeout_seconds"
                    )}
                    min={1}
                    max={600}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.crawling.workers.request_timeout_seconds",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
                <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-gray-200 bg-white/80 hover:bg-white transition p-3">
                  <div class="flex-1">
                    <div class="text-sm font-medium text-gray-800">
                      초당 요청 제한(RPS)
                    </div>
                    <div class="text-xs text-gray-500">
                      Rate limit (비워두면 기본값)
                    </div>
                  </div>
                  <input
                    type="number"
                    class="text-lg px-3 py-2 w-24 text-center rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300"
                    value={
                      settingsState.getNestedValue(
                        "user.crawling.workers.max_requests_per_second"
                      ) ?? ""
                    }
                    min={1}
                    max={1000}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.crawling.workers.max_requests_per_second",
                        e.currentTarget.value ? +e.currentTarget.value : null
                      )
                    }
                  />
                </label>
              </div>
            </div>

            {/* 배치 처리 */}
            <div class="mt-6">
              <div class="flex items-center gap-2 mb-2">
                <span class="w-1.5 h-4 rounded bg-indigo-300"></span>
                <div class="text-sm font-semibold text-gray-800">배치 처리</div>
              </div>
              <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-gray-200 bg-white/80 hover:bg-white transition p-3">
                  <div class="flex-1">
                    <div class="text-sm font-medium text-gray-800">
                      배치 처리 사용
                    </div>
                  </div>
                  <input
                    type="checkbox"
                    class="w-4 h-4"
                    checked={settingsState.getNestedValue(
                      "user.batch.enable_batch_processing"
                    )}
                    onChange={(e) =>
                      settingsState.updateNestedField(
                        "user.batch.enable_batch_processing",
                        e.currentTarget.checked
                      )
                    }
                  />
                </label>
              </div>
            </div>

            {/* 로깅 */}
            <div class="mt-6">
              <div class="flex items-center gap-2 mb-2">
                <span class="w-1.5 h-4 rounded bg-indigo-300"></span>
                <div class="text-sm font-semibold text-gray-800">로깅</div>
              </div>
              <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-gray-200 bg-white/80 hover:bg-white transition p-3">
                  <div class="flex-1">
                    <div class="text-sm font-medium text-gray-800">
                      콘솔 출력
                    </div>
                  </div>
                  <input
                    type="checkbox"
                    class="w-4 h-4"
                    checked={
                      settingsState.getNestedValue(
                        "user.logging.console_output"
                      ) || false
                    }
                    onChange={(e) =>
                      settingsState.updateNestedField(
                        "user.logging.console_output",
                        e.currentTarget.checked
                      )
                    }
                  />
                </label>
                <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-gray-200 bg-white/80 hover:bg-white transition p-3">
                  <div class="flex-1">
                    <div class="text-sm font-medium text-gray-800">
                      파일 저장
                    </div>
                  </div>
                  <input
                    type="checkbox"
                    class="w-4 h-4"
                    checked={
                      settingsState.getNestedValue(
                        "user.logging.file_output"
                      ) || false
                    }
                    onChange={(e) =>
                      settingsState.updateNestedField(
                        "user.logging.file_output",
                        e.currentTarget.checked
                      )
                    }
                  />
                </label>
              </div>
              <div class="grid grid-cols-1 md:grid-cols-1 gap-4">
                <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-gray-200 bg-white/80 hover:bg-white transition p-3 mt-2">
                  <div class="flex-1">
                    <div class="text-sm font-medium text-gray-800">
                      로그 레벨
                    </div>
                    <div class="text-xs text-gray-500">
                      trace | debug | info | warn | error
                    </div>
                  </div>
                  <input
                    type="text"
                    class="text-lg px-3 py-2 w-36 text-center rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300"
                    value={
                      settingsState.getNestedValue("user.logging.level") ||
                      "info"
                    }
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.logging.level",
                        e.currentTarget.value
                      )
                    }
                  />
                </label>
              </div>
            </div>
          </fieldset>

          {/* 고급 설정 (토글) */}
          <Show when={showAdvanced()}>
            <fieldset class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
              <legend class="px-2 text-lg font-semibold text-gray-800">
                고급 설정
              </legend>
              <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mt-4">
                {/* 로깅 고급 */}
                <label class="text-sm font-medium text-gray-800 flex items-center gap-2">
                  JSON 형식 출력
                  <input
                    type="checkbox"
                    class="w-4 h-4 ml-auto"
                    checked={
                      settingsState.getNestedValue(
                        "user.logging.json_format"
                      ) || false
                    }
                    onChange={(e) =>
                      settingsState.updateNestedField(
                        "user.logging.json_format",
                        e.currentTarget.checked
                      )
                    }
                  />
                </label>
                <label class="text-sm font-medium text-gray-800">
                  파일 이름 전략
                  <div class="text-xs text-gray-500">
                    unified | daily | timestamp
                  </div>
                  <input
                    type="text"
                    class="mt-1 text-lg px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300 w-full text-center"
                    value={
                      settingsState.getNestedValue(
                        "user.logging.file_naming_strategy"
                      ) || "unified"
                    }
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.logging.file_naming_strategy",
                        e.currentTarget.value
                      )
                    }
                  />
                </label>
                <label class="text-sm font-medium text-gray-800">
                  로그 보관 최대 개수
                  <div class="text-xs text-gray-500">
                    최신 로그만 유지하려면 낮게 설정
                  </div>
                  <input
                    type="number"
                    class="mt-1 text-lg px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300 text-center"
                    value={
                      settingsState.getNestedValue("user.logging.max_files") ||
                      5
                    }
                    min={1}
                    max={50}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.logging.max_files",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
                <label class="text-sm font-medium text-gray-800 flex items-center gap-2">
                  종료 시 로그 정리
                  <input
                    type="checkbox"
                    class="w-4 h-4 ml-auto"
                    checked={
                      settingsState.getNestedValue(
                        "user.logging.auto_cleanup_logs"
                      ) || false
                    }
                    onChange={(e) =>
                      settingsState.updateNestedField(
                        "user.logging.auto_cleanup_logs",
                        e.currentTarget.checked
                      )
                    }
                  />
                </label>
                <label class="text-sm font-medium text-gray-800 flex items-center gap-2">
                  최신 로그만 유지
                  <input
                    type="checkbox"
                    class="w-4 h-4 ml-auto"
                    checked={
                      settingsState.getNestedValue(
                        "user.logging.keep_only_latest"
                      ) || false
                    }
                    onChange={(e) =>
                      settingsState.updateNestedField(
                        "user.logging.keep_only_latest",
                        e.currentTarget.checked
                      )
                    }
                  />
                </label>
                <label class="text-sm font-medium text-gray-800 flex items-center gap-2">
                  간결한 시작 로그
                  <input
                    type="checkbox"
                    class="w-4 h-4 ml-auto"
                    checked={
                      settingsState.getNestedValue(
                        "user.logging.concise_startup"
                      ) || true
                    }
                    onChange={(e) =>
                      settingsState.updateNestedField(
                        "user.logging.concise_startup",
                        e.currentTarget.checked
                      )
                    }
                  />
                </label>
              </div>

              {/* 배치 고급 */}
              <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mt-6">
                <label class="text-sm font-medium text-gray-800">
                  배치 재시도 횟수
                  <div class="text-xs text-gray-500">
                    실패 시 재시도 최대 횟수
                  </div>
                  <input
                    type="number"
                    class="mt-1 text-lg px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300 text-center"
                    value={settingsState.getNestedValue(
                      "user.batch.batch_retry_limit"
                    )}
                    min={0}
                    max={50}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.batch.batch_retry_limit",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
              </div>

              {/* 크롤링 고급 */}
              <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mt-6">
                <label class="text-sm font-medium text-gray-800">
                  목록 재시도 횟수
                  <div class="text-xs text-gray-500">리스트 페이지 실패 시</div>
                  <input
                    type="number"
                    class="mt-1 text-lg px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300 text-center"
                    value={settingsState.getNestedValue(
                      "user.crawling.product_list_retry_count"
                    )}
                    min={0}
                    max={50}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.crawling.product_list_retry_count",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
                <label class="text-sm font-medium text-gray-800">
                  상세 재시도 횟수
                  <div class="text-xs text-gray-500">상세 페이지 실패 시</div>
                  <input
                    type="number"
                    class="mt-1 text-lg px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300 text-center"
                    value={settingsState.getNestedValue(
                      "user.crawling.product_detail_retry_count"
                    )}
                    min={0}
                    max={50}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.crawling.product_detail_retry_count",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
              </div>

              {/* 워커 고급 */}
              <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mt-6">
                <label class="text-sm font-medium text-gray-800">
                  User-Agent
                  <div class="text-xs text-gray-500">HTTP 요청 헤더 식별자</div>
                  <input
                    type="text"
                    class="mt-1 text-lg px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300 w-full text-center"
                    value={
                      settingsState.getNestedValue(
                        "user.crawling.workers.user_agent"
                      ) ?? ""
                    }
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.crawling.workers.user_agent",
                        e.currentTarget.value
                      )
                    }
                  />
                </label>
                <label class="text-sm font-medium text-gray-800">
                  동기화 User-Agent(선택)
                  <div class="text-xs text-gray-500">
                    특정 동기화 흐름에 사용
                  </div>
                  <input
                    type="text"
                    class="mt-1 text-lg px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300 w-full text-center"
                    value={
                      settingsState.getNestedValue(
                        "user.crawling.workers.user_agent_sync"
                      ) ?? ""
                    }
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.crawling.workers.user_agent_sync",
                        e.currentTarget.value
                      )
                    }
                  />
                </label>
                <label class="text-sm font-medium text-gray-800 flex items-center gap-2">
                  리다이렉트 허용
                  <input
                    type="checkbox"
                    class="w-4 h-4 ml-auto"
                    checked={
                      settingsState.getNestedValue(
                        "user.crawling.workers.follow_redirects"
                      ) || false
                    }
                    onChange={(e) =>
                      settingsState.updateNestedField(
                        "user.crawling.workers.follow_redirects",
                        e.currentTarget.checked
                      )
                    }
                  />
                </label>
                <label class="text-sm font-medium text-gray-800 flex items-center gap-2">
                  robots.txt 준수
                  <input
                    type="checkbox"
                    class="w-4 h-4 ml-auto"
                    checked={
                      settingsState.getNestedValue(
                        "user.crawling.workers.respect_robots_txt"
                      ) || false
                    }
                    onChange={(e) =>
                      settingsState.updateNestedField(
                        "user.crawling.workers.respect_robots_txt",
                        e.currentTarget.checked
                      )
                    }
                  />
                </label>
                <label class="text-sm font-medium text-gray-800">
                  DB 배치 크기
                  <div class="text-xs text-gray-500">DB 쓰기 최적화</div>
                  <input
                    type="number"
                    class="mt-1 text-lg px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300 text-center"
                    value={settingsState.getNestedValue(
                      "user.crawling.workers.db_batch_size"
                    )}
                    min={1}
                    max={10000}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.crawling.workers.db_batch_size",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
                <label class="text-sm font-medium text-gray-800">
                  DB 최대 동시성
                  <div class="text-xs text-gray-500">동시 DB 작업 제한</div>
                  <input
                    type="number"
                    class="mt-1 text-lg px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300 text-center"
                    value={settingsState.getNestedValue(
                      "user.crawling.workers.db_max_concurrency"
                    )}
                    min={1}
                    max={10000}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.crawling.workers.db_max_concurrency",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
                <label class="text-sm font-medium text-gray-800">
                  요청 재시도 최대 횟수
                  <div class="text-xs text-gray-500">
                    네트워크 일시 오류 대비
                  </div>
                  <input
                    type="number"
                    class="mt-1 text-lg px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300 text-center"
                    value={settingsState.getNestedValue(
                      "user.crawling.workers.max_retries"
                    )}
                    min={0}
                    max={100}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.crawling.workers.max_retries",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
              </div>

              {/* 타이밍 */}
              <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mt-6">
                <label class="text-sm font-medium text-gray-800">
                  스케줄 주기(ms)
                  <div class="text-xs text-gray-500">배경 작업 스케줄 간격</div>
                  <input
                    type="number"
                    class="mt-1 text-lg px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300 text-center"
                    value={settingsState.getNestedValue(
                      "user.crawling.timing.scheduler_interval_ms"
                    )}
                    min={10}
                    max={600000}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.crawling.timing.scheduler_interval_ms",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
                <label class="text-sm font-medium text-gray-800">
                  종료 타임아웃(초)
                  <div class="text-xs text-gray-500">
                    정리 작업 최대 대기 시간
                  </div>
                  <input
                    type="number"
                    class="mt-1 text-lg px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300 text-center"
                    value={settingsState.getNestedValue(
                      "user.crawling.timing.shutdown_timeout_seconds"
                    )}
                    min={1}
                    max={3600}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.crawling.timing.shutdown_timeout_seconds",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
                <label class="text-sm font-medium text-gray-800">
                  상태 갱신 주기(초)
                  <div class="text-xs text-gray-500">
                    진행 상황 통계 출력 간격
                  </div>
                  <input
                    type="number"
                    class="mt-1 text-lg px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300 text-center"
                    value={settingsState.getNestedValue(
                      "user.crawling.timing.stats_interval_seconds"
                    )}
                    min={1}
                    max={3600}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.crawling.timing.stats_interval_seconds",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
                <label class="text-sm font-medium text-gray-800">
                  재시도 지연(ms)
                  <div class="text-xs text-gray-500">
                    실패 후 재시도까지 대기
                  </div>
                  <input
                    type="number"
                    class="mt-1 text-lg px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300 text-center"
                    value={settingsState.getNestedValue(
                      "user.crawling.timing.retry_delay_ms"
                    )}
                    min={0}
                    max={600000}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.crawling.timing.retry_delay_ms",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
                <label class="text-sm font-medium text-gray-800">
                  작업 타임아웃(초)
                  <div class="text-xs text-gray-500">
                    전체 오퍼레이션 최대 시간
                  </div>
                  <input
                    type="number"
                    class="mt-1 text-lg px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300 text-center"
                    value={settingsState.getNestedValue(
                      "user.crawling.timing.operation_timeout_seconds"
                    )}
                    min={1}
                    max={3600}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "user.crawling.timing.operation_timeout_seconds",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
              </div>

              {/* 고급(엔진) */}
              <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mt-6">
                <label class="text-sm font-medium text-gray-800">
                  마지막 페이지 탐색 시작점
                  <div class="text-xs text-gray-500">
                    마지막 페이지 유추 시작 위치
                  </div>
                  <input
                    type="number"
                    class="mt-1 text-lg px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300 text-center"
                    value={settingsState.getNestedValue(
                      "advanced.last_page_search_start"
                    )}
                    min={1}
                    max={20000}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "advanced.last_page_search_start",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
                <label class="text-sm font-medium text-gray-800">
                  최대 탐색 시도 횟수
                  <div class="text-xs text-gray-500">마지막 페이지 탐색</div>
                  <input
                    type="number"
                    class="mt-1 text-lg px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300 text-center"
                    value={settingsState.getNestedValue(
                      "advanced.max_search_attempts"
                    )}
                    min={1}
                    max={100}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "advanced.max_search_attempts",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
                <label class="text-sm font-medium text-gray-800">
                  고급 요청 타임아웃(초)
                  <div class="text-xs text-gray-500">특정 엔진 내부 용도</div>
                  <input
                    type="number"
                    class="mt-1 text-lg px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300 text-center"
                    value={settingsState.getNestedValue(
                      "advanced.request_timeout_seconds"
                    )}
                    min={1}
                    max={600}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "advanced.request_timeout_seconds",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
                <label class="text-sm font-medium text-gray-800 col-span-1 md:col-span-3">
                  상품 셀렉터 목록 (쉼표로 구분)
                  <div class="text-xs text-gray-500">
                    페이지 내 상품 영역을 찾는 CSS 선택자
                  </div>
                  <input
                    type="text"
                    class="mt-1 text-lg px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300 w-full text-center"
                    value={(
                      settingsState.getNestedValue(
                        "advanced.product_selectors"
                      ) || []
                    ).join(", ")}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "advanced.product_selectors",
                        e.currentTarget.value
                          .split(",")
                          .map((s) => s.trim())
                          .filter(Boolean)
                      )
                    }
                  />
                </label>
                <label class="text-sm font-medium text-gray-800">
                  실패 임계값
                  <div class="text-xs text-gray-500">연속 실패 허용 수</div>
                  <input
                    type="number"
                    class="mt-1 text-lg px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300 text-center"
                    value={
                      settingsState.getNestedValue(
                        "advanced.failure_policy.failure_threshold"
                      ) ?? 5
                    }
                    min={0}
                    max={100}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "advanced.failure_policy.failure_threshold",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
                <label class="text-sm font-medium text-gray-800">
                  제거 유예(초)
                  <div class="text-xs text-gray-500">
                    실패 세션 정리 유예 시간
                  </div>
                  <input
                    type="number"
                    class="mt-1 text-lg px-3 py-2 rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-indigo-300 text-center"
                    value={
                      settingsState.getNestedValue(
                        "advanced.failure_policy.removal_grace_secs"
                      ) ?? 10
                    }
                    min={0}
                    max={3600}
                    onInput={(e) =>
                      settingsState.updateNestedField(
                        "advanced.failure_policy.removal_grace_secs",
                        +e.currentTarget.value
                      )
                    }
                  />
                </label>
              </div>
            </fieldset>
          </Show>
          {/* 앱 관리 정보 (읽기 전용, 토글) */}
          <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20">
            <div class="p-4 flex items-center justify-between">
              <legend class="px-2 text-lg font-semibold text-gray-800">
                앱 관리 정보 (읽기 전용)
              </legend>
              <button
                type="button"
                class="text-sm text-blue-600 hover:text-blue-700 transition-colors duration-200"
                onClick={() => setShowAppManaged((p) => !p)}
              >
                {showAppManaged() ? "▲ 숨기기" : "▼ 펼치기"}
              </button>
            </div>
            <Show when={showAppManaged()}>
              <div class="px-6 pb-6">
                <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <div class="text-xs text-gray-500">
                      마지막으로 확인된 최대 페이지
                    </div>
                    <div class="text-sm font-medium text-gray-800">
                      {String(
                        settingsState.getNestedValue(
                          "app_managed.last_known_max_page"
                        ) ?? ""
                      )}
                    </div>
                  </div>
                  <div>
                    <div class="text-xs text-gray-500">최근 성공 크롤 시간</div>
                    <div class="text-sm font-medium text-gray-800">
                      {String(
                        settingsState.getNestedValue(
                          "app_managed.last_successful_crawl"
                        ) ?? ""
                      )}
                    </div>
                  </div>
                  <div>
                    <div class="text-xs text-gray-500">최근 크롤 제품 수</div>
                    <div class="text-sm font-medium text-gray-800">
                      {String(
                        settingsState.getNestedValue(
                          "app_managed.last_crawl_product_count"
                        ) ?? ""
                      )}
                    </div>
                  </div>
                  <div>
                    <div class="text-xs text-gray-500">
                      페이지당 평균 제품 수
                    </div>
                    <div class="text-sm font-medium text-gray-800">
                      {String(
                        settingsState.getNestedValue(
                          "app_managed.avg_products_per_page"
                        ) ?? ""
                      )}
                    </div>
                  </div>
                  <div>
                    <div class="text-xs text-gray-500">설정 버전</div>
                    <div class="text-sm font-medium text-gray-800">
                      {String(
                        settingsState.getNestedValue(
                          "app_managed.config_version"
                        ) ?? ""
                      )}
                    </div>
                  </div>
                  <div class="md:col-span-2">
                    <div class="text-xs text-gray-500">창 상태 저장값</div>
                    <pre class="text-xs bg-slate-50 border border-slate-200 rounded p-2 overflow-x-auto">
                      {String(
                        settingsState.getNestedValue(
                          "app_managed.window_state"
                        ) ?? ""
                      )}
                    </pre>
                  </div>
                </div>
              </div>
            </Show>
          </div>

          {/* Actions */}
          <div class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-4 flex items-center justify-end gap-2">
            <button
              type="button"
              class="px-4 py-2 rounded-lg border border-gray-200 bg-white hover:bg-gray-50 text-gray-700"
              onClick={() => setShowModal(true)}
            >
              미리보기
            </button>
            <button
              type="submit"
              class={`px-4 py-2 rounded-lg text-white shadow transition-all duration-200 ${
                settingsState.isLoading
                  ? "bg-gray-400 cursor-not-allowed"
                  : "bg-gradient-to-r from-blue-600 to-indigo-600 hover:from-blue-700 hover:to-indigo-700"
              }`}
              disabled={settingsState.isLoading}
            >
              {settingsState.isLoading ? "저장 중..." : "설정 저장"}
            </button>
          </div>
        </form>

        {/* Toast */}
        <Show when={showMessage()}>
          <div
            class={`fixed top-5 right-5 z-[1000] px-4 py-2 rounded-md text-white shadow-lg ${
              saveMessage().includes("❌") ? "bg-rose-500" : "bg-emerald-500"
            }`}
          >
            {saveMessage()}
          </div>
        </Show>

        {/* Modal */}
        <Show when={showModal()}>
          <div
            class="fixed inset-0 bg-black/50 backdrop-blur-sm z-[1000] flex items-center justify-center"
            onClick={(e) => e.target === e.currentTarget && setShowModal(false)}
          >
            <div class="bg-white rounded-xl shadow-2xl max-w-2xl w-[90vw] max-h-[80vh] overflow-y-auto p-6">
              <h3 class="text-lg font-semibold text-gray-800">설정 미리보기</h3>
              <pre class="mt-3 p-3 rounded-lg bg-slate-50 border border-slate-200 text-xs overflow-x-auto">
                {JSON.stringify(settingsState.settings, null, 2)}
              </pre>
              <div class="mt-4 flex items-center justify-end gap-2">
                <button
                  class="px-4 py-2 rounded-lg text-white bg-indigo-600 hover:bg-indigo-700"
                  onClick={handleSave}
                >
                  저장
                </button>
                <button
                  class="px-4 py-2 rounded-lg border border-gray-200 bg-white hover:bg-gray-50 text-gray-700"
                  onClick={() => setShowModal(false)}
                >
                  취소
                </button>
              </div>
            </div>
          </div>
        </Show>
      </div>
    </div>
  );
};

// Removed unused default export; use named export only.
