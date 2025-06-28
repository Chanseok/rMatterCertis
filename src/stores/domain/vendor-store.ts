// Vendor store for managing vendor data and operations
import { createStore } from 'solid-js/store';
import { createSignal } from 'solid-js';
import type { 
  VendorResponseDto, 
  CreateVendorDto, 
  UpdateVendorDto
} from '../../types/domain';
import { apiAdapter, safeApiCall } from '../../platform/tauri';

// ============================================================================
// Vendor Store State
// ============================================================================

interface VendorState {
  vendors: VendorResponseDto[];
  selectedVendor: VendorResponseDto | null;
  searchQuery: string;
  loading: boolean;
  error: string | null;
  lastUpdated: Date | null;
}

interface VendorActions {
  // CRUD Operations
  createVendor: (dto: CreateVendorDto) => Promise<boolean>;
  updateVendor: (id: string, dto: UpdateVendorDto) => Promise<boolean>;
  deleteVendor: (id: string) => Promise<boolean>;
  
  // Data Loading
  loadAllVendors: () => Promise<void>;
  loadVendorById: (id: string) => Promise<void>;
  searchVendorsByName: (name: string) => Promise<void>;
  
  // UI State Management
  setSelectedVendor: (vendor: VendorResponseDto | null) => void;
  setSearchQuery: (query: string) => void;
  clearError: () => void;
  refresh: () => Promise<void>;
}

// ============================================================================
// Store Creation
// ============================================================================

