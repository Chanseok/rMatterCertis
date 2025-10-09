// Frontend listener for structured crawl events (schema v1)
// Responsibilities:
// 1. Subscribe to Tauri 'crawl_updates' channel
// 2. Basic runtime validation & version check
// 3. Sequence gap detection
// 4. Fan-out to registered subscribers (observer pattern)
// 5. Provide simple singleton accessor
//
// NOTE: Keep minimal; advanced buffering/replay will move to a dedicated module later.

import { listen } from '@tauri-apps/api/event';
import { CrawlEvent, CRAWL_EVENT_SCHEMA_VERSION, isCrawlEvent } from '../types/crawlEvents';

export interface CrawlUpdatesListenerOptions {
  strictVersion?: boolean; // if true, drop events with mismatched schema_version
  gapWarningThreshold?: number; // how many gaps before escalating (unused yet)
  onGapDetected?: (expected: number, got: number) => void;
  onVersionMismatch?: (ev: any) => void;
  onInvalidPayload?: (raw: unknown) => void;
}

export type CrawlEventSubscriber = (ev: CrawlEvent) => void;

class CrawlUpdatesListener {
  private started = false;
  private lastSequence: number | null = null;
  private subs: Set<CrawlEventSubscriber> = new Set();
  private opts: CrawlUpdatesListenerOptions;
  private unlisten: (() => void) | null = null;

  constructor(opts?: CrawlUpdatesListenerOptions) {
    this.opts = { strictVersion: false, gapWarningThreshold: 1, ...opts };
  }

  subscribe(fn: CrawlEventSubscriber): () => void {
    this.subs.add(fn);
    return () => this.subs.delete(fn);
  }

  getLastSequence() {
    return this.lastSequence;
  }

  async start() {
    if (this.started) return;
    this.started = true;
    this.unlisten = await listen('crawl_updates', (evt) => {
      const payload = (evt as any).payload;
      if (!isCrawlEvent(payload)) {
        this.opts.onInvalidPayload?.(payload);
        return;
      }
      // Version check
      if (payload.schema_version !== CRAWL_EVENT_SCHEMA_VERSION) {
        if (this.opts.strictVersion) {
          this.opts.onVersionMismatch?.(payload);
          return;
        } else {
          // still forward but notify
          this.opts.onVersionMismatch?.(payload);
        }
      }
      // Sequence gap detection
      if (this.lastSequence !== null) {
        const expected = this.lastSequence + 1;
        if (payload.sequence !== expected) {
          this.opts.onGapDetected?.(expected, payload.sequence);
        }
      }
      this.lastSequence = payload.sequence;
      // Fan out
      for (const fn of this.subs) {
        try { fn(payload); } catch (e) { /* swallow individual subscriber errors */ }
      }
    });
  }

  async stop() {
    if (this.unlisten) {
      this.unlisten();
      this.unlisten = null;
    }
    this.started = false;
    this.lastSequence = null;
  }
}

// Singleton pattern (lazy)
let _instance: CrawlUpdatesListener | null = null;
export function getCrawlUpdatesListener(opts?: CrawlUpdatesListenerOptions) {
  if (!_instance) {
    _instance = new CrawlUpdatesListener(opts);
  }
  return _instance;
}

// ensureCrawlUpdatesRunning removed (unused)
