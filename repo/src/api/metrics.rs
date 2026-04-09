use actix_web::{get, web, HttpResponse};

use crate::db::DbPool;
use crate::errors::AppError;
use crate::metrics::{gather_metrics, update_pool_gauges};
use crate::middleware::auth::AdminAuth;

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(metrics_endpoint);
}

#[get("/internal/metrics")]
async fn metrics_endpoint(
    pool: web::Data<DbPool>,
    _auth: AdminAuth,
) -> Result<HttpResponse, AppError> {
    // Refresh pool gauges before scraping so Prometheus sees current values
    let state = pool.state();
    let active = state.connections.saturating_sub(state.idle_connections);
    update_pool_gauges(active, state.idle_connections);

    let body = gather_metrics();
    Ok(HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4")
        .body(body))
}
