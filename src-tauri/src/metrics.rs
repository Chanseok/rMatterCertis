use once_cell::sync::Lazy;
use prometheus::{IntCounterVec, IntGauge, Registry, Encoder, TextEncoder};
use std::net::SocketAddr;
use hyper::{Body, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};
use tokio::task::JoinHandle;
use tracing::{info, error};

pub static REGISTRY: Lazy<Registry> = Lazy::new(Registry::new);

pub static EVENT_EMITTED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::Opts::new("mattercertis_event_emitted_total", "Structured crawl events emitted"),
        &["event_type"],
    ).unwrap();
    REGISTRY.register(Box::new(c.clone())).unwrap();
    c
});

pub static EVENT_EMIT_FAIL_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::Opts::new("mattercertis_event_emit_fail_total", "Failed crawl events emission"),
        &["event_type", "error_category"],
    ).unwrap();
    REGISTRY.register(Box::new(c.clone())).unwrap();
    c
});

pub static EVENT_THROTTLED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let c = IntCounterVec::new(
        prometheus::Opts::new("mattercertis_event_throttled_total", "Throttled (skipped) events"),
        &["event_type"],
    ).unwrap();
    REGISTRY.register(Box::new(c.clone())).unwrap();
    c
});

pub static EVENT_GAP_TOTAL: Lazy<IntGauge> = Lazy::new(|| {
    let g = IntGauge::new("mattercertis_event_gap_total", "Detected sequence gaps (incremental)").unwrap();
    REGISTRY.register(Box::new(g.clone())).unwrap();
    g
});

pub async fn start_metrics_server(port: u16) -> JoinHandle<()> {
    let addr = SocketAddr::from(([127,0,0,1], port));
    info!(?addr, "Starting metrics server");
    // Use a never-ending pending future so the server stays alive for the app lifetime.
    let shutdown_future = async {
        futures::future::pending::<()>().await;
    };
    Server::bind(&addr)
        .serve(make_service_fn(|_conn| async {
            Ok::<_, hyper::Error>(service_fn(|req: Request<Body>| async move {
                if req.uri().path() == "/metrics" { serve_metrics().await } else { Ok(Response::builder().status(404).body(Body::from("not found")).unwrap()) }
            }))
        }))
        .with_graceful_shutdown(shutdown_future)
        .await
        .map_err(|e| error!(?e, "metrics server error")).ok();
    info!("Metrics server stopped");
    tokio::task::spawn(async {} ) // dummy handle not used
}

async fn serve_metrics() -> Result<Response<Body>, hyper::Error> {
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    Ok(Response::builder()
        .status(200)
        .header("Content-Type", encoder.format_type())
        .body(Body::from(buffer))
        .unwrap())
}

pub fn inc_emitted(event_type: &str) { let _ = EVENT_EMITTED_TOTAL.with_label_values(&[event_type]).inc(); }
pub fn inc_emit_fail(event_type: &str, category: &str) { let _ = EVENT_EMIT_FAIL_TOTAL.with_label_values(&[event_type, category]).inc(); }
pub fn inc_throttled(event_type: &str) { let _ = EVENT_THROTTLED_TOTAL.with_label_values(&[event_type]).inc(); }

// Gaps are rare; treat as gauge increment (can reset externally if needed)
pub fn add_gap(delta: i64) { EVENT_GAP_TOTAL.add(delta); }
