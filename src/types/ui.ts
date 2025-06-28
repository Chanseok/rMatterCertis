// UI-specific types for components and state management
// This file contains TypeScript interfaces for UI components, themes, and user preferences

// ============================================================================
// UI Preferences & Settings
// ============================================================================

export interface UIPreferences {
  theme: 'light' | 'dark' | 'system';
  sidebarCollapsed: boolean;
  autoRefresh: boolean;
  pageSize: number;
  showAdvancedOptions: boolean;
  language: 'en' | 'ko';
}

export interface ViewState {
  // 섹션 확장 상태
  dbSectionExpanded: boolean;
  productsSectionExpanded: boolean;
  logsSectionExpanded: boolean;
  settingsSectionExpanded: boolean;
  
  // 모달 상태
  deleteModalVisible: boolean;
  settingsModalVisible: boolean;
  exportModalVisible: boolean;
  addVendorModalVisible: boolean;
  
  // 로딩 상태
  isRefreshing: boolean;
  isExporting: boolean;
  isSaving: boolean;
}

export interface SearchFilterState {
  searchQuery: string;
  filterBy: string;
  sortBy: string;
  sortOrder: 'asc' | 'desc';
  currentPage: number;
}

// ============================================================================
// Component Props
// ============================================================================

export interface ButtonProps {
  variant?: 'primary' | 'secondary' | 'danger' | 'ghost' | 'outline';
  size?: 'xs' | 'sm' | 'md' | 'lg' | 'xl';
  disabled?: boolean;
  loading?: boolean;
  fullWidth?: boolean;
  onClick?: () => void;
  type?: 'button' | 'submit' | 'reset';
  children: any;
  class?: string;
}

export interface ProgressBarProps {
  value: number;
  max: number;
  showLabel?: boolean;
  variant?: 'default' | 'success' | 'warning' | 'error';
  size?: 'sm' | 'md' | 'lg';
  animated?: boolean;
  class?: string;
}

export interface ModalProps {
  open: boolean;
  onClose: () => void;
  title?: string;
  size?: 'sm' | 'md' | 'lg' | 'xl' | 'full';
  showCloseButton?: boolean;
  closeOnBackdrop?: boolean;
  closeOnEscape?: boolean;
  actions?: any;
  children: any;
  class?: string;
}

export interface ToastProps {
  type: 'success' | 'error' | 'warning' | 'info';
  message: string;
  duration?: number;
  position?: 'top-right' | 'top-left' | 'bottom-right' | 'bottom-left';
  onClose?: () => void;
}

export interface SpinnerProps {
  size?: 'xs' | 'sm' | 'md' | 'lg' | 'xl';
  variant?: 'circular' | 'dots' | 'bars';
  color?: 'blue' | 'gray' | 'green' | 'red' | 'yellow' | 'white';
  label?: string;
  center?: boolean;
  class?: string;
}

export interface DataTableColumn<T> {
  key: keyof T;
  title: string;
  sortable?: boolean;
  width?: string;
  render?: (value: any, record: T) => any;
  align?: 'left' | 'center' | 'right';
}

export interface DataTableProps<T> {
  data: T[];
  columns: DataTableColumn<T>[];
  loading?: boolean;
  pagination?: {
    current: number;
    pageSize: number;
    total: number;
    onChange: (page: number, pageSize?: number) => void;
  };
  onRowClick?: (record: T) => void;
  rowKey: keyof T;
  class?: string;
}

export interface SearchFilterOption {
  label: string;
  value: string;
}

export interface SearchFilterProps {
  searchQuery?: string;
  onSearchChange?: (query: string) => void;
  filterOptions?: SearchFilterOption[];
  selectedFilter?: string;
  onFilterChange?: (filter: string) => void;
  sortOptions?: SearchFilterOption[];
  selectedSort?: string;
  onSortChange?: (sort: string) => void;
  onReset?: () => void;
  placeholder?: string;
  compact?: boolean;
  showReset?: boolean;
  class?: string;
}

// ============================================================================
// Feature Component Props  
// ============================================================================

export interface VendorFormData {
  name: string;
  url: string;
  description?: string;
}

