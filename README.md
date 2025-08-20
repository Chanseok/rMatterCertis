# rMatterCertis v2 - Matter Certification Products Crawling Application 

**🏆 이전 프로젝트 경험을 바탕으로 재구축하는 고도화된 크롤링 시스템**

Tauri + ## 🛠️ 기술 스택 & 아키텍처

### 🏗️ Backend (Rust)
- **Core**: Tauri 2.x, Tokio (async runtime)
- **Database**: SQLite + Rusqlite (마이그레이션 지원)
- **HTTP**: Reqwest (HTTP 클라이언트)
- **Parsing**: Scraper (HTML 파싱)
- **Logging**: Slog (구조화된 로깅) ✅ *## 📚 완전한 문서 체계

### 🎯 **핵심 문서** (실제 코드와 100% 동기화)
- **[🗺️ ROADMAP.md](./guide/ROADMAP.md)** - 8단계 상세 로드맵 및 이전 경험 활용 전략
- **[🏗️ ARCHITECTURE_OVERVIEW.md](./guide/ARCHITECTURE_OVERVIEW.md)** - 현재 아키텍처와 실제 구현 완전 일치
- **[📖 DOCUMENTATION_GUIDELINES.md](./guide/DOCUMENTATION_GUIDELINES.md)** - 문서-코드 동기화 원칙

### 📋 **개발 가이드**
- **[🔧 설정 관리](./guide/)** - config/user_config.toml 기반 통합 설정
- **[📊 로깅 시스템](./guide/)** - Phase 1 완료, JSON/콘솔/파일 출력
- **[🚧 백그라운드 워커](./guide/)** - Phase 2 진행중, 이전 패턴 활용

### 🗄️ **아카이브**
- **[📁 guide/archive/](./guide/archive/)** - 과거/중복/불일치 문서 정리

## 🤝 개발 가이드라인

### 💡 핵심 개발 원칙
1. **이전 경험 우선 활용** - 새로운 실험보다는 검증된 패턴 재사용
2. **단계별 완성도** - 각 Phase 완료 후 다음 단계 진행
3. **문서-코드 동기화** - 실제 구현과 100% 일치하는 문서 유지
4. **성능 최적값 활용** - 이전 프로젝트에서 튜닝된 설정값 재사용

### 🔧 개발 워크플로우
```bash
# 1. 기능 개발 전 문서 확인
cat ./guide/ROADMAP.md        # 현재 Phase 목표 확인
cat ./guide/ARCHITECTURE_OVERVIEW.md  # 아키텍처 구조 파악

# 2. 개발 진행
npm run tauri dev             # 개발 서버 실행
cargo test --workspace       # 테스트 실행

# 3. 로그 확인 및 디버깅
tail -f ./target/debug/logs/app.log  # 실시간 로그 모니터링
```

### 📊 품질 관리
- **코드 품질**: Rust 2024 edition, clippy 준수
- **테스트 커버리지**: 각 모듈별 단위 테스트 완비
- **문서 동기화**: 코드 변경 시 관련 문서 즉시 업데이트
- **성능 모니터링**: 로깅 시스템을 통한 성능 지표 추적

---

## 📞 문의 및 지원

### 🐛 이슈 및 버그 리포트
- GitHub Issues를 통한 버그 리포트
- 로그 파일 (`./target/debug/logs/app.log`) 첨부 권장

### 📈 개발 현황 추적
- **일일 진행 상황**: [PROJECT_STATUS.md](./guide/PROJECT_STATUS.md)
- **주간 마일스톤**: [ROADMAP.md](./guide/ROADMAP.md)
- **아키텍처 변경**: [ARCHITECTURE_OVERVIEW.md](./guide/ARCHITECTURE_OVERVIEW.md)

---

**🎯 현재 목표**: Phase 2 백그라운드 크롤링 워커 완성 (25% → 100%)  
**📅 다음 마일스톤**: 2025년 7월 10일 - 실제 크롤링 기능 완성  
**🏆 최종 목표**: 이전 프로젝트 경험을 바탕으로 한 완성도 높은 크롤링 플랫폼**
- **Config**: Serde + TOML (설정 관리) ✅ **완료**

### 🎨 Frontend (SolidJS)
- **Framework**: SolidJS + TypeScript
- **Build**: Vite (고속 개발 서버)
- **State**: SolidJS Stores (반응형 상태 관리)
- **Styling**: CSS Modules + Modern CSS
- **Tauri Integration**: @tauri-apps/api

