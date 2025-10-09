# 동기화 모드 검증 계획

## 📋 개요

세 가지 동기화 모드가 SessionActor 기반 아키텍처로 통합되었습니다:
1. **빠른 동기화 (Fast Sync)** - 리스트 페이지만 크롤링, 좌표 갱신
2. **스마트 동기화 (Smart Sync)** - 빠른 동기화 + 자동 제품 보완
3. **제품 보완 동기화 (Complement Crawl)** - 핵심 필드 누락 제품만 재크롤링

이 문서는 각 모드의 검증 항목과 예상 동작을 정리합니다.

---

## 🏃 빠른 동기화 (Fast Sync)

### 기능 설명
- 전체 페이지의 리스트만 크롤링
- 제품 URL과 좌표(page_id, index_in_page) 갱신
- 제품 상세 정보는 수집하지 않음 (빠른 실행)

### 검증 항목

#### 1. 시작 및 세션 생성
- [ ] 버튼 클릭 시 즉시 응답
- [ ] 세션 ID가 반환되고 프론트엔드에 저장됨
- [ ] 로그에 "🆔 세션 ID: fast-sync-..." 출력
- [ ] 로그에 "💡 백그라운드에서 작업이 진행됩니다" 메시지

#### 2. 중지 버튼
- [ ] 빠른 동기화 실행 중 중지 버튼 활성화
- [ ] 중지 버튼 클릭 시 "🛑 작업 중지 요청..." 로그
- [ ] 세션이 실제로 취소됨 (백엔드 로그 확인)
- [ ] UI 상태가 초기화됨 (isRunning → false)

#### 3. 이벤트 수신
- [ ] `actor-session-started` 이벤트 수신 (세션 시작)
- [ ] `list-page-batch-started` 이벤트 수신 (배치 시작)
- [ ] `list-page-progress` 이벤트 수신 (각 페이지 진행)
- [ ] `actor-session-completed` 이벤트 수신 (세션 완료)
- [ ] 완료 이벤트 수신 후 isRunning → false

#### 4. DB 업데이트 확인
```sql
-- 좌표가 갱신되었는지 확인
SELECT COUNT(*) FROM integrated_products 
WHERE page_id IS NOT NULL AND index_in_page IS NOT NULL;

-- 최근 업데이트 확인
SELECT COUNT(*) FROM integrated_products 
WHERE updated_at > datetime('now', '-5 minutes');
```

#### 5. 성능
- [ ] 100페이지 크롤링이 3분 이내 완료 (목표)
- [ ] 메모리 사용량이 500MB 이하 유지
- [ ] CPU 사용률이 합리적 범위 내

---

## 🧠 스마트 동기화 (Smart Sync)

### 기능 설명
- 빠른 동기화 실행
- 빠른 동기화 완료 후 자동으로 제품 보완 크롤링 실행
- 두 단계가 하나의 세션으로 연결됨

### 검증 항목

#### 1. 시작 및 세션 생성
- [ ] 버튼 클릭 시 즉시 응답
- [ ] 세션 ID가 반환되고 저장됨
- [ ] 로그에 "🧠 스마트 동기화 시작" 출력
- [ ] 로그에 shallow_crawl 결과의 session_id 추출 로그

#### 2. 중지 버튼
- [ ] 실행 중 중지 버튼 활성화
- [ ] 중지 클릭 시 전체 세션 취소
- [ ] 빠른 동기화 중이든 보완 크롤링 중이든 중지 가능

#### 3. 이벤트 수신
- [ ] `actor-session-started` (빠른 동기화)
- [ ] 리스트 페이지 크롤링 이벤트들
- [ ] 리스트 완료 후 `actor-session-completed`
- [ ] 자동으로 제품 보완 크롤링 시작 (새 세션)
- [ ] `actor-session-started` (제품 보완)
- [ ] 제품 상세 크롤링 이벤트들
- [ ] 최종 `actor-session-completed`

#### 4. DB 업데이트 확인
```sql
-- 좌표 갱신 확인 (빠른 동기화)
SELECT COUNT(*) FROM integrated_products 
WHERE page_id IS NOT NULL;

-- 상세 정보 갱신 확인 (보완 크롤링)
SELECT COUNT(*) FROM integrated_products 
WHERE certification_date IS NOT NULL 
  AND transport_interface IS NOT NULL;
```

