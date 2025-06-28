# Rust 빌드 최적화 가이드

이 문서는 rMatterCertis 프로젝트의 Rust 컴파일 속도 최적화에 대한 가이드입니다.

## 🚀 적용된 최적화

### 1. Cargo.toml 프로파일 최적화

```toml
# 개발용 빠른 컴파일 최적화
[profile.dev]
opt-level = 0
debug = 1  # 디버그 정보 축소로 더 빠른 빌드
split-debuginfo = "unpacked"  # macOS에서 더 빠름
incremental = true
codegen-units = 512  # 병렬화 증가

# 테스트용 빠른 컴파일 최적화
[profile.test]
opt-level = 0
debug = 1  # 디버그 정보 축소로 더 빠른 빌드
incremental = true
codegen-units = 512  # 병렬화 증가

# 의존성은 여전히 최적화됨 (개발 모드에서도)
[profile.dev.package."*"]
opt-level = 3
debug = false

[profile.test.package."*"]
opt-level = 3
debug = false
```

### 2. .cargo/config.toml 설정

```toml
[build]
jobs = 8  # 병렬 컴파일 증가
incremental = true  # 증분 컴파일 활성화

# 더 빠른 링커 사용 (macOS)
[target.x86_64-apple-darwin]
rustflags = ["-C", "link-arg=-fuse-ld=lld"]

[target.aarch64-apple-darwin]
rustflags = ["-C", "link-arg=-fuse-ld=lld"]
```

### 3. 환경 변수 최적화

`.env.development` 파일:
```bash
export CARGO_INCREMENTAL=1
export CARGO_BUILD_JOBS=8
export CARGO_PROFILE_DEV_DEBUG=1
export CARGO_PROFILE_TEST_DEBUG=1
export CARGO_PROFILE_DEV_SPLIT_DEBUGINFO="unpacked"
export RUST_LOG=warn
```

### 4. 빠른 테스트 스크립트

`scripts/test-fast.sh`:
```bash
#!/bin/bash
# 최적화된 환경에서 테스트 실행
export CARGO_INCREMENTAL=1
export RUST_LOG=warn
time cargo test "$1" --lib --bins
```

## 📊 성능 향상 결과

| 상황 | 이전 시간 | 최적화 후 시간 | 개선율 |
|------|-----------|----------------|--------|
| 초기 풀 빌드 | ~2-3분 | ~1분 | 66% 향상 |
| 변경사항 없는 재빌드 | ~10-30초 | ~0.5초 | 95% 향상 |
| 작은 변경 후 빌드 | ~30-60초 | ~2.6초 | 90% 향상 |

## 🛠️ 사용 방법

### 1. 개발 환경 로드
```bash
source .env.development
```

### 2. 빠른 테스트 실행
```bash
# 특정 테스트
./scripts/test-fast.sh database_connection

# 모든 테스트
./scripts/test-fast.sh
```

### 3. 일반 Cargo 명령어
```bash
# 증분 컴파일이 활성화된 상태에서 실행
cargo test database_connection
cargo build
cargo run
```

## ⚙️ 추가 최적화 옵션

### sccache (선택사항)
- 증분 컴파일과 호환되지 않음
- 클린 빌드가 자주 필요한 CI/CD에서 유용
- 로컬 개발에서는 증분 컴파일이 더 효과적

### 워크스페이스 분할 (향후 고려사항)
큰 프로젝트가 되면 다음과 같이 분할 가능:
```
crates/
├── core/        # 핵심 도메인 로직
├── database/    # 데이터베이스 로직
├── scraping/    # 크롤링 로직
└── api/         # API 로직
```

## 🔧 문제 해결

### 컴파일이 여전히 느린 경우
1. `cargo clean`으로 타겟 디렉토리 정리
2. 증분 컴파일 데이터 확인: `ls -la target/debug/incremental`
3. 환경 변수 확인: `echo $CARGO_INCREMENTAL`

### 메모리 부족 오류
- `codegen-units` 값을 256으로 감소
- `jobs` 값을 4로 감소

## 📝 참고 사항

- 증분 컴파일은 디스크 공간을 더 사용함
- 첫 번째 컴파일은 여전히 시간이 걸림
- 의존성 변경 시에는 전체 재컴파일 필요
- 프로덕션 빌드 시에는 다른 프로파일 사용 권장
