//! Integration test: drive the RateLimit middleware through a real
//! Actix App to confirm the sliding-window logic and 429 response
//! bodies match production behaviour.

use actix_web::{get, http::StatusCode, test, web, App, HttpResponse};
use std::sync::Mutex;
use vitalpath::security::rate_limit::{
    new_store, new_token_user_cache, RateLimit, WINDOW_SECS,
};

/// All tests in this file mutate `RATE_LIMIT_MAX`, so they must run
/// serially within this binary even though cargo test parallelises by
/// default. Holding this mutex for the duration of each test guarantees
/// the env var is stable while any one test is running.
static ENV_LOCK: Mutex<()> = Mutex::new(());

#[get("/ping")]
async fn ping() -> HttpResponse {
    HttpResponse::Ok().body("pong")
}

#[actix_web::test]
async fn unauthenticated_caller_gets_429_after_limit() {
    let _g = ENV_LOCK.lock().unwrap();
    // Force the limit very low for this test only.
    std::env::set_var("RATE_LIMIT_MAX", "3");

    let store = new_store();
    let cache = new_token_user_cache();
    let store_data = web::Data::new(store.clone());
    let cache_data = web::Data::new(cache.clone());

    let app = test::init_service(
        App::new()
            .app_data(store_data)
            .app_data(cache_data)
            .wrap(RateLimit::new(store.clone(), cache.clone()))
            .service(ping),
    )
    .await;

    // First three requests pass.
    for _ in 0..3 {
        let req = test::TestRequest::get()
            .uri("/ping")
            .peer_addr("127.0.0.1:1234".parse().unwrap())
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    // The fourth must be blocked.
    let req = test::TestRequest::get()
        .uri("/ping")
        .peer_addr("127.0.0.1:1234".parse().unwrap())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    let retry = resp
        .headers()
        .get("Retry-After")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(retry, WINDOW_SECS.to_string());

    // Cleanup so other tests aren't affected.
    std::env::remove_var("RATE_LIMIT_MAX");
}

#[actix_web::test]
async fn login_endpoint_is_excluded_from_rate_limit() {
    let _g = ENV_LOCK.lock().unwrap();
    std::env::set_var("RATE_LIMIT_MAX", "1");

    let store = new_store();
    let cache = new_token_user_cache();

    // Define an /auth/login route locally — the middleware bails out
    // for any path starting with /auth/login regardless of method.
    #[get("/auth/login")]
    async fn login_stub() -> HttpResponse {
        HttpResponse::Ok().body("login")
    }

    let app = test::init_service(
        App::new()
            .wrap(RateLimit::new(store.clone(), cache.clone()))
            .service(login_stub),
    )
    .await;

    // 5 requests — none should be limited because /auth/login is excluded.
    for _ in 0..5 {
        let req = test::TestRequest::get()
            .uri("/auth/login")
            .peer_addr("127.0.0.1:5555".parse().unwrap())
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    std::env::remove_var("RATE_LIMIT_MAX");
}

#[actix_web::test]
async fn distinct_clients_have_independent_quotas() {
    let _g = ENV_LOCK.lock().unwrap();
    std::env::set_var("RATE_LIMIT_MAX", "2");

    let store = new_store();
    let cache = new_token_user_cache();

    let app = test::init_service(
        App::new()
            .wrap(RateLimit::new(store.clone(), cache.clone()))
            .service(ping),
    )
    .await;

    // Client A: spends quota.
    for _ in 0..2 {
        let req = test::TestRequest::get()
            .uri("/ping")
            .peer_addr("10.0.0.1:1111".parse().unwrap())
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }
    let req = test::TestRequest::get()
        .uri("/ping")
        .peer_addr("10.0.0.1:1111".parse().unwrap())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);

    // Client B: still has the full quota.
    let req = test::TestRequest::get()
        .uri("/ping")
        .peer_addr("10.0.0.2:2222".parse().unwrap())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    std::env::remove_var("RATE_LIMIT_MAX");
}
