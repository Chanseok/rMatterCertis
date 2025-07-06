import { createSignal, createEffect, For, Show } from "solid-js";
import { CrawlingService } from "../services/crawlingService";
import type { Product, MatterProduct, DatabaseSummary, ProductSearchRequest } from "../types/crawling";

const CrawlingResults = () => {
  const [products, setProducts] = createSignal<Product[]>([]);
  const [matterProducts, setMatterProducts] = createSignal<MatterProduct[]>([]);
  const [summary, setSummary] = createSignal<DatabaseSummary | null>(null);
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [searchQuery, setSearchQuery] = createSignal("");
  const [selectedManufacturer, setSelectedManufacturer] = createSignal<string>("");
  const [activeTab, setActiveTab] = createSignal<"summary" | "products" | "matter">("summary");

  // Load initial data
  createEffect(() => {
    loadSummary();
  });

  const loadSummary = async () => {
    try {
      setLoading(true);
      setError(null);
      const summaryData = await CrawlingService.getDatabaseSummary();
      setSummary(summaryData);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load summary");
    } finally {
      setLoading(false);
    }
  };

  const loadProducts = async () => {
    try {
      setLoading(true);
      setError(null);
      const productData = await CrawlingService.getProducts();
      setProducts(productData);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load products");
    } finally {
      setLoading(false);
    }
  };

  const loadMatterProducts = async () => {
    try {
      setLoading(true);
      setError(null);
      const matterData = await CrawlingService.getMatterProducts();
      setMatterProducts(matterData);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load Matter products");
    } finally {
      setLoading(false);
    }
  };

  const searchProducts = async () => {
    if (!searchQuery().trim()) {
      loadProducts();
      return;
    }

    try {
      setLoading(true);
      setError(null);
      const searchRequest: ProductSearchRequest = {
        query: searchQuery(),
        manufacturer: selectedManufacturer() || undefined,
        limit: 50
      };
      const searchResult = await CrawlingService.searchProducts(searchRequest);
      setProducts(searchResult.products);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to search products");
    } finally {
      setLoading(false);
    }
  };

  const filterByManufacturer = async (manufacturer: string) => {
    try {
      setLoading(true);
      setError(null);
      const filteredProducts = await CrawlingService.getProductsByManufacturer(manufacturer);
      setProducts(filteredProducts);
      setSelectedManufacturer(manufacturer);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to filter products");
    } finally {
      setLoading(false);
    }
  };

  const refreshData = () => {
    loadSummary();
    if (activeTab() === "products") {
      loadProducts();
    } else if (activeTab() === "matter") {
      loadMatterProducts();
    }
  };

  const getUniqueManufacturers = () => {
    const manufacturers = new Set<string>();
    products().forEach(p => {
      if (p.manufacturer) manufacturers.add(p.manufacturer);
    });
    return Array.from(manufacturers).sort();
  };

  return (
    <div class="crawling-results">
      <div class="results-header">
        <h2>Crawling Results</h2>
        <div class="results-actions">
          <button class="btn-primary" onClick={refreshData} disabled={loading()}>
            {loading() ? "Loading..." : "Refresh"}
          </button>
        </div>
      </div>

      <Show when={error()}>
        <div class="error-message">
          <strong>Error:</strong> {error()}
        </div>
      </Show>

      {/* Tab Navigation */}
      <div class="results-tabs">
        <button 
          class={`tab-btn ${activeTab() === "summary" ? "active" : ""}`}
          onClick={() => setActiveTab("summary")}
        >
          Summary
        </button>
        <button 
          class={`tab-btn ${activeTab() === "products" ? "active" : ""}`}
          onClick={() => {
            setActiveTab("products");
            if (products().length === 0) loadProducts();
          }}
        >
          Products ({summary()?.total_products || 0})
        </button>
        <button 
          class={`tab-btn ${activeTab() === "matter" ? "active" : ""}`}
          onClick={() => {
            setActiveTab("matter");
            if (matterProducts().length === 0) loadMatterProducts();
          }}
        >
          Matter Products ({summary()?.total_matter_products || 0})
        </button>
      </div>

      {/* Summary Tab */}
      <Show when={activeTab() === "summary"}>
        <div class="summary-tab">
          <Show when={summary()} fallback={<div class="loading">Loading summary...</div>}>
            <div class="summary-grid">
              <div class="summary-card">
                <h3>Total Products</h3>
                <div class="summary-value">{summary()!.total_products}</div>
              </div>
              <div class="summary-card">
                <h3>Matter Products</h3>
                <div class="summary-value">{summary()!.total_matter_products}</div>
              </div>
              <div class="summary-card">
                <h3>Vendors</h3>
                <div class="summary-value">{summary()!.total_vendors}</div>
              </div>
              <div class="summary-card">
                <h3>Manufacturers</h3>
                <div class="summary-value">{summary()!.unique_manufacturers}</div>
              </div>
            </div>
            <div class="last-updated">
              Last updated: {new Date(summary()!.last_updated).toLocaleString()}
            </div>
          </Show>
        </div>
      </Show>

      {/* Products Tab */}
      <Show when={activeTab() === "products"}>
        <div class="products-tab">
          {/* Search and Filter */}
          <div class="search-section">
            <div class="search-row">
              <input
                type="text"
                placeholder="Search products..."
                value={searchQuery()}
                onInput={(e) => setSearchQuery(e.target.value)}
                class="search-input"
              />
              <button class="btn-primary" onClick={searchProducts}>
                Search
              </button>
            </div>
            <div class="filter-row">
              <select
                value={selectedManufacturer()}
                onChange={(e) => {
                  const manufacturer = e.target.value;
                  if (manufacturer) {
                    filterByManufacturer(manufacturer);
                  } else {
                    setSelectedManufacturer("");
                    loadProducts();
                  }
                }}
                class="filter-select"
              >
                <option value="">All Manufacturers</option>
                <For each={getUniqueManufacturers()}>
                  {(manufacturer) => (
                    <option value={manufacturer}>{manufacturer}</option>
                  )}
                </For>
              </select>
            </div>
          </div>

          {/* Products List */}
          <div class="products-list">
            <Show when={products().length > 0} fallback={
              <div class="no-data">
                {loading() ? "Loading products..." : "No products found"}
              </div>
            }>
              <div class="products-grid">
                <For each={products()}>
                  {(product) => (
                    <div class="product-card">
                      <div class="product-header">
                        <h4>{product.manufacturer || "Unknown Manufacturer"}</h4>
                        <span class="product-id">#{product.page_id}-{product.index_in_page}</span>
                      </div>
                      <div class="product-info">
                        <p><strong>Model:</strong> {product.model || "N/A"}</p>
                        <p><strong>Certificate ID:</strong> {product.certificate_id || "N/A"}</p>
                        <p><strong>Added:</strong> {new Date(product.created_at).toLocaleDateString()}</p>
                      </div>
                      <Show when={product.url && product.url !== `page_${product.page_id}_item_${product.index_in_page}`}>
                        <div class="product-url">
                          <a href={product.url} target="_blank" rel="noopener noreferrer">
                            View Details
                          </a>
                        </div>
                      </Show>
                    </div>
                  )}
                </For>
              </div>
            </Show>
          </div>
        </div>
      </Show>

      {/* Matter Products Tab */}
      <Show when={activeTab() === "matter"}>
        <div class="matter-products-tab">
          <div class="matter-products-list">
            <Show when={matterProducts().length > 0} fallback={
              <div class="no-data">
                {loading() ? "Loading Matter products..." : "No Matter products found"}
              </div>
            }>
              <div class="matter-products-grid">
                <For each={matterProducts()}>
                  {(product) => (
                    <div class="matter-product-card">
                      <div class="product-header">
                        <h4>{product.manufacturer || "Unknown Manufacturer"}</h4>
                        <div class="product-badges">
                          <Show when={product.device_type}>
                            <span class="badge device-type">{product.device_type}</span>
                          </Show>
                          <span class="product-id">#{product.page_id}-{product.index_in_page}</span>
                        </div>
                      </div>
                      <div class="product-details">
                        <div class="detail-row">
                          <strong>Model:</strong> {product.model || "N/A"}
                        </div>
                        <div class="detail-row">
                          <strong>Certificate ID:</strong> {product.certificate_id || "N/A"}
                        </div>
                        <Show when={product.vid || product.pid}>
                          <div class="detail-row">
                            <strong>VID/PID:</strong> {product.vid || "N/A"} / {product.pid || "N/A"}
                          </div>
                        </Show>
                        <Show when={product.certification_date}>
                          <div class="detail-row">
                            <strong>Certification Date:</strong> {product.certification_date}
                          </div>
                        </Show>
                        <Show when={product.software_version || product.hardware_version}>
                          <div class="detail-row">
                            <strong>Versions:</strong> SW: {product.software_version || "N/A"}, HW: {product.hardware_version || "N/A"}
                          </div>
                        </Show>
                        <div class="detail-row">
                          <strong>Added:</strong> {new Date(product.created_at).toLocaleDateString()}
                        </div>
                      </div>
                    </div>
                  )}
                </For>
              </div>
            </Show>
          </div>
        </div>
      </Show>
    </div>
  );
};

export default CrawlingResults;
