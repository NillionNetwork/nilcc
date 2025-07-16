use axum::{extract::State, http::StatusCode, Json};
use bollard::{query_parameters::ListContainersOptionsBuilder, secret::ContainerSummaryStateEnum, Docker};
use serde::Serialize;
use std::sync::Arc;
use tracing::error;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Container {
    names: Vec<String>,
    image: String,
    image_id: String,
    state: String,
}

pub(crate) async fn handler(docker: State<Arc<Docker>>) -> Result<Json<Vec<Container>>, StatusCode> {
    let options = ListContainersOptionsBuilder::new().all(true).build();
    let containers = docker.list_containers(Some(options)).await.map_err(|e| {
        error!("Failed to fetch logs: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let containers = containers
        .into_iter()
        .map(|c| Container {
            // get rid of the `/` at the beginning of container names
            names: c.names.unwrap_or_default().into_iter().map(|n| n.trim_start_matches('/').to_string()).collect(),
            image: c.image.unwrap_or_default(),
            image_id: c.image_id.unwrap_or_default(),
            state: c.state.unwrap_or(ContainerSummaryStateEnum::EMPTY).to_string(),
        })
        .collect();
    Ok(Json(containers))
}
