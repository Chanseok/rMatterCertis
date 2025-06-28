// UI store for managing user interface preferences and view states
import { createStore } from 'solid-js/store';
import { createSignal } from 'solid-js';
import type { 
  UIPreferences, 
  ViewState, 
  SearchFilterState
} from '../../types/ui';

// ============================================================================
// UI Store State
// ============================================================================

interface UIState {
  preferences: UIPreferences;
  viewState: ViewState;
  searchFilter: SearchFilterState;
  activeTab: string;
  sidebarCollapsed: boolean;
}

interface UIActions {
  // Preferences
  setTheme: (theme: UIPreferences['theme']) => void;
  toggleSidebar: () => void;
  setAutoRefresh: (enabled: boolean) => void;
  setPageSize: (size: number) => void;
  setShowAdvancedOptions: (show: boolean) => void;
  setLanguage: (language: UIPreferences['language']) => void;
  
  // View State
  toggleSection: (section: keyof Pick<ViewState, 'dbSectionExpanded' | 'productsSectionExpanded' | 'logsSectionExpanded' | 'settingsSectionExpanded'>) => void;
  showModal: (modal: keyof Pick<ViewState, 'deleteModalVisible' | 'settingsModalVisible' | 'exportModalVisible' | 'addVendorModalVisible'>) => void;
  hideModal: (modal: keyof Pick<ViewState, 'deleteModalVisible' | 'settingsModalVisible' | 'exportModalVisible' | 'addVendorModalVisible'>) => void;
  hideAllModals: () => void;
  setLoading: (type: keyof Pick<ViewState, 'isRefreshing' | 'isExporting' | 'isSaving'>, loading: boolean) => void;
  
  // Search & Filter
  setSearchQuery: (query: string) => void;
  setFilterBy: (filterBy: string) => void;
  setSortBy: (sortBy: string) => void;
  setSortOrder: (order: 'asc' | 'desc') => void;
  setCurrentPage: (page: number) => void;
  resetSearchFilter: () => void;
  
  // Navigation
  setActiveTab: (tab: string) => void;
  
  // Persistence
  savePreferences: () => void;
  loadPreferences: () => void;
}

// ============================================================================
// Default Values
// ============================================================================

const defaultPreferences: UIPreferences = {
  theme: 'system',
  sidebarCollapsed: false,
  autoRefresh: false,
  pageSize: 25,
  showAdvancedOptions: false,
  language: 'en',
};

const defaultViewState: ViewState = {
  dbSectionExpanded: true,
  productsSectionExpanded: true,
  logsSectionExpanded: false,
  settingsSectionExpanded: false,
  deleteModalVisible: false,
  settingsModalVisible: false,
  exportModalVisible: false,
  addVendorModalVisible: false,
  isRefreshing: false,
  isExporting: false,
  isSaving: false,
};

const defaultSearchFilter: SearchFilterState = {
  searchQuery: '',
  filterBy: 'all',
  sortBy: 'created_at',
  sortOrder: 'desc',
  currentPage: 1,
};

// ============================================================================
// Store Creation
// ============================================================================

