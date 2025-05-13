use axum::response::{IntoResponse, Response};
use reqwest::StatusCode;

pub(crate) async fn handler() -> Response {
    (StatusCode::OK, "OK").into_response()
}
