/**
 * ExpandableSection - 확장 가능한 섹션 컴포넌트
 * SolidJS-UI-Implementation-Guide.md를 기반으로 구현
 */

import { Component, JSX, Show } from 'solid-js';

interface ExpandableSectionProps {
  title: string;
  isExpanded: boolean;
  onToggle: (expanded: boolean) => void;
  icon?: string;
  children: JSX.Element;
}

export const ExpandableSection: Component<ExpandableSectionProps> = (props) => {
  const handleToggle = () => {
    props.onToggle(!props.isExpanded);
  };

  return (
    <div class="bg-white dark:bg-gray-800 rounded-lg shadow-md border border-gray-200 dark:border-gray-700">
      {/* 헤더 */}
      <button
        onClick={handleToggle}
        class="w-full px-6 py-4 flex items-center justify-between text-left hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors rounded-t-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-inset"
      >
        <div class="flex items-center space-x-3">
          {props.icon && (
            <span class="text-lg">{props.icon}</span>
          )}
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">
            {props.title}
          </h3>
        </div>
        <div class={`transition-transform duration-200 ${props.isExpanded ? 'rotate-180' : ''}`}>
          <svg class="w-5 h-5 text-gray-500 dark:text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
          </svg>
        </div>
      </button>

      {/* 콘텐츠 */}
      <Show when={props.isExpanded}>
        <div class="px-6 pb-6 border-t border-gray-200 dark:border-gray-700">
          <div class="pt-4">
            {props.children}
          </div>
        </div>
      </Show>
    </div>
  );
};