export function createUIStore() {
  // Reactive state
  const [state, setState] = createStore<UIState>({
    preferences: { ...defaultPreferences },
    viewState: { ...defaultViewState },
    searchFilter: { ...defaultSearchFilter },
    activeTab: 'dashboard',
    sidebarCollapsed: false,
  });

  // Theme detection signal
  const [systemTheme, setSystemTheme] = createSignal<'light' | 'dark'>('light');

  // ========================================================================
  // System Theme Detection
  // ========================================================================

  const detectSystemTheme = () => {
    if (typeof window !== 'undefined' && window.matchMedia) {
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
      setSystemTheme(mediaQuery.matches ? 'dark' : 'light');
      
      // Listen for theme changes
      mediaQuery.addEventListener('change', (e) => {
        setSystemTheme(e.matches ? 'dark' : 'light');
      });
    }
  };

  // Initialize theme detection
  detectSystemTheme();

  // ========================================================================
  // Preference Actions
  // ========================================================================

  const setTheme = (theme: UIPreferences['theme']) => {
    setState('preferences', 'theme', theme);
    applyTheme();
    savePreferences();
  };

  const applyTheme = () => {
    const { theme } = state.preferences;
    const effectiveTheme = theme === 'system' ? systemTheme() : theme;
    
    if (typeof document !== 'undefined') {
      document.documentElement.setAttribute('data-theme', effectiveTheme);
    }
  };

  const toggleSidebar = () => {
    setState('sidebarCollapsed', !state.sidebarCollapsed);
    setState('preferences', 'sidebarCollapsed', state.sidebarCollapsed);
    savePreferences();
  };

  const setAutoRefresh = (enabled: boolean) => {
    setState('preferences', 'autoRefresh', enabled);
    savePreferences();
  };

  const setPageSize = (size: number) => {
    setState('preferences', 'pageSize', size);
    savePreferences();
  };

  const setShowAdvancedOptions = (show: boolean) => {
    setState('preferences', 'showAdvancedOptions', show);
    savePreferences();
  };

  const setLanguage = (language: UIPreferences['language']) => {
    setState('preferences', 'language', language);
    savePreferences();
  };

  // ========================================================================
  // View State Actions
  // ========================================================================

  const toggleSection = (section: keyof Pick<ViewState, 'dbSectionExpanded' | 'productsSectionExpanded' | 'logsSectionExpanded' | 'settingsSectionExpanded'>) => {
    setState('viewState', section, !state.viewState[section]);
  };

  const showModal = (modal: keyof Pick<ViewState, 'deleteModalVisible' | 'settingsModalVisible' | 'exportModalVisible' | 'addVendorModalVisible'>) => {
    setState('viewState', modal, true);
  };

  const hideModal = (modal: keyof Pick<ViewState, 'deleteModalVisible' | 'settingsModalVisible' | 'exportModalVisible' | 'addVendorModalVisible'>) => {
    setState('viewState', modal, false);
  };

  const hideAllModals = () => {
    setState('viewState', {
      deleteModalVisible: false,
      settingsModalVisible: false,
      exportModalVisible: false,
      addVendorModalVisible: false,
    });
  };

  const setLoading = (type: keyof Pick<ViewState, 'isRefreshing' | 'isExporting' | 'isSaving'>, loading: boolean) => {
    setState('viewState', type, loading);
  };

  // ========================================================================
  // Search & Filter Actions
  // ========================================================================

  const setSearchQuery = (query: string) => {
    setState('searchFilter', 'searchQuery', query);
    setState('searchFilter', 'currentPage', 1); // Reset to first page
  };

  const setFilterBy = (filterBy: string) => {
    setState('searchFilter', 'filterBy', filterBy);
    setState('searchFilter', 'currentPage', 1);
  };

  const setSortBy = (sortBy: string) => {
    setState('searchFilter', 'sortBy', sortBy);
    setState('searchFilter', 'currentPage', 1);
  };

  const setSortOrder = (order: 'asc' | 'desc') => {
    setState('searchFilter', 'sortOrder', order);
    setState('searchFilter', 'currentPage', 1);
  };

  const setCurrentPage = (page: number) => {
    setState('searchFilter', 'currentPage', page);
  };

  const resetSearchFilter = () => {
    setState('searchFilter', { ...defaultSearchFilter });
  };

  // ========================================================================
  // Navigation Actions
  // ========================================================================

  const setActiveTab = (tab: string) => {
    setState('activeTab', tab);
  };

  // ========================================================================
  // Persistence
  // ========================================================================

  const savePreferences = () => {
    if (typeof localStorage !== 'undefined') {
      try {
        localStorage.setItem('matter-certis-preferences', JSON.stringify(state.preferences));
      } catch (error) {
        console.warn('Failed to save preferences to localStorage:', error);
      }
    }
  };

  const loadPreferences = () => {
    if (typeof localStorage !== 'undefined') {
      try {
        const saved = localStorage.getItem('matter-certis-preferences');
        if (saved) {
          const preferences = JSON.parse(saved);
          setState('preferences', { ...defaultPreferences, ...preferences });
          setState('sidebarCollapsed', preferences.sidebarCollapsed || false);
          applyTheme();
        }
      } catch (error) {
        console.warn('Failed to load preferences from localStorage:', error);
      }
    }
  };

  // Initialize preferences on store creation
  loadPreferences();

  // ========================================================================
  // Return Store API
  // ========================================================================

  const actions: UIActions = {
    setTheme,
    toggleSidebar,
    setAutoRefresh,
    setPageSize,
    setShowAdvancedOptions,
    setLanguage,
    toggleSection,
    showModal,
    hideModal,
    hideAllModals,
    setLoading,
    setSearchQuery,
    setFilterBy,
    setSortBy,
    setSortOrder,
    setCurrentPage,
    resetSearchFilter,
    setActiveTab,
    savePreferences,
    loadPreferences,
  };

  return {
    // State (read-only)
    state,
    
    // System theme
    systemTheme,
    
    // Actions
    ...actions,
    
    // Computed getters
    get effectiveTheme() {
      const { theme } = state.preferences;
      return theme === 'system' ? systemTheme() : theme;
    },
    
    get hasActiveSearch() {
      return state.searchFilter.searchQuery.length > 0;
    },
    
    get hasActiveFilters() {
      return state.searchFilter.filterBy !== 'all' || state.searchFilter.sortBy !== 'created_at';
    },
    
    get isAnyModalVisible() {
      const { viewState } = state;
      return viewState.deleteModalVisible || 
             viewState.settingsModalVisible || 
             viewState.exportModalVisible || 
             viewState.addVendorModalVisible;
    },
    
    get isAnyLoadingActive() {
      const { viewState } = state;
      return viewState.isRefreshing || viewState.isExporting || viewState.isSaving;
    },
    
    get currentSearchParams() {
      return {
        query: state.searchFilter.searchQuery,
        filter: state.searchFilter.filterBy,
        sort: state.searchFilter.sortBy,
        order: state.searchFilter.sortOrder,
        page: state.searchFilter.currentPage,
        pageSize: state.preferences.pageSize,
      };
    },
  };
}

// ============================================================================
// Store Type Export
// ============================================================================

export type UIStore = ReturnType<typeof createUIStore>;
