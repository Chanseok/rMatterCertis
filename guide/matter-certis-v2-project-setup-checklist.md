# Matter Certis v2 - 프로젝트 생성 체크리스트

## 📋 프로젝트 초기화 완료 가이드

이 체크리스트를 따라 새로운 Matter Certis v2 프로젝트를 처음부터 생성할 수 있습니다.

### 🔧 사전 준비사항

#### 필수 개발 환경
- [ ] **Rust 1.75.0+** 설치 (`rustup update`)
- [ ] **Node.js 18.0.0+** 설치
- [ ] **npm 또는 yarn** 패키지 매니저
- [ ] **Git** 버전 관리
- [ ] **VS Code** (권장 IDE)

#### VS Code 확장 프로그램 (권장)
- [ ] `rust-analyzer` - Rust 언어 지원
- [ ] `Tauri` - Tauri 개발 지원
- [ ] `SolidJS` - SolidJS 지원
- [ ] `Prettier` - 코드 포맷팅
- [ ] `ESLint` - TypeScript 린팅

### 📂 1단계: 프로젝트 생성

```bash
# 1. Tauri 프로젝트 생성
npm create tauri-app@latest matter-certis-v2

# 2. 프로젝트 설정 선택
# - Framework: Vanilla
# - TypeScript: Yes
# - Package manager: npm (또는 yarn)

# 3. 프로젝트 디렉토리로 이동
cd matter-certis-v2

# 4. 기본 의존성 설치
npm install
```

### 🗂️ 2단계: 프로젝트 구조 생성

```bash
# Rust 백엔드 구조
mkdir -p src-tauri/src/domain/{entities,repositories,services}
mkdir -p src-tauri/src/application/{use_cases,dto}
mkdir -p src-tauri/src/infrastructure/{database,http,config}
mkdir -p src-tauri/src/commands

# TypeScript 프론트엔드 구조
mkdir -p src/{components,stores,services,types,utils}
mkdir -p src/components/{ui,features,layout}
mkdir -p src/components/features/{dashboard,vendors,products,crawling,settings}

# 기타 디렉토리
mkdir -p tests/{rust,frontend}
mkdir -p docs
mkdir -p scripts
```

### ⚙️ 3단계: 핵심 설정 파일 생성

#### Cargo.toml 업데이트
```toml
# src-tauri/Cargo.toml에 의존성 추가
[dependencies]
tauri = { version = "2.0", features = ["api-all"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
reqwest = { version = "0.11", features = ["json", "cookies", "gzip"] }
sqlx = { version = "0.7", features = ["sqlite", "runtime-tokio-rustls", "chrono"] }
scraper = "0.18"
anyhow = "1.0"
thiserror = "1.0"
rayon = "1.7"
futures = "0.3"
config = "0.13"
tracing = "0.1"
tracing-subscriber = "0.3"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
async-trait = "0.1"
```

#### package.json 업데이트
```json
{
  "dependencies": {
    "solid-js": "^1.8.0",
    "@solidjs/router": "^0.10.0",
    "@tauri-apps/api": "^2.0.0",
    "@kobalte/core": "^0.12.0",
    "solid-primitives": "^1.8.0",
    "date-fns": "^2.30.0",
    "nanoid": "^5.0.0"
  },
  "devDependencies": {
    "@types/node": "^20.0.0",
    "typescript": "^5.3.0",
    "vite": "^5.0.0",
    "vite-plugin-solid": "^2.8.0",
    "vite-tsconfig-paths": "^4.2.0",
    "vitest": "^1.0.0",
    "@solidjs/testing-library": "^0.8.0",
    "autoprefixer": "^10.4.0",
    "postcss": "^8.4.0",
    "tailwindcss": "^3.4.0"
  }
}
```

#### Tailwind CSS 설정
```bash
# Tailwind 설치 및 초기화
npm install -D tailwindcss postcss autoprefixer
npx tailwindcss init -p
```

```js
// tailwind.config.js
/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {},
  },
  plugins: [],
}
```

### 🏗️ 4단계: 기본 파일 생성

#### Rust 기본 파일들
```bash
# 각 모듈의 mod.rs 파일 생성
touch src-tauri/src/domain/mod.rs
touch src-tauri/src/domain/entities/mod.rs
touch src-tauri/src/domain/repositories/mod.rs
touch src-tauri/src/domain/services/mod.rs
touch src-tauri/src/application/mod.rs
touch src-tauri/src/application/use_cases/mod.rs
touch src-tauri/src/application/dto/mod.rs
touch src-tauri/src/infrastructure/mod.rs
touch src-tauri/src/infrastructure/database/mod.rs
touch src-tauri/src/infrastructure/http/mod.rs
touch src-tauri/src/infrastructure/config/mod.rs
touch src-tauri/src/commands/mod.rs
```

#### TypeScript 기본 파일들
```bash
# 기본 컴포넌트 및 스토어 파일 생성
touch src/types/domain.ts
touch src/types/api.ts
touch src/services/index.ts
touch src/stores/index.ts
touch src/utils/index.ts
```

### 🎨 5단계: SolidJS 및 Vite 설정

#### vite.config.ts
```typescript
import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';
import tsconfigPaths from 'vite-tsconfig-paths';

export default defineConfig({
  plugins: [solid(), tsconfigPaths()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    target: process.env.TAURI_PLATFORM == 'windows' ? 'chrome105' : 'safari13',
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
});
```

