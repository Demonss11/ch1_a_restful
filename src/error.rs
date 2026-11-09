use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    NotFound { resource: &'static str, id: String },
    Conflict(String),
    BadRequest(String),
    Internal,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, title, detail) = match self {
            AppError::NotFound { resource, id } => (
                StatusCode::NOT_FOUND,
                format!("{resource} not found"),
                format!("'{id}' is unknown"),
            ),
            AppError::Conflict(reason) => (StatusCode::CONFLICT, "Conflict".into(), reason),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "Bad Request".into(), msg),
            AppError::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error".into(),
                "Unexpected error".into(),
            ),
        };

        let body = json!({
            "type": format!("https://demo.dev/err/{}", title.to_lowercase().replace(' ', "-")),
            "title": title,
            "status": status.as_u16(),
            "detail": detail,
            "_links": [
                {"rel": "entry", "method": "GET", "href": "/"}
            ]
        });

        (
            status,
            [("content-type", "application/problem+json")],
            Json(body),
        )
            .into_response()
    }
}
