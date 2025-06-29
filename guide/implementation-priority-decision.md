# Matter Certis v2 - 구현 우선순위 결정

**📅 작성일: 2025년 1월 28일**  
**🎯 목적: 크롤링 엔진 vs 설정 관리 구현 순서 최종 결정**

## 🔍 현재 상황 분석

### ✅ 이미 구현된 핵심 구조
1. **크롤링 엔진 기본 골격**: 완전히 구현됨 (727라인)
   - WebCrawler 클래스, CrawlingConfig, 세션 관리
   - HTTP 클라이언트, 병렬 처리, 에러 핸들링
   - 데이터 추출 및 저장 로직

2. **Frontend 크롤링 UI**: 기본 구현 완료
   - CrawlingForm, CrawlingDashboard, CrawlingResults
   - Tauri 명령어 연동 (`start_crawling`, `get_crawling_status`)

3. **Backend 데이터 흐름**: 완전히 구현됨
   - SessionManager, Repository 패턴, Use Cases
   - DTO 계층, 데이터베이스 마이그레이션

### 🔄 부분 구현된 부분
1. **설정 관리**: 설계는 완료, 실제 구현 필요
   - ConfigManager 파일 I/O 로직
   - 설정 잠금 시스템
   - Frontend/Backend 설정 동기화

2. **크롤링 세부 로직**: 기본 구조는 있으나 개선 필요
   - 제품 데이터 추출 정확도
   - 에러 복구 및 재시도
   - 성능 최적화

## 🎯 결정: 크롤링 엔진 우선 구현

### 📋 근거

#### 1. **즉시 가치 제공**
- 크롤링 엔진은 앱의 핵심 기능 (설정은 보조 기능)
- 사용자가 바로 데이터 수집을 시작할 수 있음
- MVP(Minimum Viable Product) 관점에서 우선순위 높음

#### 2. **기술적 준비도**
- 크롤링 엔진 기본 구조 727라인 완성 (95% 구현됨)
- Frontend UI와 Backend API 연동 완료
- 추가 구현량이 상대적으로 적음

#### 3. **설정 관리의 현재 상태**
- 현재 기본 설정으로도 크롤링 가능
- 하드코딩된 설정으로 테스트 및 개발 진행 가능
- 설정 시스템은 크롤링이 안정화된 후 추가해도 늦지 않음

#### 4. **개발 효율성**
- 크롤링 로직 완성 → 실제 데이터로 테스트 가능
- 실제 크롤링 결과를 보면서 설정 시스템 설계 개선 가능
- 사용자 피드백을 받아 설정 항목 우선순위 결정

## 📋 구체적 구현 계획

### Phase 1: 크롤링 엔진 완성 (1-2주)

#### 🎯 목표: 안정적이고 정확한 데이터 수집

#### 주요 구현 항목

##### 1. 제품 데이터 추출 정확도 개선
```rust
// 현재: 기본 HTML 파싱
// 목표: 정확한 Matter 제품 데이터 추출

impl WebCrawler {
    // 개선할 메서드들
    async fn extract_matter_products(&self, page: &CrawledPage) -> Result<Vec<ExtractedMatterProduct>>;
    async fn extract_product_links(&self, content: &str, domains: &[String]) -> Result<Vec<String>>;
    async fn parse_certification_details(&self, product_url: &str) -> Result<MatterProductDetails>;
}
```

##### 2. 에러 복구 및 재시도 시스템
```rust
// 구현할 기능
- HTTP 요청 실패 시 지수 백오프 재시도
- 임시 네트워크 오류 자동 복구
- 세션 중단 후 재시작 지원
- 부분 완료된 크롤링 재개
```

##### 3. 성능 최적화
```rust
// 구현할 기능
- 동적 동시성 조절 (서버 응답 시간 기반)
- 스마트 큐 관리 (중복 URL 제거, 우선순위)
- 메모리 사용량 최적화
- 진행 상황 실시간 업데이트
```

