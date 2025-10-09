# Scripts 폴더

이 폴더는 개발, 테스트, 분석에 사용되는 유틸리티 스크립트를 포함합니다.

## 📊 분석 스크립트

### analyze_crawl_pages.py
좌표 갱신/크롤링 로그를 분석하여 페이지별 제품 수집 결과를 보고합니다.

**사용법:**
```bash
# 기본 로그 파일 분석 (src-tauri/target/debug/logs/back_front.log)
python3 scripts/analyze_crawl_pages.py

# 특정 로그 파일 분석
python3 scripts/analyze_crawl_pages.py path/to/logfile.log
```

**출력 내용:**
- 총 처리된 페이지 수 및 범위
- 완전/불완전 페이지 통계
- 총 수집된 제품 수 및 수집률
- 페이지 번호 기준 (0-based vs 1-based) 확인
- 불완전 페이지 목록 (있는 경우)

**예시 출력:**
```
📊 좌표 갱신/크롤링 분석 결과
============================================================
📁 로그 파일: /Users/.../back_front.log

📈 페이지 통계:
  총 처리된 페이지: 596
  페이지 범위: 1 ~ 596

✅ 완전 페이지:
  12개 제품 페이지: 595
  3개 제품 페이지 (마지막): 1

📦 제품 통계:
  총 수집된 제품: 7,143
  예상 제품 수: 7,143
  ✅ 수집률: 100% (완벽!)

✅ 불완전 페이지: 0
✅ 모든 페이지에서 예상된 개수의 제품을 성공적으로 수집!

📍 페이지 번호 기준:
  가장 작은 페이지: 1
  가장 큰 페이지: 596
  ✅ 페이지 번호가 1부터 시작 (1-based, 정상)
```

---

## 🗄️ 데이터베이스 스크립트

### db_consistency_check.sh
데이터베이스 정합성을 검사합니다.

### sqlite_dedup_by_url.sh
URL 기준 중복 제거를 수행합니다.

### fix_id_mismatches.sh
ID 불일치를 수정합니다.

---

## 🧪 테스트 스크립트

### test-fast.sh
빠른 테스트 실행

### test-db-fast.sh
DB 관련 빠른 테스트

### run_and_verify.sh
실행 후 검증

### run_and_verify_unified.sh
통합 실행 및 검증

---

## 🔍 코드 품질 스크립트

### audit_tauri_commands_usage.sh
Tauri 명령어 사용 현황 분석

### audit_backend_unused.sh
백엔드 미사용 코드 검사

### find_unused_frontend.mjs
프론트엔드 미사용 코드 검색

### lint_rust.sh
Rust 코드 린팅

### fix_clippy_errors.sh
Clippy 오류 수정

---

## 🛠️ 유틸리티 스크립트

### generate_types.sh
타입 정의 파일 생성

### generate_dep_graphs.sh
의존성 그래프 생성

### serve_dep_graphs.sh
의존성 그래프 서버 실행

### cleanup_backups.sh
백업 파일 정리

### precommit.sh
커밋 전 검사

---

## 📁 아카이브

### _archived_20251009/
2025년 10월 9일 기준 아카이브된 스크립트

### _backups/
백업 파일
