# rMatterCertis 사용자 가이드

## 📖 소개

rMatterCertis는 [CSA (Connectivity Standards Alliance)](https://csa-iot.org) 웹사이트에서 Matter 인증 제품 정보를 자동으로 수집하고 관리하는 데스크톱 애플리케이션입니다.

**크롤링 대상 페이지:**
- Matter 제품 목록: https://csa-iot.org/csa-iot_products/ (Matter 필터 적용)
- 각 제품별 상세 페이지에서 인증 정보, 디바이스 타입, 제조사 정보 등 추출

## ✨ 주요 기능

- **🔄 증분 크롤링**: 신규/업데이트된 제품만 자동 감지하여 수집
- **📊 로컬 데이터베이스**: SQLite 기반으로 모든 데이터를 로컬에 저장
- **📈 실시간 모니터링**: 크롤링 진행 상황을 단계별로 시각화
- **💾 Excel 백업/복원**: 데이터를 Excel 파일로 내보내기 및 가져오기
- **🎯 Device Type 편집기**: Matter device type 정보 직접 수정
- **📅 인증 타임라인**: 연도별/월별 인증 추이 차트
- **🔍 고급 필터링**: 카테고리, 제조사, 디바이스 타입별 필터링

## 💻 시스템 요구사항

### macOS
- **OS**: macOS 10.15 (Catalina) 이상
- **프로세서**: Apple Silicon (M1/M2/M3/M4) 또는 Intel
- **메모리**: 최소 4GB RAM 권장
- **디스크**: 200MB 이상 여유 공간

### Windows (아직 지원되지 않습니다)
- **OS**: Windows 10 (64-bit) 이상
- **메모리**: 최소 4GB RAM 권장  
- **디스크**: 200MB 이상 여유 공간

## 📥 설치 방법

### macOS 설치

1. **다운로드**
   - [GitHub Releases](https://github.com/Chanseok/rMatterCertis/releases) 페이지 방문
   - 최신 버전에서 시스템에 맞는 파일 다운로드:
     - **Apple Silicon (M1/M2/M3/M4)**: `rMatterCertis_[버전]_aarch64.dmg`
     - **Intel Mac**: `rMatterCertis_[버전]_x64.dmg`

2. **설치**
   - 다운로드한 DMG 파일을 더블클릭
   - rMatterCertis 아이콘을 Applications 폴더로 드래그 앤 드롭

3. **실행**
   - Finder에서 Applications > rMatterCertis 실행
   
4. **보안 설정** (최초 실행 시)
   - "개발자를 확인할 수 없음" 경고 발생 시:
     1. `시스템 환경설정` → `보안 및 개인정보 보호` → `일반` 탭
     2. "확인 없이 열기" 버튼 클릭
     3. 다시 rMatterCertis 실행

### Windows 설치

1. **다운로드**
   - [GitHub Releases](https://github.com/Chanseok/rMatterCertis/releases) 페이지 방문
   - `rMatterCertis_[버전]_x64_en-US.msi` 다운로드

2. **설치**
   - MSI 파일 더블클릭하여 설치 마법사 실행
   - 지시에 따라 설치 완료

3. **실행**
   - 시작 메뉴 또는 바탕화면 바로가기에서 실행

## 🎮 사용 방법

### 화면 구성

애플리케이션은 3개의 주요 탭으로 구성됩니다:

1. **🔬 Crawling 탭**: 크롤링 실행 및 모니터링
2. **⚙️ 설정 탭**: 크롤링 설정 및 시스템 구성
3. **🗄️ 로컬DB 탭**: 데이터 조회, 분석, 백업/복원

---

### 1️⃣ Crawling 탭 - 크롤링 실행

#### 크롤링 시작

1. **Crawling 탭** 클릭
2. 세션 상태 카드에서 현재 상태 확인
3. **컨트롤 패널**에서 `크롤링 시작` 버튼 클릭

#### 크롤링 진행 모니터링

크롤링이 시작되면 실시간으로 진행 상황을 확인할 수 있습니다:

**📊 Stage 통계 패널:**
- **Stage 1 (List Page)**: 제품 목록 페이지 크롤링 진행률
  - 시작/완료/실패/재시도 건수
  - 진행률 프로그레스 바
- **Stage 2 (Product Detail)**: 개별 제품 상세 정보 수집
  - 상세 페이지 크롤링 통계
  - 성공/실패/재시도 카운트

**⏱️ 예상 시간:**
- 남은 시간 추정치 (List Page / Detail 단계별)
- 전체 완료 예상 시간

**🎯 진행률 패널:**
- **List Page Progress**: 페이지별 진행 상태 (색상으로 구분)
  - 🔵 초록색: 완료
  - 🟡 노란색: 진행 중
  - 🔴 빨간색: 실패
- **Complement Crawl Progress**: 보완 크롤링 진행 (필요시)

#### 크롤링 중지

- `중지` 버튼 클릭
- 진행 중인 작업은 안전하게 종료되며, 수집된 데이터는 저장됩니다

---

### 2️⃣ 설정 탭 - 크롤링 구성

#### 기본 설정

**크롤링 범위:**
- 크롤링할 페이지 범위는 자동으로 감지됩니다
- 오래된 페이지부터 역순으로 크롤링 (Oldest First 전략)

**동시 실행 수:**
- 기본값: 시스템에 최적화된 값 자동 설정
- 수동 조정 가능 (너무 높으면 네트워크 부하 발생 가능)

**재시도 설정:**
- 실패한 요청 자동 재시도
- 최대 재시도 횟수 설정

---

### 3️⃣ 로컬DB 탭 - 데이터 관리

#### 데이터 조회

**요약 통계:**
- 전체 제품 수
- 고유 제품 수 (중복 제거)
- 카테고리별 분포
- 제조사별 분포
- Device Type별 분포

**인증 타임라인:**
- 연도별/월별 인증 제품 추이 차트
- 날짜 범위 슬라이더로 기간 필터링

#### 고급 필터링

**필터 적용:**
1. 카테고리/제조사/Device Type 버튼 클릭
2. 원하는 항목 선택 (다중 선택 가능)
3. 적용하면 모든 통계가 필터된 데이터로 업데이트

**날짜 범위 필터:**
- 타임라인 하단의 슬라이더로 인증 날짜 범위 조정
- 시작일/종료일 직접 입력 가능

#### Excel 백업

**백업 생성:**
1. `Backup to Excel` 버튼 클릭
2. 저장 위치 선택
3. Excel 파일 (.xlsx) 생성 완료

**백업 포함 내용:**
- 모든 제품 정보 (ID, 이름, 제조사, 모델, Matter 버전 등)
- 디바이스 타입 정보
- 인증 날짜, URL 등 메타데이터

#### Excel 복원

**복원 방법:**
1. `Restore from Excel` 버튼 클릭
2. 이전에 백업한 Excel 파일 선택
3. 복원 옵션 선택:
   - **덮어쓰기**: 기존 데이터 삭제 후 복원
   - **병합**: 기존 데이터 유지하면서 새 데이터 추가
4. 복원 완료

**⚠️ 주의:**
- 덮어쓰기는 모든 기존 데이터를 삭제합니다
- 중요한 데이터는 복원 전 백업 권장

#### Device Type 편집기

1. `Device Type Editor` 버튼 클릭
2. Matter Device Type 목록 확인
3. 항목 클릭하여 수정:
   - ID, 이름, 카테고리 편집
   - 새 Device Type 추가
   - 기존 항목 삭제
4. `저장` 버튼으로 변경사항 저장

---

## 💡 활용 팁

### 크롤링 최적화

**첫 크롤링 (전체 수집):**
- 처음 실행 시 모든 페이지를 크롤링하므로 시간이 오래 걸립니다 (30분~1시간)
- 안정적인 네트워크 환경에서 실행 권장
- 크롤링 중 중지하더라도 수집된 데이터는 저장됩니다

**증분 크롤링 (업데이트):**
- 두 번째 실행부터는 신규/변경된 제품만 감지하여 수집
- 짧은 시간 내 완료 (보통 5~10분)
- 주기적으로 실행하여 최신 데이터 유지

### 데이터 분석

**필터 조합:**
- 여러 필터를 동시에 적용하여 특정 조건의 제품 분석 가능
- 예: "특정 제조사의 특정 카테고리 제품"

**타임라인 활용:**
- 인증 추이를 통해 시장 트렌드 파악
- 특정 기간의 신규 인증 제품 집중 분석

**Excel 데이터 활용:**
- 백업한 Excel 파일로 추가 분석 가능
- 피벗 테이블, 차트 등 Excel 기능 활용

### 백업 전략

**정기 백업:**
- 중요한 크롤링 후 즉시 백업 권장
- 주기적인 백업으로 데이터 손실 방지

**버전 관리:**
- 백업 파일명에 날짜 포함 (예: `matter_products_2025-01-11.xlsx`)
- 여러 시점의 백업 유지로 시계열 분석 가능

---

## 🗂️ 데이터 저장 위치

### macOS
```
~/Library/Application Support/matter-certis-v2/
├── database/
│   └── matter_certis.db      # 메인 데이터베이스
├── logs/
│   └── matter-certis-v2.log  # 애플리케이션 로그
└── config/                    # 설정 파일 (자동 생성)
```

**실제 경로 예시:**
```
/Users/[사용자명]/Library/Application Support/matter-certis-v2/database/matter_certis.db
```

### Windows
```
C:\Users\[사용자명]\AppData\Local\matter-certis-v2\
├── database\
│   └── matter_certis.db      # 메인 데이터베이스
├── logs\
│   └── matter-certis-v2.log  # 애플리케이션 로그
└── config\                    # 설정 파일 (자동 생성)
```

**실제 경로 예시:**
```
C:\Users\[사용자명]\AppData\Local\matter-certis-v2\database\matter_certis.db
```

---

## ❓ 자주 묻는 질문 (FAQ)

**Q: 크롤링이 실패하면 어떻게 하나요?**
- A: 네트워크 연결 확인 후 재시도하면 됩니다. 실패한 페이지는 자동으로 재시도됩니다.

**Q: 데이터베이스가 손상되면?**
- A: Excel 백업에서 복원하거나, 데이터베이스 파일을 삭제하고 처음부터 크롤링하세요.

**Q: 크롤링 중 앱을 종료하면?**
- A: 수집된 데이터는 저장됩니다. 다음 실행 시 중단된 시점부터 재개됩니다.

**Q: 크롤링 속도가 느려요.**
- A: 네트워크 상태 확인 후, 설정에서 동시 실행 수를 조정해보세요.

**Q: Excel 백업 파일이 안 열려요.**
- A: Excel 2016 이상 버전이 필요합니다. 없으면 Google Sheets에서 열 수 있습니다.

---

## 🔄 업데이트

### 새 버전 확인

1. [GitHub Releases](https://github.com/Chanseok/rMatterCertis/releases) 페이지 방문
2. 최신 버전 확인

### 업데이트 방법

1. **중요 데이터 백업** (Excel로)
2. 기존 앱 종료
3. 새 버전 다운로드 및 설치
4. 실행하면 기존 데이터베이스 자동 인식

**⚠️ 주의:** 데이터베이스는 자동으로 유지되지만, 만약을 위해 업데이트 전 Excel 백업을 권장합니다.

---

## 📞 문제 해결 및 지원

### 로그 확인

문제 발생 시 로그 파일을 확인하세요:
- **macOS**: `~/Library/Application Support/matter-certis-v2/logs/matter-certis-v2.log`
- **Windows**: `C:\Users\[사용자명]\AppData\Local\matter-certis-v2\logs\matter-certis-v2.log`

### 문의

- **GitHub Issues**: https://github.com/Chanseok/rMatterCertis/issues
- 버그 리포트, 기능 제안 환영합니다!

---

## 📄 라이선스

MIT License - 자유롭게 사용, 수정, 배포 가능합니다.

---

**🎉 rMatterCertis를 사용해주셔서 감사합니다!**

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

**버전**: 0.8.x  
**마지막 업데이트**: 2025년 10월
