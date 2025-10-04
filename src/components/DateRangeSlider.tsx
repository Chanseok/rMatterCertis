/**
 * DateRangeSlider - 향상된 날짜 범위 슬라이더 컴포넌트
 * - 타이핑 입력과 슬라이더 양방향 동기화
 * - 기본: 주 단위 (7일), Option: 일 단위, Option+Shift: 월 단위 (30일)
 */

import { Component, createSignal, createEffect } from 'solid-js';

interface DateRangeSliderProps {
  minDate: string; // YYYY-MM-DD
  maxDate: string; // YYYY-MM-DD
  startDate: string; // YYYY-MM-DD
  endDate: string; // YYYY-MM-DD
  onChange: (start: string, end: string) => void;
}

export const DateRangeSlider: Component<DateRangeSliderProps> = (props) => {
  const [startInput, setStartInput] = createSignal(props.startDate);
  const [endInput, setEndInput] = createSignal(props.endDate);
  
  // Props 변경시 입력 동기화
  createEffect(() => {
    setStartInput(props.startDate);
    setEndInput(props.endDate);
  });
  
  // minDate 기준으로 상대적인 일수로 변환 (0부터 시작)
  const dateToNum = (dateStr: string): number => {
    const minTime = new Date(props.minDate).getTime();
    const targetTime = new Date(dateStr).getTime();
    return Math.round((targetTime - minTime) / (1000 * 60 * 60 * 24));
  };
  
  // 상대적인 일수를 날짜로 변환
  const numToDate = (num: number): string => {
    const minTime = new Date(props.minDate).getTime();
    const date = new Date(minTime + num * 1000 * 60 * 60 * 24);
    return date.toISOString().split('T')[0];
  };
  
  // 슬라이더 변경 핸들러 (세밀도 지원)
  const handleSliderChange = (value: number, isStart: boolean, event: Event) => {
    const inputEvent = event as InputEvent;
    const isOption = (inputEvent as any).altKey;  // macOS에서 Option 키는 altKey
    const isShift = (inputEvent as any).shiftKey;
    
    // 키 조합에 따라 스냅 단위 결정
    let finalValue = value;
    if (isOption && isShift) {
      // Option+Shift: 월 단위 (30일)
      finalValue = Math.round(value / 30) * 30;
    } else if (isOption) {
      // Option: 일 단위 (1일) - 값 그대로 사용
      finalValue = Math.round(value);
    } else {
      // 기본: 주 단위 (7일)
      finalValue = Math.round(value / 7) * 7;
    }
    
    const newDate = numToDate(finalValue);
    
    if (isStart) {
      setStartInput(newDate);
      if (newDate <= endInput()) {
        props.onChange(newDate, endInput());
      }
    } else {
      setEndInput(newDate);
      if (newDate >= startInput()) {
        props.onChange(startInput(), newDate);
      }
    }
  };
  
  // 입력 필드 변경 핸들러
  const handleInputChange = (value: string, isStart: boolean) => {
    // 로컬 signal 항상 업데이트 (타이핑 가능하도록)
    if (isStart) {
      setStartInput(value);
    } else {
      setEndInput(value);
    }
    
    // 유효한 날짜 형식인 경우에만 부모에 전달
    const dateRegex = /^\d{4}-\d{2}-\d{2}$/;
    if (!dateRegex.test(value)) return;
    
    // 날짜 범위 검증
    if (isStart) {
      if (value <= endInput()) {
        props.onChange(value, endInput());
      }
    } else {
      if (value >= startInput()) {
        props.onChange(startInput(), value);
      }
    }
  };
  
  const minNum = () => dateToNum(props.minDate);
  const maxNum = () => dateToNum(props.maxDate);
  const startNum = () => dateToNum(startInput());
  const endNum = () => dateToNum(endInput());
  
  return (
    <div class="bg-gradient-to-r from-blue-50 via-indigo-50 to-purple-50 border-2 border-blue-300 rounded-xl p-5 shadow-md">
      <div class="flex items-center justify-between mb-4">
        <div class="flex items-center gap-3">
          <span class="text-2xl">📅</span>
          <h3 class="text-base font-bold text-gray-800">Certification Date Range</h3>
        </div>
        <div class="flex items-center gap-3 text-sm">
          <div class="flex items-center gap-2 bg-white px-3 py-1.5 rounded-lg shadow-sm">
            <label class="text-gray-600 font-medium">From:</label>
            <input 
              type="text" 
              value={startInput()}
              placeholder="YYYY-MM-DD"
              class="border-2 border-blue-200 rounded px-2 py-1 focus:border-blue-500 focus:outline-none w-28 text-sm"
              onInput={e => handleInputChange(e.currentTarget.value, true)}
            />
          </div>
          <span class="text-gray-400">→</span>
          <div class="flex items-center gap-2 bg-white px-3 py-1.5 rounded-lg shadow-sm">
            <label class="text-gray-600 font-medium">To:</label>
            <input 
              type="text" 
              value={endInput()}
              placeholder="YYYY-MM-DD"
              class="border-2 border-blue-200 rounded px-2 py-1 focus:border-blue-500 focus:outline-none w-28 text-sm"
              onInput={e => handleInputChange(e.currentTarget.value, false)}
            />
          </div>
        </div>
      </div>
      
      <div class="space-y-3">
        <div class="flex items-center gap-3">
          <span class="text-xs font-medium text-gray-600 w-12">Start</span>
          <input 
            type="range" 
            min={minNum()} 
            max={maxNum()} 
            value={startNum()}
            step="1"
            class="flex-1 h-2 bg-blue-200 rounded-lg appearance-none cursor-pointer slider-thumb-blue"
            onInput={e => handleSliderChange(Number(e.currentTarget.value), true, e)}
          />
        </div>
        <div class="flex items-center gap-3">
          <span class="text-xs font-medium text-gray-600 w-12">End</span>
          <input 
            type="range" 
            min={minNum()} 
            max={maxNum()} 
            value={endNum()}
            step="1"
            class="flex-1 h-2 bg-purple-200 rounded-lg appearance-none cursor-pointer slider-thumb-purple"
            onInput={e => handleSliderChange(Number(e.currentTarget.value), false, e)}
          />
        </div>
      </div>
      
      <div class="flex justify-between items-center mt-3 text-xs text-gray-500">
        <span class="bg-white px-2 py-1 rounded">{props.minDate}</span>
        <div class="flex gap-2 text-[10px]">
          <span class="bg-blue-100 text-blue-700 px-2 py-0.5 rounded">드래그: 주</span>
          <span class="bg-green-100 text-green-700 px-2 py-0.5 rounded">⌥ Option: 일</span>
          <span class="bg-purple-100 text-purple-700 px-2 py-0.5 rounded">⌥⇧ Opt+Shift: 월</span>
        </div>
        <span class="bg-white px-2 py-1 rounded">{props.maxDate}</span>
      </div>
    </div>
  );
};
