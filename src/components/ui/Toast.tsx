import { Show, createSignal, createEffect, onCleanup } from 'solid-js';
import { Portal } from 'solid-js/web';
import { splitProps } from 'solid-js';
import type { ToastProps } from '../../types/ui';

/**
 * Toast 알림 컴포넌트
 * 
 * 기능:
 * - 다양한 메시지 타입 (success, error, warning, info)
 * - 위치 설정 (top-right, top-left, bottom-right, bottom-left)
 * - 자동 닫기 (duration 설정 가능)
 * - 수동 닫기 버튼
 * - 애니메이션 트랜지션
 * - 접근성 지원
 */
const Toast = (props: ToastProps) => {
  const [local] = splitProps(props, [
    'type',
    'message',
    'duration',
    'position',
    'onClose'
  ]);

  const [visible, setVisible] = createSignal(true);

  // 자동 닫기 타이머
  createEffect(() => {
    if (local.duration !== 0) {
      const timer = setTimeout(() => {
        handleClose();
      }, local.duration || 5000);

      onCleanup(() => clearTimeout(timer));
    }
  });

  const handleClose = () => {
    setVisible(false);
    // 애니메이션 완료 후 실제 제거
    setTimeout(() => {
      local.onClose?.();
    }, 300);
  };

  const getPositionClasses = () => {
    const position = local.position || 'top-right';
    
    const positions: Record<string, string> = {
      'top-right': 'top-4 right-4',
      'top-left': 'top-4 left-4',
      'bottom-right': 'bottom-4 right-4',
      'bottom-left': 'bottom-4 left-4'
    };
    
    return positions[position];
  };

  const getTypeClasses = () => {
    const type = local.type;
    
    const types: Record<string, { bg: string; border: string; text: string; icon: string }> = {
      success: {
        bg: 'bg-green-50',
        border: 'border-green-200',
        text: 'text-green-800',
        icon: 'text-green-400'
      },
      error: {
        bg: 'bg-red-50',
        border: 'border-red-200',
        text: 'text-red-800',
        icon: 'text-red-400'
      },
      warning: {
        bg: 'bg-yellow-50',
        border: 'border-yellow-200',
        text: 'text-yellow-800',
        icon: 'text-yellow-400'
      },
      info: {
        bg: 'bg-blue-50',
        border: 'border-blue-200',
        text: 'text-blue-800',
        icon: 'text-blue-400'
      }
    };
    
    return types[type];
  };

  const getIcon = () => {
    const type = local.type;
    
    switch (type) {
      case 'success':
        return (
          <svg class="w-5 h-5" fill="currentColor" viewBox="0 0 20 20">
            <path 
              fill-rule="evenodd" 
              d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" 
              clip-rule="evenodd" 
            />
          </svg>
        );
      case 'error':
        return (
          <svg class="w-5 h-5" fill="currentColor" viewBox="0 0 20 20">
            <path 
              fill-rule="evenodd" 
              d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" 
              clip-rule="evenodd" 
            />
          </svg>
        );
      case 'warning':
        return (
          <svg class="w-5 h-5" fill="currentColor" viewBox="0 0 20 20">
            <path 
              fill-rule="evenodd" 
              d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z" 
              clip-rule="evenodd" 
            />
          </svg>
        );
      case 'info':
      default:
        return (
          <svg class="w-5 h-5" fill="currentColor" viewBox="0 0 20 20">
            <path 
              fill-rule="evenodd" 
              d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" 
              clip-rule="evenodd" 
            />
          </svg>
        );
    }
  };

  const typeClasses = getTypeClasses();

  return (
    <Show when={visible()}>
      <Portal>
        <div
          class={`
            fixed z-50 
            ${getPositionClasses()}
            transition-opacity duration-300 ease-in-out
            ${visible() ? 'opacity-100' : 'opacity-0 pointer-events-none'}
          `}
        >
          <div
            class={`
              max-w-sm w-full
              ${typeClasses.bg}
              ${typeClasses.border}
              border
              rounded-lg
              shadow-lg
              p-4
            `}
            role="alert"
            aria-live="assertive"
            aria-atomic="true"
          >
            <div class="flex items-start">
              {/* Icon */}
              <div class={`flex-shrink-0 ${typeClasses.icon}`}>
                {getIcon()}
              </div>

              {/* Message */}
              <div class={`ml-3 ${typeClasses.text}`}>
                <p class="text-sm font-medium">
                  {local.message}
                </p>
              </div>

              {/* Close Button */}
              <div class="ml-auto pl-3">
                <div class="-mx-1.5 -my-1.5">
                  <button
                    type="button"
                    class={`
                      inline-flex rounded-md p-1.5
                      ${typeClasses.text}
                      hover:${typeClasses.bg}
                      focus:outline-none
                      focus:ring-2
                      focus:ring-offset-2
                      focus:ring-offset-${typeClasses.bg}
                      focus:ring-indigo-500
                      transition-colors
                    `}
                    onClick={handleClose}
                    aria-label="알림 닫기"
                  >
                    <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
                      <path 
                        fill-rule="evenodd" 
                        d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z" 
                        clip-rule="evenodd" 
                      />
                    </svg>
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </Portal>
    </Show>
  );
};

export default Toast;
