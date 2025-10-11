# rMatterCertis v0.8.1 Release Notes

## 🎉 첫 공식 릴리즈!

rMatterCertis v0.8.1은 CSA (Connectivity Standards Alliance) 웹사이트에서 Matter 인증 제품 정보를 자동으로 수집하고 관리하는 데스크톱 애플리케이션입니다.

---

## 🌟 주요 기능

### 🔄 자동 크롤링
- **대상 사이트**: [CSA-IOT Products](https://csa-iot.org/csa-iot_products/) (Matter 인증 제품)
- **증분 크롤링**: 신규/업데이트된 제품만 자동 감지하여 수집
- **실시간 모니터링**: 크롤링 진행 상황을 단계별로 시각화
  - Stage 1: List Page 크롤링 (제품 목록 수집)
  - Stage 2: Product Detail 크롤링 (상세 정보 수집)

### 📊 로컬 데이터베이스 관리
- **SQLite 기반**: 모든 데이터를 로컬에 안전하게 저장
- **고급 필터링**: 카테고리, 제조사, Device Type, 전송 인터페이스별 필터
- **통계 대시보드**: 
  - 전체/고유 제품 수
  - 카테고리별/제조사별/Device Type별 분포
- **인증 타임라인**: 연도별/월별 인증 추이 차트

### 💾 Excel 백업/복원
- **백업**: 모든 제품 데이터를 Excel (.xlsx) 파일로 내보내기
- **복원**: Excel 파일에서 데이터 가져오기 (덮어쓰기/병합 옵션)
- **데이터 포함**: 제품 정보, 디바이스 타입, 인증 날짜, URL 등 모든 메타데이터

### 🎯 Device Type 편집기
- Matter Device Type 정보 직접 수정
- ID, 이름, 카테고리 편집
- 새 Device Type 추가/삭제

---

## 📥 설치 방법

### macOS

**시스템 요구사항:**
- macOS 10.15 (Catalina) 이상
- Apple Silicon (M1/M2/M3/M4) 또는 Intel 프로세서
- 최소 4GB RAM 권장

**설치 파일:**
- `rMatterCertis_0.8.1_universial.dmg`


**설치 절차:**
1. DMG 파일 다운로드
2. DMG 파일을 더블클릭하여 마운트
3. rMatterCertis 아이콘을 Applications 폴더로 드래그
4. Applications에서 실행

**⚠️ 보안 주의사항 (중요!)**

이 앱은 아직 Apple 코드 서명이 되어 있지 않아 macOS Gatekeeper가 실행을 차단합니다.

**증상:**
- "rMatterCertis을(를) 열지 않음" 다이얼로그 표시
- "손상을 줄 수 있고, 악성 코드가 없음을 확인할 수 없습니다" 메시지
- 선택 옵션: "완료" 또는 "휴지통으로 이동"만 표시

**해결 방법 (시스템 설정 사용 - 권장):**

1. 다이얼로그에서 **"완료"** 클릭
2. `시스템 설정` 앱 열기
3. `개인정보 보호 및 보안` → `보안` 섹션으로 이동
4. 아래쪽에 "rMatterCertis이(가) 차단되었습니다" 메시지 표시
5. **"그래도 열기"** 버튼 클릭
6. 관리자 암호 입력
7. 확인 다이얼로그에서 **"열기"** 클릭
8. 이제 rMatterCertis가 실행됩니다!

> **⚠️ 참고**: macOS Sequoia (15.x)에서는 우클릭 → "열기" 방법이나 `xattr` 터미널 명령이 작동하지 않을 수 있습니다. 시스템 설정을 통한 방법을 사용하세요.

> **📌 참고**: 이 앱은 오픈소스이며, 소스 코드를 직접 확인하실 수 있습니다.  
> GitHub에서 코드를 검토하신 후 로컬에서 직접 빌드하실 수도 있습니다.


---

## 🎮 빠른 시작 가이드

### 1. 첫 크롤링 실행

1. **Crawling 탭** 열기
2. `크롤링 시작` 버튼 클릭
3. 진행 상황 모니터링:
   - List Page Progress: 페이지별 진행 상태 (색상으로 구분)
   - Product Detail Progress: 상세 페이지 수집 진행률
   - 예상 완료 시간 확인

> **💡 팁**: 첫 크롤링은 전체 데이터를 수집하므로 30분~1시간 소요됩니다.  
> 두 번째 실행부터는 신규/변경 제품만 수집하여 5~10분 내 완료됩니다.

### 2. 데이터 조회 및 분석

1. **로컬DB 탭** 열기
2. 요약 통계에서 전체 데이터 확인
3. 필터 적용:
   - 카테고리/제조사/Device Type 버튼 클릭
   - 원하는 항목 선택 (다중 선택 가능)
4. 타임라인 차트로 인증 추이 분석

### 3. 백업 생성

1. **로컬DB 탭**에서 `Backup to Excel` 클릭
2. 저장 위치 선택
3. Excel 파일 생성 완료

---

## 🗂️ 데이터 저장 위치

### macOS
```
~/Library/Application Support/matter-certis-v2/
├── database/
│   └── matter_certis.db      # 메인 데이터베이스
├── logs/
│   └── matter-certis-v2.log  # 애플리케이션 로그
└── config/                    # 설정 파일
```

**실제 경로:**
```
/Users/[사용자명]/Library/Application Support/matter-certis-v2/database/matter_certis.db
```

---

## 🔧 기술 스택

### Frontend
- **SolidJS**: 반응형 UI 프레임워크
- **TypeScript**: 타입 안전성
- **TailwindCSS**: 스타일링

### Backend
- **Rust**: 고성능 크롤링 엔진
- **Tauri v2.6**: 크로스 플랫폼 데스크톱 프레임워크
- **SQLite**: 로컬 데이터베이스
- **Tokio**: 비동기 런타임

### CI/CD
- **GitHub Actions**: 자동 빌드 파이프라인
- **2단계 검증**: Preflight (2-3분) + Release Build (8-10분)

---

## 📝 알려진 이슈

## 📝 알려진 이슈

### macOS Gatekeeper 보안 경고 ⚠️

**증상:**
- 앱 설치 후 실행 시 "rMatterCertis을(를) 열지 않음" 다이얼로그
- "손상을 줄 수 있고, 악성 코드가 없음을 확인할 수 없습니다" 경고
- macOS Sequoia (15.x)를 포함한 모든 최신 macOS 버전에서 발생

**원인:**
- 앱이 Apple 개발자 계정으로 코드 서명되지 않음
- macOS Gatekeeper가 서명되지 않은 앱 실행을 차단

**해결 (시스템 설정 사용):**
1. 경고 다이얼로그에서 **"완료"** 클릭
2. `시스템 설정` → `개인정보 보호 및 보안` → `보안` 섹션
3. 아래쪽에 "rMatterCertis이(가) 차단되었습니다" 메시지 확인
4. **"확인 없이 열기"** 버튼 클릭
5. 관리자 암호 입력 후 확인
6. 이제 rMatterCertis가 정상 실행됩니다

> **💡 참고**: macOS Sequoia에서는 우클릭 → "열기" 또는 `xattr` 명령이 작동하지 않습니다.

### Windows SmartScreen 경고

**증상:**
- Windows Defender SmartScreen 경고 표시

**해결:**
- "추가 정보" 클릭 → "실행" 선택

---

## 🔮 향후 계획

- **코드 서명**: Apple/Microsoft 개발자 인증서를 통한 앱 서명 (Gatekeeper/SmartScreen 경고 해결)
- **자동 업데이트**: 앱 내에서 새 버전 자동 감지 및 업데이트
- **추가 통계**: 더 상세한 데이터 분석 및 시각화


---

## 🔄 업데이트 안내

새 버전 출시 시:
1. [Releases 페이지](https://github.com/Chanseok/rMatterCertis/releases) 확인
2. 중요 데이터를 Excel로 백업 (권장)
3. 기존 앱 종료 후 새 버전 설치
4. 실행하면 기존 데이터베이스 자동 인식

---

## 📚 상세 문서

- **사용자 가이드**: [USER_GUIDE.md](https://github.com/Chanseok/rMatterCertis/blob/main/USER_GUIDE.md)
- **GitHub Repository**: [Chanseok/rMatterCertis](https://github.com/Chanseok/rMatterCertis)
- **이슈 리포트**: [GitHub Issues](https://github.com/Chanseok/rMatterCertis/issues)

---

## 📄 라이선스

MIT License - 자유롭게 사용, 수정, 배포 가능합니다.

---

## 🙏 감사의 말

Matter 생태계 발전에 기여하는 CSA와 모든 제조사에게 감사드립니다.

**rMatterCertis v0.8.1을 사용해주셔서 감사합니다!** 🎉
