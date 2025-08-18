Generalized Actor Events (Backend → Frontend)

- When MC_FEATURE_EVENTS_GENERALIZED_ONLY=true (default), backend emits a single unified channel:
  - Event name: `actor-event`
  - Payload is enriched with `seq`, `backend_ts`, `event_name` (original), and flattened `variant` + fields of AppEvent.
- When the flag is false, backend keeps legacy behavior:
  - Emits per-variant event names (e.g., `actor-session-started`, `actor-progress`, ...), plus synthetic page lifecycle events where applicable.

Frontend migration tips:
- Subscribe to `actor-event` and branch on `variant` to handle all cases in one place.
- During transition, you can support both paths by wiring both `actor-event` and legacy names.
