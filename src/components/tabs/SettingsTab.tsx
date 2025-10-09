/**
 * SettingsTab - 설정 탭 컴포넌트
 * settingsStore를 기반으로 한 실제 백엔드 연동 설정 UI
 */
import { Component, createSignal, onMount, For, Show } from "solid-js";
import { emit } from "@tauri-apps/api/event";
import { settingsState } from "../../stores/settingsStore";
import { CONFIG_PRESETS } from "../../types/config";
import { AdvancedSettingsSection } from "../settings/AdvancedSettingsSection";

export const SettingsTab: Component = () => {
  const [saveMessage, setSaveMessage] = createSignal<string>("");
  const [showMessage, setShowMessage] = createSignal(false);
  const [showModal, setShowModal] = createSignal(false);
  const [showAppManaged, setShowAppManaged] = createSignal(false);
  
  // 각 섹션별 접기/펼치기 상태
  const [showRange, setShowRange] = createSignal(true);
  const [showConcurrency, setShowConcurrency] = createSignal(true);
  const [showTiming, setShowTiming] = createSignal(true);
  const [showLogging, setShowLogging] = createSignal(true);
  const [showAdvanced, setShowAdvanced] = createSignal(false); // 고급 설정은 기본 접힌 상태

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
          <h3 class="text-lg font-semibold text-gray-800 mb-2">프리셋</h3>
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
            
            {/* 📊 범위 · 크기 - 최우선 설정 */}
            <div class="mt-4">
              <button
                type="button"
                class="flex items-center gap-2 mb-2 w-full hover:bg-gray-50 rounded-lg p-2 -ml-2 transition-colors"
                onClick={() => setShowRange(!showRange())}
              >
                <span class="w-1.5 h-4 rounded bg-blue-400"></span>
                <div class="text-sm font-semibold text-gray-800">
                  📊 범위 · 크기
                </div>
                <span class="text-xs text-gray-500 flex-1 text-left">크롤링 범위 및 데이터 저장 설정</span>
                <span class="text-gray-400 text-sm">{showRange() ? '▼' : '▶'}</span>
              </button>
              <Show when={showRange()}>
                <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-gray-200 bg-white/80 hover:bg-white transition p-3">
                  <div class="flex-1">
                    <div class="text-sm font-medium text-gray-800">
                      페이지 범위
                    </div>
                    <div class="text-xs text-gray-500">
                      크롤링할 최대 페이지 수
                    </div>
                  </div>
                  <div class="flex items-center gap-2">
                    <input
                      type="number"
                      class="text-lg px-3 py-2 w-24 text-right rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-blue-300 [&::-webkit-inner-spin-button]:ml-2 [&::-webkit-outer-spin-button]:ml-2"
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
                    <span class="text-sm text-gray-500">페이지</span>
                  </div>
                </label>
                <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-gray-200 bg-white/80 hover:bg-white transition p-3">
                  <div class="flex-1">
                    <div class="text-sm font-medium text-gray-800">
                      배치 크기
                    </div>
                    <div class="text-xs text-gray-500">
                      목록 페이지를 몇 개씩 묶어서 처리 (예: 5이면 5페이지씩 묶음)
                    </div>
                  </div>
                  <div class="flex items-center gap-2">
                    <input
                      type="number"
                      class="text-lg px-3 py-2 w-24 text-right rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-blue-300 [&::-webkit-inner-spin-button]:ml-2 [&::-webkit-outer-spin-button]:ml-2"
                      value={settingsState.getNestedValue(
                        "user.batch.batch_size"
                      )}
                      min={1}
                      max={100}
                      onInput={(e) =>
                        settingsState.updateNestedField(
                          "user.batch.batch_size",
                          +e.currentTarget.value
                        )
                      }
                    />
                    <span class="text-sm text-gray-500">페이지</span>
                  </div>
                </label>
              </div>
              </Show>
            </div>

            {/* 🔴 동시성 & 속도 제어 (Concurrency & Rate Limiting) */}
            <div class="mt-6">
              <button
                type="button"
                class="flex items-center gap-2 mb-2 w-full hover:bg-gray-50 rounded-lg p-2 -ml-2 transition-colors"
                onClick={() => setShowConcurrency(!showConcurrency())}
              >
                <span class="w-1.5 h-4 rounded bg-red-400"></span>
                <div class="text-sm font-semibold text-gray-800">
                  🔴 동시성 & 속도 제어
                </div>
                <span class="text-xs text-gray-500 flex-1 text-left">서버 부하 제어 핵심 설정</span>
                <span class="text-gray-400 text-sm">{showConcurrency() ? '▼' : '▶'}</span>
              </button>
              
              <Show when={showConcurrency()}>
              {/* 안내 메시지 */}
              <div class="mb-3 p-3 bg-blue-50 border border-blue-200 rounded-lg">
                <div class="flex items-start gap-2">
                  <span class="text-blue-600 text-sm">💡</span>
                  <div class="flex-1 text-xs text-blue-800 leading-relaxed">
                    <strong>설정 간 관계:</strong> "최대 동시 요청"은 모든 작업의 상한선입니다. 
                    목록/상세 동시 처리는 이 값을 초과할 수 없습니다. 
                    RPS는 초당 요청 수를 제한하며, 일반적으로 최대 동시 요청보다 크거나 같게 설정합니다.
                  </div>
                </div>
              </div>

              {/* 전역 제한 (Global Limits) */}
              <div class="mb-4 p-3 bg-red-50/50 border border-red-200 rounded-lg">
                <div class="text-xs font-semibold text-red-800 mb-2">⚠️ 전역 제한 (Global Limits)</div>
                <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
                  <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-red-300 bg-white hover:bg-red-50/30 transition p-3">
                    <div class="flex-1">
                      <div class="flex items-center gap-1">
                        <span class="text-red-600">🔴</span>
                        <div class="text-sm font-medium text-gray-800">
                          최대 동시 요청
                        </div>
                      </div>
                      <div class="text-xs text-gray-500">
                        모든 작업의 동시 실행 상한선
                      </div>
                    </div>
                    <div class="flex items-center gap-2">
                      <input
                        type="number"
                        class="text-lg px-3 py-2 w-24 text-right rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-red-300 [&::-webkit-inner-spin-button]:ml-2 [&::-webkit-outer-spin-button]:ml-2"
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
                      <span class="text-sm text-gray-500">개</span>
                    </div>
                  </label>
                  <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-red-300 bg-white hover:bg-red-50/30 transition p-3">
                    <div class="flex-1">
                      <div class="flex items-center gap-1">
                        <span class="text-red-600">🔴</span>
                        <div class="text-sm font-medium text-gray-800">
                          초당 요청 제한 (RPS)
                        </div>
                      </div>
                      <div class="text-xs text-gray-500">
                        1초에 보낼 수 있는 최대 요청 수
                      </div>
                    </div>
                    <div class="flex items-center gap-2">
                      <input
                        type="number"
                        class="text-lg px-3 py-2 w-24 text-right rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-red-300 [&::-webkit-inner-spin-button]:ml-2 [&::-webkit-outer-spin-button]:ml-2"
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
                      <span class="text-sm text-gray-500">req/s</span>
                    </div>
                  </label>
                </div>
              </div>

              {/* 작업별 동시성 (Task-specific Concurrency) */}
              <div class="p-3 bg-yellow-50/50 border border-yellow-200 rounded-lg">
                <div class="text-xs font-semibold text-yellow-800 mb-2">작업별 동시성 (Task-specific)</div>
                <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
                  <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-yellow-300 bg-white hover:bg-yellow-50/30 transition p-3">
                    <div class="flex-1">
                      <div class="flex items-center gap-1">
                        <span class="text-yellow-600">🟡</span>
                        <div class="text-sm font-medium text-gray-800">
                          목록 동시 처리
                        </div>
                      </div>
                      <div class="text-xs text-gray-500">
                        리스트 페이지 처리 동시성 (≤ 최대 동시 요청)
                      </div>
                    </div>
                    <div class="flex items-center gap-2">
                      <input
                        type="number"
                        class="text-lg px-3 py-2 w-24 text-right rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-yellow-300 [&::-webkit-inner-spin-button]:ml-2 [&::-webkit-outer-spin-button]:ml-2"
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
                      <span class="text-sm text-gray-500">개</span>
                    </div>
                  </label>
                  <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-yellow-300 bg-white hover:bg-yellow-50/30 transition p-3">
                    <div class="flex-1">
                      <div class="flex items-center gap-1">
                        <span class="text-yellow-600">🟡</span>
                        <div class="text-sm font-medium text-gray-800">
                          상세 동시 처리
                        </div>
                      </div>
                      <div class="text-xs text-gray-500">
                        상세 페이지 처리 동시성 (≤ 최대 동시 요청)
                      </div>
                    </div>
                    <div class="flex items-center gap-2">
                      <input
                        type="number"
                        class="text-lg px-3 py-2 w-24 text-right rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-yellow-300 [&::-webkit-inner-spin-button]:ml-2 [&::-webkit-outer-spin-button]:ml-2"
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
                      <span class="text-sm text-gray-500">개</span>
                    </div>
                  </label>
                </div>
              </div>
              </Show>
            </div>

            {/* ⏱️ 타이밍 & 재시도 - 타임아웃, 지연, 재시도 통합 */}
            <div class="mt-6">
              <button
                type="button"
                class="flex items-center gap-2 mb-2 w-full hover:bg-gray-50 rounded-lg p-2 -ml-2 transition-colors"
                onClick={() => setShowTiming(!showTiming())}
              >
                <span class="w-1.5 h-4 rounded bg-orange-400"></span>
                <div class="text-sm font-semibold text-gray-800">
                  ⏱️ 타이밍 & 재시도
                </div>
                <span class="text-xs text-gray-500 flex-1 text-left">시간 제한 및 요청 간격 설정</span>
                <span class="text-gray-400 text-sm">{showTiming() ? '▼' : '▶'}</span>
              </button>
              <Show when={showTiming()}>
              <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
                <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-gray-200 bg-white/80 hover:bg-white transition p-3">
                  <div class="flex-1">
                    <div class="text-sm font-medium text-gray-800">
                      요청 타임아웃
                    </div>
                    <div class="text-xs text-gray-500">
                      HTTP 요청 최대 대기 시간
                    </div>
                  </div>
                  <div class="flex items-center gap-2">
                    <input
                      type="number"
                      class="text-lg px-3 py-2 w-24 text-right rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-orange-300 [&::-webkit-inner-spin-button]:ml-2 [&::-webkit-outer-spin-button]:ml-2"
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
                    <span class="text-sm text-gray-500">초</span>
                  </div>
                </label>
                <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-gray-200 bg-white/80 hover:bg-white transition p-3">
                  <div class="flex-1">
                    <div class="text-sm font-medium text-gray-800">
                      요청 지연
                    </div>
                    <div class="text-xs text-gray-500">
                      서버 부하 방지를 위한 요청 간 간격
                    </div>
                  </div>
                  <div class="flex items-center gap-2">
                    <input
                      type="number"
                      class="text-lg px-3 py-2 w-24 text-right rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-orange-300 [&::-webkit-inner-spin-button]:ml-2 [&::-webkit-outer-spin-button]:ml-2"
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
                    <span class="text-sm text-gray-500">ms</span>
                  </div>
                </label>
                <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-gray-200 bg-white/80 hover:bg-white transition p-3">
                  <div class="flex-1">
                    <div class="text-sm font-medium text-gray-800">
                      배치 지연
                    </div>
                    <div class="text-xs text-gray-500">배치 간 대기 시간</div>
                  </div>
                  <div class="flex items-center gap-2">
                    <input
                      type="number"
                      class="text-lg px-3 py-2 w-24 text-right rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-orange-300 [&::-webkit-inner-spin-button]:ml-2 [&::-webkit-outer-spin-button]:ml-2"
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
                    <span class="text-sm text-gray-500">ms</span>
                  </div>
                </label>
              </div>
              </Show>
            </div>

            {/* 📝 로깅 */}
            <div class="mt-6">
              <button
                type="button"
                class="flex items-center gap-2 mb-2 w-full hover:bg-gray-50 rounded-lg p-2 -ml-2 transition-colors"
                onClick={() => setShowLogging(!showLogging())}
              >
                <span class="w-1.5 h-4 rounded bg-cyan-400"></span>
                <div class="text-sm font-semibold text-gray-800">
                  📝 로깅
                </div>
                <span class="text-xs text-gray-500 flex-1 text-left">로그 출력 및 저장 설정</span>
                <span class="text-gray-400 text-sm">{showLogging() ? '▼' : '▶'}</span>
              </button>
              <Show when={showLogging()}>
              <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-gray-200 bg-white/80 hover:bg-white transition p-3">
                  <div class="flex-1">
                    <div class="text-sm font-medium text-gray-800">
                      콘솔 출력
                    </div>
                    <div class="text-xs text-gray-500">
                      터미널에 로그 표시
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
                    <div class="text-xs text-gray-500">
                      로그 파일로 저장
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
              <div class="grid grid-cols-1 md:grid-cols-1 gap-4 mt-4">
                <label class="flex items-center justify-between gap-4 text-sm font-medium text-gray-800 rounded-lg border border-gray-200 bg-white/80 hover:bg-white transition p-3">
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
                    class="text-lg px-3 py-2 w-32 text-center rounded-md bg-white border border-gray-200 focus:outline-none focus:ring-2 focus:ring-cyan-300"
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
              </Show>
            </div>
          </fieldset>

          {/* 고급 설정 (카테고리별 그룹핑) */}
          <fieldset class="bg-white/90 backdrop-blur-sm rounded-2xl shadow-xl border border-white/20 p-6">
            <button
              type="button"
              class="flex items-center gap-2 w-full hover:bg-gray-50 rounded-lg p-2 -ml-2 transition-colors"
              onClick={() => setShowAdvanced(!showAdvanced())}
            >
              <legend class="px-2 text-lg font-semibold text-gray-800">
                🔧 고급 설정
              </legend>
              <span class="text-xs text-gray-500 ml-2">
                (검증된 40개 설정 • 6개 카테고리)
              </span>
              <span class="text-gray-400 text-sm ml-auto">{showAdvanced() ? '▼ 숨기기' : '▶ 펼치기'}</span>
            </button>
            <Show when={showAdvanced()}>
              <div class="mt-4">
                <AdvancedSettingsSection />
              </div>
            </Show>
          </fieldset>
          
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
