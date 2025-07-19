
# React에서 SolidJS로 UI 마이그레이션 가이드

이 문서는 기존 React + MobX 기반의 UI를 SolidJS + Rust/Tauri 스택으로 효과적으로 마이그레이션하고, SolidJS의 장점을 최대한 활용하여 UI를 구현하는 방법을 안내합니다.

## 1. 핵심 개념: React vs SolidJS

| 개념 | React | SolidJS | 마이그레이션 전략 |
| --- | --- | --- | --- |
| **렌더링** | Virtual DOM (VDOM) | Fine-Grained Reactivity | 컴포넌트 전체를 재실행하는 대신, 데이터 변경 시 필요한 DOM만 직접 업데이트하도록 코드를 변경해야 합니다. `useState` -> `createSignal`, `useEffect` -> `createEffect`를 사용합니다. |
| **상태관리** | `useState`, `useReducer`, MobX | `createSignal`, `createStore` | React의 `useState`는 SolidJS의 `createSignal`로, 복잡한 객체 상태(MobX Store)는 `createStore`로 변환합니다. |
| **컴포넌트** | 함수 컴포넌트가 상태 변경 시 재실행 | 컴포넌트는 한 번만 실행. JSX 내의 Signal/Store 구독 부분만 재실행 | 컴포넌트 로직을 Signal과 Effect 중심으로 재구성해야 합니다. 컴포넌트 최상위 스코프에 비싼 연산을 두지 않도록 주의합니다. |
| **Props** | 일반 JavaScript 객체 | 반응성 객체 (getter 프록시) | Props를 구조 분해 할당(`destructuring`)하면 반응성을 잃습니다. `props.value` 형태로 직접 접근하거나, `splitProps`를 사용해야 합니다. |

---

## 2. 프로젝트 구조 변환 전략

기존 `src/ui` 구조를 SolidJS 프로젝트에 맞게 변환하는 전략입니다.

| 기존 (React) | 새로운 (SolidJS) | 설명 |
| --- | --- | --- |
| `src/ui/components` | `src/components` | SolidJS 컴포넌트들을 위치시킵니다. React 컴포넌트를 SolidJS 문법으로 변환합니다. |
| `src/ui/hooks` | `src/hooks` 또는 `src/signals` | React의 커스텀 훅은 SolidJS의 `createSignal`, `createEffect`를 활용한 커스텀 Signal 또는 유틸리티 함수로 변환합니다. |
| `src/ui/stores` | `src/stores` | MobX 스토어는 SolidJS의 `createStore`를 사용하여 반응형 스토어로 변환합니다. |
| `src/ui/viewmodels` | `src/viewmodels` (선택적) | ViewModel의 계산된 속성들은 SolidJS의 `createMemo`를 사용하여 메모이제이션된 Signal로 변환할 수 있습니다. 복잡한 UI 로직을 분리하는 패턴은 그대로 유지할 수 있습니다. |

---

## 3. 코드 변환 예시: `CrawlingDashboard`

`CrawlingDashboard.tsx` 컴포넌트를 SolidJS로 변환하는 예시입니다.

### 3.1. React + MobX (기존 코드)

```typescript
// src/ui/components/CrawlingDashboard.tsx

import React, { useEffect, useState, useMemo } from 'react';
import { observer } from 'mobx-react-lite';
import { useCrawlingStore } from '../hooks/useCrawlingStore';

const CrawlingDashboard: React.FC = observer(() => {
  const { status, progress, startCrawling, stopCrawling } = useCrawlingStore();
  const [localState, setLocalState] = useState(0);

  useEffect(() => {
    console.log('Status changed:', status);
  }, [status]);

  const derivedValue = useMemo(() => {
    return `Progress: ${progress.percentage}%`;
  }, [progress.percentage]);

  return (
    <div>
      <p>Status: {status}</p>
      <p>{derivedValue}</p>
      <button onClick={() => startCrawling()}>Start</button>
      <button onClick={() => stopCrawling()}>Stop</button>
    </div>
  );
});
```

### 3.2. SolidJS (변환 후 코드)

