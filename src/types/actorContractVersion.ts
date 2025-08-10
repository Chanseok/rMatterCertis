// Auto-synced Actor Contract Version (Rust <-> TS)
// DO NOT EDIT MANUALLY without updating `src-tauri/src/new_architecture/actors/contract.rs`
// ActorContractVersion: v1
export const ACTOR_CONTRACT_VERSION = 1 as const;

export function assertActorContractVersion(expected: number) {
  if (expected !== ACTOR_CONTRACT_VERSION) {
    throw new Error(`Actor contract version mismatch: expected ${expected} runtime ${ACTOR_CONTRACT_VERSION}`);
  }
}
