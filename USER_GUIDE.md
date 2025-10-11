# rMatterCertis 설치 및 사용 가이드

## 소개

rMatterCertis는 E-commerce 사이트에서 Matter 인증 제품 정보를 크롤링하고 관리하는 데스크톱 애플리케이션입니다.

## 주요 기능

- 🔍 **자동 크롤링**: Matter 인증 제품 정보 자동 수집
- 📊 **데이터베이스 관리**: SQLite 기반 로컬 데이터베이스
- 📈 **실시간 진행 상황**: 크롤링 진행률 실시간 추적
- 💾 **백업/복원**: Excel 형식으로 데이터 백업 및 복원
- 🎯 **Device Type 관리**: Matter device type 정보 편집

## 시스템 요구사항

### macOS
- macOS 10.15 (Catalina) 이상
- Apple Silicon (M1/M2/M3) 또는 Intel 프로세서

### Windows
- Windows 10 이상
- 64-bit 운영체제

## 설치 방법

### macOS

1. [Releases 페이지](https://github.com/Chanseok/rMatterCertis/releases)에서 최신 버전 다운로드
   - **Apple Silicon (M1/M2/M3)**: `rMatterCertis_[version]_aarch64.dmg`
   - **Intel Mac**: `rMatterCertis_[version]_x64.dmg`

2. DMG 파일을 더블클릭하여 마운트

3. rMatterCertis 아이콘을 Applications 폴더로 드래그

4. Applications 폴더에서 rMatterCertis 실행

**보안 주의사항**: 최초 실행 시 "인증되지 않은 개발자" 경고가 나타날 수 있습니다.
- `시스템 환경설정` > `보안 및 개인 정보 보호` > `일반` 탭에서 "확인 없이 열기" 클릭

### Windows

1. [Releases 페이지](https://github.com/Chanseok/rMatterCertis/releases)에서 최신 버전 다운로드
   - `rMatterCertis_[version]_x64_en-US.msi`

2. MSI 파일을 더블클릭하여 설치 마법사 실행

3. 설치 마법사의 지시에 따라 설치 완료

4. 시작 메뉴 또는 바탕화면 바로가기에서 실행

## 사용 방법

### 1. 초기 설정

애플리케이션을 처음 실행하면:

1. **Settings 탭**에서 크롤링 설정 구성
   - 크롤링 범위 설정 (시작/종료 페이지)
   - 동시 실행 수, 재시도 횟수 등 조정

2. **Local DB 탭**에서 데이터베이스 상태 확인

### 2. 크롤링 실행

**Crawling Engine 탭**에서:

1. **Basic Controls** 섹션:
   - `Start Crawling`: 크롤링 시작
   - `Stop Crawling`: 크롤링 중지
   - `Pause/Resume`: 일시 정지/재개

2. **Progress Monitoring**:
   - List Page Progress: 목록 페이지 크롤링 진행률
   - Product Details Progress: 제품 상세 페이지 진행률
   - Stage Statistics: 각 단계별 통계

### 3. 데이터베이스 관리

**Local DB 탭**에서:

1. **데이터 조회**:
   - 제품 목록 확인
   - 필터링 및 검색
   - Device type별 분류

2. **백업 생성**:
   - `Backup to Excel` 버튼 클릭
   - 저장 위치 선택
   - Excel 파일로 내보내기 완료

3. **데이터 복원**:
   - `Restore from Excel` 버튼 클릭
   - Excel 파일 선택
   - 복원 옵션 선택 (덮어쓰기/병합)

### 4. 고급 기능

**Settings 탭**에서:

1. **Advanced Settings**:
   - 크롤링 전략 커스터마이징
   - 데이터 검증 옵션
   - 로깅 레벨 조정

2. **Device Types Editor**:
   - Matter device type 정보 편집
   - 새 device type 추가
   - 기존 항목 수정

### 5. 문제 해결

**Diagnostics 기능**:

1. **Database Diagnostics**:
   - 데이터베이스 무결성 검사
   - 인덱스 재구축
   - 통계 업데이트

2. **Repair Functions**:
   - Null coordinates 수정
   - 중복 데이터 정리
   - 테이블 일관성 복구

## 크롤링 모드

### 1. Full Crawl (전체 크롤링)
- 모든 페이지를 처음부터 크롤링
- 초기 데이터 수집 시 사용

### 2. Complement Crawl (보완 크롤링)
- 누락된 데이터만 선택적으로 수집
- 부분적으로 실패한 크롤링 완료

### 3. Shallow Sync (얕은 동기화)
- 최신 데이터만 빠르게 업데이트
- 정기적인 데이터 갱신 시 사용

## 데이터 내보내기/가져오기

### Excel 백업 형식

생성되는 Excel 파일은 다음 시트를 포함합니다:

- **Products**: 제품 기본 정보
- **MatterProducts**: Matter 특화 정보
- **ProductURLs**: URL 정보
- **DeviceTypes**: Device type 매핑
- **Metadata**: 백업 메타데이터

### 복원 옵션

1. **Replace All**: 기존 데이터 삭제 후 복원
2. **Merge**: 기존 데이터 유지하며 병합
3. **Update Only**: 존재하는 항목만 업데이트

## 성능 최적화 팁

1. **크롤링 설정**:
   - 동시 실행 수를 시스템 사양에 맞게 조정
   - 네트워크 상태에 따라 타임아웃 설정 변경

2. **데이터베이스**:
   - 주기적으로 데이터베이스 최적화 실행
   - 백업 후 오래된 데이터 정리

3. **모니터링**:
   - 메모리 사용량 확인
   - 크롤링 실패율 모니터링

## 자주 묻는 질문 (FAQ)

### Q: 크롤링이 중간에 멈췄어요
A: `Pause` 버튼을 누른 후 `Resume`으로 재개하거나, Settings에서 재시도 횟수를 늘려보세요.

### Q: 데이터가 중복되어 있어요
A: Local DB 탭의 Diagnostics > "Check Duplicates" 기능으로 중복 확인 및 정리가 가능합니다.

### Q: Excel 백업이 실패해요
A: 파일이 다른 프로그램에서 열려있는지 확인하고, 충분한 디스크 공간이 있는지 확인하세요.

### Q: 크롤링 속도가 너무 느려요
A: Settings에서 동시 실행 수를 늘리거나, Shallow Sync 모드를 사용해보세요.

## 데이터 위치

### macOS
```
~/Library/Application Support/com.r-matter-certis/
```

### Windows
```
C:\Users\[username]\AppData\Roaming\com.r-matter-certis\
```

데이터베이스 파일: `matter_certis.db`

## 업데이트

새 버전이 출시되면:

1. [Releases 페이지](https://github.com/Chanseok/rMatterCertis/releases) 확인
2. 최신 버전 다운로드
3. 기존 앱 종료
4. 새 버전 설치 (데이터는 자동으로 유지됨)

**주의**: 중요한 데이터는 업데이트 전 Excel로 백업하는 것을 권장합니다.

## 문제 보고

버그나 기능 제안이 있으시면:

- [GitHub Issues](https://github.com/Chanseok/rMatterCertis/issues)에 등록
- 문제 재현 단계와 스크린샷 첨부
- 시스템 정보 (OS, 버전) 포함

## 라이선스

MIT License - 자세한 내용은 [LICENSE](LICENSE) 파일을 참조하세요.

## 기술 지원

- 문서: [GitHub Wiki](https://github.com/Chanseok/rMatterCertis/wiki)
- 이슈: [GitHub Issues](https://github.com/Chanseok/rMatterCertis/issues)
- 개발자: Chanseok <hi007chans@gmail.com>

---

**버전**: 0.8.0  
**마지막 업데이트**: 2025년 10월
