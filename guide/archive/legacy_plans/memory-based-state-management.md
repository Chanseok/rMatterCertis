# rMatterCertis v2 - 메모리 기반 상태 관리 아키텍처

## 🎯 아키텍처 개선 배경

기존 `crawling_sessions` 테이블을 이용한 상태 관리는 다음 문제점이 있었습니다:

### ❌ 기존 방식의 문제점
- **성능 이슈**: 빈번한 DB 업데이트로 디스크 I/O 부하 증가
- **확장성 제한**: 동시 크롤링 시 SQLite 잠금 문제
- **시스템 부하**: 매 페이지마다 UPDATE 쿼리 실행

### ✅ 새로운 접근 방식: 메모리 기반 상태 관리

업계 표준인 **"상태 관리 계층 + 최종 결과만 DB 저장"** 방식을 적용합니다.

## 🏗️ 새로운 아키텍처

### 1. 메모리 상태 관리 계층

```rust
// 크롤링 세션 상태를 메모리에서 관리
pub struct CrawlingSessionState {
    pub session_id: String,
    pub status: SessionStatus,
    pub stage: CrawlingStage,
    pub current_page: u32,
    pub total_pages: u32,
    pub products_found: u32,
    pub errors_count: u32,
    pub started_at: DateTime<Utc>,
    pub estimated_completion: Option<DateTime<Utc>>,
}

// 전역 상태 관리자
pub struct SessionManager {
    sessions: Arc<Mutex<HashMap<String, CrawlingSessionState>>>,
}
```

### 2. 실시간 상태 업데이트

```rust
impl SessionManager {
    // 빠른 메모리 업데이트 (디스크 I/O 없음)
    pub fn update_progress(&self, session_id: &str, page: u32, products: u32) {
        // Arc<Mutex<T>>로 스레드 안전한 상태 업데이트
    }
    
    // UI용 즉시 상태 조회
    pub fn get_session_status(&self, session_id: &str) -> Option<CrawlingSessionState> {
        // 메모리에서 즉시 반환
    }
}
```

### 3. 최종 결과만 DB 저장

```sql
-- crawling_sessions 테이블 제거
-- crawling_results 테이블로 대체 (최종 결과만 저장)
CREATE TABLE crawling_results (
    session_id TEXT PRIMARY KEY,
    status TEXT NOT NULL,  -- Completed, Failed, Stopped
    stage TEXT NOT NULL,
    total_pages INTEGER NOT NULL,
    products_found INTEGER NOT NULL,
    errors_count INTEGER NOT NULL,
    started_at DATETIME NOT NULL,
    completed_at DATETIME NOT NULL,
    execution_time_seconds INTEGER NOT NULL,
    config_snapshot TEXT  -- JSON
);
```

## 📊 성능 개선 효과

| 항목 | 기존 방식 | 새로운 방식 | 개선 효과 |
|------|-----------|-------------|-----------|
| **상태 업데이트** | DB UPDATE 쿼리 | 메모리 연산 | 100-1000배 빠름 |
| **디스크 I/O** | 매 페이지마다 | 최종 1회만 | 99% 감소 |
| **UI 응답성** | DB 쿼리 필요 | 즉시 반환 | 실시간 |
| **동시 크롤링** | 잠금 문제 | 스레드 안전 | 확장 가능 |

## 🔄 데이터 흐름

### 크롤링 시작
1. **SessionManager**에 새 세션 등록 (메모리)
2. UI는 session_id로 실시간 상태 조회

### 크롤링 진행
1. 각 페이지 처리 후 메모리 상태만 업데이트
2. UI는 polling으로 진행률 표시 (즉시 응답)

### 크롤링 완료
1. 최종 결과를 `crawling_results` 테이블에 저장
2. 메모리에서 세션 정리 (옵션)

## 🛠️ 구현 가이드

### Phase 3 구현 순서

1. **메모리 상태 관리 계층 구현**
   - `SessionManager` 구조체
   - 스레드 안전한 상태 업데이트
   - 실시간 상태 조회 API

2. **DB 스키마 마이그레이션**
   - `crawling_sessions` 테이블 제거
   - `crawling_results` 테이블 추가

3. **크롤링 엔진 통합**
   - 메모리 상태 업데이트 연동
   - UI 실시간 피드백

4. **테스트 및 검증**
   - 성능 벤치마크
   - 동시 크롤링 테스트

## 💡 추가 최적화 기회

### WebSocket/Server-Sent Events
실시간 UI 업데이트를 위해 polling 대신 push 방식 고려

### 상태 영속화 옵션
중요한 세션은 주기적으로 체크포인트 저장

### 분산 처리 확장
향후 여러 워커 프로세스 지원 시 Redis 등 고려

## 🎉 기대 효과

- **성능**: 크롤링 속도 대폭 향상
- **사용자 경험**: 실시간 진행률 표시
- **안정성**: DB 잠금 문제 해결
- **확장성**: 동시 여러 크롤링 지원
- **시스템 자원**: CPU/디스크 사용량 감소

이 아키텍처는 **업계 표준 방식**으로, 확장성과 성능을 모두 확보할 수 있는 최적의 설계입니다.
