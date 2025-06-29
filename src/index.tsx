/* @refresh reload */
console.log("🚀 Index.tsx is loading...");

import { render } from "solid-js/web";

console.log("✅ solid-js/web imported successfully");

// Simple test component first
function SimpleTest() {
  console.log("🔥 SimpleTest component rendering");
  return (
    <div style="padding: 20px; background: red; color: white; font-size: 24px;">
      <h1>SIMPLE TEST - IF YOU SEE THIS, SOLIDJS WORKS!</h1>
      <p>Current time: {new Date().toLocaleTimeString()}</p>
    </div>
  );
}

const root = document.getElementById("root");
console.log("🌟 Root element found:", root);

if (root) {
  console.log("🎬 Starting render...");
  render(() => <SimpleTest />, root);
  console.log("✅ Render completed successfully");
} else {
  console.error("❌ Root element not found!");
  document.body.innerHTML = "<h1 style='color: red; padding: 20px;'>ERROR: Root element not found!</h1>";
}
