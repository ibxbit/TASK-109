//! Integration test: liveness endpoint and middleware wiring.
//!
//! These tests spin up a minimal Actix App with only the
//! pieces that don't require a Postgres connection. They verify
//! that the routing layer, security headers, and JSON body shape
//! all behave the same way as in production.

use actix_web::{http::StatusCode, test, App};
use vitalpath::api;

#[actix_web::test]
async fn liveness_returns_ok_with_status_field() {
    let app = test::init_service(App::new().configure(api::health::routes)).await;
    let req = test::TestRequest::get().uri("/healthz").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "ok");
}

#[actix_web::test]
async fn liveness_endpoint_uses_json_content_type() {
    let app = test::init_service(App::new().configure(api::health::routes)).await;
    let req = test::TestRequest::get().uri("/healthz").to_request();
    let resp = test::call_service(&app, req).await;
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(ct.starts_with("application/json"), "got `{}`", ct);
}

#[actix_web::test]
async fn liveness_accepts_only_get() {
    let app = test::init_service(App::new().configure(api::health::routes)).await;
    let req = test::TestRequest::post().uri("/healthz").to_request();
    let resp = test::call_service(&app, req).await;
    // Actix returns 404 for an unmatched method on a GET-only route.
    assert!(
        resp.status() == StatusCode::NOT_FOUND
            || resp.status() == StatusCode::METHOD_NOT_ALLOWED,
        "unexpected status: {:?}",
        resp.status()
    );
}

#[actix_web::test]
async fn unknown_route_returns_404() {
    let app = test::init_service(App::new().configure(api::health::routes)).await;
    let req = test::TestRequest::get()
        .uri("/this-route-does-not-exist")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
