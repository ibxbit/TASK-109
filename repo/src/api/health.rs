use actix_web::{get, web, HttpResponse};
use chrono::Utc;
use serde_json::json;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

use crate::db::DbPool;
use crate::metrics::{estimate_p95_ms, update_pool_gauges};

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(health_check);
    cfg.service(liveness_check);
}

/// Liveness check — always 200 while the process is running.
/// Docker health check uses this endpoint so the container becomes "healthy"
/// the moment the HTTP server binds, independent of DB state.
#[get("/healthz")]
async fn liveness_check() -> HttpResponse {
    HttpResponse::Ok().json(json!({"status": "ok"}))
}

/// Readiness check — 200 only after migrations + seeding are complete and
/// the DB can be reached.  run_tests.sh waits on `.status == "ok"` here
/// before starting any API tests.
#[get("/health")]
async fn health_check(
    pool: web::Data<DbPool>,
    db_ready: web::Data<Arc<AtomicBool>>,
) -> HttpResponse {
    // While DB initialisation (migrations, seeding) is still running, tell
    // callers to wait.  curl -sf fails on 503, so the test-runner loop
    // continues polling until we flip the flag.
    if !db_ready.load(Ordering::SeqCst) {
        return HttpResponse::ServiceUnavailable().json(json!({
            "status": "starting",
            "timestamp": Utc::now().to_rfc3339(),
            "message": "database initialization in progress"
        }));
    }

    let pool_inner = pool.clone();

    // Run the DB ping on the blocking thread pool to avoid stalling the async executor
    let (db_ok, ping_ms) = web::block(move || {
        let start = Instant::now();
        let result = pool_inner.get().map(|mut conn| {
            // Simple round-trip: execute a trivial query to measure DB latency
            use diesel::sql_query;
            use diesel::RunQueryDsl;
            sql_query("SELECT 1").execute(&mut conn).ok();
        });
        let elapsed_ms = start.elapsed().as_millis() as u64;
        (result.is_ok(), elapsed_ms)
    })
    .await
    .unwrap_or((false, 0));

    // Update pool gauges while we have pool access
    let state = pool.state();
    let active = state.connections.saturating_sub(state.idle_connections);
    update_pool_gauges(active, state.idle_connections);

    let p95 = estimate_p95_ms();
    let status = if db_ok { "ok" } else { "degraded" };
    let code = if db_ok { 200 } else { 503 };

    HttpResponse::build(actix_web::http::StatusCode::from_u16(code).unwrap()).json(json!({
        "status": status,
        "timestamp": Utc::now().to_rfc3339(),
        "checks": {
            "database": {
                "status": if db_ok { "ok" } else { "unavailable" },
                "ping_ms": ping_ms
            }
        },
        "pool": {
            "connections": state.connections,
            "idle": state.idle_connections,
            "active": active
        },
        "metrics": {
            "p95_latency_ms": p95
        }
    }))
}
