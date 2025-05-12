use axum::{routing::post, Router};

pub(crate) mod report;

/// Build a router for the entire service.
pub fn build_router() -> Router {
    Router::new().nest("/api/v1", Router::new().route("/report/generate", post(report::generate::handler)))
}
