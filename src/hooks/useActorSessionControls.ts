// Tauri v2: invoke 는 @tauri-apps/api/core 경로 사용
import { invoke } from '@tauri-apps/api/core';
import { ACTOR_CONTRACT_VERSION, assertActorContractVersion } from '../types/actorContractVersion';

export interface SessionControlResult<T=unknown> {
  ok: boolean;
  message: string;
  data?: T;
  sessionId?: string;
}

async function call<T>(cmd: string, args: Record<string, unknown>): Promise<SessionControlResult<T>> {
  try {
    const resp: any = await invoke(cmd, args);
    return {
      ok: !!resp.success,
      message: resp.message ?? '',
      data: resp.data as T | undefined,
      sessionId: resp.session_id as string | undefined,
    };
  } catch (e: any) {
    return { ok: false, message: e?.toString?.() || 'unknown error' };
  }
}

export function useActorSessionControls() {
  // Version assertion (throws if mismatch)
  assertActorContractVersion(ACTOR_CONTRACT_VERSION);
  return {
  pause: (sessionId: string) => call('pause_session', { session_id: sessionId }),
  resume: (sessionId: string) => call('resume_session', { session_id: sessionId }),
  status: (sessionId: string) => call('get_session_status', { session_id: sessionId }),
    shutdown: () => call('request_graceful_shutdown', {}),
  };
}
