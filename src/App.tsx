import { createSignal } from "solid-js";
import logo from "./assets/logo.svg";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
  const [greetMsg, setGreetMsg] = createSignal("");
  const [name, setName] = createSignal("");
  const [dbStatus, setDbStatus] = createSignal("");
  const [dbInfo, setDbInfo] = createSignal("");

  async function greet() {
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    setGreetMsg(await invoke("greet", { name: name() }));
  }

  async function testDatabase() {
    try {
      const result = await invoke("test_database_connection");
      setDbStatus(`✅ ${result}`);
    } catch (error) {
      setDbStatus(`❌ ${error}`);
    }
  }

  async function getDatabaseInfo() {
    try {
      const result = await invoke("get_database_info");
      setDbInfo(`📊 ${result}`);
    } catch (error) {
      setDbInfo(`❌ ${error}`);
    }
  }

  return (
    <main class="container">
      <h1>Welcome to Tauri + Solid</h1>

      <div class="row">
        <a href="https://vitejs.dev" target="_blank">
          <img src="/vite.svg" class="logo vite" alt="Vite logo" />
        </a>
        <a href="https://tauri.app" target="_blank">
          <img src="/tauri.svg" class="logo tauri" alt="Tauri logo" />
        </a>
        <a href="https://solidjs.com" target="_blank">
          <img src={logo} class="logo solid" alt="Solid logo" />
        </a>
      </div>
      <p>Click on the Tauri, Vite, and Solid logos to learn more.</p>

      <form
        class="row"
        onSubmit={(e) => {
          e.preventDefault();
          greet();
        }}
      >
        <input
          id="greet-input"
          onChange={(e) => setName(e.currentTarget.value)}
          placeholder="Enter a name..."
        />
        <button type="submit">Greet</button>
      </form>
      <p>{greetMsg()}</p>

      <div class="row">
        <h2>rMatterCertis Database Tests</h2>
      </div>
      
      <div class="row">
        <button onClick={testDatabase}>Test Database Connection</button>
        <button onClick={getDatabaseInfo}>Get Database Info</button>
      </div>
      
      <div class="row">
        <p>{dbStatus()}</p>
        <p>{dbInfo()}</p>
      </div>
    </main>
  );
}

export default App;
