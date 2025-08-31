# rMatterCertis v2 — Actor-Model Based Crawling Platform

**Tauri + Rust + SolidJS 기반의 고성능, 고확장성 E-commerce 제품 데이터 크롤링 플랫폼**

이 프로젝트는 이전 크롤링 시스템 개발 경험을 바탕으로, Actor 모델과 최신 비동기 처리 기술을 적용하여 안정성과 유지보수성, 확장성을 극대화한 차세대 크롤링 엔진을 구현하는 것을 목표로 합니다.

---

## 🏛️ 핵심 아키텍처

본 프로젝트는 **계층적 Actor 모델**을 기반으로 설계되어, 복잡한 비동기 크롤링 작업을 명확하고 안정적으로 처리합니다.

1.  **명확한 계층 구조**: 모든 크롤링 작업은 **`Session` → `Batch` → `Stage` → `Task`** 로 이어지는 명확한 계층 구조에 따라 수행됩니다. 이를 통해 전체 작업의 흐름을 쉽게 추적하고 관리할 수 있습니다.

2.  **Strategy + Template Method 패턴**: 각 `Stage`(작업 단계)의 구체적인 실행 로직은 **Strategy 패턴**을 통해 독립적으로 구현됩니다. `StageActor`는 실행의 '틀(Template)'만 제공하고, 실제 로직은 외부에서 주입된 `StageStrategy` 구현체에게 위임합니다. 이 구조는 새로운 크롤링 단계를 추가할 때 기존 코드를 수정할 필요가 없는, OCP 원칙을 준수하는 유연한 설계를 제공합니다.

3.  **단일 통합 이벤트 시스템**: 모든 백엔드의 상태 변화는 단일화된 `AppEvent` enum을 통해 실시간으로 프론트엔드에 전달됩니다. 이 세분화된 이벤트 스트림은 UI에서 전체 진행률부터 개별 Task 단위의 상태까지 동적으로 시각화할 수 있는 기반이 됩니다.

4.  **관심사의 명확한 분리 (SoC)**:
    *   **Planning vs. Execution**: `CrawlingPlanner`가 분석 및 계획을 전담하고, `Actor`들은 수립된 계획을 실행하는 역할만 수행하여 책임이 명확합니다.
    *   **Domain vs. Infrastructure**: 순수한 비즈니스 로직과 이벤트 발행, 로깅 등 횡단 관심사가 명확히 분리되어 유지보수성이 높습니다.

**📖 상세 아키텍처 설계 및 최종 개선 계획은 [re-arch-plan-final4.md](./guide/re-arch-plan-final4.md) 문서를 참고하십시오.**

---

## 🚀 시작하기

### 1. 개발 환경 설정

```bash
# 저장소 클론
git clone <repository-url>
cd rMatterCertis

# Rust 안정 버전 설치
rustup install stable
rustup default stable

# Tauri CLI 설치 (v2)
cargo install tauri-cli --version "^2.0"

# 프론트엔드 의존성 설치
npm install
```

### 2. 타입 생성 및 개발 서버 실행

백엔드(Rust)의 타입이 변경될 때마다, 아래 명령어를 실행하여 프론트엔드(TypeScript) 타입을 동기화해야 합니다.

```bash
# 1. Rust -> TypeScript 타입 자동 생성
bash scripts/generate_types.sh

# 2. 타입 체크 (옵션)
npm run type-check

# 3. Tauri 개발 모드 실행 🚀
npm run tauri:dev

# (통합 실행 스크립트)
npm run dev:unified
```

---

## 📁 프로젝트 구조

- **`/` (루트)**: `package.json`, `vite.config.ts` 등 프론트엔드 설정 파일 위치.
- **`src/`**: SolidJS와 TypeScript로 작성된 프론트엔드 소스 코드.
- **`src-tauri/`**: Rust로 작성된 백엔드 소스 코드 (Tauri, Actor 기반 크롤링 엔진).
- **`guide/`**: 아키텍처, 개발 가이드 등 핵심 설계 문서.
- **`docs/`**: 이벤트, SQL, 로깅 정책 등 세부 명세 문서.

---

## 🛠️ 주요 스크립트

- `npm run tauri:dev`: 개발 모드로 앱을 실행합니다.
- `npm run tauri:build`: 프로덕션용으로 앱을 빌드합니다.
- `bash scripts/generate_types.sh`: Rust 타입으로부터 TypeScript 타입을 다시 생성합니다.
- `bash scripts/lint_rust.sh`: Rust 코드의 린트를 실행합니다.