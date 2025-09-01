# Event-related Feature Flags

Unified-only model: the backend always emits the single unified channel "actor-event". Legacy event paths have been removed.

Removed flags
- MC_FEATURE_EVENTS_GENERALIZED_ONLY — unified is always on
- MC_FEATURE_LEGACY_DOMAIN_EVENTS — legacy emissions removed
- MC_FEATURE_EMIT_PAGETASK_LEGACY — PageTask* variants deleted; use PageLifecycle

Notes
- Frontend should subscribe to 'actor-event' and dispatch by payload.event_name and variant.
- Any references to actor-* direct names are for convenience-only shims and may be pruned.
