import { createSignal, onMount, For, Show } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { CrawlingDashboard } from "./components/CrawlingDashboard";
import { CrawlingForm } from "./components/CrawlingForm";
import "./App.css";

interface Vendor {
  vendor_id: string;
  vendor_number: number;
  vendor_name: string;
  company_legal_name: string;
  created_at: string;
}

interface DatabaseSummary {
  total_vendors: number;
  total_products: number;
  total_matter_products: number;
  database_size_mb: number;
  last_crawling_date?: string;
}

type AppTab = "dashboard" | "crawling" | "database" | "vendors";

function App() {
  const [currentTab, setCurrentTab] = createSignal<AppTab>("crawling");
  const [showCrawlingForm, setShowCrawlingForm] = createSignal(false);
  
  // Database state
  const [dbStatus, setDbStatus] = createSignal("");
  const [dbSummary, setDbSummary] = createSignal<DatabaseSummary | null>(null);
  const [vendors, setVendors] = createSignal<Vendor[]>([]);
  
  // Form state for creating vendor
  const [vendorNumber, setVendorNumber] = createSignal("");
  const [vendorName, setVendorName] = createSignal("");
  const [companyLegalName, setCompanyLegalName] = createSignal("");

  onMount(async () => {
    await testDatabase();
    await loadDatabaseSummary();
    await loadVendors();
  });

  async function testDatabase() {
    try {
      const result = await invoke("test_database_connection");
      setDbStatus(`✅ ${result}`);
    } catch (error) {
      setDbStatus(`❌ ${error}`);
    }
  }

  async function loadDatabaseSummary() {
    try {
      const summary = await invoke<DatabaseSummary>("get_database_summary");
      setDbSummary(summary);
    } catch (error) {
      console.error("Failed to load database summary:", error);
    }
  }

  async function loadVendors() {
    try {
      const allVendors = await invoke<Vendor[]>("get_all_vendors");
      setVendors(allVendors);
    } catch (error) {
      console.error("Failed to load vendors:", error);
    }
  }

  async function createVendor() {
    try {
      const vendorNumberValue = parseInt(vendorNumber());
      if (isNaN(vendorNumberValue)) {
        alert("벤더 번호는 숫자여야 합니다.");
        return;
      }

      const dto = {
        vendor_number: vendorNumberValue,
        vendor_name: vendorName(),
        company_legal_name: companyLegalName()
      };

      await invoke("create_vendor", { dto });
      
      // Clear form
      setVendorNumber("");
      setVendorName("");
      setCompanyLegalName("");
      
      // Reload data
      await loadVendors();
      await loadDatabaseSummary();
      
      alert("✅ 벤더가 성공적으로 생성되었습니다!");
    } catch (error) {
      alert(`❌ 벤더 생성 실패: ${error}`);
    }
  }

  async function deleteVendor(vendorId: string) {
    if (!confirm("정말로 이 벤더를 삭제하시겠습니까?")) {
      return;
    }

    try {
      await invoke("delete_vendor", { vendorId });
      await loadVendors();
      await loadDatabaseSummary();
      alert("✅ 벤더가 삭제되었습니다!");
    } catch (error) {
      alert(`❌ 벤더 삭제 실패: ${error}`);
    }
  }

  const handleStartCrawling = () => {
    setShowCrawlingForm(true);
  };

  const handleCrawlingStarted = (sessionId: string) => {
    setShowCrawlingForm(false);
    setCurrentTab("crawling");
    alert(`✅ 크롤링이 시작되었습니다! 세션 ID: ${sessionId.slice(0, 8)}`);
  };

  const handleCrawlingFormCancel = () => {
    setShowCrawlingForm(false);
  };

  const renderTabContent = () => {
    switch (currentTab()) {
      case "crawling":
        return <CrawlingDashboard onStartCrawling={handleStartCrawling} />;
      
      case "database":
        return (
          <div class="database-section">
            <h2>📊 데이터베이스 상태</h2>
            <p>{dbStatus()}</p>
            <Show when={dbSummary()}>
              <div class="db-summary">
                <div class="summary-grid">
                  <div class="summary-card">
                    <h3>총 벤더</h3>
                    <div class="summary-value">{dbSummary()!.total_vendors}</div>
                  </div>
                  <div class="summary-card">
                    <h3>총 제품</h3>
                    <div class="summary-value">{dbSummary()!.total_products}</div>
                  </div>
                  <div class="summary-card">
                    <h3>Matter 제품</h3>
                    <div class="summary-value">{dbSummary()!.total_matter_products}</div>
                  </div>
                  <div class="summary-card">
                    <h3>DB 크기</h3>
                    <div class="summary-value">{dbSummary()!.database_size_mb.toFixed(2)}MB</div>
                  </div>
                </div>
              </div>
            </Show>
            <div class="database-actions">
              <button class="btn btn-primary" onClick={testDatabase}>DB 연결 테스트</button>
              <button class="btn btn-secondary" onClick={loadDatabaseSummary}>요약 새로고침</button>
            </div>
          </div>
        );
      
      case "vendors":
        return (
          <div class="vendors-section">
            <h2>🏢 벤더 관리</h2>
            
            {/* Create Vendor Form */}
            <div class="vendor-form">
              <h3>새 벤더 추가</h3>
              <div class="form-row">
                <input
                  type="number"
                  placeholder="벤더 번호 (숫자)"
                  value={vendorNumber()}
                  onInput={(e) => setVendorNumber(e.currentTarget.value)}
                />
                <input
                  type="text"
                  placeholder="벤더명"
                  value={vendorName()}
                  onInput={(e) => setVendorName(e.currentTarget.value)}
                />
                <input
                  type="text"
                  placeholder="법인명"
                  value={companyLegalName()}
                  onInput={(e) => setCompanyLegalName(e.currentTarget.value)}
                />
                <button class="btn btn-primary" onClick={createVendor}>벤더 생성</button>
              </div>
            </div>

            {/* Vendors List */}
            <div class="vendors-list">
              <h3>등록된 벤더 목록 ({vendors().length}개)</h3>
              <Show 
                when={vendors().length > 0} 
                fallback={<p class="empty-message">등록된 벤더가 없습니다.</p>}
              >
                <div class="vendors-grid">
                  <For each={vendors()}>
                    {(vendor) => (
                      <div class="vendor-card">
                        <div class="vendor-info">
                          <h4>{vendor.vendor_name}</h4>
                          <p><strong>번호:</strong> {vendor.vendor_number}</p>
                          <p><strong>법인명:</strong> {vendor.company_legal_name}</p>
                          <p><strong>등록일:</strong> {new Date(vendor.created_at).toLocaleDateString('ko-KR')}</p>
                        </div>
                        <button 
                          class="btn btn-danger btn-sm"
                          onClick={() => deleteVendor(vendor.vendor_id)}
                        >
                          삭제
                        </button>
                      </div>
                    )}
                  </For>
                </div>
              </Show>
            </div>
          </div>
        );
      
      default:
        return <div>알 수 없는 탭입니다.</div>;
    }
  };

  return (
    <main class="app">
      <header class="app-header">
        <h1>rMatterCertis</h1>
        <p>Matter 인증 제품 크롤링 및 관리 시스템</p>
      </header>

      <nav class="app-nav">
        <button 
          class={`nav-tab ${currentTab() === "crawling" ? "active" : ""}`}
          onClick={() => setCurrentTab("crawling")}
        >
          🕷️ 크롤링
        </button>
        <button 
          class={`nav-tab ${currentTab() === "database" ? "active" : ""}`}
          onClick={() => setCurrentTab("database")}
        >
          📊 데이터베이스
        </button>
        <button 
          class={`nav-tab ${currentTab() === "vendors" ? "active" : ""}`}
          onClick={() => setCurrentTab("vendors")}
        >
          🏢 벤더 관리
        </button>
      </nav>

      <div class="app-content">
        {renderTabContent()}
      </div>

      <Show when={showCrawlingForm()}>
        <CrawlingForm
          onSuccess={handleCrawlingStarted}
          onCancel={handleCrawlingFormCancel}
        />
      </Show>

      <footer class="app-footer">
        <p>Phase 3: 크롤링 엔진 및 프론트엔드 구현 완료 🎉</p>
      </footer>
    </main>
  );
}

export default App;
