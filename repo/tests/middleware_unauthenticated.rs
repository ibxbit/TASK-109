//! Integration test: AuthenticatedUser extractor refuses requests
//! without a Bearer token.
//!
//! Driving this through a real Actix App proves the FromRequest
//! impl correctly returns 401 (instead of, say, panicking) when
//! the Authorization header is missing or malformed. We can't
//! exercise the success path here without a Postgres connection.

use actix_web::{get, http::StatusCode, test, App, HttpResponse};
use vitalpath::middleware::auth::AuthenticatedUser;

#[get("/echo")]
async fn echo(_auth: AuthenticatedUser) -> HttpResponse {
    HttpResponse::Ok().body("ok")
}

#[actix_web::test]
async fn missing_authorization_header_returns_401() {
    let app = test::init_service(App::new().service(echo)).await;
    let req = test::TestRequest::get().uri("/echo").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn malformed_authorization_header_returns_401() {
    let app = test::init_service(App::new().service(echo)).await;
    let req = test::TestRequest::get()
        .uri("/echo")
        .insert_header(("Authorization", "Token xyz"))   // not "Bearer ..."
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn empty_bearer_token_with_no_pool_yields_500() {
    // With a Bearer token but no DbPool app_data, the extractor reaches
    // the pool-lookup branch and returns AppError::Internal → 500.
    // This pins that contract: missing infra → 500, missing creds → 401.
    let app = test::init_service(App::new().service(echo)).await;
    let req = test::TestRequest::get()
        .uri("/echo")
        .insert_header(("Authorization", "Bearer some-token"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