export interface VendorFormProps {
  vendor?: any; // VendorResponseDto 타입이지만 any로 임시 처리
  mode: 'create' | 'edit';
  onSubmit?: (data: any) => void | Promise<void>;
  onCancel?: () => void;
  loading?: boolean;
  isModal?: boolean;
  modalOpen?: boolean;
  class?: string;
}

export interface VendorManagementProps {
  class?: string;
}

// ============================================================================
// Form Types
// ============================================================================

export interface FormField {
  name: string;
  label: string;
  type: 'text' | 'number' | 'email' | 'url' | 'textarea' | 'select' | 'checkbox';
  required?: boolean;
  placeholder?: string;
  options?: { label: string; value: string | number }[];
  validation?: {
    pattern?: RegExp;
    min?: number;
    max?: number;
    minLength?: number;
    maxLength?: number;
  };
}

export interface FormProps {
  fields: FormField[];
  initialValues?: Record<string, any>;
  onSubmit: (values: Record<string, any>) => void | Promise<void>;
  loading?: boolean;
  submitText?: string;
  cancelText?: string;
  onCancel?: () => void;
  class?: string;
}

export interface FormErrors {
  [field: string]: string[];
}

// ============================================================================
// Layout Types
// ============================================================================

export interface SidebarItem {
  id: string;
  label: string;
  icon?: string;
  path?: string;
  onClick?: () => void;
  disabled?: boolean;
  badge?: string | number;
  children?: SidebarItem[];
}

export interface TabItem {
  id: string;
  label: string;
  content: any;
  disabled?: boolean;
  closable?: boolean;
}

export interface BreadcrumbItem {
  label: string;
  path?: string;
  onClick?: () => void;
}

// ============================================================================
// Chart & Visualization Types
// ============================================================================

export interface ChartDataPoint {
  label: string;
  value: number;
  color?: string;
}

export interface TimeSeriesPoint {
  timestamp: string;
  value: number;
}

export interface MetricCardProps {
  title: string;
  value: string | number;
  change?: {
    value: number;
    type: 'increase' | 'decrease';
    period: string;
  };
  icon?: string;
  color?: 'blue' | 'green' | 'red' | 'yellow' | 'purple';
}

// ============================================================================
// Loading & Error States
// ============================================================================

export interface LoadingState {
  isLoading: boolean;
  message?: string;
  progress?: number;
}

export interface ErrorState {
  hasError: boolean;
  message?: string;
  code?: string;
  retry?: () => void;
}

export interface AsyncState<T> {
  data?: T;
  loading: boolean;
  error?: string;
}

// ============================================================================
// Navigation & Routing
// ============================================================================

export interface NavigationItem {
  id: string;
  label: string;
  path: string;
  icon?: string;
  children?: NavigationItem[];
  exact?: boolean;
}

export interface RouteDefinition {
  path: string;
  component: any;
  exact?: boolean;
  title?: string;
  meta?: Record<string, any>;
}

// ============================================================================
// Theme System
// ============================================================================

export interface ThemeColors {
  primary: string;
  secondary: string;
  success: string;
  warning: string;
  error: string;
  info: string;
  background: string;
  surface: string;
  text: string;
  textSecondary: string;
  border: string;
}

export interface ThemeConfig {
  colors: ThemeColors;
  borderRadius: string;
  spacing: Record<string, string>;
  typography: {
    fontFamily: string;
    fontSize: Record<string, string>;
    fontWeight: Record<string, string>;
  };
  shadows: Record<string, string>;
  transitions: Record<string, string>;
}

// ============================================================================
// Event Types
// ============================================================================

export interface UIEvent<T = any> {
  type: string;
  payload?: T;
  timestamp: number;
}

export interface KeyboardShortcut {
  key: string;
  modifier?: 'ctrl' | 'alt' | 'shift' | 'meta';
  action: () => void;
  description?: string;
}

// ============================================================================
// Utility UI Types
// ============================================================================

export type ComponentSize = 'xs' | 'sm' | 'md' | 'lg' | 'xl';
export type ComponentVariant = 'primary' | 'secondary' | 'success' | 'warning' | 'error' | 'info';
export type Position = 'top' | 'bottom' | 'left' | 'right' | 'center';
export type Alignment = 'start' | 'center' | 'end' | 'stretch';

export interface ComponentBaseProps {
  class?: string;
  style?: Record<string, string | number>;
  id?: string;
  'data-testid'?: string;
}
