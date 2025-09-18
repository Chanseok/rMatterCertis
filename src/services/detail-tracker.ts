// Product Detail Tracking Module
// Provides stable counting based on keyed ProductDetailEvent stream.
// Non-invasive: can be adopted incrementally alongside legacy counting.

export type ProductDetailPhase =
  | 'scheduled'
  | 'fetch_started'
  | 'fetch_succeeded'
  | 'fetch_failed'
  | 'parse_started'
  | 'parse_succeeded'
  | 'parse_failed'
  | 'persist_started'
  | 'persist_succeeded'
  | 'persist_failed';

export interface ProductDetailEvent {
  key: string | number;            // stable product key (hash or slug-hash)
  attempt: number;                 // 1-based attempt number
  phase: ProductDetailPhase;
  ts?: number;                     // epoch ms
  page?: number;                   // optional source page for diagnostics
  meta?: Record<string, any>;      // optional miscellaneous metadata
  final?: boolean;                 // true at terminal persist_* phase
  error_code?: string;
}

interface ProductState {
  attempts: number;                // max attempt observed
  fetchSucceeded: boolean;
  fetchFailed: boolean;            // true if final fetch failed with no later success
  parseSucceeded: boolean;
  parseFailed: boolean;
  persistSucceeded: boolean;
  persistFailed: boolean;
  lastAttempt: number;
  page?: number;
  lastPhase?: ProductDetailPhase;
  lastTs?: number;
}

export interface DetailTrackerSnapshot {
  products: number;
  attempts: number;        // sum of attempts across products
  retries: number;         // attempts - products
  fetch: { started: number; succeeded: number; failed: number; };
  parse: { succeeded: number; failed: number; };
  persist: { succeeded: number; failed: number; };
  inferredStarted: number; // fetch.succeeded + fetch.failed (distinct products)
}

export class DetailTracker {
  private state = new Map<string | number, ProductState>();
  private eventsSeen = 0;

  ingest(e: ProductDetailEvent) {
    this.eventsSeen++;
    const key = e.key;
    if (e.attempt <= 0 || !Number.isFinite(e.attempt)) e.attempt = 1;
    let ps = this.state.get(key);
    if (!ps) {
      ps = {
        attempts: e.attempt,
        fetchSucceeded: false,
        fetchFailed: false,
        parseSucceeded: false,
        parseFailed: false,
        persistSucceeded: false,
        persistFailed: false,
        lastAttempt: e.attempt,
        page: e.page,
        lastPhase: e.phase,
        lastTs: e.ts,
      };
      this.state.set(key, ps);
    } else {
      if (e.attempt > ps.lastAttempt) {
        ps.lastAttempt = e.attempt;
        ps.attempts = e.attempt; // attempts always highest attempt number
      }
      if (e.page != null) ps.page = e.page;
      ps.lastPhase = e.phase;
      ps.lastTs = e.ts;
    }

    switch (e.phase) {
      case 'fetch_succeeded':
        ps.fetchSucceeded = true; ps.fetchFailed = false; break;
      case 'fetch_failed':
        if (!ps.fetchSucceeded) ps.fetchFailed = true; break;
      case 'parse_succeeded':
        ps.parseSucceeded = true; ps.parseFailed = false; break;
      case 'parse_failed':
        if (!ps.parseSucceeded) ps.parseFailed = true; break;
      case 'persist_succeeded':
        ps.persistSucceeded = true; ps.persistFailed = false; break;
      case 'persist_failed':
        if (!ps.persistSucceeded) ps.persistFailed = true; break;
    }
  }

  snapshot(): DetailTrackerSnapshot {
    let attempts = 0;
    let fetchStartedDistinct = 0; // products with any fetch phase
    let fetchSucceeded = 0;
    let fetchFailed = 0;
    let parseSucceeded = 0;
    let parseFailed = 0;
    let persistSucceeded = 0;
    let persistFailed = 0;

    for (const ps of this.state.values()) {
      attempts += ps.attempts;
      const sawFetch = ps.fetchSucceeded || ps.fetchFailed || (ps.lastPhase && ps.lastPhase.startsWith('fetch_'));
      if (sawFetch) fetchStartedDistinct++;
      if (ps.fetchSucceeded) fetchSucceeded++;
      else if (ps.fetchFailed) fetchFailed++;
      if (ps.parseSucceeded) parseSucceeded++;
      else if (ps.parseFailed) parseFailed++;
      if (ps.persistSucceeded) persistSucceeded++;
      else if (ps.persistFailed) persistFailed++;
    }

    const products = this.state.size;
    const retries = attempts - products;

    return {
      products,
      attempts,
      retries: retries < 0 ? 0 : retries,
      fetch: { started: fetchStartedDistinct, succeeded: fetchSucceeded, failed: fetchFailed },
      parse: { succeeded: parseSucceeded, failed: parseFailed },
      persist: { succeeded: persistSucceeded, failed: persistFailed },
      inferredStarted: fetchSucceeded + fetchFailed,
    };
  }

  reset() { this.state.clear(); this.eventsSeen = 0; }
  size() { return this.state.size; }
  getEventsSeen() { return this.eventsSeen; }
}

// Helper to derive a stable key from a URL (simple canonicalization). Real implementation might move to Rust backend.
export function canonicalUrlKey(raw: string): string {
  try {
    const u = new URL(raw);
    const host = u.host.toLowerCase();
    let path = u.pathname.replace(/\/+/g, '/');
    if (path.endsWith('/')) path = path.slice(0, -1);
    return host + path;
  } catch {
    return raw.trim();
  }
}