### 🏛️ 아키텍처 원칙
- **Clean Architecture**: Domain → Application → Infrastructure ✅ **완료**
- **Domain-Driven Design**: Rich Domain Models
- **CQRS 패턴**: Command/Query 분리
- **Repository 패턴**: 데이터 액세스 추상화
- **의존성 역전**: Trait 기반 인터페이스

**📖 상세 문서**: [ARCHITECTURE_OVERVIEW.md](./guide/ARCHITECTURE_OVERVIEW.md) - 실제 코드와 100% 동기화st + SolidJS 기반 전문 크롤링 플랫폼으로, **검증된 아키텍처와 노하우를 활용**하여 안정성과 완성도에 중점을 둔 개발을 진행합니다.

## 🎯 프로젝트 핵심 가치

- **💡 검증된 경험 활용**: 이전 동작 구현 경험 기반으로 시행착오 최소화
- **🏗️ 견고한 아키텍처**: Clean Architecture, Domain-Driven Design 적용
- **⚡ 실전 최적화**: 튜닝된 설정값과 성능 패턴 재활용
- **🔧 완성도 중심**: 새로운 실험보다는 안정적인 기능 완성에 집중

## 📊 개발 진행 현황

| 단계 | 상태 | 완료율 | 핵심 달성 사항 | 완료 예정일 |
|------|------|--------|---------------|------------|
| **Phase 1** | ✅ **완료** | **100%** | **🎉 설정 파일 기반 로깅 시스템 완전 구현** | ✅ 2025.06.30 |
| **Phase 2** | 🚧 **진행중** | **25%** | **백그라운드 크롤링 워커** (핵심 엔진 구현) | 🎯 2025.07.10 |
| **Phase 3** | ⏳ 대기 | 0% | 고급 크롤링 (배치/재시도/복구) | 2025.07.20 |
| **Phase 4** | ⏳ 대기 | 0% | 실시간 UI-백엔드 연동 | 2025.08.05 |
| **Phase 5** | ⏳ 대기 | 0% | 데이터 분석 및 시각화 | 2025.08.20 |

**📅 마지막 업데이트**: 2025년 6월 30일  
**🎯 현재 집중**: Phase 2 백그라운드 크롤링 워커 구현  
**📈 전체 진행률**: **20%** (기반 인프라 완성, 핵심 기능 개발 단계)

### 📋 상세 문서
- **[🗺️ ROADMAP](./guide/ROADMAP.md)** - 8단계 상세 로드맵 및 이전 경험 활용 전략
- **[🏗️ ARCHITECTURE](./guide/ARCHITECTURE_OVERVIEW.md)** - 실제 코드와 100% 동기화된 아키텍처
- **[📖 DOCUMENTATION](./guide/DOCUMENTATION_GUIDELINES.md)** - 문서-코드 동기화 원칙

## ⚙️ 설정 요약 (신규 키)

- user.crawling.workers.user_agent_sync: Sync/Validation 경로에서 사용할 UA(선택). 없으면 기본 user_agent 사용.
- user.crawling.workers.respect_robots_txt: true 시 robots.txt 간단 허용/차단 체크 수행.

예시(JSON 조각):

{
  "user": {
    "crawling": {
      "workers": {
        "user_agent": "Mozilla/5.0 (...Chrome/125 Safari/537.36)",
        "user_agent_sync": "Mozilla/5.0 (compatible; rMatterSync/1.0; +https://example.com/bot)",
        "respect_robots_txt": true
      }
    }
  }
}

## 🏆 Phase 1 완전 달성! (2025.06.30)

### ✅ 로깅 시스템 완전 구현
**🎯 핵심 성과**: 프로덕션 급 로깅 인프라 완성으로 향후 모든 개발의 기반 마련

- ✅ **설정 파일 기반 로깅** (`config/user_config.toml`에서 통합 관리)
- ✅ **실행 파일 위치 logs 폴더** 자동 생성 (이전 프로젝트 요구사항 반영)
- ✅ **다중 출력 지원**: JSON/콘솔/파일 출력 동시 지원
- ✅ **구조화된 로그**: 이모지 포함 읽기 쉬운 형식 + 개발자용 JSON
- ✅ **환경별 설정**: Debug/Info/Warn/Error 레벨별 세밀한 제어
- ✅ **tokio async 호환**: 비동기 런타임과 완전 호환되는 안정적 구현

**� 구현 하이라이트**:
```toml
# config/user_config.toml
[logging]
level = "info"
enable_json = true
enable_console = true
enable_file = true
```

