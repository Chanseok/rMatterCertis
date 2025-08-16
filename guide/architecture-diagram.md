# Actor-based Crawling Architecture (v2)

```mermaid
flowchart LR
  UI[Frontend UI]
  subgraph Tauri Backend
    CMD[Unified Command: start_unified_crawling]
    EM[EventEmitter]
    STATE[AppState]
    subgraph Actor System
      SA[SessionActor]
      BA[BatchActor]
      STA[StageActor]
    end
    DB[(Sqlite DB)]
    POOL[[Global Sqlite Pool (OnceLock)]]
    HTTP[HttpClient + GlobalRateLimiter]
  end

  UI -->|invoke| CMD
  CMD --> SA
  SA -->|sequential batches| BA
  BA -->|spawns & orchestrates| STA
  STA -->|CRUD| DB
  DB <--> POOL
  STA -->|requests| HTTP
  EM -->|events| UI
  STATE --> EM
  STATE --> POOL
```

- Single entrypoint command on the backend; legacy starts are archived.
- SessionActor decides batch count; batches execute sequentially. Inside each batch, StageActor tasks run concurrently under limits.
- Global Sqlite pool is initialized at startup and reused everywhere.
- Events are emitted synchronously and streamed to the UI.
