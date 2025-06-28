import { Show } from 'solid-js';
import { splitProps } from 'solid-js';
import type { SpinnerProps } from '../../types/ui';

/**
 * Spinner (로딩 인디케이터) 컴포넌트
 * 
 * 기능:
 * - 다양한 크기 지원 (xs, sm, md, lg, xl)
 * - 여러 스타일 (circular, dots, bars)
 * - 색상 커스터마이징
 * - 텍스트 라벨 표시
 * - 중앙 정렬 옵션
 */
const Spinner = (props: SpinnerProps) => {
  const [local, others] = splitProps(props, [
    'size',
    'variant',
    'color',
    'label',
    'center',
    'class'
  ]);

  const getSizeClasses = () => {
    const size = local.size || 'md';
    
    const sizes: Record<string, string> = {
      xs: 'w-4 h-4',
      sm: 'w-5 h-5',
      md: 'w-6 h-6',
      lg: 'w-8 h-8',
      xl: 'w-12 h-12'
    };
    
    return sizes[size];
  };

  const getColorClasses = () => {
    const color = local.color || 'blue';
    
    const colors: Record<string, string> = {
      blue: 'text-blue-600',
      gray: 'text-gray-600',
      green: 'text-green-600',
      red: 'text-red-600',
      yellow: 'text-yellow-600',
      white: 'text-white'
    };
    
    return colors[color];
  };

  const CircularSpinner = () => (
    <svg
      class={`animate-spin ${getSizeClasses()} ${getColorClasses()}`}
      xmlns="http://www.w3.org/2000/svg"
      fill="none"
      viewBox="0 0 24 24"
      {...others}
    >
      <circle
        class="opacity-25"
        cx="12"
        cy="12"
        r="10"
        stroke="currentColor"
        stroke-width="4"
      />
      <path
        class="opacity-75"
        fill="currentColor"
        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
      />
    </svg>
  );

  const DotsSpinner = () => {
    const dotSize = local.size === 'xs' ? 'w-1.5 h-1.5' : 
                   local.size === 'sm' ? 'w-2 h-2' :
                   local.size === 'lg' ? 'w-3 h-3' :
                   local.size === 'xl' ? 'w-4 h-4' : 'w-2.5 h-2.5';
    
    return (
      <div class="flex space-x-1">
        <div 
          class={`${dotSize} bg-current rounded-full animate-bounce ${getColorClasses()}`}
          style={{ "animation-delay": "0ms" }}
        />
        <div 
          class={`${dotSize} bg-current rounded-full animate-bounce ${getColorClasses()}`}
          style={{ "animation-delay": "150ms" }}
        />
        <div 
          class={`${dotSize} bg-current rounded-full animate-bounce ${getColorClasses()}`}
          style={{ "animation-delay": "300ms" }}
        />
      </div>
    );
  };

  const BarsSpinner = () => {
    const barWidth = local.size === 'xs' ? 'w-0.5' : 
                    local.size === 'sm' ? 'w-1' :
                    local.size === 'lg' ? 'w-1.5' :
                    local.size === 'xl' ? 'w-2' : 'w-1';
    
    const barHeight = local.size === 'xs' ? 'h-4' : 
                     local.size === 'sm' ? 'h-5' :
                     local.size === 'lg' ? 'h-8' :
                     local.size === 'xl' ? 'h-12' : 'h-6';
    
    return (
      <div class="flex items-end space-x-1">
        <div 
          class={`${barWidth} ${barHeight} bg-current animate-pulse ${getColorClasses()}`}
          style={{ "animation-delay": "0ms" }}
        />
        <div 
          class={`${barWidth} ${barHeight} bg-current animate-pulse ${getColorClasses()}`}
          style={{ "animation-delay": "100ms" }}
        />
        <div 
          class={`${barWidth} ${barHeight} bg-current animate-pulse ${getColorClasses()}`}
          style={{ "animation-delay": "200ms" }}
        />
        <div 
          class={`${barWidth} ${barHeight} bg-current animate-pulse ${getColorClasses()}`}
          style={{ "animation-delay": "300ms" }}
        />
      </div>
    );
  };

  const SpinnerComponent = () => {
    const variant = local.variant || 'circular';
    
    switch (variant) {
      case 'dots':
        return <DotsSpinner />;
      case 'bars':
        return <BarsSpinner />;
      case 'circular':
      default:
        return <CircularSpinner />;
    }
  };

  const containerClasses = () => {
    return [
      local.center ? 'flex flex-col items-center justify-center' : 'inline-flex items-center',
      local.class || ''
    ].filter(Boolean).join(' ');
  };

  return (
    <div class={containerClasses()} role="status" aria-live="polite">
      <SpinnerComponent />
      
      <Show when={local.label}>
        <span class={`
          text-sm text-gray-600 
          ${local.center ? 'mt-2' : 'ml-2'}
        `}>
          {local.label}
        </span>
      </Show>
      
      {/* Screen reader only text */}
      <span class="sr-only">로딩 중...</span>
    </div>
  );
};

export default Spinner;