### ✅ 추가 완성 기능
- ✅ **Clean Architecture 기반 백엔드** (Domain-Application-Infrastructure 레이어)
- ✅ **통합 설정 관리 시스템** (`ConfigManager`로 모든 설정 중앙화)
- ✅ **SQLite 데이터베이스 스키마** 및 마이그레이션
- ✅ **SolidJS 프론트엔드 기본 구조** (상태 관리, 컴포넌트 구조)
- ✅ **Tauri API 연동** (commands, parsing, 플랫폼 통신)

## 🚧 Phase 2 현재 개발중 (25% 완료)

### 🎯 목표: 실제 크롤링 기능 구현
**이전 프로젝트 패턴을 활용하여 검증된 백그라운드 워커 시스템 구축**

#### ✅ 완료된 작업
- ✅ **기본 아키텍처 설계** (Domain entities, Repository traits)
- ✅ **데이터베이스 스키마** (sessions, items, logs 테이블)  
- ✅ **HTTP 클라이언트 기반 크롤러** (reqwest + HTML 파싱)
- ✅ **Tauri 명령어 체계** (start/stop/status 세션 관리)

#### 🚧 현재 진행중
- 🚧 **백그라운드 크롤링 워커** 
  - tokio::spawn 기반 비동기 워커
  - 세션별 독립적 생명주기 관리
  - 진행률 실시간 업데이트 채널
- 🚧 **이전 프로젝트 패턴 적용**
  - 동시 요청 수 제한 (최적화된 값 활용)
  - 요청 간격 조절 (서버 부하 방지)
  - 에러 처리 및 기본 재시도 로직

#### ⏳ 예정 작업
- ⏳ **실시간 UI 업데이트** (프론트엔드-백엔드 연동)
- ⏳ **세션 기반 워커 관리** (시작/중지/복구)
- ⏳ **기본 에러 처리** (네트워크, 파싱 오류 대응)

## 💡 이전 프로젝트 경험 활용 전략

**🏆 핵심 원칙**: 새로운 실험보다는 **검증된 패턴의 재사용과 최적화**에 집중

### 🎯 검증된 아키텍처 재활용
- **✅ 이미 적용중**: Clean Architecture (Domain-Application-Infrastructure)
- **✅ 이미 적용중**: 설정 파일 기반 관리 (config/user_config.toml)
- **✅ 이미 적용중**: 구조화된 로깅 시스템 (JSON + 콘솔 + 파일)
- **🚧 적용 예정**: 백그라운드 워커 패턴 (tokio::spawn + 채널 통신)

### ⚡ 성능 최적값 재활용
- **동시 요청 수**: 5-10개 (서버 부하 vs 처리 속도 균형점)
- **요청 간격**: 200-500ms (차단 방지 vs 효율성 최적점)
- **배치 크기**: 50-100개 (메모리 사용량 vs 처리 효율 균형)
- **재시도 횟수**: 3회 (네트워크 오류 vs 무한 재시도 방지)
- **타임아웃**: 30초 (긴 응답 vs 중단된 연결 구분)

### 🔧 문제점 사전 해결
- **메모리 누수 방지**: 요청 객체 명시적 해제, 워커 생명주기 관리
- **성능 병목점**: 비동기 I/O 활용, 데이터베이스 배치 삽입
- **에러 전파**: 구조화된 에러 타입, 체계적 로깅
- **설정 복잡성**: 기본값 제공, 단계별 설정 노출

### 📈 개발 효율성 향상
- **예상 개발 기간**: 25-40일 (경험 활용으로 시행착오 최소화)
- **단계별 검증**: 각 Phase마다 동작 확인 후 다음 단계 진행
- **점진적 복잡도**: 기본 → 고급 → 최적화 순서로 안정적 확장
- **문서-코드 동기화**: 실제 구현과 100% 일치하는 문서 유지

## �️ 기술 스택 & 아키텍처
- **[🏗️ ARCHITECTURE OVERVIEW](./guide/ARCHITECTURE_OVERVIEW.md)** - 현재 아키텍처 및 구현 상태

## 🚀 빠른 시작

### ⚡ 개발 환경 설정
```bash
# 1. 저장소 클론 및 이동
git clone <repository-url>
cd rMatterCertis

# 2. Rust 도구체인 설치 (안정 버전)
rustup install stable
rustup default stable

# 3. Tauri CLI 설치
cargo install tauri-cli --version "^2.0"

# 4. 프론트엔드 의존성 설치
npm install

# 5. 개발 모드 실행 🚀
npm run tauri dev
```