#### 5. 로그 시퀀스
```
1. 🧠 스마트 동기화 시작
2. 🆔 세션 ID: shallow-sync-...
3. 💡 백그라운드에서 작업이 진행됩니다
4. [리스트 크롤링 로그들...]
5. ✅ 빠른 동기화 완료
6. [자동 전환]
7. 🔧 제품 보완 크롤링 시작
8. [상세 크롤링 로그들...]
9. ✅ 전체 동기화 완료
```

---

## 🔧 제품 보완 동기화 (Complement Crawl)

### 기능 설명
- 핵심 필드(cert_date, transport, device_type_ids) 누락 제품만 선별
- URL 목록을 직접 SessionActor에 전달
- 리스트 크롤링 건너뛰고 바로 ProductDetailCrawling 실행

### 검증 항목

#### 1. 시작 및 세션 생성
- [ ] 버튼 클릭 시 누락 제품 수 쿼리 (query_missing_products)
- [ ] 누락 제품이 없으면 즉시 완료 메시지
- [ ] 누락 제품이 있으면 세션 ID 반환
- [ ] 로그에 "🔧 제품 보완 동기화 시작" 출력
- [ ] 로그에 "🔄 N개 제품 병렬 재크롤링 시작"

#### 2. 중지 버튼
- [ ] 실행 중 중지 버튼 활성화
- [ ] 중지 클릭 시 세션 취소
- [ ] 진행 중인 제품 크롤링 중단

#### 3. URL 기반 크롤링 확인
- [ ] 백엔드 로그에 "🎯 [URL-based Mode]" 출력
- [ ] 백엔드 로그에 "skipping list crawling" 출력
- [ ] ListPageCrawling 단계가 실행되지 않음
- [ ] 바로 ProductDetailCrawling 시작

#### 4. 이벤트 수신
- [ ] `actor-session-started` (URL 모드)
- [ ] `product-detail-progress` 이벤트들
- [ ] `actor-session-completed` (완료)
- [ ] 리스트 크롤링 이벤트는 발생하지 않음

#### 5. DB 업데이트 확인
```sql
-- 보완 전: 누락 제품 수 확인
SELECT COUNT(*) FROM integrated_products 
WHERE certification_date IS NULL 
   OR transport_interface IS NULL 
   OR primary_device_type_ids IS NULL;

-- 보완 후: 누락 제품 수 감소 확인
SELECT COUNT(*) FROM integrated_products 
WHERE certification_date IS NOT NULL 
  AND transport_interface IS NOT NULL 
  AND primary_device_type_ids IS NOT NULL;
```

#### 6. 성능
- [ ] 누락 제품 100개 크롤링이 2분 이내 (목표)
- [ ] 동시성 12로 병렬 처리 확인
- [ ] 메모리 효율적 처리

---

## 🔍 공통 검증 항목

### 1. 아키텍처 일관성
- [ ] 세 가지 모드 모두 SessionActor 사용
- [ ] 세 가지 모두 session_id 반환
- [ ] 세 가지 모두 중지 버튼 지원
- [ ] 세 가지 모두 백그라운드 실행

### 2. 이벤트 시스템
- [ ] 모든 이벤트가 프론트엔드에 전달됨
- [ ] 이벤트 순서가 논리적으로 일관됨
- [ ] 에러 이벤트 처리가 적절함

### 3. 에러 처리
- [ ] 네트워크 에러 시 재시도
- [ ] 재시도 횟수 제한 준수
- [ ] 에러 메시지가 명확함
- [ ] 부분 실패 시에도 정상 완료 가능

### 4. UI/UX
- [ ] 버튼 상태 관리가 정확함 (활성화/비활성화)
- [ ] 진행 상태 메시지가 명확함
- [ ] 로그 출력이 이해하기 쉬움
- [ ] 완료 후 상태 초기화가 적절함

### 5. 로깅
- [ ] 백엔드 로그에 세션 ID 추적 가능
- [ ] 각 단계별 로그가 명확함
- [ ] 성능 메트릭 로그 출력
- [ ] 에러 로그에 충분한 컨텍스트 포함

