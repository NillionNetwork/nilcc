use axum::{routing::get, Router};
use bollard::Docker;
use std::sync::Arc;

pub(crate) mod containers;

pub fn create_router(docker: Arc<Docker>) -> Router {
    Router::new().nest(
        "/api/v1",
        Router::new()
            .route("/containers/logs", get(containers::logs::handler))
            .route("/containers/list", get(containers::list::handler))
            .with_state(docker),
    )
}
