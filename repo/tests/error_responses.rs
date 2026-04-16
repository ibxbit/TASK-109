//! Integration test: AppError → HttpResponse mapping.
//!
//! These exercises drive the `ResponseError` impl through Actix's
//! response builder so we cover the JSON-serialisation path and
//! confirm the canonical reason/message contract clients depend on.

use actix_web::{body::to_bytes, http::StatusCode, ResponseError};
use vitalpath::errors::AppError;

async fn read_body(err: AppError) -> (StatusCode, serde_json::Value) {
    let resp = err.error_response();
    let status = resp.status();
    let body = to_bytes(resp.into_body())
        .await
        .expect("body should be readable");
    let json: serde_json::Value =
        serde_json::from_slice(&body).expect("body should be valid JSON");
    (status, json)
}

#[actix_web::test]
async fn not_found_renders_404_with_message() {
    let (status, body) = read_body(AppError::NotFound("widget".into())).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "Not Found");
    assert!(body["message"].as_str().unwrap().contains("widget"));
}

#[actix_web::test]
async fn unauthorized_renders_401() {
    let (status, body) = read_body(AppError::Unauthorized).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "Unauthorized");
}

#[actix_web::test]
async fn forbidden_renders_403() {
    let (status, body) = read_body(AppError::Forbidden).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "Forbidden");
}

#[actix_web::test]
async fn bad_request_renders_400() {
    let (status, body) = read_body(AppError::BadRequest("missing field".into())).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["message"].as_str().unwrap().contains("missing field"));
}

#[actix_web::test]
async fn conflict_renders_409() {
    let (status, _body) = read_body(AppError::Conflict("dup".into())).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[actix_web::test]
async fn conflict_with_data_renders_arbitrary_payload() {
    let payload = serde_json::json!({
        "code": "GOAL_OVERLAP",
        "existing": [1, 2, 3]
    });
    let (status, body) = read_body(AppError::ConflictWithData(payload.clone())).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body, payload);
}

#[actix_web::test]
async fn too_many_requests_renders_429() {
    let (status, _body) = read_body(AppError::TooManyRequests("slow down".into())).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[actix_web::test]
async fn internal_error_renders_500_without_leaking_details() {
    let (status, body) =
        read_body(AppError::Internal(anyhow::anyhow!("private debug detail"))).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    // The Display impl returns the static "Internal server error" text —
    // the wrapped anyhow chain must NOT leak into the client response.
    assert_eq!(body["message"], "Internal server error");
    assert!(!body["message"]
        .as_str()
        .unwrap()
        .contains("private debug detail"));
}
