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
