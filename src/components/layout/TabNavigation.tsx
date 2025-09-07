/**
 * TabNavigation - 탭 네비게이션 컴포넌트
 * SolidJS-UI-Implementation-Guide.md를 기반으로 구현
 */

import { For, Component, createSignal, onMount, onCleanup } from "solid-js";
import { tabState, setActiveTab } from "../../stores/tabStore";
import { windowState } from "../../stores/windowStore";

export const TabNavigation: Component = () => {
  // Live clock (updates every 30s)
  const formatTime = () =>
    new Date().toLocaleTimeString("ko-KR", {
      hour: "2-digit",
      minute: "2-digit",
    });
  const [clock, setClock] = createSignal<string>(formatTime());

  onMount(() => {
    const id = setInterval(() => setClock(formatTime()), 30_000);
    // Initialize immediately to avoid hydration mismatch / stale time
    setClock(formatTime());
    onCleanup(() => clearInterval(id));
  });

  const handleTabClick = (tabId: string) => {
    setActiveTab(tabId);
    // 마지막 활성 탭을 windowState에 저장
    windowState.setLastActiveTab(tabId);
  };

  return (
    <div class="bg-white dark:bg-gray-800 shadow-sm">
      <div class="px-6 pt-4">
        <div class="flex items-center justify-between">
          {/* 탭 버튼들 */}
          <div class="flex space-x-1">
            <For each={tabState.tabs}>
              {(tab, index) => (
                <button
                  data-tab={tab.id}
                  onClick={() => handleTabClick(tab.id)}
                  class={`
                    relative px-6 py-3 font-medium text-sm whitespace-nowrap
                    transition-all duration-200 ease-in-out rounded-t-lg
                    focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500
                    ${
                      tabState.activeTab === tab.id
                        ? `${tab.theme.bg} ${tab.theme.text} ${
                            tab.theme.border
                          } border-t border-l border-r border-b-0 shadow-md -mb-px z-10 dark:${tab.theme.bg.replace(
                            "50",
                            "900"
                          )} dark:${tab.theme.text.replace("700", "300")}`
                        : "bg-gray-50 text-gray-500 hover:text-gray-700 hover:bg-gray-100 border border-transparent hover:border-gray-200 dark:bg-gray-700 dark:text-gray-400 dark:hover:text-gray-300 dark:hover:bg-gray-600"
                    }
                    ${index() === 0 ? "ml-0" : ""}
                  `}
                  style={{
                    "box-shadow":
                      tabState.activeTab === tab.id
                        ? "0 -2px 8px rgba(0,0,0,0.04), 0 2px 4px rgba(0,0,0,0.02)"
                        : "none",
                  }}
                >
                  <span class="mr-2 text-base">{tab.icon}</span>
                  <span class="font-semibold">{tab.label}</span>

                  {/* 활성 탭에 그라데이션 언더라인 */}
                  {tabState.activeTab === tab.id && (
                    <div
                      class={`absolute bottom-0 left-0 right-0 h-1 bg-gradient-to-r ${tab.theme.accent} rounded-b-lg`}
                    />
                  )}
                </button>
              )}
            </For>
          </div>

          {/* 현재 시간 표시 (live) */}
          
          <div class="text-lg flex items-center gap-1 text-xs text-gray-600 dark:text-gray-300 select-none">
            <span aria-hidden>🕑</span>
            <span class="tabular-nums font-medium text-gray-700 dark:text-gray-200">
              {clock()}
            </span>
          </div>
          {/* 빠른 액세스 버튼들 */}
          <div class="flex items-center gap-2"></div>
        </div>
      </div>
    </div>
  );
};
