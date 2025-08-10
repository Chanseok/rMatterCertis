//! Actor Contract Version constant
//!
//! 중앙에서 단일 버전 값만 변경하도록 하여 Rust <-> TypeScript 간 동기화를 단순화.
//! 규칙:
//! - 증가 시(additive 변경만 허용) 기존 필드/이벤트 제거 금지
//! - Deprecated 단계(문서 + 주석) 없이 변경 금지
//! - TS 파일(`src/types/actorContractVersion.ts`)과 값 동기화 필요
pub const ACTOR_CONTRACT_VERSION: u32 = 1;