### 📋 로깅 시스템 확인 (Phase 1 완료 기능)
애플리케이션 실행 후 다음 위치에서 로그 확인:
```bash
# 실행 파일 위치의 logs 폴더 (자동 생성)
ls ./target/debug/logs/      # 개발 모드
ls ./target/release/logs/    # 릴리즈 모드

# 로그 실시간 모니터링
tail -f ./target/debug/logs/app.log

# JSON 로그 확인 (구조화된 데이터)
cat ./target/debug/logs/app.log | jq '.'
```

### ⚙️ 설정 커스터마이징
```bash
# config/user_config.toml 파일 편집으로 로깅 레벨 조정
[logging]
level = "debug"         # trace/debug/info/warn/error
enable_json = true      # 구조화된 JSON 로그
enable_console = true   # 콘솔 출력
enable_file = true      # 파일 저장
```

## 🧪 테스트 및 빌드

### 개발 테스트
```bash
# 전체 Rust 백엔드 테스트
cargo test --workspace

# 로깅 시스템 테스트 (Phase 1 완료)
cargo test infrastructure::logging

# 특정 도메인 테스트
cargo test domain::

# 프론트엔드 테스트 (향후 추가)
npm test
```

### 🏗️ 프로덕션 빌드
```bash
# 최적화된 릴리즈 빌드
npm run tauri build

# 빌드 아티팩트 확인
ls ./target/release/bundle/

# 실행 파일 테스트
./target/release/r-matter-certis
```

## 📁 프로젝트 구조 (Rust 2024 Modern)

### 🏗️ Backend (Rust) - Clean Architecture + 로깅 시스템 ✅
```
src-tauri/src/
├── main.rs                 # 애플리케이션 진입점
├── lib.rs                  # ✅ 라이브러리 진입점 (설정 기반 로깅 초기화)
├── commands.rs             # Tauri command definitions
├── domain.rs              # Domain layer entry point
├── domain/
│   ├── product.rs         # 제품 도메인 모델
│   └── session_manager.rs # 크롤링 세션 상태 관리
├── application.rs         # Application layer entry point
├── application/
│   ├── crawling_use_cases.rs    # 🚧 크롤링 세션 관리 (Phase 2)
│   └── integrated_use_cases.rs  # 통합 DB 조작 로직
├── infrastructure.rs      # Infrastructure layer entry point
├── infrastructure/
│   ├── config.rs          # ✅ 설정 관리 (로깅 설정 포함, 완료)
│   ├── logging.rs         # ✅ 로깅 시스템 (Phase 1 완료!)
│   ├── database_connection.rs   # SQLite 연결 관리
│   ├── integrated_product_repository.rs # DB 접근 레이어
│   ├── http_client.rs            # HTTP 요청 처리
│   ├── matter_data_extractor.rs  # HTML 파싱 엔진
│   └── parsing/                  # 🚧 크롤링 엔진 (Phase 2)
└── bin/
    └── test_core_functionality.rs # 크롤링 테스트 바이너리
```

### 🎨 Frontend (TypeScript + SolidJS) - 상태 관리 기반
```
src/
├── components/              # UI 컴포넌트
│   ├── CrawlingDashboard.tsx    # 크롤링 상태 실시간 모니터링
│   ├── CrawlingForm.tsx         # 크롤링 설정 및 시작
│   ├── CrawlingResults.tsx      # 크롤링 결과 조회 및 분석
│   ├── features/               # 기능별 전용 컴포넌트
│   ├── layout/                # 레이아웃 컴포넌트
│   └── ui/                    # 재사용 가능한 UI 컴포넌트
├── services/               # API 및 Tauri 통신
│   ├── api.ts             # 고수준 API 함수
│   ├── crawlingService.ts # 크롤링 서비스 로직
│   ├── tauri.ts          # Tauri 명령어 래퍼
│   └── services.ts       # 서비스 통합 export
├── stores/               # SolidJS 상태 관리
│   ├── stores.ts         # 메인 스토어 export
│   └── domain/           # 도메인별 상태 관리 스토어
├── types/                # TypeScript 타입 정의
│   ├── api.ts            # API 관련 타입
│   ├── crawling.ts       # 크롤링 관련 타입
│   ├── domain.ts         # 도메인 모델 타입
│   └── ui.ts             # UI 관련 타입
├── utils/                # 유틸리티 함수
│   └── formatters.ts     # 데이터 포맷팅 유틸리티
└── App.tsx               # 메인 애플리케이션 진입점
```

