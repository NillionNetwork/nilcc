use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct Version {
    git_hash: &'static str,
    built_at: &'static str,
}

pub(crate) async fn handler() -> impl IntoResponse {
    let version = Version { git_hash: env!("BUILD_GIT_COMMIT_HASH"), built_at: env!("BUILD_TIMESTAMP") };
    (StatusCode::OK, Json(version))
}
