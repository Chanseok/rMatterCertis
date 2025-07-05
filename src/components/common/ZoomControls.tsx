/**
 * ZoomControls - 화면 확대/축소 컨트롤 컴포넌트
 */

import { Component } from 'solid-js';
import { windowState } from '../../stores/windowStore';

export const ZoomControls: Component = () => {
  const formatZoom = () => {
    return Math.round(windowState.state.zoomLevel * 100);
  };

  return (
    <div style="display: flex; align-items: center; gap: 8px; padding: 8px; background: #f8fafc; border: 1px solid #e5e7eb; border-radius: 6px;">
      <button
        onClick={() => windowState.zoomOut()}
        style="padding: 4px 8px; background: #6b7280; color: white; border: none; border-radius: 4px; font-size: 12px; cursor: pointer; transition: background-color 0.2s;"
        onMouseOver={(e) => e.currentTarget.style.background = '#4b5563'}
        onMouseOut={(e) => e.currentTarget.style.background = '#6b7280'}
        title="축소"
      >
        −
      </button>
      
      <span style="font-size: 12px; font-weight: 500; color: #374151; min-width: 50px; text-align: center;">
        {formatZoom()}%
      </span>
      
      <button
        onClick={() => windowState.zoomIn()}
        style="padding: 4px 8px; background: #6b7280; color: white; border: none; border-radius: 4px; font-size: 12px; cursor: pointer; transition: background-color 0.2s;"
        onMouseOver={(e) => e.currentTarget.style.background = '#4b5563'}
        onMouseOut={(e) => e.currentTarget.style.background = '#6b7280'}
        title="확대"
      >
        +
      </button>
      
      <button
        onClick={() => windowState.resetZoom()}
        style="padding: 4px 8px; background: #3b82f6; color: white; border: none; border-radius: 4px; font-size: 12px; cursor: pointer; transition: background-color 0.2s;"
        onMouseOver={(e) => e.currentTarget.style.background = '#2563eb'}
        onMouseOut={(e) => e.currentTarget.style.background = '#3b82f6'}
        title="원래 크기로"
      >
        🔍
      </button>
    </div>
  );
};
