import { createSignal, onMount, For } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
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

function App() {
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

  return (
    <main class="container">
      <h1>rMatterCertis - Matter 인증 제품 관리</h1>

      {/* Database Status */}
      <div class="section">
        <h2>📊 데이터베이스 상태</h2>
        <p>{dbStatus()}</p>
        {dbSummary() && (
          <div class="db-summary">
            <p>📈 총 벤더: {dbSummary()!.total_vendors}개</p>
            <p>📦 총 제품: {dbSummary()!.total_products}개</p>
            <p>🔗 Matter 제품: {dbSummary()!.total_matter_products}개</p>
            <p>💾 DB 크기: {dbSummary()!.database_size_mb.toFixed(2)}MB</p>
          </div>
        )}
      </div>

      {/* Vendor Management */}
      <div class="section">
        <h2>🏢 벤더 관리</h2>
        
        {/* Create Vendor Form */}
        <div class="form">
          <h3>새 벤더 추가</h3>
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
          <button onClick={createVendor}>벤더 생성</button>
        </div>

        {/* Vendors List */}
        <div class="vendors-list">
          <h3>등록된 벤더 목록 ({vendors().length}개)</h3>
          <For each={vendors()}>
            {(vendor) => (
              <div class="vendor-item">
                <div class="vendor-info">
                  <h4>{vendor.vendor_name}</h4>
                  <p>번호: {vendor.vendor_number}</p>
                  <p>법인명: {vendor.company_legal_name}</p>
                  <p>등록일: {new Date(vendor.created_at).toLocaleDateString('ko-KR')}</p>
                </div>
                <button 
                  class="delete-btn"
                  onClick={() => deleteVendor(vendor.vendor_id)}
                >
                  삭제
                </button>
              </div>
            )}
          </For>
          
          {vendors().length === 0 && (
            <p class="empty-message">등록된 벤더가 없습니다.</p>
          )}
        </div>
      </div>

      {/* Test Buttons */}
      <div class="section">
        <h2>🧪 테스트 기능</h2>
        <button onClick={testDatabase}>DB 연결 테스트</button>
        <button onClick={loadDatabaseSummary}>DB 요약 새로고침</button>
        <button onClick={loadVendors}>벤더 목록 새로고침</button>
      </div>

      <div class="info">
        <p>Matter 인증 제품 크롤링 및 관리 시스템</p>
        <p>Phase 2: 백엔드 도메인 구현 완료 🎉</p>
      </div>
    </main>
  );
}

export default App;