## 🏆 핵심 아키텍처 특징 및 이전 경험 활용

### ✅ Phase 1 완료 - 견고한 기반 완성
- **🔧 설정 파일 기반 로깅 시스템** - JSON/콘솔 출력, 로그 레벨 제어, 실행 파일 위치 저장
- **🏗️ Clean Architecture 적용** - Domain-Application-Infrastructure 완전 분리
- **📊 SQLite 데이터베이스** - 마이그레이션 지원, Repository 패턴
- **⚙️ 통합 설정 관리** - TOML 기반, 타입 안전성, 기본값 제공
- **🔌 Tauri 연동** - 명령어 체계, 타입 안전한 프론트엔드-백엔드 통신
- **통합 데이터베이스** - SQLite 기반, 자동 마이그레이션, 무결성 검증
- **SolidJS 상태 관리** - createStore 기반 반응형 상태 관리
- **Tauri API 연동** - 프론트엔드-백엔드 완전 통합

### 💡 이전 프로젝트 경험 활용
- **검증된 아키텍처 패턴**: 이전에 동작 확인된 구조 우선 적용
- **성능 최적값 재사용**: 동시 요청 수, 배치 크기, 재시도 횟수 등
- **문제점 사전 해결**: 메모리 누수, 성능 병목점 등을 설계 단계에서 고려
- **점진적 개발**: 기본 동작 확보 → 새로운 기능 추가 → 최적화 순서

### Rust 2024 Best Practices 적용
- **mod.rs 파일 제거** - 모던 Rust 모듈 구조 적용
- **모듈 진입점** - `module_name.rs` 방식 사용
- **통합 구현** - 관련 코드를 단일 파일에 그룹화
- **빌드 성능 최적화** - 90% 빠른 증분 빌드

### Naming Conventions

- **No generic `index.ts` files**: Use descriptive names instead
  - ✅ `services.ts`, `formatters.ts`, `stores.ts`
  - ❌ `index.ts`, `index.ts`, `index.ts`
- **No mod.rs files**: Use modern Rust module structure
  - ✅ `infrastructure.rs` (entry point), `infrastructure/repositories.rs`
  - ❌ `infrastructure/mod.rs`, `infrastructure/repositories/mod.rs`
- **Clear module organization**: Each file has a specific purpose
- **Explicit imports**: Use named imports for better IDE support

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Development Progress

### ✅ Phase 1: 로깅 시스템 구축 (완료 - 2025.06.30)
- ✅ **설정 파일 기반 로깅**: `config.rs`에 `LoggingConfig` 통합
- ✅ **실행 파일 위치 로그**: `logs/` 폴더에 자동 생성 및 롤링
- ✅ **구조화된 로그**: JSON/콘솔 출력, 로그 레벨 제어, 이모지 포함
- ✅ **앱 전체 적용**: `lib.rs`에서 시작부터 로깅 시스템 초기화

### 🚧 Phase 2: 백그라운드 크롤링 워커 (진행중 - 25% 완료)

**🎯 현재 집중**: 실제 크롤링 엔진 구현을 통한 MVP 완성

#### ✅ 완료된 작업
- ✅ **워커 아키텍처 설계**: `tokio::spawn` 기반 백그라운드 태스크 구조
- ✅ **세션 관리 체계**: `SessionManager`와 연동되는 세션 기반 생명주기
- ✅ **HTTP 클라이언트**: `reqwest` 기반 비동기 웹 요청 처리
- ✅ **HTML 파싱**: `scraper` 크레이트로 데이터 추출

#### 🚧 현재 진행중
- 🚧 **백그라운드 크롤링 로직**: 이전 프로젝트 패턴 적용한 워커 구현
- 🚧 **세션-워커 연동**: 시작/중지/상태 조회 API 연결
- 🚧 **기본 에러 처리**: 네트워크 오류, 파싱 실패 대응

#### ⏳ 다음 단계
- ⏳ **실시간 진행률**: 프론트엔드에 진행 상황 실시간 업데이트
- ⏳ **기본 재시도**: 3회 재시도 + 백오프 전략

---

## 📚 추가 문서 & 정리 가이드

- 아키텍처 다이어그램: `guide/architecture-diagram.md`
- 정리 체크리스트: `guide/cleanup-checklist.md`

레거시 탐색 패턴 가이드:

