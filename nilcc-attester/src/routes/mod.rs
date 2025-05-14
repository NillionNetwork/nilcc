use axum::{
    routing::{get, post},
    Router,
};

pub(crate) mod health;
pub(crate) mod report;

/// Build a router for the entire service.
pub fn build_router() -> Router {
    Router::new()
        .route("/health", get(health::handler))
        .nest("/api/v1", Router::new().route("/report/generate", post(report::generate::handler)))
}
