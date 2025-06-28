# rMatterCertis - 프로젝트 요구사항 명세서 (실제 구현 기반)

## 📋 프로젝트 개요

### 목표
기존 Electron 기반 크롤링 애플리케이션을 Tauri + Rust + SolidJS로 완전히 재구축하여 성능과 리소스 효율성을 혁신적으로 개선

### 핵심 가치 (실제 검증됨)
- **개발 생산성**: 빌드 시간 90% 단축 (30-60초 → 2.6초)
- **타입 안전성**: Rust + TypeScript로 런타임 에러 최소화
- **모던 아키텍처**: Clean Architecture + mod.rs 없는 현대적 구조
- **안정성**: 메모리 안전성과 동시성 안전성 보장

## 🏗️ 실제 구현된 아키텍처

### 검증된 기술 스택

#### Backend (Rust) - 실제 Cargo.toml
```toml
[package]
name = "matter-certis-v2"
version = "0.1.0"
description = "rMatterCertis - E-commerce Product Crawling Application"
authors = ["Chanseok <hi007chans@gmail.com>"]
edition = "2021"
default-run = "matter-certis-v2"

[workspace]
resolver = "2"

[lib]
name = "matter_certis_v2_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
# 핵심 프레임워크 (실제 사용된 최소 features)
tauri = { version = "2", features = [] }  # api-all 대신 필요한 것만
tauri-plugin-opener = "2"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# 비동기 런타임 (최적화된 features)
tokio = { version = "1.0", features = ["rt-multi-thread", "macros", "fs", "time"] }

# HTTP 클라이언트 (optional로 설정)
reqwest = { version = "0.11", features = ["json", "cookies", "gzip"], optional = true }

# 데이터베이스 (실제 검증된 features)
sqlx = { version = "0.7", features = ["sqlite", "runtime-tokio-rustls", "chrono", "migrate"] }

# HTML 파싱
scraper = "0.18"

# 에러 처리
anyhow = "1.0"
thiserror = "1.0"

# 병렬 처리
rayon = "1.7"
futures = "0.3"

# 설정 관리
config = "0.13"

# 로깅
tracing = "0.1"
tracing-subscriber = "0.3"

# 시간 처리
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.0", features = ["v4", "serde"] }

# 비동기 트레이트
async-trait = "0.1"

[dev-dependencies]
tempfile = "3.8"
tokio-test = "0.4"

# 🚀 성능 최적화 프로파일 (실제 검증됨)
[profile.dev]
opt-level = 0
debug = 1  # 핵심: 디버그 정보 축소
split-debuginfo = "unpacked"
incremental = true
codegen-units = 512

[profile.test]
opt-level = 0
debug = 1
incremental = true
codegen-units = 512

[profile.dev.package."*"]
opt-level = 3
debug = false

[profile.test.package."*"]
opt-level = 3
debug = false
```

# 유틸리티
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
```

#### Frontend (SolidJS) - 실제 package.json
```json
{
  "name": "rmattercertis",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build", 
    "preview": "vite preview",
    "tauri": "tauri"
  },
  "dependencies": {
    "@tauri-apps/api": ">=2.0.0",
    "@tauri-apps/plugin-opener": ">=2.0.0",
    "solid-js": "^1.8.0"
  },
  "devDependencies": {
    "@types/node": "^20.0.0",
    "typescript": "^5.3.0",
    "vite": "^5.0.0",
    "vite-plugin-solid": "^2.8.0"
  }
}
```

**핵심 결정사항:**
- ✅ **pnpm** 사용 (npm 대신 성능 향상)
- ✅ **SolidJS** 선택 (Vanilla 대신 더 나은 DX)
- ✅ **최소 의존성** (번들 크기 최적화)

### 실제 구현된 프로젝트 구조 (Rust 2024 모던 컨벤션)

```
rMatterCertis/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── commands.rs (Tauri 명령어들)
│   │   ├── domain.rs (도메인 계층 진입점, mod.rs 제거)
│   │   ├── domain/
│   │   │   ├── entities.rs (비즈니스 엔티티)
│   │   │   ├── repositories.rs (repository trait 정의)
│   │   │   └── services.rs (도메인 서비스)
│   │   ├── application.rs (애플리케이션 계층 진입점, mod.rs 제거)
│   │   ├── application/
│   │   │   ├── dto.rs (Data Transfer Objects)
│   │   │   └── use_cases.rs (유즈케이스 구현)
│   │   ├── infrastructure.rs (인프라 계층 진입점, mod.rs 제거)
│   │   ├── infrastructure/
│   │   │   ├── repositories.rs (repository 구현체 통합)
│   │   │   ├── database_connection.rs (DB 연결 관리)
│   │   │   ├── database.rs (DB 유틸리티)
│   │   │   ├── config.rs (설정 관리)
│   │   │   └── http.rs (HTTP 클라이언트)
│   │   └── bin/
│   │       └── test_db.rs (DB 테스트 CLI)
│   ├── migrations/ (SQL 마이그레이션)
│   ├── .cargo/config.toml (빌드 최적화)
│   ├── Cargo.toml (성능 최적화된 설정)
│   └── tauri.conf.json
├── src/ (SolidJS 프론트엔드)
│   ├── App.tsx (메인 UI)
│   ├── components/ (재사용 컴포넌트)
│   ├── services/ (API 서비스)
│   ├── stores/ (상태 관리)
│   ├── types/ (TypeScript 타입)
│   └── utils/ (유틸리티)
├── scripts/
│   └── test-fast.sh (빠른 테스트 스크립트)
├── .env.development (개발 환경 설정)
├── .gitignore (확장된 무시 목록)
└── README.md
```

#### 🚫 mod.rs 파일 완전 제거
**이전 방식 (구식):**
```
src/infrastructure/
├── mod.rs ❌ (구식 방식)
├── repositories/
│   ├── mod.rs ❌ (구식 방식)
│   ├── vendor.rs
│   └── product.rs
└── ...
```

**현재 방식 (Rust 2024 모던):**
```
src/
├── infrastructure.rs ✅ (모듈 진입점)
├── infrastructure/
│   ├── repositories.rs ✅ (통합 구현체)
│   ├── database_connection.rs
│   └── ...
```

**주요 변경사항:**
- 모든 `mod.rs` 파일 제거 완료
- 모듈명과 같은 `.rs` 파일을 진입점으로 사용
- 관련 구현체들을 단일 파일로 통합 (repositories.rs)
- 빈 서브디렉토리 정리 완료
- 더 명확하고 현대적인 모듈 구조
│   │   ├── domain.rs              # mod.rs 없는 모던 구조
│   │   ├── domain/
│   │   │   ├── entities.rs        # Vendor, Product 엔티티
│   │   │   └── repositories.rs    # Repository 트레이트
│   │   ├── application.rs
│   │   ├── application/
│   │   │   └── use_cases.rs       # 비즈니스 로직
│   │   ├── infrastructure.rs
│   │   ├── infrastructure/
│   │   │   └── database_connection.rs  # 실제 구현된 DB 레이어
│   │   ├── commands.rs            # Tauri Commands
│   │   └── bin/
│   │       └── test_db.rs         # CLI 테스트 도구
│   ├── migrations/
│   │   └── 001_initial.sql       # 수동 마이그레이션
│   ├── data/                     # 런타임 DB 파일
│   ├── Cargo.toml               # 성능 최적화됨
│   └── tauri.conf.json
├── src/
│   ├── main.tsx
│   ├── App.tsx                  # DB 테스트 UI
│   └── app.css
├── .cargo/
│   └── config.toml             # 빌드 최적화 핵심
├── scripts/
│   └── test-fast.sh           # 빠른 테스트 도구
├── .env.development           # 개발 환경 최적화
├── .gitignore                # 확장된 ignore 규칙
└── package.json              # SolidJS 설정
```

