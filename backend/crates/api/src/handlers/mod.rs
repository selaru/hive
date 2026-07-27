pub mod agents;
pub mod mcp;
pub mod projects;
pub mod providers;
pub mod skills;
pub mod teams;
pub mod tools;

use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

/// Unified error response type for all handlers.
pub struct ApiError(anyhow::Error, StatusCode);

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let code = self.1;
        let msg  = self.0.to_string();
        tracing::warn!("API error {}: {}", code, msg);
        (code, Json(json!({ "error": { "message": msg } }))).into_response()
    }
}

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self {
        ApiError(e.into(), StatusCode::INTERNAL_SERVER_ERROR)
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
