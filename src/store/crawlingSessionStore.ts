import { createStore } from 'solid-js/store';

export type PageProcessingState =
  | 'Queued'
  | 'RequestSent'
  | 'AwaitingResponse'
  | 'ResponseReceived'
  | 'Processing'
  | 'Completed'
  | 'Failed'
  | 'Retrying';

interface PageState {
  pageNumber: number;
  status: PageProcessingState;
  timeoutSetting?: number;
  currentAttempt?: number;
  totalDurationMs?: number;
  requestSentAt?: string;
  responseReceivedAt?: string;
  productsFound?: number;
  error?: string | null;
}

interface ProductState {
  productId: string;
  productUrl: string;
  status: 'Queued' | 'RequestSent' | 'AwaitingResponse' | 'Processing' | 'Completed' | 'Failed' | 'Retrying';
  currentAttempt?: number;
  totalDurationMs?: number;
  error?: string | null;
}

interface BatchState {
  batchId: string;
  stage: string;
  progress: number;
  pages: Record<number, PageState>;
  products: Record<string, ProductState>;
  concurrency: {
    maxConcurrency: number;
    activeTasks: number;
    queuedTasks: number;
    completedTasks: number;
  };
}

interface CrawlingSessionStore {
  sessionId: string | null;
  status: 'idle' | 'running' | 'paused' | 'completed' | 'failed';
  totalBatches: number;
  currentBatch: number;
  overallProgress: number;
  estimatedTimeRemaining: number;
  elapsedTime: number;
  batches: Record<string, BatchState>;
  liveStats: {
    totalPages: number;
    processedPages: number;
    totalProducts: number;
    processedProducts: number;
    newItems: number;
    updatedItems: number;
    errors: number;
  };
}

const [state, setState] = createStore<CrawlingSessionStore>({
  sessionId: null,
  status: 'idle',
  totalBatches: 0,
  currentBatch: 0,
  overallProgress: 0,
  estimatedTimeRemaining: 0,
  elapsedTime: 0,
  batches: {},
  liveStats: {
    totalPages: 0,
    processedPages: 0,
    totalProducts: 0,
    processedProducts: 0,
    newItems: 0,
    updatedItems: 0,
    errors: 0,
  },
});

export const crawlingSessionStore = { state, setState };
