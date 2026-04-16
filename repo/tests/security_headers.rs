//! Integration test: assert that the security-headers middleware
//! used in production is wired the same way when configured by
//! tests, by reproducing the header bundle and asserting it
//! attaches to every response — including 404s.

use actix_web::{middleware::DefaultHeaders, test, App};
use vitalpath::api;

fn security_headers() -> DefaultHeaders {
    // Mirrors `src/main.rs`. If main.rs adds/removes a header,
    // this list MUST be updated in lockstep.
    DefaultHeaders::new()
        .add(("X-Content-Type-Options", "nosniff"))
        .add(("X-Frame-Options", "DENY"))
        .add(("X-XSS-Protection", "1; mode=block"))
        .add(("Referrer-Policy", "no-referrer"))
        .add(("Content-Security-Policy", "default-src 'none'"))
        .add(("Cache-Control", "no-store, no-cache, must-revalidate"))
        .add((
            "Strict-Transport-Security",
            "max-age=31536000; includeSubDomains",
        ))
}

#[actix_web::test]
async fn liveness_response_carries_all_security_headers() {
    let app = test::init_service(
        App::new()
            .wrap(security_headers())
            .configure(api::health::routes),
    )
    .await;
    let req = test::TestRequest::get().uri("/healthz").to_request();
    let resp = test::call_service(&app, req).await;

    let h = resp.headers();
    assert_eq!(h.get("X-Content-Type-Options").unwrap(), "nosniff");
    assert_eq!(h.get("X-Frame-Options").unwrap(), "DENY");
    assert_eq!(h.get("X-XSS-Protection").unwrap(), "1; mode=block");
    assert_eq!(h.get("Referrer-Policy").unwrap(), "no-referrer");
    assert_eq!(
        h.get("Content-Security-Policy").unwrap(),
        "default-src 'none'"
    );
    assert!(h
        .get("Cache-Control")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("no-store"));
    assert!(h
        .get("Strict-Transport-Security")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("max-age=31536000"));
}

#[actix_web::test]
async fn not_found_response_also_carries_security_headers() {
    let app = test::init_service(
        App::new()
            .wrap(security_headers())
            .configure(api::health::routes),
    )
    .await;
    let req = test::TestRequest::get().uri("/nope").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
    // DefaultHeaders is supposed to attach to every response, including ones
    // synthesised by the framework — guard against regressions.
    assert!(resp.headers().get("X-Content-Type-Options").is_some());
}
