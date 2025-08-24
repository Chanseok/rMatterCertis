/* @refresh reload */
console.log("🚀 Index.tsx is loading...");

import { render } from "solid-js/web";
import "./index.css";

console.log("✅ solid-js/web and CSS imported successfully");

// 각 탭 컴포넌트들을 개별적으로 테스트해보겠습니다
try {
  console.log("📝 Importing SettingsTab...");
  import('./components/tabs/SettingsTab').then(() => {
    console.log("✅ SettingsTab imported successfully");
  }).catch(error => {
    console.error("❌ SettingsTab import failed:", error);
  });
} catch (error) {
  console.error("❌ SettingsTab import error:", error);
}

// AppWithTabs를 직접 테스트해보겠습니다
import { AppWithTabs } from "./components/AppWithTabs";
// Attempt to explicitly show the Tauri window once the frontend is ready
import { getCurrentWindow } from "@tauri-apps/api/window";

console.log("✅ AppWithTabs imported successfully");

// AppWithTabs 직접 테스트
const root = document.getElementById("root");
console.log("🌟 Root element found:", root);

if (root) {
  console.log("🎬 Starting render with AppWithTabs...");
  try {
    render(() => <AppWithTabs />, root);
    console.log("✅ AppWithTabs render completed successfully");
    // Explicitly show window (in case initial visible=false or delayed)
    try {
      const win = getCurrentWindow();
      win.show().then(() => console.log("🪟 Tauri window show() invoked"));
    } catch (e) {
      console.warn("⚠️ Failed to call window.show():", e);
    }

    // Optional dev: auto-start a sync on launch if env flags are set
    // Usage (zsh): VITE_AUTO_SYNC_RANGES="512-500" VITE_AUTO_SYNC_METHOD="basic|partial" VITE_AUTO_SYNC_DELAY=1500 npm run tauri:dev
    // - METHOD=basic will expand ranges into pages[] and call startBasicSyncPages (persists via page_filter path)
    // - METHOD=partial will call start_partial_sync with ranges string (includes full flow)
    try {
      const autoExpr = (import.meta as any)?.env?.VITE_AUTO_SYNC_RANGES as string | undefined;
      const autoMethod = ((import.meta as any)?.env?.VITE_AUTO_SYNC_METHOD as string | undefined)?.toLowerCase() || 'basic';
      const autoDelayMsRaw = (import.meta as any)?.env?.VITE_AUTO_SYNC_DELAY as string | undefined;
      const autoDryRunRaw = (import.meta as any)?.env?.VITE_AUTO_SYNC_DRYRUN as string | undefined;
      const autoDelayMs = Number.isFinite(Number(autoDelayMsRaw)) ? Number(autoDelayMsRaw) : 1200;
      const autoDryRun = typeof autoDryRunRaw === 'string' ? ['1','true','yes','on'].includes(autoDryRunRaw.toLowerCase()) : false;
      if (autoExpr && autoExpr.trim().length > 0) {
        console.log(`[AutoSync] Scheduled: method=${autoMethod} ranges=\"${autoExpr}\" dryRun=${autoDryRun} delayMs=${autoDelayMs}`);
        setTimeout(async () => {
          try {
            const mod = await import('./services/tauri-api');
            const tauriApi = (mod as any).tauriApi ?? new (mod as any).TauriApiService();
            const parseRanges = (expr: string): number[] => {
              const pages: number[] = [];
              for (const token of expr.split(',').map(t => t.trim()).filter(Boolean)) {
                const m = token.match(/^(-?\d+)\s*-\s*(-?-?\d+)$/);
                if (m) {
                  const a = parseInt(m[1], 10);
                  const b = parseInt(m[2], 10);
                  if (Number.isFinite(a) && Number.isFinite(b)) {
                    if (a >= b) { for (let p=a; p>=b; p--) pages.push(p); }
                    else { for (let p=a; p<=b; p++) pages.push(p); }
                  }
                  continue;
                }
                const n = parseInt(token, 10);
                if (Number.isFinite(n)) pages.push(n);
              }
              // dedupe preserving order
              const seen = new Set<number>();
              return pages.filter(p => (seen.has(p) ? false : (seen.add(p), true)));
            };

            if (autoMethod === 'partial') {
              console.log(`[AutoSync] Invoking start_partial_sync: \"${autoExpr}\" dryRun=${autoDryRun}`);
              await tauriApi.startPartialSync(autoExpr, autoDryRun);
            } else {
              const pages = parseRanges(autoExpr);
              console.log(`[AutoSync] Invoking startBasicSyncPages: pages=[${pages.join(', ')}] dryRun=${autoDryRun}`);
              if (pages.length > 0) await tauriApi.startBasicSyncPages(pages, autoDryRun);
            }
            console.log('[AutoSync] Invocation submitted');
          } catch (e) {
            console.error('[AutoSync] Failed to invoke auto sync:', e);
          }
        }, autoDelayMs);
      }
    } catch (e) {
      console.warn('[AutoSync] skipped (env not available or error)', e);
    }
  } catch (error) {
    console.error("❌ Error rendering AppWithTabs:", error);
    render(() => (
      <div style="padding: 20px; background: red; color: white; font-family: sans-serif;">
        <h1>❌ AppWithTabs Error</h1>
        <p>Error: {String(error)}</p>
      </div>
    ), root);
  }
} else {
  console.error("❌ Root element not found!");
  document.body.innerHTML = "<h1 style='color: red; padding: 20px;'>ERROR: Root element not found!</h1>";
}
