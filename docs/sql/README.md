# SQL Enhancements

이 디렉터리는 운영/데이터 품질 향상을 위한 수동 SQL 스크립트를 보관합니다. 코드에서 자동 실행하지 않으며, 필요 시 수동으로 적용합니다.

주의사항
- 적용 전 백업을 반드시 생성하세요.
- SQLite 기준 스크립트입니다. 다른 DBMS에서는 호환되지 않을 수 있습니다.
- 동일/유사 스크립트 중 v2는 기본값/널 처리 등을 보수적으로 바꾼 변형 버전입니다.

적용 방법(예: SQLite)
```bash
# 데이터베이스 파일 경로 예시: src-tauri/target/dev.db
sqlite3 path/to/your.db < production_enhancement_v2.sql
```

파일 개요
- production_enhancement.sql: 감사 필드/인덱스/트리거/레거시 뷰 추가(기본값 포함)
- production_enhancement_v2.sql: 같은 목적의 보수적 변형(널 허용 후 일괄 세팅)
