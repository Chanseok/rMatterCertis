# Matter Certis v2 - 프로젝트 요구사항 명세서

## 📋 프로젝트 개요

### 목표
기존 Electron 기반 크롤링 애플리케이션을 Tauri + Rust + SolidJS로 완전히 재구축하여 성능과 리소스 효율성을 혁신적으로 개선

### 핵심 가치
- **성능 최우선**: 메모리 사용량 70% 감소 (500MB → 150MB)
- **타입 안전성**: Rust + TypeScript로 런타임 에러 최소화
- **경량화**: 번들 크기 70% 감소 (100MB → 30MB)
- **안정성**: 메모리 안전성과 동시성 안전성 보장

## 🏗️ 아키텍처 설계

### 기술 스택 결정

#### Backend (Rust)
```toml
[dependencies]
# 핵심 프레임워크
tauri = { version = "2.0", features = ["api-all"] }
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.0", features = ["full"] }

# HTTP 클라이언트 (단일 라이브러리)
reqwest = { version = "0.11", features = ["json", "cookies", "gzip"] }

# 데이터베이스
sqlx = { version = "0.7", features = ["sqlite", "runtime-tokio-rustls", "chrono"] }

# HTML 파싱
scraper = "0.18"

# 에러 처리
anyhow = "1.0"
thiserror = "1.0"

# 비동기/병렬 처리
rayon = "1.7"
futures = "0.3"

# 설정 관리
config = "0.13"
serde_json = "1.0"

# 로깅
tracing = "0.1"
tracing-subscriber = "0.3"

# 유틸리티
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.0", features = ["v4", "serde"] }
```

#### Frontend (TypeScript + SolidJS)
```json
{
  "dependencies": {
    "solid-js": "^1.8.0",
    "@solidjs/router": "^0.10.0",
    "vite": "^5.0.0",
    "vite-plugin-solid": "^2.8.0",
    "@tauri-apps/api": "^2.0.0",
    "solid-primitives": "^1.8.0",
    "@kobalte/core": "^0.12.0",
    "date-fns": "^2.30.0",
    "nanoid": "^5.0.0"
  }
}
```

### 아키텍처 계층

```
┌─────────────────────────────────────────────────────────────┐
│                  Presentation Layer                         │
│                    (SolidJS)                                │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────────┐ │
│  │ Dashboard   │ │ Settings    │ │ Reports & Analytics     │ │
│  │ Components  │ │ Management  │ │ Data Visualization      │ │
│  └─────────────┘ └─────────────┘ └─────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                                │
┌─────────────────────────────────────────────────────────────┐
│                 Application Layer                           │
│                  (TypeScript)                               │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────────┐ │
│  │ State       │ │ Domain      │ │ Service Adapters        │ │
│  │ Management  │ │ Models      │ │ (Tauri Commands)        │ │
│  └─────────────┘ └─────────────┘ └─────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                                │
┌─────────────────────────────────────────────────────────────┐
│                   Domain Layer                              │
│                     (Rust)                                  │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────────┐ │
│  │ Entities    │ │ Use Cases   │ │ Domain Services         │ │
│  │ & Value     │ │ (Business   │ │ (Business Rules)        │ │
│  │ Objects     │ │ Logic)      │ │                         │ │
│  └─────────────┘ └─────────────┘ └─────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                                │
┌─────────────────────────────────────────────────────────────┐
│                Infrastructure Layer                         │
│                     (Rust)                                  │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────────┐ │
│  │ Database    │ │ HTTP Client │ │ Configuration           │ │
│  │ Repository  │ │ (reqwest)   │ │ & External Services     │ │
│  └─────────────┘ └─────────────┘ └─────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## 🎯 기능 요구사항

### 1. 크롤링 엔진
- **HTTP 기반 크롤링**: reqwest를 활용한 효율적인 데이터 수집
- **병렬 처리**: 최대 동시 요청 수 제어 (기본값: 10)
- **Rate Limiting**: 웹사이트별 요청 제한 준수
- **재시도 로직**: 실패한 요청에 대한 지능적 재시도
- **Session 관리**: 쿠키 및 인증 정보 유지
- **HTML 파싱**: scraper를 이용한 DOM 요소 추출

### 2. 데이터 관리
- **SQLite 데이터베이스**: 로컬 데이터 저장
- **실시간 동기화**: 크롤링 진행 상황 실시간 업데이트
- **데이터 검증**: 수집된 데이터의 무결성 검증
- **중복 제거**: 동일한 제품 데이터의 중복 방지
- **데이터 내보내기**: JSON, CSV, Excel 형식 지원

### 3. 사용자 인터페이스
- **대시보드**: 크롤링 진행 상황 및 통계 시각화
- **설정 관리**: 크롤링 대상 사이트 및 파라미터 설정
- **실시간 로그**: 크롤링 과정의 상세 로그 표시
- **데이터 뷰어**: 수집된 데이터의 검색 및 필터링
- **알림 시스템**: 크롤링 완료, 오류 발생 시 알림

### 4. 성능 및 안정성
- **메모리 효율성**: 150MB 이하 메모리 사용
- **빠른 시작**: 1초 이내 애플리케이션 시작
- **에러 복구**: 네트워크 오류, 파싱 오류 등에 대한 robust 처리
- **로깅**: 상세한 디버깅 정보 제공

## 📊 성능 목표

| 메트릭 | 현재 (Electron) | 목표 (Tauri) | 개선율 |
|--------|----------------|---------------|--------|
| 메모리 사용량 | ~500MB | ~150MB | 70% 감소 |
| 번들 크기 | ~100MB | ~30MB | 70% 감소 |
| 시작 시간 | ~3초 | ~1초 | 66% 개선 |
| CPU 사용률 | 기준값 | 20-30% 개선 | 성능 향상 |

## 🔧 기술적 제약사항

### 환경 요구사항
- **운영체제**: Windows 10+, macOS 10.15+, Linux (Ubuntu 20.04+)
- **메모리**: 최소 4GB RAM
- **디스크**: 최소 500MB 여유 공간
- **네트워크**: 인터넷 연결 필수

### 개발 환경
- **Rust**: 1.75.0 이상
- **Node.js**: 18.0.0 이상
- **TypeScript**: 5.3.0 이상

## 📋 데이터 모델

### 핵심 엔티티

#### Product (제품)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: String,
    pub name: String,
    pub price: Option<f64>,
    pub currency: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub product_url: String,
    pub vendor_id: String,
    pub category: Option<String>,
    pub in_stock: bool,
    pub collected_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
```

