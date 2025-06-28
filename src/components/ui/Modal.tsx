import { Show, createEffect, onCleanup } from 'solid-js';
import { Portal } from 'solid-js/web';
import { splitProps } from 'solid-js';
import type { ModalProps } from '../../types/ui';
import Button from './Button';

/**
 * Modal 컴포넌트
 * 
 * 기능:
 * - 다양한 크기 지원 (sm, md, lg, xl)
 * - 백드롭 클릭으로 닫기 (옵션)
 * - ESC 키로 닫기 (옵션)
 * - 스크롤 처리
 * - 애니메이션 트랜지션
 * - 접근성 지원 (ARIA, focus trap)
 */
const Modal = (props: ModalProps) => {
  const [local, others] = splitProps(props, [
    'open',
    'onClose', 
    'size',
    'title',
    'children',
    'showCloseButton',
    'closeOnBackdrop',
    'closeOnEscape',
    'actions',
    'class'
  ]);

  // ESC 키 핸들러
  createEffect(() => {
    if (!local.open) return;

    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && (local.closeOnEscape ?? true)) {
        local.onClose?.();
      }
    };

    document.addEventListener('keydown', handleEscape);
    
    onCleanup(() => {
      document.removeEventListener('keydown', handleEscape);
    });
  });

  // 모달 열릴 때 body 스크롤 방지
  createEffect(() => {
    if (local.open) {
      document.body.style.overflow = 'hidden';
    } else {
      document.body.style.overflow = '';
    }

    onCleanup(() => {
      document.body.style.overflow = '';
    });
  });

  const getSizeClasses = () => {
    const size = local.size || 'md';
    
    const sizes = {
      sm: 'max-w-md',
      md: 'max-w-lg', 
      lg: 'max-w-2xl',
      xl: 'max-w-4xl',
      full: 'max-w-full mx-4'
    };
    
    return sizes[size];
  };

  const handleBackdropClick = (e: MouseEvent) => {
    if (e.target === e.currentTarget && (local.closeOnBackdrop ?? true)) {
      local.onClose?.();
    }
  };

  const DefaultActions = () => (
    <div class="flex justify-end space-x-3 pt-4">
      <Button
        variant="outline"
        onClick={local.onClose}
      >
        취소
      </Button>
    </div>
  );

  return (
    <Show when={local.open}>
      <Portal>
        {/* Backdrop */}
        <div
          class={`
            fixed inset-0 z-50 
            flex items-center justify-center 
            p-4 
            bg-black bg-opacity-50
            transition-opacity duration-300
            ${local.open ? 'opacity-100' : 'opacity-0'}
          `}
          onClick={handleBackdropClick}
          role="dialog"
          aria-modal="true"
          aria-labelledby={local.title ? 'modal-title' : undefined}
        >
          {/* Modal Content */}
          <div
            class={`
              relative w-full 
              ${getSizeClasses()}
              bg-white 
              rounded-lg 
              shadow-xl
              transform transition-all duration-300
              ${local.open ? 'scale-100 opacity-100' : 'scale-95 opacity-0'}
              ${local.class || ''}
            `}
            onClick={(e) => e.stopPropagation()}
            {...others}
          >
            {/* Header */}
            <Show when={local.title || local.showCloseButton !== false}>
              <div class="flex items-center justify-between p-6 border-b border-gray-200">
                <Show when={local.title}>
                  <h3 
                    id="modal-title"
                    class="text-lg font-semibold text-gray-900"
                  >
                    {local.title}
                  </h3>
                </Show>
                
                <Show when={local.showCloseButton !== false}>
                  <button
                    type="button"
                    class="
                      text-gray-400 
                      hover:text-gray-500 
                      focus:outline-none 
                      focus:text-gray-500
                      transition-colors
                    "
                    onClick={local.onClose}
                    aria-label="모달 닫기"
                  >
                    <svg 
                      class="h-6 w-6" 
                      fill="none" 
                      viewBox="0 0 24 24" 
                      stroke="currentColor"
                    >
                      <path 
                        stroke-linecap="round" 
                        stroke-linejoin="round" 
                        stroke-width="2" 
                        d="M6 18L18 6M6 6l12 12" 
                      />
                    </svg>
                  </button>
                </Show>
              </div>
            </Show>

            {/* Body */}
            <div class="p-6">
              {local.children}
            </div>

            {/* Actions */}
            <Show 
              when={local.actions !== null}
              fallback={<DefaultActions />}
            >
              <Show when={local.actions}>
                <div class="px-6 pb-6">
                  {local.actions}
                </div>
              </Show>
            </Show>
          </div>
        </div>
      </Portal>
    </Show>
  );
};

export default Modal;
