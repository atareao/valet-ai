use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error("Unprocessable entity: {0}")]
    UnprocessableEntity(String),
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<crate::embeddings::provider::EmbeddingError> for AppError {
    fn from(err: crate::embeddings::provider::EmbeddingError) -> Self {
        AppError::Internal(err.to_string())
    }
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            Self::UnprocessableEntity(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg.clone()),
            Self::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            Self::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };
        (status, Json(ApiError { error: message })).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    /// `Conflict` must answer `409 Conflict` in the same `{"error": "..."}`
    /// shape as the rest of the variants.
    #[tokio::test]
    async fn conflict_maps_to_409_with_error_body() {
        let response = AppError::Conflict("stale mark".to_string()).into_response();

        assert_eq!(response.status(), StatusCode::CONFLICT);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let json: serde_json::Value = serde_json::from_slice(&body).expect("body is JSON");
        assert_eq!(json, serde_json::json!({"error": "stale mark"}));
    }
}