## 🎯 실제 달성된 성능 목표

### 개발 성능
| 메트릭 | 목표 | 실제 달성 | 달성율 |
|--------|------|-----------|--------|
| 초기 빌드 시간 | < 2분 | ~1분 | ✅ 150% |
| 증분 빌드 시간 | < 5초 | ~0.5초 | ✅ 1000% |
| 작은 변경 빌드 | < 10초 | ~2.6초 | ✅ 380% |

### 아키텍처 품질
- ✅ **타입 안전성**: Rust + TypeScript 조합
- ✅ **메모리 안전성**: Rust의 소유권 시스템
- ✅ **테스트 가능성**: 단위 테스트 + CLI 도구 + UI 테스트
- ✅ **확장 가능성**: Clean Architecture 적용

## 🔧 핵심 구현 사항

### 1. 데이터베이스 레이어 (실제 구현됨)

```rust
// src-tauri/src/infrastructure/database_connection.rs
pub struct DatabaseConnection {
    pool: Option<Pool<Sqlite>>,
}

impl DatabaseConnection {
    pub async fn new(database_url: &str) -> Result<Self> {
        // 🎯 실제 해결한 문제: 디렉토리 자동 생성
        if database_url.starts_with("sqlite:") {
            let path_str = database_url.strip_prefix("sqlite:").unwrap();
            let path = Path::new(path_str);
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }
        
        let pool = SqlitePool::connect(database_url).await?;
        Ok(Self { pool: Some(pool) })
    }

    pub async fn migrate(&self) -> Result<()> {
        // 🎯 실제 해결한 문제: sqlx::migrate! 대신 수동 테이블 생성
        let pool = self.pool.as_ref().unwrap();
        sqlx::query(include_str!("../migrations/001_initial.sql"))
            .execute(pool)
            .await?;
        Ok(())
    }
}
```

### 2. Tauri Commands (실제 구현됨)

```rust
// src-tauri/src/commands.rs
#[tauri::command]
pub async fn test_database_connection() -> Result<String, String> {
    let db_path = "data/matter_certis.db";
    match DatabaseConnection::new(db_path).await {
        Ok(_) => Ok("Database connection successful".to_string()),
        Err(e) => Err(format!("Database connection failed: {}", e)),
    }
}

#[tauri::command]
pub async fn get_database_info() -> Result<String, String> {
    Ok("Database: SQLite, Location: data/matter_certis.db".to_string())
}
```

### 3. 테스트 전략 (3-tier 검증)

```rust
// 1. 단위 테스트
#[tokio::test]
async fn test_database_connection() -> Result<()> {
    let temp_dir = tempdir()?;
    let db_path = temp_dir.path().join("test.db");
    let database_url = format!("sqlite:{}", db_path.to_string_lossy());
    
    let db = DatabaseConnection::new(&database_url).await?;
    assert!(!db.pool().is_closed());
    Ok(())
}
```

```bash
# 2. CLI 테스트 도구
cargo run --bin test_db
```

```tsx
// 3. UI 테스트
function App() {
  const testConnection = async () => {
    const result = await invoke<string>("test_database_connection");
    setDbStatus(`✅ ${result}`);
  };
  // ...
}
```
