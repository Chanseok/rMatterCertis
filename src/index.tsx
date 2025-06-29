/* @refresh reload */
console.log("🚀 Index.tsx is loading...");

import { render } from "solid-js/web";
import App from "./App";

console.log("✅ solid-js/web and App imported successfully");

const root = document.getElementById("root");
console.log("🌟 Root element found:", root);

if (root) {
  console.log("🎬 Starting render...");
  render(() => <App />, root);
  console.log("✅ Render completed successfully");
} else {
  console.error("❌ Root element not found!");
  document.body.innerHTML = "<h1 style='color: red; padding: 20px;'>ERROR: Root element not found!</h1>";
}