---

## 📊 테스트 시나리오

### 시나리오 1: 정상 실행 (각 모드별)
1. 해당 모드 버튼 클릭
2. 세션 ID 확인
3. 진행 로그 관찰
4. 완료 이벤트 대기
5. DB 업데이트 확인

### 시나리오 2: 중지 버튼 (각 모드별)
1. 해당 모드 시작
2. 진행 중 중지 버튼 클릭
3. 취소 로그 확인
4. 세션 상태 확인 (취소됨)
5. UI 상태 초기화 확인

### 시나리오 3: 연속 실행
1. 빠른 동기화 실행 → 완료 대기
2. 스마트 동기화 실행 → 완료 대기
3. 제품 보완 동기화 실행 → 완료 대기
4. 세션 간 간섭 없음 확인

### 시나리오 4: 에러 처리
1. 네트워크 끊김 시뮬레이션
2. 에러 재시도 확인
3. 최종 에러 이벤트 확인
4. UI 상태 복구 확인

---

## ✅ 검증 체크리스트

### 빠른 동기화
- [ ] 시작 및 세션 생성 (5개 항목)
- [ ] 중지 버튼 (4개 항목)
- [ ] 이벤트 수신 (5개 항목)
- [ ] DB 업데이트 (2개 쿼리)
- [ ] 성능 (3개 항목)

### 스마트 동기화
- [ ] 시작 및 세션 생성 (4개 항목)
- [ ] 중지 버튼 (3개 항목)
- [ ] 이벤트 수신 (8개 항목)
- [ ] DB 업데이트 (2개 쿼리)
- [ ] 로그 시퀀스 (9개 단계)

### 제품 보완 동기화
- [ ] 시작 및 세션 생성 (5개 항목)
- [ ] 중지 버튼 (3개 항목)
- [ ] URL 기반 크롤링 (4개 항목)
- [ ] 이벤트 수신 (4개 항목)
- [ ] DB 업데이트 (2개 쿼리)
- [ ] 성능 (3개 항목)

### 공통 검증
- [ ] 아키텍처 일관성 (4개 항목)
- [ ] 이벤트 시스템 (3개 항목)
- [ ] 에러 처리 (4개 항목)
- [ ] UI/UX (4개 항목)
- [ ] 로깅 (4개 항목)

---

## 🎯 성공 기준

### 필수 (Must Have)
- ✅ 세 가지 모드 모두 정상 실행됨
- ✅ 중지 버튼이 모든 모드에서 작동함
- ✅ DB 업데이트가 정확함
- ✅ 치명적 버그 없음

### 권장 (Should Have)
- ✅ 성능 목표 달성 (빠른: 3분, 보완: 2분)
- ✅ 로그 출력이 명확함
- ✅ 에러 처리가 적절함
- ✅ UI 피드백이 즉각적임

### 선택 (Nice to Have)
- ⭐ 메모리 사용 최적화
- ⭐ 진행률 표시 정확도 향상
- ⭐ 에러 복구 자동화
- ⭐ 성능 메트릭 대시보드

---

## 📝 테스트 실행 기록

### 테스트 날짜: YYYY-MM-DD
### 테스터: [이름]

| 모드 | 시나리오 | 결과 | 비고 |
|------|---------|------|------|
| 빠른 동기화 | 정상 실행 | ⬜ | |
| 빠른 동기화 | 중지 버튼 | ⬜ | |
| 스마트 동기화 | 정상 실행 | ⬜ | |
| 스마트 동기화 | 중지 버튼 | ⬜ | |
| 제품 보완 | 정상 실행 | ⬜ | |
| 제품 보완 | 중지 버튼 | ⬜ | |
| 연속 실행 | 3모드 순차 | ⬜ | |
| 에러 처리 | 네트워크 끊김 | ⬜ | |

### 발견된 이슈
1. [이슈 설명]
2. [이슈 설명]

### 개선 제안
1. [제안 내용]
2. [제안 내용]

---

## 🔗 관련 문서
- [complement_crawl_refactoring_plan.md](./complement_crawl_refactoring_plan.md)
- [architecture.md](./architecture.md)
- [session_registry_architecture.md](./session_registry_architecture.md)
