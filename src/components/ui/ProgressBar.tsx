import { Show } from 'solid-js';
import { splitProps } from 'solid-js';
import type { ProgressBarProps } from '../../types/ui';

/**
 * ProgressBar 컴포넌트
 * 
 * 기능:
 * - 진행률 표시 (0-100%)
 * - 다양한 크기 지원 (sm, md, lg)
 * - 상태별 색상 (default, success, warning, error)
 * - 애니메이션 지원
 * - 라벨 표시 옵션
 * - 접근성 지원 (ARIA)
 */
const ProgressBar = (props: ProgressBarProps) => {
  const [local, others] = splitProps(props, [
    'value',
    'max',
    'showLabel',
    'variant',
    'size',
    'animated',
    'class'
  ]);

  const percentage = () => {
    const max = local.max || 100;
    const value = Math.min(Math.max(local.value || 0, 0), max);
    return Math.round((value / max) * 100);
  };

  const getBaseClasses = () => {
    return [
      'w-full',
      'bg-gray-200',
      'rounded-full',
      'overflow-hidden',
      'transition-all',
      'duration-300'
    ].join(' ');
  };

  const getSizeClasses = () => {
    const size = local.size || 'md';
    
    const sizes = {
      sm: 'h-2',
      md: 'h-3',
      lg: 'h-4'
    };
    
    return sizes[size];
  };

  const getVariantClasses = () => {
    const variant = local.variant || 'default';
    
    const variants = {
      default: 'bg-blue-600',
      success: 'bg-green-600',
      warning: 'bg-yellow-500',
      error: 'bg-red-600'
    };
    
    return variants[variant];
  };

  const getProgressClasses = () => {
    return [
      'h-full',
      'transition-all',
      'duration-500',
      'ease-out',
      getVariantClasses(),
      local.animated ? 'animate-pulse' : ''
    ].filter(Boolean).join(' ');
  };

  const containerClasses = () => {
    return [
      getBaseClasses(),
      getSizeClasses(),
      local.class || ''
    ].filter(Boolean).join(' ');
  };

  return (
    <div class="w-full">
      {/* Label */}
      <Show when={local.showLabel}>
        <div class="flex justify-between items-center mb-2">
          <span class="text-sm font-medium text-gray-700">
            진행률
          </span>
          <span class="text-sm text-gray-500">
            {percentage()}%
          </span>
        </div>
      </Show>

      {/* Progress Bar */}
      <div
        class={containerClasses()}
        role="progressbar"
        aria-valuenow={local.value}
        aria-valuemin={0}
        aria-valuemax={local.max || 100}
        aria-valuetext={`${percentage()}%`}
        {...others}
      >
        <div
          class={getProgressClasses()}
          style={{ width: `${percentage()}%` }}
        />
      </div>
      
      {/* Value Display for Small Progress Bars */}
      <Show when={local.showLabel && (local.size === 'sm')}>
        <div class="mt-1 text-xs text-gray-500 text-right">
          {local.value} / {local.max || 100}
        </div>
      </Show>
    </div>
  );
};

export default ProgressBar;