export function createVendorStore() {
  // Reactive state
  const [state, setState] = createStore<VendorState>({
    vendors: [],
    selectedVendor: null,
    searchQuery: '',
    loading: false,
    error: null,
    lastUpdated: null,
  });

  // Loading signal for individual operations
  const [isCreating, setIsCreating] = createSignal(false);
  const [isUpdating, setIsUpdating] = createSignal(false);
  const [isDeleting, setIsDeleting] = createSignal(false);

  // ========================================================================
  // Helper Functions
  // ========================================================================

  const setLoading = (loading: boolean) => {
    setState('loading', loading);
  };

  const setError = (error: string | null) => {
    setState('error', error);
  };

  const setVendors = (vendors: VendorResponseDto[]) => {
    setState({
      vendors,
      lastUpdated: new Date(),
      error: null,
    });
  };

  // ========================================================================
  // CRUD Operations
  // ========================================================================

  const createVendor = async (dto: CreateVendorDto): Promise<boolean> => {
    setIsCreating(true);
    setError(null);

    try {
      const result = await safeApiCall(() => apiAdapter.createVendor(dto));
      
      if (result.error) {
        setError(result.error.message);
        return false;
      }

      if (result.data) {
        // Add new vendor to the list
        setState('vendors', (vendors) => [...vendors, result.data!]);
        setState('lastUpdated', new Date());
        return true;
      }

      return false;
    } catch (error) {
      setError('Failed to create vendor');
      return false;
    } finally {
      setIsCreating(false);
    }
  };

  const updateVendor = async (id: string, dto: UpdateVendorDto): Promise<boolean> => {
    setIsUpdating(true);
    setError(null);

    try {
      const result = await safeApiCall(() => apiAdapter.updateVendor(id, dto));
      
      if (result.error) {
        setError(result.error.message);
        return false;
      }

      if (result.data) {
        // Update vendor in the list
        setState('vendors', (vendors) =>
          vendors.map((vendor) =>
            vendor.vendor_id === id ? result.data! : vendor
          )
        );
        
        // Update selected vendor if it matches
        if (state.selectedVendor?.vendor_id === id) {
          setState('selectedVendor', result.data);
        }
        
        setState('lastUpdated', new Date());
        return true;
      }

      return false;
    } catch (error) {
      setError('Failed to update vendor');
      return false;
    } finally {
      setIsUpdating(false);
    }
  };

  const deleteVendor = async (id: string): Promise<boolean> => {
    setIsDeleting(true);
    setError(null);

    try {
      const result = await safeApiCall(() => apiAdapter.deleteVendor(id));
      
      if (result.error) {
        setError(result.error.message);
        return false;
      }

      // Remove vendor from the list
      setState('vendors', (vendors) =>
        vendors.filter((vendor) => vendor.vendor_id !== id)
      );
      
      // Clear selected vendor if it matches
      if (state.selectedVendor?.vendor_id === id) {
        setState('selectedVendor', null);
      }
      
      setState('lastUpdated', new Date());
      return true;
    } catch (error) {
      setError('Failed to delete vendor');
      return false;
    } finally {
      setIsDeleting(false);
    }
  };

  // ========================================================================
  // Data Loading
  // ========================================================================

  const loadAllVendors = async (): Promise<void> => {
    setLoading(true);
    setError(null);

    try {
      const result = await safeApiCall(() => apiAdapter.getAllVendors());
      
      if (result.error) {
        setError(result.error.message);
        return;
      }

      if (result.data) {
        setVendors(result.data);
      }
    } catch (error) {
      setError('Failed to load vendors');
    } finally {
      setLoading(false);
    }
  };

  const loadVendorById = async (id: string): Promise<void> => {
    setLoading(true);
    setError(null);

    try {
      const result = await safeApiCall(() => apiAdapter.getVendorById(id));
      
      if (result.error) {
        setError(result.error.message);
        return;
      }

      if (result.data) {
        setState('selectedVendor', result.data);
      }
    } catch (error) {
      setError('Failed to load vendor');
    } finally {
      setLoading(false);
    }
  };

  const searchVendorsByName = async (name: string): Promise<void> => {
    setLoading(true);
    setError(null);

    try {
      const result = await safeApiCall(() => apiAdapter.searchVendorsByName(name));
      
      if (result.error) {
        setError(result.error.message);
        return;
      }

      if (result.data) {
        setVendors(result.data);
      }
    } catch (error) {
      setError('Failed to search vendors');
    } finally {
      setLoading(false);
    }
  };

  // ========================================================================
  // UI State Management
  // ========================================================================

  const setSelectedVendor = (vendor: VendorResponseDto | null) => {
    setState('selectedVendor', vendor);
  };

  const setSearchQuery = (query: string) => {
    setState('searchQuery', query);
  };

  const clearError = () => {
    setError(null);
  };

  const refresh = async (): Promise<void> => {
    if (state.searchQuery) {
      await searchVendorsByName(state.searchQuery);
    } else {
      await loadAllVendors();
    }
  };

  // ========================================================================
  // Return Store API
  // ========================================================================

  const actions: VendorActions = {
    createVendor,
    updateVendor,
    deleteVendor,
    loadAllVendors,
    loadVendorById,
    searchVendorsByName,
    setSelectedVendor,
    setSearchQuery,
    clearError,
    refresh,
  };

  return {
    // State (read-only)
    state,
    
    // Loading indicators
    isCreating,
    isUpdating,
    isDeleting,
    
    // Actions
    ...actions,
    
    // Computed getters
    get hasVendors() {
      return state.vendors.length > 0;
    },
    
    get hasError() {
      return !!state.error;
    },
    
    get isOperationPending() {
      return isCreating() || isUpdating() || isDeleting();
    },
    
    get filteredVendors() {
      if (!state.searchQuery) return state.vendors;
      
      const query = state.searchQuery.toLowerCase();
      return state.vendors.filter(
        (vendor) =>
          vendor.vendor_name.toLowerCase().includes(query) ||
          vendor.company_legal_name.toLowerCase().includes(query) ||
          vendor.vendor_number.toString().includes(query)
      );
    },
  };
}

// ============================================================================
// Store Type Export
// ============================================================================

export type VendorStore = ReturnType<typeof createVendorStore>;
