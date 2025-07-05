# Workspace 분리 전략 가이드

## 🎯 분리 기준점

### 현재 상황 (권장: 분리하지 않음)
- **코드량**: ~5,000줄
- **빌드 시간**: 47초 (sccache 적용)
- **의존성**: 레이어 간 강한 결합

### 분리 고려 시점
- **코드량**: 10,000줄 이상
- **빌드 시간**: 2분 이상 (캐시 없이)
- **팀 규모**: 3명 이상 개발자

## 🏗️ 미래 Workspace 구조 (단계별)

### Phase 1: Core 분리
```toml
[workspace]
members = [
    "crates/matter-core",      # Domain + Application
    "crates/matter-infra",     # Infrastructure
    "crates/matter-cli",       # Tauri App
]
```

### Phase 2: 세분화
```toml
[workspace]
members = [
    "crates/matter-domain",    # 순수 도메인 로직
    "crates/matter-app",       # Use Cases
    "crates/matter-db",        # Database 관련
    "crates/matter-http",      # HTTP 클라이언트
    "crates/matter-parser",    # HTML 파싱
    "crates/matter-tauri",     # Tauri 앱
]
```

## 📊 분리 이점 vs 비용

### 이점
- **빌드 시간**: 변경된 크레이트만 재컴파일
- **모듈화**: 명확한 책임 분리
- **재사용성**: 개별 크레이트 독립 사용 가능
- **병렬 빌드**: 크레이트 간 병렬 컴파일

### 비용
- **설정 복잡성**: Cargo.toml 관리 증가
- **의존성 관리**: 버전 동기화 필요
- **개발 속도**: 크레이트 간 변경 시 복잡성
- **디버깅**: 크레이트 경계에서 디버깅 어려움

## 🚀 현재 최적화 우선순위

### 1순위: sccache 최적화 (완료)
- [x] 빌드 시간 62% 단축 (2분 8초 → 47초)
- [x] 100% 캐시 히트율 달성

### 2순위: 코드 품질 향상
- [ ] 테스트 커버리지 증가
- [ ] 문서화 완성
- [ ] CI/CD 파이프라인 최적화

### 3순위: 기능 확장
- [ ] 멀티 사이트 지원
- [ ] 실시간 모니터링
- [ ] 스케줄링 기능

## 🎯 분리 시점 판단 기준

### 정량적 기준
```bash
# 코드 라인 수 확인
find src-tauri/src -name "*.rs" -exec wc -l {} + | tail -1

# 빌드 시간 측정 (캐시 없이)
cargo clean && time cargo build

# 의존성 복잡도 확인
cargo tree --depth 3
```

### 정성적 기준
- 단일 기능 수정 시 여러 모듈 변경 필요
- 팀원 간 코드 충돌 빈발
- 특정 기능만 재사용하고 싶은 요구사항

## 💡 권장사항

### 현재 (2025년 7월)
**단일 크레이트 유지**
- 현재 빌드 시간 충분히 빠름
- 개발 생산성 최우선
- 추가 최적화보다 기능 개발 집중

### 미래 (코드량 2배 증가 시)
**선택적 분리 검토**
- 빌드 시간이 실제 문제가 될 때
- 팀 규모 확장 시
- 외부 라이브러리로 공개할 계획이 있을 때

---

> **결론**: 현재는 workspace 분리보다 sccache 최적화로 충분한 성능을 달성했습니다. 
> 프로젝트가 더 성장했을 때 재검토하는 것이 바람직합니다.