```typescript
// src/components/CrawlingDashboard.tsx

import { createSignal, createEffect, onCleanup, Component } from 'solid-js';
import { crawlingStore } from '../stores/crawlingStore'; // SolidJS 스토어

const CrawlingDashboard: Component = () => {
  // MobX Store -> SolidJS Store (createStore 사용)
  // useCrawlingStore() 훅 대신 스토어를 직접 가져와 사용합니다.
  const [state, setState] = crawlingStore;

  // useState -> createSignal
  const [localState, setLocalState] = createSignal(0);

  // useEffect -> createEffect
  createEffect(() => {
    // state.status가 변경될 때마다 이 블록이 자동으로 재실행됩니다.
    console.log('Status changed:', state.status);
  });

  // useMemo -> createMemo (또는 JSX에서 직접 함수 호출)
  // SolidJS는 JSX 내에서 함수를 호출하면 해당 함수가 의존하는 Signal이 변경될 때만
  // 해당 부분을 업데이트하므로, 간단한 경우는 createMemo가 필요 없습니다.
  const derivedValue = () => `Progress: ${state.progress.percentage}%`;

  // 컴포넌트는 한 번만 실행됩니다.
  console.log('Component rendered once');

  return (
    <div>
      {/* Signal/Store 값을 함수처럼 호출하여 구독합니다. */}
      <p>Status: {state.status}</p>
      <p>{derivedValue()}</p>
      
      {/* 이벤트 핸들러는 동일하게 사용합니다. */}
      <button onClick={() => setState('status', 'running')}>Start</button>
      <button onClick={() => setState('status', 'stopped')}>Stop</button>
    </div>
  );
};

export default CrawlingDashboard;
```

---

## 4. 상태 관리 마이그레이션: MobX Store -> SolidJS `createStore`

`AppStore.ts`와 같은 MobX 스토어를 SolidJS의 `createStore`로 변환하는 방법입니다.

### 4.1. MobX Store (기존 코드)

```typescript
// src/ui/stores/AppStore.ts
import { makeObservable, observable, action } from 'mobx';

export class AppStore {
  @observable accessor appMode = 'development';

  constructor() {
    makeObservable(this);
  }

  @action
  toggleAppMode() {
    this.appMode = this.appMode === 'development' ? 'production' : 'development';
  }
}
```

### 4.2. SolidJS `createStore` (변환 후 코드)

SolidJS에서는 클래스 기반 스토어보다 `createStore`를 사용한 함수형 스토어 패턴이 더 일반적입니다.

```typescript
// src/stores/appStore.ts
import { createStore } from 'solid-js/store';

// 스토어의 타입 정의
interface AppState {
  appMode: 'development' | 'production';
}

// 스토어 생성 함수
function createAppStore() {
  const [state, setState] = createStore<AppState>({
    appMode: 'development',
  });

  // MobX의 action과 유사한 역할을 하는 함수들
  const toggleAppMode = () => {
    setState('appMode', (prevMode) =>
      prevMode === 'development' ? 'production' : 'development'
    );
  };

  return { state, toggleAppMode };
}

// 싱글턴 인스턴스로 export
export const appStore = createAppStore();
```

**컴포넌트에서 사용법:**

```typescript
import { appStore } from '../stores/appStore';

const MyComponent: Component = () => {
  const { state, toggleAppMode } = appStore;

  return (
    <div>
      <p>App Mode: {state.appMode}</p>
      <button onClick={toggleAppMode}>Toggle Mode</button>
    </div>
  );
};
```

---

## 5. 스타일링: Tailwind CSS 재사용

기존 `tailwind.config.js`, `postcss.config.js`, `index.css` 파일은 대부분 그대로 SolidJS 프로젝트에서 사용할 수 있습니다.

**설정 단계:**

1.  **의존성 설치**:
    ```bash
    npm install -D tailwindcss postcss autoprefixer
    ```

2.  **설정 파일 생성**:
    ```bash
    npx tailwindcss init -p
    ```
    이 명령은 `tailwind.config.js`와 `postcss.config.js`를 생성합니다. 기존 프로젝트의 설정을 이 파일들에 복사합니다.

3.  **`tailwind.config.js` 설정**:
    `content` 경로에 SolidJS 컴포넌트 파일(`*.{js,jsx,ts,tsx}`)을 포함하도록 수정합니다.

    ```javascript
    // tailwind.config.js
    export default {
      content: [
        "./src/**/*.{js,jsx,ts,tsx}",
        "./index.html",
      ],
      theme: {
        extend: {},
      },
      plugins: [],
    }
    ```

4.  **CSS 파일 임포트**:
    `src/index.css` (또는 메인 CSS 파일)에 Tailwind 지시문을 추가하고, 이 파일을 메인 진입점(`src/index.tsx`)에서 임포트합니다.

    ```css
    /* src/index.css */
    @tailwind base;
    @tailwind components;
    @tailwind utilities;
    ```

    ```typescript
    // src/index.tsx
    import { render } from 'solid-js/web';
    import './index.css'; // CSS 파일 임포트
    import App from './App';

    render(() => <App />, document.getElementById('root') as HTMLElement);
    ```

이 가이드를 통해 React에서 SolidJS로의 전환을 체계적으로 진행하고, SolidJS의 반응성 모델을 최대한 활용하여 더 빠르고 효율적인 UI를 구축할 수 있을 것입니다.
