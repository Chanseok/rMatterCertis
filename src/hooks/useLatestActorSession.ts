import { createSignal, onMount, onCleanup } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';

interface SessionListResponse { sessions: string[] }
interface StatusPayload {
  session_id: string;
  status: string;
  pages: { processed: number; total: number; percent: number; failed: number; failed_rate: number };
  details?: { total: number; completed: number; failed: number; downshifted: boolean; downshift_meta: any };
  metrics?: { elapsed_ms: number; throughput_pages_per_min: number; eta_ms: number };
  resume_token?: string;
}

export function useLatestActorSession(pollMs = 2500) {
  const [sessionId, setSessionId] = createSignal<string | null>(null);
  const [status, setStatus] = createSignal<StatusPayload | null>(null);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  const [sessions, setSessions] = createSignal<string[]>([]);
  const [auto, setAuto] = createSignal(true);
  let timer: number | null = null;

  const fetchSessions = async () => {
    try {
      const resp = await invoke<{ sessions: string[] }>('list_actor_sessions');
      if (resp.sessions && resp.sessions.length > 0) {
        setSessions(resp.sessions);
        if (!sessionId() || !resp.sessions.includes(sessionId()!)) {
          setSessionId(resp.sessions[0]);
        }
      }
    } catch (e:any) {
      setError(e.toString());
    }
  };

  const fetchStatus = async () => {
    const sid = sessionId();
    if (!sid) return;
    try {
      const result = await invoke<any>('get_session_status', { session_id: sid });
      if (result?.data) {
        setStatus(result.data as StatusPayload);
        setError(null);
      }
    } catch (e:any) {
      setError(e.toString());
    } finally { setLoading(false); }
  };

  const tick = async () => {
    await fetchSessions();
    await fetchStatus();
  };

  onMount(async () => {
    await tick();
    // Periodic polling
    timer = setInterval(() => { if (auto()) { tick(); } }, pollMs) as unknown as number;

    // Listen for explicit refresh triggers from other UI parts (e.g. start buttons)
    const forceRefresh = (e: Event) => {
      const detail: any = (e as CustomEvent).detail;
      if (detail?.sessionId) {
        setSessionId(detail.sessionId);
      }
      // Immediate tick plus a slight delayed tick to catch freshly created session
      tick();
      setTimeout(tick, 600);
    };
    window.addEventListener('actorSessionRefresh', forceRefresh as any);
    onCleanup(() => window.removeEventListener('actorSessionRefresh', forceRefresh as any));
  });
  onCleanup(() => { if (timer) clearInterval(timer); });

  return { sessionId, status, setStatus, loading, error, refresh: tick, sessions, setSessionId, auto, setAuto };
}
