use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Forbidden")]
    Forbidden,

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[allow(dead_code)]
    #[error("Conflict with data")]
    ConflictWithData(serde_json::Value),

    #[error("Too many requests: {0}")]
    TooManyRequests(String),

    #[error("Internal server error")]
    Internal(#[from] anyhow::Error),

    #[error("Database error")]
    Database(#[from] diesel::result::Error),
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
    message: String,
}

impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Forbidden => StatusCode::FORBIDDEN,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::ConflictWithData(_) => StatusCode::CONFLICT,
            AppError::TooManyRequests(_) => StatusCode::TOO_MANY_REQUESTS,
            AppError::Internal(_) | AppError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        match self {
            AppError::ConflictWithData(data) => {
                HttpResponse::build(StatusCode::CONFLICT).json(data)
            }
            _ => {
                let status = self.status_code();
                let body = ErrorBody {
                    error: status.canonical_reason().unwrap_or("error").to_string(),
                    message: self.to_string(),
                };
                HttpResponse::build(status).json(body)
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────
// Unit tests — exhaustive status-code mapping and error formatting.
// ─────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_maps_to_404() {
        let err = AppError::NotFound("goal".into());
        assert_eq!(err.status_code(), StatusCode::NOT_FOUND);
        assert!(err.to_string().contains("goal"));
    }

    #[test]
    fn unauthorized_maps_to_401() {
        assert_eq!(AppError::Unauthorized.status_code(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn forbidden_maps_to_403() {
        assert_eq!(AppError::Forbidden.status_code(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn bad_request_maps_to_400() {
        let err = AppError::BadRequest("bad".into());
        assert_eq!(err.status_code(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn conflict_maps_to_409() {
        assert_eq!(
            AppError::Conflict("dup".into()).status_code(),
            StatusCode::CONFLICT
        );
    }

    #[test]
    fn conflict_with_data_maps_to_409() {
        let err = AppError::ConflictWithData(serde_json::json!({"a": 1}));
        assert_eq!(err.status_code(), StatusCode::CONFLICT);
    }

    #[test]
    fn too_many_requests_maps_to_429() {
        assert_eq!(
            AppError::TooManyRequests("slow down".into()).status_code(),
            StatusCode::TOO_MANY_REQUESTS
        );
    }

    #[test]
    fn internal_error_maps_to_500() {
        let err = AppError::Internal(anyhow::anyhow!("boom"));
        assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn database_error_maps_to_500() {
        let err = AppError::Database(diesel::result::Error::NotFound);
        assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn from_anyhow_yields_internal_variant() {
        let err: AppError = anyhow::anyhow!("wrapped").into();
        assert!(matches!(err, AppError::Internal(_)));
    }

    #[test]
    fn from_diesel_yields_database_variant() {
        let err: AppError = diesel::result::Error::NotFound.into();
        assert!(matches!(err, AppError::Database(_)));
    }

    #[test]
    fn error_response_renders_json_body() {
        // Smoke-test: building a response shouldn't panic for any variant.
        for err in [
            AppError::NotFound("x".into()),
            AppError::Unauthorized,
            AppError::Forbidden,
            AppError::BadRequest("x".into()),
            AppError::Conflict("x".into()),
            AppError::TooManyRequests("x".into()),
            AppError::Internal(anyhow::anyhow!("x")),
            AppError::Database(diesel::result::Error::NotFound),
            AppError::ConflictWithData(serde_json::json!({"k": "v"})),
        ] {
            let resp = err.error_response();
            assert!(resp.status().is_client_error() || resp.status().is_server_error());
        }
    }
}
