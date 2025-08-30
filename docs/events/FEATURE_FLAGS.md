# Event-related Feature Flags

Source of truth: environment variables. Values: "1"/"true" enable, "0"/"false" disable (case-insensitive).

- MC_FEATURE_EVENTS_GENERALIZED_ONLY (default: false)
  - Use single unified frontend event channel ("actor-event") and generalized routing.
- MC_FEATURE_LEGACY_DOMAIN_EVENTS (default: false)
  - Emit legacy domain::events::CrawlingEvent payloads via SystemStateBroadcaster.
  - Set to 0 to silence legacy emissions after FE migrates to AppEvent.

Removed flags
- MC_FEATURE_EMIT_PAGETASK_LEGACY (removed)
  - The PageTask* variants were deleted from AppEvent; UI must rely on PageLifecycle.

Notes
- Flags are additive/migration aids; defaults preserve current behavior.
- Turning flags off should not break the new AppEvent flows.
