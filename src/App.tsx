import { createSignal, onMount, For, Show } from "solid-js";
import { StoreProvider, useStores } from "./stores/index.tsx";
import { apiAdapter } from "./platform/tauri";
import type { CreateVendorDto } from "./types/domain";
import "./App.css";

type AppTab = "dashboard" | "crawling" | "results" | "database" | "vendors";

// Main App Component with Store Provider
function App() {
  return (
    <StoreProvider>
      <AppContent />
    </StoreProvider>
  );
}

// App Content Component (inside StoreProvider)
function AppContent() {
  const { uiStore, vendorStore } = useStores();
  const [showCrawlingForm, setShowCrawlingForm] = createSignal(false);
  
  // Local state for database status (can be moved to store later)
  const [dbStatus, setDbStatus] = createSignal("");
  
  // Form state for creating vendor
  const [vendorNumber, setVendorNumber] = createSignal("");
  const [vendorName, setVendorName] = createSignal("");
  const [companyLegalName, setCompanyLegalName] = createSignal("");

  onMount(async () => {
    // Test database connection
    await testDatabase();
    // Initialize vendor store with data
    await vendorStore.loadAllVendors();
  });

  async function testDatabase() {
    try {
      const result = await apiAdapter.testDatabaseConnection();
      setDbStatus(`✅ ${result}`);
    } catch (error) {
      setDbStatus(`❌ ${error}`);
    }
  }

  async function createVendor() {
    const vendorNumberNum = parseInt(vendorNumber());
    if (isNaN(vendorNumberNum) || !vendorName() || !companyLegalName()) {
      alert("Please fill in all required fields");
      return;
    }

    const dto: CreateVendorDto = {
      vendor_number: vendorNumberNum,
      vendor_name: vendorName(),
      company_legal_name: companyLegalName(),
    };

    const success = await vendorStore.createVendor(dto);
    if (success) {
      // Clear form
      setVendorNumber("");
      setVendorName("");
      setCompanyLegalName("");
      alert("Vendor created successfully!");
    } else {
      alert(`Failed to create vendor: ${vendorStore.state.error}`);
    }
  }

  const currentTab = () => uiStore.state.activeTab as AppTab;
  const setCurrentTab = (tab: AppTab) => uiStore.setActiveTab(tab);

  return (
    <div class="app">
      <div class="header">
        <h1>Matter Certis v2</h1>
        <div class="tab-navigation">
          <button
            class={currentTab() === "dashboard" ? "tab active" : "tab"}
            onClick={() => setCurrentTab("dashboard")}
          >
            Dashboard
          </button>
          <button
            class={currentTab() === "crawling" ? "tab active" : "tab"}
            onClick={() => setCurrentTab("crawling")}
          >
            Crawling
          </button>
          <button
            class={currentTab() === "results" ? "tab active" : "tab"}
            onClick={() => setCurrentTab("results")}
          >
            Results
          </button>
          <button
            class={currentTab() === "database" ? "tab active" : "tab"}
            onClick={() => setCurrentTab("database")}
          >
            Database
          </button>
          <button
            class={currentTab() === "vendors" ? "tab active" : "tab"}
            onClick={() => setCurrentTab("vendors")}
          >
            Vendors
          </button>
        </div>
      </div>

      <div class="content">
        <Show when={currentTab() === "dashboard"}>
          <div>Dashboard placeholder - will integrate with stores</div>
        </Show>

        <Show when={currentTab() === "crawling"}>
          <div class="crawling-section">
            <div class="section-header">
              <h2>Crawling Management</h2>
              <button
                class="btn-primary"
                onClick={() => setShowCrawlingForm(!showCrawlingForm())}
              >
                {showCrawlingForm() ? "Hide Form" : "Start New Crawling"}
              </button>
            </div>
            <Show when={showCrawlingForm()}>
              <div>Crawling form placeholder - will integrate with crawling store</div>
            </Show>
          </div>
        </Show>

        <Show when={currentTab() === "results"}>
          <div>Results placeholder - will integrate with product store</div>
        </Show>

        <Show when={currentTab() === "database"}>
          <div class="database-section">
            <h2>Database Management</h2>
            <div class="status-box">
              <h3>Connection Status</h3>
              <p>{dbStatus()}</p>
            </div>
          </div>
        </Show>

        <Show when={currentTab() === "vendors"}>
          <div class="vendors-section">
            <h2>Vendor Management</h2>
            
            {/* Vendor Creation Form */}
            <div class="vendor-form">
              <h3>Create New Vendor</h3>
              <div class="form-group">
                <label>Vendor Number:</label>
                <input
                  type="number"
                  value={vendorNumber()}
                  onInput={(e) => setVendorNumber(e.currentTarget.value)}
                  placeholder="Enter vendor number"
                />
              </div>
              <div class="form-group">
                <label>Vendor Name:</label>
                <input
                  type="text"
                  value={vendorName()}
                  onInput={(e) => setVendorName(e.currentTarget.value)}
                  placeholder="Enter vendor name"
                />
              </div>
              <div class="form-group">
                <label>Company Legal Name:</label>
                <input
                  type="text"
                  value={companyLegalName()}
                  onInput={(e) => setCompanyLegalName(e.currentTarget.value)}
                  placeholder="Enter company legal name"
                />
              </div>
              <button
                class="btn-primary"
                onClick={createVendor}
                disabled={vendorStore.isCreating()}
              >
                {vendorStore.isCreating() ? "Creating..." : "Create Vendor"}
              </button>
            </div>

            {/* Vendor List */}
            <div class="vendor-list">
              <h3>Existing Vendors ({vendorStore.state.vendors.length})</h3>
              <Show when={vendorStore.state.loading}>
                <p>Loading vendors...</p>
              </Show>
              <Show when={vendorStore.state.error}>
                <p class="error">Error: {vendorStore.state.error}</p>
                <button onClick={() => vendorStore.clearError()}>Clear Error</button>
              </Show>
              <Show when={!vendorStore.state.loading && vendorStore.hasVendors}>
                <div class="vendor-grid">
                  <For each={vendorStore.state.vendors}>
                    {(vendor) => (
                      <div class="vendor-card">
                        <h4>{vendor.vendor_name}</h4>
                        <p><strong>Number:</strong> {vendor.vendor_number}</p>
                        <p><strong>Legal Name:</strong> {vendor.company_legal_name}</p>
                        <p><strong>Created:</strong> {new Date(vendor.created_at).toLocaleDateString()}</p>
                        <div class="vendor-actions">
                          <button 
                            class="btn-danger"
                            onClick={() => vendorStore.deleteVendor(vendor.vendor_id)}
                            disabled={vendorStore.isDeleting()}
                          >
                            {vendorStore.isDeleting() ? "Deleting..." : "Delete"}
                          </button>
                        </div>
                      </div>
                    )}
                  </For>
                </div>
              </Show>
              <Show when={!vendorStore.state.loading && !vendorStore.hasVendors}>
                <p>No vendors found. Create one above to get started.</p>
              </Show>
            </div>
          </div>
        </Show>
      </div>
    </div>
  );
}

export default App;