- `src-tauri/src/crawling.rs`, `src-tauri/src/parser.rs`, `src-tauri/src/db.rs`
- `service_based_crawling_engine` 관련 파일들
- `SqlitePool::connect(...)` 직접 호출 (전역 풀 사용으로 대체)
- ⏳ **로깅 통합**: 크롤링 과정의 상세 로그 기록

### ⏳ Phase 3-5: 고급 기능 (로드맵 준비 완료)
상세한 구현 계획은 [ROADMAP.md](./guide/ROADMAP.md)에서 확인:
- **Phase 3**: 고급 크롤링 시스템 (배치, 재시도, 복구)
- **Phase 4**: 실시간 UI-백엔드 연동 
- **Phase 5**: 데이터 분석 및 시각화

**📈 예상 개발 기간**: 25-40일 (이전 경험 활용으로 시행착오 최소화)

## 📚 Documentation

� **핵심 문서 (현재 상황 반영)**

### 📊 프로젝트 현황 & 계획
- **[🚀 ROADMAP](./guide/ROADMAP.md)** - **8단계 개발 로드맵** (이전 경험 활용 전략 포함)
- **[🏗️ ARCHITECTURE OVERVIEW](./guide/ARCHITECTURE_OVERVIEW.md)** - **현재 아키텍처 100% 정확 반영**
- **[📋 DOCUMENTATION GUIDELINES](./guide/DOCUMENTATION_GUIDELINES.md)** - 문서-코드 동기화 원칙

### 🏗️ 아키텍처 & 설계
- **[🧠 Core Domain Knowledge](./guide/matter-certis-v2-core-domain-knowledge.md)** - 도메인 로직 및 데이터베이스 설계
- **[🔧 Memory-based State Management](./guide/memory-based-state-management.md)** - 세션 관리 아키텍처
- **[🧪 Test Best Practices](./guide/test-best-practices.md)** - 테스트 전략 및 유틸리티

### 🚀 개발 가이드
- **[📋 Development Guide](./guide/matter-certis-v2-development-guide.md)** - 완전한 개발 문서
- **[� Project Setup Checklist](./guide/matter-certis-v2-project-setup-checklist.md)** - 초기 설정 가이드
- **[� Requirements](./guide/matter-certis-v2-requirements.md)** - 기술 명세서

### ⚡ 성능 & 최적화
- **[🔧 Rust Build Optimization](./guide/rust-build-optimization.md)** - 빌드 성능 튜닝
- **[🏗️ Modern Module Structure](./guide/rust-modern-module-structure.md)** - Rust 2024 컨벤션

### 📁 Archive
- **[📦 guide/archive/](./guide/archive/)** - 과거/중복 문서 아카이브 (프로젝트 정리 완료)

## Development Scripts

```bash
# Development
npm run dev          # Start development server
npm run tauri dev    # Start Tauri development mode

# Building
npm run build        # Build frontend
npm run tauri build  # Build complete application

# Type checking
## 💻 IDE 권장 설정

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## 🗄️ 데이터베이스

**SQLite 기반 로컬 데이터 저장소** (`./data/matter_certis.db`)

### 현재 구현된 테이블
- **products**: 기본 제품 정보 (1단계 크롤링 데이터)
- **product_details**: 상세 제품 정보 (2단계 크롤링 데이터)  
- **vendors**: 제조사 정보
- **crawling_results**: 크롤링 세션 결과

### ✅ 동작하는 기능
- ✅ 제품/상세 정보 CRUD, 검색 및 필터링, 통계 조회  
- ✅ 데이터 유효성 검증, 자동 마이그레이션 (앱 시작 시)

## 📦 레거시 아카이브

과거 개발 과정의 코드들은 프로젝트 정리를 위해 아카이브되었습니다:

- **위치**: `src-tauri/src/application/archive/`
- **아카이브 파일**: `use_cases_archive_YYYYMMDD_HHMMSS.zip`
- **목적**: 개발 히스토리 보존하면서 프로젝트 구조 정리

필요시 복원: `cd src-tauri/src/application/archive/ && unzip use_cases_archive_*.zip`

## 🧹 Code Cleanup & Modernization

As part of the ongoing modernization effort:
- Legacy UI components have been moved to `src/legacy/` directory and excluded from TypeScript checking
- The application now uses a modern tab-based UI structure based on SolidJS
- Entry point has been updated to use `AppTabBased.tsx`
- All TypeScript errors have been resolved for active codebase
- Project structure follows clean architecture principles outlined in the guides

For more information on the UI implementation, see [SolidJS-UI-Implementation-Guide.md](./guide/SolidJS-UI-Implementation-Guide.md).
