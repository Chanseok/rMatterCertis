// Global store management with SolidJS context
import { createContext, useContext, ParentComponent } from 'solid-js';
import { createVendorStore, type VendorStore } from './domain/vendor-store';
import { createCrawlingStore, type CrawlingStore } from './domain/crawling-store';
import { createUIStore, type UIStore } from './domain/ui-store';

// ============================================================================
// Store Context
// ============================================================================

interface GlobalStores {
  vendorStore: VendorStore;
  crawlingStore: CrawlingStore;
  uiStore: UIStore;
}

const StoreContext = createContext<GlobalStores>();

// ============================================================================
// Store Provider
// ============================================================================

export const StoreProvider: ParentComponent = (props) => {
  // Create all stores
  const vendorStore = createVendorStore();
  const crawlingStore = createCrawlingStore();
  const uiStore = createUIStore();

  const stores: GlobalStores = {
    vendorStore,
    crawlingStore,
    uiStore,
  };

  return (
    <StoreContext.Provider value={stores}>
      {props.children}
    </StoreContext.Provider>
  );
};

// ============================================================================
// Store Hooks
// ============================================================================

export function useStores(): GlobalStores {
  const context = useContext(StoreContext);
  if (!context) {
    throw new Error('useStores must be used within a StoreProvider');
  }
  return context;
}

export function useVendorStore(): VendorStore {
  return useStores().vendorStore;
}

export function useCrawlingStore(): CrawlingStore {
  return useStores().crawlingStore;
}

export function useUIStore(): UIStore {
  return useStores().uiStore;
}

// ============================================================================
// Store Actions (Convenience Functions)
// ============================================================================

/**
 * Initialize all stores with default data
 */
export async function initializeStores(stores: GlobalStores) {
  try {
    // Load vendor data
    await stores.vendorStore.loadAllVendors();
    
    // Load active crawling sessions
    await stores.crawlingStore.loadActiveSessions();
    
    // UI preferences are loaded automatically in createUIStore
    
    console.log('✅ All stores initialized successfully');
  } catch (error) {
    console.error('❌ Failed to initialize stores:', error);
  }
}

/**
 * Clean up resources when application closes
 */
export function cleanupStores(stores: GlobalStores) {
  // Stop auto-refresh for crawling
  stores.crawlingStore.stopAutoRefresh();
  
  // Save UI preferences
  stores.uiStore.savePreferences();
  
  console.log('🧹 Store cleanup completed');
}

// ============================================================================
// Store Utilities
// ============================================================================

/**
 * Get consolidated loading state across all stores
 */
export function useGlobalLoadingState(): () => boolean {
  const { vendorStore, crawlingStore, uiStore } = useStores();
  
  return () => {
    return (
      vendorStore.state.loading ||
      vendorStore.isOperationPending ||
      crawlingStore.state.loading ||
      crawlingStore.isOperationPending ||
      uiStore.state.viewState.isRefreshing ||
      uiStore.state.viewState.isExporting ||
      uiStore.state.viewState.isSaving
    );
  };
}

/**
 * Get consolidated error state across all stores
 */
export function useGlobalErrorState(): () => string[] {
  const { vendorStore, crawlingStore } = useStores();
  
  return () => {
    const errors: string[] = [];
    
    if (vendorStore.state.error) {
      errors.push(`Vendor: ${vendorStore.state.error}`);
    }
    
    if (crawlingStore.state.error) {
      errors.push(`Crawling: ${crawlingStore.state.error}`);
    }
    
    return errors;
  };
}

/**
 * Clear all errors across stores
 */
export function clearAllErrors(stores: GlobalStores) {
  stores.vendorStore.clearError();
  stores.crawlingStore.clearError();
}

// ============================================================================
// Type Exports
// ============================================================================

export type { GlobalStores, VendorStore, CrawlingStore, UIStore };
