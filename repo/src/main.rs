use actix_web::{web, App, HttpServer};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
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
    // Write to stderr immediately — stderr is unbuffered, so this survives even
    // if the process is killed before stdout can be flushed (e.g. OOM-SIGKILL).
    eprintln!("[vitalpath] process started");

    // Structured JSON logging
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    eprintln!("[vitalpath] loading config");
    let cfg = config::AppConfig::from_env();

    // Ensure exports directory exists (filesystem-only, no DB needed)
    std::fs::create_dir_all(&cfg.exports_dir).expect("failed to create exports directory");

    eprintln!("[vitalpath] initialising DB pool");
    let pool = db::init_pool(&cfg.database_url);

    // Build the field cipher once; share across all requests via Arc.
    let cipher = web::Data::new(crypto::FieldCipher::new(
        &cfg.field_encryption_key,
        &cfg.encryption_key_version,
    ));

    // ── Rate-limit store — shared across all Actix workers ────
    let rate_limit_store = security::rate_limit::new_store();
    let token_user_cache = security::rate_limit::new_token_user_cache();
    let token_user_cache_data = web::Data::new(token_user_cache.clone());

    // Shared readiness flag — false until migrations + seeding complete.
    // The /health endpoint returns 503 while this is false so run_tests.sh
    // waits correctly before attempting API calls.
    let db_ready = Arc::new(AtomicBool::new(false));
    let db_ready_data = web::Data::new(db_ready.clone());

    // Spawn notification workers now — they tolerate pool failures gracefully
    // and will begin working once the DB becomes available.
    notifications::start_delivery_worker(pool.clone());
    notifications::start_schedule_worker(pool.clone());

    // ── DB initialisation runs in a blocking thread so the HTTP server can
    // bind and answer /healthz IMMEDIATELY, satisfying the Docker health check
    // and the test-runner's readiness wait without any delay. ─────────────────
    {
        let pool_bg = pool.clone();
        let db_ready_bg = db_ready.clone();
        tokio::task::spawn_blocking(move || {
            eprintln!("[vitalpath] waiting for database");
            db::wait_for_db(&pool_bg);

            eprintln!("[vitalpath] running migrations");
            db::run_migrations(&pool_bg);

            eprintln!("[vitalpath] seeding initial data");
            db::seed_initial_data(&pool_bg);

            // Key-rotation health check — non-fatal if it fails
            match pool_bg.get() {
                Ok(mut conn) => crypto::check_key_rotation(&mut conn),
                Err(e) => {
                    tracing::warn!("Key rotation check skipped: DB connection unavailable — {e}")
                }
            }

            // Signal readiness — /health will now return {"status":"ok"}
            db_ready_bg.store(true, Ordering::SeqCst);
            eprintln!("[vitalpath] database initialised — server is fully ready");
        });
    }

    eprintln!("[vitalpath] binding HTTP server on {}:{}", cfg.host, cfg.port);
    info!(host = %cfg.host, port = cfg.port, "VitalPath starting");

    let pool = web::Data::new(pool);
    let cfg_data = web::Data::new(cfg.clone());

    HttpServer::new(move || {
        // ── Security headers applied to every response ────────
        let security_headers = actix_web::middleware::DefaultHeaders::new()
            .add(("X-Content-Type-Options", "nosniff"))
            .add(("X-Frame-Options", "DENY"))
            .add(("X-XSS-Protection", "1; mode=block"))
            .add(("Referrer-Policy", "no-referrer"))
            .add(("Content-Security-Policy", "default-src 'none'"))
            .add(("Cache-Control", "no-store, no-cache, must-revalidate"))
            .add((
                "Strict-Transport-Security",
                "max-age=31536000; includeSubDomains",
            ));

        App::new()
            .app_data(pool.clone())
            .app_data(cfg_data.clone())
            .app_data(cipher.clone())
            .app_data(token_user_cache_data.clone())
            .app_data(db_ready_data.clone())
            .wrap(security_headers)
            // Structured request/response tracing
            .wrap(tracing_actix_web::TracingLogger::default())
            // Per-user sliding-window rate limit (60 req / 60 s)
            .wrap(security::rate_limit::RateLimit::new(
                rate_limit_store.clone(),
                token_user_cache.clone(),
            ))
            // Prometheus metrics: latency, request counts, error rates (Outer to capture everything)
            .wrap(middleware::telemetry::Telemetry)
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
