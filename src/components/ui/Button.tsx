// Button component with multiple variants and states
import { splitProps } from 'solid-js';
import type { ButtonProps } from '../../types/ui';

const Button = (props: ButtonProps) => {
  const [local, others] = splitProps(props, [
    'variant', 
    'size', 
    'loading', 
    'disabled', 
    'fullWidth', 
    'children', 
    'class'
  ]);

  const getBaseClasses = () => {
    return [
      'inline-flex',
      'items-center',
      'justify-center',
      'font-medium',
      'rounded-md',
      'border',
      'transition-all',
      'duration-200',
      'focus:outline-none',
      'focus:ring-2',
      'focus:ring-offset-2',
      'disabled:opacity-50',
      'disabled:cursor-not-allowed',
      local.fullWidth ? 'w-full' : ''
    ].filter(Boolean).join(' ');
  };

  const getVariantClasses = () => {
    const variant = local.variant || 'primary';
    
    const variants = {
      primary: [
        'bg-blue-600',
        'border-blue-600',
        'text-white',
        'hover:bg-blue-700',
        'hover:border-blue-700',
        'focus:ring-blue-500',
        'active:bg-blue-800'
      ].join(' '),
      
      secondary: [
        'bg-gray-200',
        'border-gray-300',
        'text-gray-900',
        'hover:bg-gray-300',
        'hover:border-gray-400',
        'focus:ring-gray-500',
        'active:bg-gray-400'
      ].join(' '),
      
      danger: [
        'bg-red-600',
        'border-red-600',
        'text-white',
        'hover:bg-red-700',
        'hover:border-red-700',
        'focus:ring-red-500',
        'active:bg-red-800'
      ].join(' '),
      
      ghost: [
        'bg-transparent',
        'border-transparent',
        'text-gray-600',
        'hover:bg-gray-100',
        'hover:text-gray-900',
        'focus:ring-gray-500'
      ].join(' '),
      
      outline: [
        'bg-transparent',
        'border-gray-300',
        'text-gray-700',
        'hover:bg-gray-50',
        'hover:border-gray-400',
        'focus:ring-gray-500'
      ].join(' ')
    };
    
    return variants[variant];
  };

  const getSizeClasses = () => {
    const size = local.size || 'md';
    
    const sizes = {
      xs: 'px-2.5 py-1.5 text-xs',
      sm: 'px-3 py-2 text-sm',
      md: 'px-4 py-2 text-sm',
      lg: 'px-4 py-2 text-base',
      xl: 'px-6 py-3 text-base'
    };
    
    return sizes[size];
  };

  const isDisabled = () => local.disabled || local.loading;

  const buttonClasses = () => {
    return [
      getBaseClasses(),
      getVariantClasses(),
      getSizeClasses(),
      local.class || ''
    ].filter(Boolean).join(' ');
  };

  const LoadingSpinner = () => (
    <svg 
      class="animate-spin -ml-1 mr-2 h-4 w-4" 
      xmlns="http://www.w3.org/2000/svg" 
      fill="none" 
      viewBox="0 0 24 24"
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

  return (
    <button
      class={buttonClasses()}
      disabled={isDisabled()}
      {...others}
    >
      {local.loading && <LoadingSpinner />}
      {local.children}
    </button>
  );
};

export default Button;