#### Vendor (벤더)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vendor {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub crawling_config: CrawlingConfig,
    pub is_active: bool,
    pub last_crawled_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
```

#### CrawlingSession (크롤링 세션)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlingSession {
    pub id: String,
    pub vendor_id: String,
    pub status: CrawlingStatus,
    pub total_pages: Option<u32>,
    pub processed_pages: u32,
    pub products_found: u32,
    pub errors_count: u32,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}
```

## 🔐 보안 요구사항

### 데이터 보안
- **로컬 데이터**: SQLite 데이터베이스 암호화
- **네트워크**: HTTPS 통신 강제
- **설정**: 민감한 설정 정보 암호화 저장

### 애플리케이션 보안
- **CSP**: Content Security Policy 적용
- **Sandbox**: Tauri 보안 모델 준수
- **Input Validation**: 모든 사용자 입력 검증

## 🧪 품질 보증

### 테스트 전략
- **단위 테스트**: 각 모듈별 90% 이상 커버리지
- **통합 테스트**: API 및 데이터베이스 통합 테스트
- **E2E 테스트**: 전체 크롤링 플로우 테스트
- **성능 테스트**: 메모리 사용량 및 속도 벤치마크

### 코드 품질
- **Rust**: clippy, rustfmt 준수
- **TypeScript**: ESLint, Prettier 준수
- **Documentation**: 모든 public API 문서화

## 📈 확장성 고려사항

### 미래 기능
- **클라우드 동기화**: 설정 및 데이터 클라우드 백업
- **플러그인 시스템**: 사용자 정의 크롤링 로직
- **API 서버**: RESTful API 제공
- **분산 크롤링**: 여러 인스턴스 간 작업 분산

### 성능 최적화
- **캐싱**: 중간 결과 캐싱
- **압축**: 데이터 압축 저장
- **인덱싱**: 데이터베이스 쿼리 최적화

## 🚀 배포 전략

### 패키징
- **단일 바이너리**: OS별 자체 포함 실행 파일
- **자동 업데이트**: Tauri 업데이트 시스템
- **설치 프로그램**: OS별 네이티브 설치 프로그램

### 배포 채널
- **GitHub Releases**: 오픈소스 배포
- **자동 빌드**: GitHub Actions CI/CD
- **코드 서명**: 보안 인증서 적용

이 요구사항 명세서를 바탕으로 단계별 개발 가이드를 작성하겠습니다.
