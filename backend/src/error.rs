use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("not found")]
    NotFound,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ApiError {
    fn code(&self) -> &'static str {
        match self {
            ApiError::NotFound => "not_found",
            ApiError::BadRequest(_) => "bad_request",
            ApiError::Conflict(_) => "conflict",
            ApiError::Sqlx(sqlx::Error::RowNotFound) => "not_found",
            ApiError::Sqlx(_) => "db_error",
            ApiError::Other(_) => "internal_error",
        }
    }

    fn status(&self) -> StatusCode {
        match self {
            ApiError::NotFound => StatusCode::NOT_FOUND,
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::Sqlx(sqlx::Error::RowNotFound) => StatusCode::NOT_FOUND,
            ApiError::Sqlx(_) | ApiError::Other(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status();
        let code = self.code();
        let message = self.to_string();
        if status.is_server_error() {
            tracing::error!(?self, "request failed");
        }
        let body = Json(json!({ "error": { "code": code, "message": message } }));
        (status, body).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