#### tsconfig.json
```json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "module": "ESNext",
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "preserve",
    "jsxImportSource": "solid-js",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "baseUrl": ".",
    "paths": {
      "@/*": ["./src/*"],
      "@/components/*": ["./src/components/*"],
      "@/stores/*": ["./src/stores/*"],
      "@/services/*": ["./src/services/*"],
      "@/types/*": ["./src/types/*"],
      "@/utils/*": ["./src/utils/*"]
    }
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

### 🛠️ 6단계: 개발 스크립트 설정

#### package.json scripts 추가
```json
{
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "tauri": "tauri",
    "tauri:dev": "tauri dev",
    "tauri:build": "tauri build",
    "test": "vitest",
    "test:ui": "vitest --ui",
    "lint": "eslint src --ext ts,tsx --report-unused-disable-directives --max-warnings 0",
    "format": "prettier --write src/**/*.{ts,tsx}",
    "type-check": "tsc --noEmit"
  }
}
```

### 🗃️ 7단계: 데이터베이스 설정

#### migrations 디렉토리 생성
```bash
mkdir -p src-tauri/migrations
```

#### 첫 번째 마이그레이션 파일 생성
```sql
-- src-tauri/migrations/20241201000001_initial.sql
CREATE TABLE IF NOT EXISTS vendors (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    base_url TEXT NOT NULL,
    crawling_config TEXT NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT 1,
    last_crawled_at DATETIME,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS products (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    price REAL,
    currency TEXT NOT NULL DEFAULT 'USD',
    description TEXT,
    image_url TEXT,
    product_url TEXT NOT NULL,
    vendor_id TEXT NOT NULL,
    category TEXT,
    in_stock BOOLEAN NOT NULL DEFAULT 1,
    collected_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (vendor_id) REFERENCES vendors (id)
);

CREATE TABLE IF NOT EXISTS crawling_sessions (
    id TEXT PRIMARY KEY,
    vendor_id TEXT NOT NULL,
    status TEXT NOT NULL,
    total_pages INTEGER,
    processed_pages INTEGER NOT NULL DEFAULT 0,
    products_found INTEGER NOT NULL DEFAULT 0,
    errors_count INTEGER NOT NULL DEFAULT 0,
    started_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at DATETIME,
    FOREIGN KEY (vendor_id) REFERENCES vendors (id)
);

CREATE INDEX IF NOT EXISTS idx_products_vendor_id ON products (vendor_id);
CREATE INDEX IF NOT EXISTS idx_products_collected_at ON products (collected_at);
CREATE INDEX IF NOT EXISTS idx_crawling_sessions_vendor_id ON crawling_sessions (vendor_id);
```

### 🎯 8단계: 첫 번째 테스트 실행

```bash
# 개발 모드에서 실행
npm run tauri:dev

# 별도 터미널에서 타입 체크
npm run type-check

# 테스트 실행
npm test
```

### 📋 최종 확인 체크리스트

#### 프로젝트 구조 확인
- [ ] `src-tauri/` Rust 백엔드 디렉토리 구조 완성
- [ ] `src/` TypeScript 프론트엔드 디렉토리 구조 완성
- [ ] `migrations/` 데이터베이스 마이그레이션 파일 존재
- [ ] 모든 설정 파일 (`Cargo.toml`, `package.json`, `tsconfig.json`, `vite.config.ts`) 완성

#### 의존성 확인
- [ ] Rust 의존성 모두 설치됨 (`cargo check` 성공)
- [ ] Node.js 의존성 모두 설치됨 (`npm install` 성공)
- [ ] TypeScript 컴파일 성공 (`npm run type-check` 성공)

#### 개발 환경 확인
- [ ] `npm run tauri:dev` 실행 성공
- [ ] 애플리케이션 창이 정상적으로 열림
- [ ] Hot reload 기능 작동
- [ ] 브라우저 개발자 도구에서 에러 없음

#### Git 설정
- [ ] `.gitignore` 파일 적절히 설정
- [ ] 초기 커밋 완료
- [ ] README.md 기본 내용 작성

### 🚀 다음 단계

프로젝트 초기화가 완료되면 다음 단계로 진행하세요:

1. **Phase 1**: [핵심 아키텍처 구현](./matter-certis-v2-development-guide.md#phase-1)
2. **Phase 2**: [백엔드 도메인 구현](./matter-certis-v2-development-guide.md#phase-2)
3. **Phase 3**: [크롤링 엔진 구현](./matter-certis-v2-development-guide.md#phase-3)
4. **Phase 4**: [프론트엔드 구현](./matter-certis-v2-phase4-5-guide.md#phase-4)
5. **Phase 5**: [통합 테스트 및 최적화](./matter-certis-v2-phase4-5-guide.md#phase-5)

### 📚 추가 리소스

- [Tauri 공식 문서](https://tauri.app/)
- [SolidJS 공식 문서](https://www.solidjs.com/)
- [SQLx 문서](https://docs.rs/sqlx/)
- [reqwest 문서](https://docs.rs/reqwest/)

---

이 체크리스트를 완료하면 Matter Certis v2 개발을 위한 견고한 기반이 마련됩니다. 각 단계를 신중히 따라가시면 성공적인 프로젝트 시작을 할 수 있습니다.
