use actix_web::{get, web, HttpResponse};
use chrono::Utc;
use serde_json::json;
use std::time::Instant;

use crate::db::DbPool;
use crate::metrics::{estimate_p95_ms, update_pool_gauges};

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(health_check);
    cfg.service(liveness_check);
}

/// Simple liveness check — returns 200 as long as the process is running.
/// Used by Docker's health check so the container becomes "healthy" once
/// the HTTP server is up, independent of database connectivity.
#[get("/healthz")]
async fn liveness_check() -> HttpResponse {
    HttpResponse::Ok().json(json!({"status": "ok"}))
}

#[get("/health")]
async fn health_check(pool: web::Data<DbPool>) -> HttpResponse {
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
