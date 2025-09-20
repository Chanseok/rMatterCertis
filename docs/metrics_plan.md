# Metrics Plan (Prometheus) - Phase 1 Completion

## Scope (Phase 1)
- Counters: emitted_total{event_type}, emit_fail_total{event_type,error_category}, throttled_total{event_type}
- Gauge: event_gap_total
- Exporter: /metrics HTTP endpoint (default 9898, override MC_METRICS_PORT)
- Integration: emission success/fail + throttling paths wired
- Test: basic counter increment test (tests/metrics_counters.rs)

Status: ✅ Completed

## Next (Phase 1.1 / Roadmap)
- Add session_started/finished/failed counters
- Add stage_started/completed counters + duration histogram (stage_duration_seconds)
- Histogram: event_emit_latency_seconds (measure mapping + emit latency)
- Add gap detection hook from frontend (invoke command or channel backflow) -> currently only backend gap gauge increment util available
- Graceful shutdown signal for metrics server (tie into app shutdown)
- Integration test: scrape /metrics and assert exposition format
- Backpressure metrics tie-in: queue_depth gauge, sampled_drop_total counter (after backpressure impl)

## Env Vars
- MC_METRICS_PORT: override exporter port (default 9898)

## Usage
Expose to Prometheus by adding scrape target:

scrape_configs:
  - job_name: 'mattercertis'
    static_configs:
      - targets: ['localhost:9898']

## Validation Checklist
- [x] Server starts once per run (log: Metrics server initialized)
- [x] Counters increase on synthetic test
- [x] No panics on multiple label registrations

---
Document created automatically during Phase 1 instrumentation.
