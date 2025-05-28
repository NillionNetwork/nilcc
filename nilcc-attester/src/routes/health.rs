use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub(crate) async fn handler() -> Response {
    (StatusCode::OK, "OK").into_response()
}