##### 4. 강화된 세션 관리
```rust
// 현재 기능 확장
- 세션 일시정지/재개
- 세션 통계 (성공률, 평균 속도 등)
- 세션 로그 상세화
- 세션 결과 내보내기
```

#### Frontend 개선사항
```typescript
// 1. 실시간 진행 상황 표시
interface CrawlingProgress {
  current_page: number;
  total_pages: number;
  success_rate: number;
  pages_per_minute: number;
  estimated_remaining_time: number;
}

// 2. 에러 상세 정보 및 복구 옵션
interface CrawlingError {
  error_type: string;
  url: string;
  retry_count: number;
  can_retry: boolean;
  suggested_action: string;
}

// 3. 세션 제어 강화
interface SessionControl {
  pause(): Promise<void>;
  resume(): Promise<void>;
  stop(): Promise<void>;
  restart_from_last(): Promise<void>;
}
```

### Phase 2: 설정 관리 시스템 (1-2주)

#### 🎯 목표: 크롤링 엔진이 안정화된 후 설정 관리 추가

크롤링 엔진이 완성되고 실제 사용해본 후에 다음을 구현:

1. **실사용 기반 설정 항목 확정**
   - 어떤 설정이 실제로 필요한지 경험적 데이터
   - 사용자가 자주 변경하는 설정 우선순위

2. **ConfigManager 구현**
   - JSON 파일 기반 설정 저장/로드
   - 환경별 설정 오버라이드
   - 설정 검증 및 기본값 처리

3. **크롤링 중 설정 잠금**
   - 활성 세션 중 설정 변경 방지
   - 사용자 친화적 안내 메시지

## 🚀 즉시 시작할 작업

### 1. 크롤링 엔진 세부 구현 완성

#### A. 제품 데이터 추출 정확도 개선
- CSA-IoT 사이트 HTML 구조 분석
- 제품 상세 정보 파싱 로직 정교화
- 다양한 페이지 형태 처리

#### B. 에러 처리 강화
- 네트워크 에러 재시도 로직
- 파싱 실패 시 대체 방법
- 세션 복구 메커니즘

#### C. 성능 최적화
- 동시 요청 수 동적 조절
- 메모리 사용량 모니터링
- 진행 속도 통계

### 2. Frontend 크롤링 경험 개선

#### A. 실시간 진행 상황 표시
- 페이지별 크롤링 진행률
- 실시간 성공률 및 속도
- 예상 완료 시간

#### B. 에러 처리 UI
- 에러 상세 정보 표시
- 재시도 옵션 제공
- 문제 해결 가이드

#### C. 세션 제어 강화
- 일시정지/재개 기능
- 세션 히스토리
- 결과 내보내기

## 📝 구현 순서 요약

```
Week 1-2: 크롤링 엔진 완성
├── 제품 데이터 추출 정확도 개선
├── 에러 복구 및 재시도 시스템
├── 성능 최적화
└── Frontend 크롤링 경험 개선

Week 3-4: 설정 관리 시스템 (크롤링 안정화 후)
├── 실사용 기반 설정 항목 확정
├── ConfigManager 구현
├── 크롤링 중 설정 잠금
└── Frontend 설정 UI 완성

Week 5+: 통합 테스트 및 최적화
├── End-to-end 테스트
├── 성능 벤치마크
├── 사용자 경험 개선
└── 배포 준비
```

## 🎯 다음 단계

1. **크롤링 엔진 제품 데이터 추출 로직 정교화**
2. **에러 처리 및 재시도 시스템 구현**
3. **Frontend 실시간 진행 상황 표시 개선**
4. **실제 크롤링 테스트 및 피드백 수집**

---

**결론**: 크롤링 엔진을 먼저 완성하여 핵심 가치를 제공하고, 실사용 경험을 바탕으로 설정 관리 시스템을 구현하는 것이 가장 효율적인 접근법입니다.
