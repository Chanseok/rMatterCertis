import { Show, createSignal, onMount } from 'solid-js';
import { splitProps } from 'solid-js';
import type { VendorManagementProps } from '../../types/ui';
import { useVendorStore } from '../../stores/index.tsx';
import { Button, DataTable, Modal, Toast, Spinner } from '../ui';
import VendorForm from './VendorForm';

/**
 * VendorManagement 컴포넌트
 * 
 * 기능:
 * - 벤더 목록 표시 및 관리
 * - 벤더 생성/수정/삭제
 * - 실시간 상태 업데이트
 */
const VendorManagement = (props: VendorManagementProps) => {
  const [local] = splitProps(props, ['class']);
  
  const vendorStore = useVendorStore();

  // Local state
  const [selectedVendor, setSelectedVendor] = createSignal(null);
  const [showForm, setShowForm] = createSignal(false);
  const [formMode, setFormMode] = createSignal<'create' | 'edit'>('create');
  const [showDeleteModal, setShowDeleteModal] = createSignal(false);
  const [deletingVendor, setDeletingVendor] = createSignal(null);

  // Initialize data on mount
  onMount(async () => {
    await vendorStore.loadAllVendors();
  });

  // Table columns configuration
  const columns = [
    {
      key: 'vendor_name' as keyof any,
      title: '벤더명',
      sortable: true,
      render: (value: string) => (
        <div class="font-medium text-gray-900">{value}</div>
      )
    },
    {
      key: 'company_legal_name' as keyof any,
      title: '법인명',
      sortable: true,
      render: (value: string) => (
        <div class="text-gray-600">{value}</div>
      )
    },
    {
      key: 'vendor_number' as keyof any,
      title: '벤더 번호',
      sortable: true,
      align: 'center' as const,
      render: (value: number) => (
        <span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-blue-100 text-blue-800">
          {value}
        </span>
      )
    },
    {
      key: 'created_at' as keyof any,
      title: '생성일',
      sortable: true,
      render: (value: string) => (
        <div class="text-sm text-gray-500">
          {new Date(value).toLocaleDateString('ko-KR')}
        </div>
      )
    },
    {
      key: 'actions' as keyof any,
      title: '작업',
      align: 'center' as const,
      render: (_: any, record: any) => (
        <div class="flex items-center justify-center space-x-2">
          <Button
            size="sm"
            variant="outline"
            onClick={() => handleEdit(record)}
          >
            수정
          </Button>
          <Button
            size="sm"
            variant="danger"
            onClick={() => handleDeleteClick(record)}
          >
            삭제
          </Button>
        </div>
      )
    }
  ];

  // Event handlers
  const handleCreate = () => {
    setFormMode('create');
    setSelectedVendor(null);
    setShowForm(true);
  };

  const handleEdit = (vendor: any) => {
    setFormMode('edit');
    setSelectedVendor(vendor);
    setShowForm(true);
  };

  const handleDeleteClick = (vendor: any) => {
    setDeletingVendor(vendor);
    setShowDeleteModal(true);
  };

  const handleFormSubmit = async (data: any) => {
    try {
      if (formMode() === 'create') {
        await vendorStore.createVendor(data);
      } else if (selectedVendor()) {
        await vendorStore.updateVendor((selectedVendor() as any).vendor_id, data);
      }
      setShowForm(false);
      setSelectedVendor(null);
    } catch (error) {
      console.error('Form submission error:', error);
    }
  };

  const handleFormCancel = () => {
    setShowForm(false);
    setSelectedVendor(null);
  };

  const handleDeleteConfirm = async () => {
    const vendor = deletingVendor();
    if (!vendor) return;

    try {
      await vendorStore.deleteVendor((vendor as any).vendor_id);
      setShowDeleteModal(false);
      setDeletingVendor(null);
    } catch (error) {
      console.error('Delete error:', error);
    }
  };

  const handleDeleteCancel = () => {
    setShowDeleteModal(false);
    setDeletingVendor(null);
  };

  const handleRowClick = (vendor: any) => {
    // 행 클릭 시 편집 모드로 이동
    handleEdit(vendor);
  };

  const handleRefresh = async () => {
    await vendorStore.refresh();
  };

  const containerClasses = () => {
    return [
      'space-y-6',
      local.class || ''
    ].filter(Boolean).join(' ');
  };

  return (
    <div class={containerClasses()}>
      {/* Header */}
      <div class="flex items-center justify-between">
        <div>
          <h1 class="text-2xl font-semibold text-gray-900">벤더 관리</h1>
          <p class="mt-1 text-sm text-gray-600">
            Matter 인증 벤더 정보를 관리합니다.
          </p>
        </div>
        <div class="flex items-center space-x-3">
          <Button
            variant="outline"
            onClick={handleRefresh}
            loading={vendorStore.state.loading}
          >
            새로고침
          </Button>
          <Button onClick={handleCreate}>
            벤더 추가
          </Button>
        </div>
      </div>

      {/* Stats */}
      <div class="bg-white shadow rounded-lg p-6">
        <div class="grid grid-cols-1 gap-5 sm:grid-cols-2">
          <div class="px-4 py-5 bg-gray-50 shadow rounded-lg overflow-hidden sm:p-6">
            <dt class="text-sm font-medium text-gray-500 truncate">
              전체 벤더
            </dt>
            <dd class="mt-1 text-3xl font-semibold text-gray-900">
              {vendorStore.state.vendors.length}
            </dd>
          </div>
          <div class="px-4 py-5 bg-gray-50 shadow rounded-lg overflow-hidden sm:p-6">
            <dt class="text-sm font-medium text-gray-500 truncate">
              선택된 벤더
            </dt>
            <dd class="mt-1 text-3xl font-semibold text-gray-900">
              {vendorStore.state.selectedVendor ? 1 : 0}
            </dd>
          </div>
        </div>
      </div>

      {/* Data Table */}
      <DataTable<any>
        data={vendorStore.state.vendors}
        columns={columns}
        loading={vendorStore.state.loading}
        onRowClick={handleRowClick}
        rowKey="vendor_id"
      />

      {/* Vendor Form Modal */}
      <VendorForm
        vendor={selectedVendor()}
        mode={formMode()}
        onSubmit={handleFormSubmit}
        onCancel={handleFormCancel}
        loading={vendorStore.state.loading}
        isModal
        modalOpen={showForm()}
      />

      {/* Delete Confirmation Modal */}
      <Modal
        open={showDeleteModal()}
        onClose={handleDeleteCancel}
        title="벤더 삭제"
        size="sm"
        actions={
          <div class="flex justify-end space-x-3">
            <Button
              variant="outline"
              onClick={handleDeleteCancel}
              disabled={vendorStore.state.loading}
            >
              취소
            </Button>
            <Button
              variant="danger"
              onClick={handleDeleteConfirm}
              loading={vendorStore.state.loading}
              disabled={vendorStore.state.loading}
            >
              삭제
            </Button>
          </div>
        }
      >
        <div class="text-sm text-gray-500">
          <Show when={deletingVendor()}>
            <p>
              "<strong class="text-gray-900">{(deletingVendor() as any)?.vendor_name}</strong>" 벤더를 정말 삭제하시겠습니까?
            </p>
            <p class="mt-2 text-red-600">
              삭제된 데이터는 복구할 수 없습니다.
            </p>
          </Show>
        </div>
      </Modal>

      {/* Loading Overlay */}
      <Show when={vendorStore.state.loading}>
        <div class="fixed inset-0 bg-black bg-opacity-25 flex items-center justify-center z-50">
          <div class="bg-white rounded-lg p-6 shadow-xl">
            <Spinner size="lg" label="처리 중..." center />
          </div>
        </div>
      </Show>

      {/* Error Toast */}
      <Show when={vendorStore.state.error}>
        <Toast
          type="error"
          message={vendorStore.state.error!}
          onClose={() => vendorStore.clearError()}
        />
      </Show>
    </div>
  );
};

export default VendorManagement;
