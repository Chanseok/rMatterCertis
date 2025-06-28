import { createSignal, Show } from 'solid-js';
import { splitProps } from 'solid-js';
import type { VendorFormProps, VendorFormData } from '../../types/ui';
import { Button, Modal } from '../ui';

/**
 * VendorForm 컴포넌트
 * 
 * 기능:
 * - 벤더 생성/수정 폼
 * - 실시간 유효성 검사
 * - 로딩 상태 처리
 * - 에러 표시
 * - 모달 또는 인라인 모드
 */
const VendorForm = (props: VendorFormProps) => {
  const [local, others] = splitProps(props, [
    'vendor',
    'mode',
    'onSubmit',
    'onCancel',
    'loading',
    'isModal',
    'modalOpen',
    'class'
  ]);

  // Form state
  const [formData, setFormData] = createSignal<VendorFormData>({
    name: local.vendor?.name || '',
    url: local.vendor?.url || '',
    description: local.vendor?.description || ''
  });

  const [errors, setErrors] = createSignal<Record<string, string>>({});

  // Update form when vendor prop changes
  if (local.vendor && local.mode === 'edit') {
    setFormData({
      name: local.vendor.name,
      url: local.vendor.url,
      description: local.vendor.description || ''
    });
  }

  const validateForm = (): boolean => {
    const newErrors: Record<string, string> = {};
    const data = formData();

    if (!data.name.trim()) {
      newErrors.name = '벤더명은 필수입니다.';
    }

    if (!data.url.trim()) {
      newErrors.url = 'URL은 필수입니다.';
    } else {
      try {
        new URL(data.url);
      } catch {
        newErrors.url = '올바른 URL 형식이 아닙니다.';
      }
    }

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleSubmit = async (e: Event) => {
    e.preventDefault();
    
    if (!validateForm()) return;

    const data = formData();
    
    if (local.mode === 'edit' && local.vendor) {
      const updateData = {
        id: local.vendor.id,
        name: data.name,
        url: data.url,
        description: data.description
      };
      await local.onSubmit?.(updateData);
    } else {
      await local.onSubmit?.(data);
    }
  };

  const handleInputChange = (field: keyof VendorFormData, value: string) => {
    setFormData(prev => ({ ...prev, [field]: value }));
    
    // Clear error when user starts typing
    if (errors()[field]) {
      setErrors(prev => {
        const newErrors = { ...prev };
        delete newErrors[field];
        return newErrors;
      });
    }
  };

  const handleModalSubmit = async () => {
    const mockEvent = new Event('submit');
    await handleSubmit(mockEvent);
  };

  const handleCancel = () => {
    setFormData({
      name: '',
      url: '',
      description: ''
    });
    setErrors({});
    local.onCancel?.();
  };

  const formClasses = () => {
    return [
      'space-y-6',
      local.class || ''
    ].filter(Boolean).join(' ');
  };

  const inputClasses = (hasError: boolean) => {
    return [
      'block w-full px-3 py-2',
      'border rounded-md shadow-sm',
      'placeholder-gray-400',
      'focus:outline-none focus:ring-2 focus:ring-offset-2',
      hasError 
        ? 'border-red-300 focus:border-red-500 focus:ring-red-500'
        : 'border-gray-300 focus:border-blue-500 focus:ring-blue-500',
      'text-sm'
    ].join(' ');
  };

  const FormContent = () => (
    <form onSubmit={handleSubmit} class={formClasses()} {...others}>
      {/* Vendor Name */}
      <div>
        <label 
          for="vendor-name" 
          class="block text-sm font-medium text-gray-700 mb-1"
        >
          벤더명 *
        </label>
        <input
          id="vendor-name"
          type="text"
          class={inputClasses(!!errors().name)}
          placeholder="벤더명을 입력하세요"
          value={formData().name}
          onInput={(e) => handleInputChange('name', e.target.value)}
          required
        />
        <Show when={errors().name}>
          <p class="mt-1 text-sm text-red-600">{errors().name}</p>
        </Show>
      </div>

      {/* Vendor URL */}
      <div>
        <label 
          for="vendor-url" 
          class="block text-sm font-medium text-gray-700 mb-1"
        >
          URL *
        </label>
        <input
          id="vendor-url"
          type="url"
          class={inputClasses(!!errors().url)}
          placeholder="https://example.com"
          value={formData().url}
          onInput={(e) => handleInputChange('url', e.target.value)}
          required
        />
        <Show when={errors().url}>
          <p class="mt-1 text-sm text-red-600">{errors().url}</p>
        </Show>
      </div>

      {/* Description */}
      <div>
        <label 
          for="vendor-description" 
          class="block text-sm font-medium text-gray-700 mb-1"
        >
          설명
        </label>
        <textarea
          id="vendor-description"
          rows="3"
          class={inputClasses(!!errors().description)}
          placeholder="벤더에 대한 설명을 입력하세요 (선택사항)"
          value={formData().description || ''}
          onInput={(e) => handleInputChange('description', e.target.value)}
        />
        <Show when={errors().description}>
          <p class="mt-1 text-sm text-red-600">{errors().description}</p>
        </Show>
      </div>

      {/* Actions */}
      <Show when={!local.isModal}>
        <div class="flex justify-end space-x-3 pt-4">
          <Button
            type="button"
            variant="outline"
            onClick={handleCancel}
            disabled={local.loading}
          >
            취소
          </Button>
          <Button
            type="submit"
            loading={local.loading}
            disabled={local.loading}
          >
            {local.mode === 'edit' ? '수정' : '생성'}
          </Button>
        </div>
      </Show>
    </form>
  );

  return (
    <Show
      when={local.isModal}
      fallback={<FormContent />}
    >
      <Modal
        open={local.modalOpen || false}
        onClose={handleCancel}
        title={local.mode === 'edit' ? '벤더 수정' : '새 벤더 추가'}
        size="md"
        actions={
          <div class="flex justify-end space-x-3">
            <Button
              variant="outline"
              onClick={handleCancel}
              disabled={local.loading}
            >
              취소
            </Button>
            <Button
              onClick={handleModalSubmit}
              loading={local.loading}
              disabled={local.loading}
            >
              {local.mode === 'edit' ? '수정' : '생성'}
            </Button>
          </div>
        }
      >
        <FormContent />
      </Modal>
    </Show>
  );
};

export default VendorForm;
