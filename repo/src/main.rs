use actix_web::{web, App, HttpServer};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod api;
mod auth;
mod config;
mod crypto;
mod db;
mod errors;
mod metrics;
mod middleware;
mod models;
mod notifications;
mod schema;
mod security;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Structured JSON logging
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    let cfg = config::AppConfig::from_env();
    let pool = db::init_pool(&cfg.database_url);

    db::run_migrations(&pool);
    db::seed_initial_data(&pool);

    // Ensure exports directory exists
    std::fs::create_dir_all(&cfg.exports_dir).expect("failed to create exports directory");

    // ── Key-rotation health check ─────────────────────────────
    {
        let mut conn = pool.get().expect("DB pool unavailable at startup");
        crypto::check_key_rotation(&mut conn);
    }

    // Background workers — spawned before the pool is moved into web::Data
    notifications::start_delivery_worker(pool.clone());
    notifications::start_schedule_worker(pool.clone());

    // Build the field cipher once; share across all requests via Arc.
    let cipher = web::Data::new(crypto::FieldCipher::new(
        &cfg.field_encryption_key,
        &cfg.encryption_key_version,
    ));

    // ── Rate-limit store — shared across all Actix workers ────
    let rate_limit_store      = security::rate_limit::new_store();
    let token_user_cache      = security::rate_limit::new_token_user_cache();
    let token_user_cache_data = web::Data::new(token_user_cache.clone());

    info!(host = %cfg.host, port = cfg.port, "VitalPath starting");

    let pool = web::Data::new(pool);
    let cfg_data = web::Data::new(cfg.clone());

    HttpServer::new(move || {
        // ── Security headers applied to every response ────────
        // These mitigate XSS, clickjacking, MIME-sniffing, and
        // information leakage.  The Content-Security-Policy is
        // restrictive because this is a pure JSON API with no HTML.
        let security_headers = actix_web::middleware::DefaultHeaders::new()
            .add(("X-Content-Type-Options",  "nosniff"))
            .add(("X-Frame-Options",          "DENY"))
            .add(("X-XSS-Protection",         "1; mode=block"))
            .add(("Referrer-Policy",           "no-referrer"))
            .add(("Content-Security-Policy",   "default-src 'none'"))
            .add(("Cache-Control",             "no-store, no-cache, must-revalidate"))
            .add(("Strict-Transport-Security", "max-age=31536000; includeSubDomains"));

        App::new()
            .app_data(pool.clone())
            .app_data(cfg_data.clone())
            .app_data(cipher.clone())
            .app_data(token_user_cache_data.clone())
            // Structured request/response tracing
            .wrap(tracing_actix_web::TracingLogger::default())
            // Prometheus metrics: latency, request counts, error rates
            .wrap(middleware::telemetry::Telemetry)
            // Security headers on every response
            .wrap(security_headers)
            // Per-user sliding-window rate limit (60 req / 60 s)
            .wrap(security::rate_limit::RateLimit::new(rate_limit_store.clone(), token_user_cache.clone()))
            .configure(api::health::routes)
            .configure(api::metrics::routes)
            .configure(api::auth::routes)
            .configure(api::health_profile::routes)
            .configure(api::metric_entries::routes)
            .configure(api::goals::routes)
            .configure(api::notifications::routes)
            .configure(api::work_orders::routes)
            .configure(api::workflows::routes)
            .configure(api::analytics::routes)
            .configure(api::audit_logs::routes)
    })
    .bind(format!("{}:{}", cfg.host, cfg.port))?
    .run()
    .await
}
