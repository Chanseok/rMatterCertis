# Rust 빌드 성능 최적화 가이드 (실제 검증된 방법)

## 🚀 개요

이 가이드는 rMatterCertis 프로젝트에서 실제로 적용하고 검증한 Rust 빌드 성능 최적화 방법들을 정리한 문서입니다.

## 📊 실제 성능 향상 결과

### Before vs After

| 시나리오 | 최적화 전 | 최적화 후 | 개선율 |
|----------|-----------|-----------|--------|
| 초기 풀 빌드 | 2-3분 | ~1분 | **66% 향상** |
| 변경사항 없는 재빌드 | 10-30초 | ~0.5초 | **95% 향상** |
| 작은 변경 후 빌드 | 30-60초 | ~2.6초 | **90% 향상** |

### 실제 측정 예시

```bash
# 최적화 전
$ time cargo test database_connection
# 약 30-60초 소요

# 최적화 후 (첫 빌드)
$ time cargo test database_connection
# real 1m0.65s, user 5m36.73s, sys 0m22.94s

# 최적화 후 (증분 빌드)  
$ time cargo test database_connection
# real 0m0.516s, user 0m0.21s, sys 0m0.15s
```

## ⚙️ 핵심 최적화 설정

### 1. Cargo.toml 프로파일 최적화

```toml
# src-tauri/Cargo.toml

[profile.dev]
opt-level = 0              # 최적화 없음 (빠른 컴파일)
debug = 1                  # 축소된 디버그 정보 (중요!)
split-debuginfo = "unpacked"  # macOS 최적화
incremental = true         # 증분 컴파일 활성화
codegen-units = 512        # 높은 병렬화 (기본값 256에서 증가)

[profile.test]
opt-level = 0
debug = 1                  # 테스트에서도 축소된 디버그 정보
incremental = true
codegen-units = 512

# 의존성은 여전히 최적화 유지 (한 번 빌드되면 재사용)
[profile.dev.package."*"]
opt-level = 3
debug = false

[profile.test.package."*"]
opt-level = 3
debug = false
```

**핵심 포인트:**
- `debug = 1`이 핵심! `debug = true` (기본값 2)에서 1로 변경하면 디버그 정보를 축소하여 빌드 속도가 크게 향상됩니다.
- `codegen-units`를 512로 증가시켜 병렬 컴파일을 극대화합니다.

### 2. .cargo/config.toml 설정

```toml
# .cargo/config.toml

[build]
jobs = 8                   # CPU 코어 수에 맞게 조정 (시스템에 따라)
incremental = true         # 전역 증분 컴파일

# macOS용 빠른 링커 (LLD 사용)
[target.x86_64-apple-darwin]
rustflags = ["-C", "link-arg=-fuse-ld=lld"]

[target.aarch64-apple-darwin]
rustflags = ["-C", "link-arg=-fuse-ld=lld"]

# 추가 최적화
[profile.dev]
debug = 1
split-debuginfo = "unpacked"

[profile.dev.package."*"]
opt-level = 3
debug = false

[profile.test.package."*"]
opt-level = 3
debug = false
```

**사전 요구사항:**
```bash
# LLD 링커 설치 (macOS)
brew install lld
```

### 3. 환경 변수 최적화

```bash
# .env.development
export CARGO_INCREMENTAL=1
export CARGO_TARGET_DIR="target"
export CARGO_BUILD_JOBS=8
export CARGO_PROFILE_DEV_DEBUG=1
export CARGO_PROFILE_TEST_DEBUG=1
export CARGO_PROFILE_DEV_SPLIT_DEBUGINFO="unpacked"
export RUST_LOG=warn  # 로그 축소

echo "🚀 Rust development environment optimized!"
```

### 4. .gitignore 최적화

```gitignore
# Rust 빌드 아티팩트
target/
*.db
*.db-shm
*.db-wal

# 캐시 디렉토리
.cargo/.package-cache
sccache/

# IDE 파일
.idea/
*.swp
*.swo
.vscode/

# macOS
.DS_Store
.AppleDouble
.LSOverride

# 환경 파일
.env.local
.env.production
```

## 🛠️ 추가 최적화 도구

### 1. sccache (선택사항)

> **주의:** sccache와 증분 컴파일은 충돌합니다. 둘 중 하나만 선택해야 합니다.

```bash
# 설치
brew install sccache

# .cargo/config.toml에 추가 (incremental = false로 설정 필요)
[build]
rustc-wrapper = "sccache"
incremental = false  # sccache 사용 시 필수
```

**언제 사용할까?**
- 증분 컴파일: 일상적인 개발에서 작은 변경사항을 자주 빌드할 때
- sccache: CI/CD나 클린 빌드가 자주 필요한 환경에서

### 2. 빠른 테스트 스크립트

```bash
#!/bin/bash
# scripts/test-fast.sh

set -e
cd "$(dirname "$0")/.."
cd src-tauri

export CARGO_INCREMENTAL=1
export RUST_LOG=warn

echo "🚀 Running fast Rust tests..."

if [ -n "$1" ]; then
    echo "🔍 Running specific test: $1"
    time cargo test "$1" --lib --bins
else
    echo "🧪 Running all tests"
    time cargo test --lib --bins
fi

echo "✅ Tests completed!"
```

```bash
chmod +x scripts/test-fast.sh
```

## 🎯 사용법

### 개발 환경 로드

```bash
# 환경 변수 로드
source .env.development

# 특정 테스트 실행
./scripts/test-fast.sh database_connection

# 모든 테스트 실행
./scripts/test-fast.sh

# 일반 cargo 명령어도 빨라짐
cargo test
cargo build
```

### 성능 모니터링

```bash
# 빌드 시간 측정
time cargo test database_connection

# 증분 컴파일 상태 확인
cargo build --verbose

# 병렬 작업 확인 (활동 모니터에서 cargo 프로세스 확인)
```

## 🔧 트러블슈팅

### 문제 1: sccache 오류

```bash
# 오류: "sccache: increment compilation is prohibited"
# 해결: .cargo/config.toml에서 sccache 비활성화
# rustc-wrapper = "sccache"  # 주석 처리
```

### 문제 2: 링커 오류

```bash
# LLD 링커가 없는 경우
brew install lld

# 또는 .cargo/config.toml에서 링커 설정 제거
# rustflags = ["-C", "link-arg=-fuse-ld=lld"]  # 주석 처리
```

### 문제 3: 여전히 느린 빌드

```bash
# 캐시 정리
cargo clean

# 환경 변수 확인
echo $CARGO_INCREMENTAL
echo $CARGO_BUILD_JOBS

# .cargo 디렉토리 권한 확인
ls -la .cargo/
```

## 📈 추가 최적화 팁

### 1. 의존성 최적화

- 불필요한 features 제거
- 선택적 의존성 사용
- 가벼운 대안 라이브러리 고려

### 2. 코드 구조 최적화

- 모듈 분리로 증분 컴파일 효율성 증대
- 거대한 단일 파일 분할
- 테스트 코드 분리

### 3. 하드웨어 최적화

- SSD 사용 (HDD 대비 빌드 속도 향상)
- 충분한 RAM (8GB 이상 권장)
- 멀티코어 CPU 활용

## 🎉 결론

위 설정들을 모두 적용하면 **일상적인 개발에서 90% 이상의 빌드 시간 단축**을 경험할 수 있습니다. 

특히 `debug = 1` 설정과 증분 컴파일의 조합이 가장 큰 효과를 보였으며, 이를 통해 개발 생산성이 크게 향상되었습니다.
