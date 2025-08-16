# Logging Policy and Routing

This project splits logs into two files to keep console noise low and make KPI/actor analytics reliable.

- back_front.log: Application and infrastructure logs. Excludes actor-event and kpi.* targets.
- events.log: Only actor-event and kpi.* targets (concise JSON lines for analytics and UI dashboards).

Routing
- All concise actor events are emitted with target="actor-event" via the actor_event_bridge.
- KPI lines use targets: kpi.plan, kpi.batch, kpi.session, kpi.execution_plan.
- The logging init wires a dedicated file layer for events.log with include filters on these targets, and separate file+console layers that exclude them.

Retention and rotation
- Keep events.log rotated daily or by size (e.g., 50MB) with 7–14 days retention for local dev.
- Keep back_front.log rotated daily with 3–7 days retention.
- Production should ship events.log to an analytics sink (e.g., OpenSearch) and keep local retention at 3 days.

Verification
- scripts/verify_runtime_plan.sh prefers KPI from events.log and stage text from back_front.log.
- The script scopes structured checks to the latest session_id to avoid cross-session mismatches in aggregated logs.

Notes
- PageLifecycle should be emitted directly from StageActor; avoid synthetic composition when direct events exist.
- DetailTask* events keep scope="session" and optional batch context populated only when mapping is available.
