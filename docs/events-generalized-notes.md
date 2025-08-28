Generalized Actor Events (Backend → Frontend)

- When `MC_FEATURE_EVENTS_GENERALIZED_ONLY=true`, backend emits a single unified channel:
  - Event name: `actor-event`
  - Payload is enriched with `seq`, `backend_ts`, `event_name` (original), and flattened `variant` + fields of AppEvent.
- When the flag is false (default), backend keeps legacy behavior for backward compatibility:
  - Emits per-variant event names (e.g., `actor-session-started`, `actor-progress`, ...), plus synthetic page lifecycle events where applicable.

Frontend migration tips:
- Subscribe to `actor-event` and branch on `variant` to handle all cases in one place.
- During transition, you can support both paths by wiring both `actor-event` and legacy names.

## Frontend wiring status (be-rearch)

- `src/services/tauri-api.ts`: Now subscribes to the unified `actor-event` when available and maps `payload.event_name` back to the legacy name when invoking the provided callback. Legacy per-name listeners are still registered for safety.
- `src/dev/eventAudit.ts`: Includes `actor-event` in the in-memory audit list so the unified stream is visible in the dev audit tool.

Verification notes:
- Type-check passed (tsc --noEmit) and the app build completed locally via `vite build` without errors.
- Backend flag default keeps generalized-only OFF, so stage-specific event names will be emitted by default. The unified channel remains available when explicitly enabled via environment.

Next steps (safe cleanup):
- Gradually prune redundant legacy listeners once confidence is high and all consumers handle the unified shape.
- Consider centralizing handling by `variant` for clearer UI flows and easier future additions.
