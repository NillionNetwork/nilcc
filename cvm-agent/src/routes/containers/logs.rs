use crate::routes::SharedState;
use axum::{Json, extract::Query, http::StatusCode};
use axum_valid::Valid;
use bollard::query_parameters::{InspectContainerOptionsBuilder, LogsOptionsBuilder};
use cvm_agent_models::logs::{ContainerLogsRequest, ContainerLogsResponse, OutputStream};
use futures::StreamExt;

pub(crate) async fn handler(
    state: SharedState,
    request: Valid<Query<ContainerLogsRequest>>,
) -> Result<Json<ContainerLogsResponse>, StatusCode> {
    let ContainerLogsRequest { container, tail, stream, max_lines } = request.0.0;
    let mut builder = LogsOptionsBuilder::new();
    if tail {
        builder = builder.tail(&max_lines.to_string());
    }
    let builder = match stream {
        OutputStream::Stdout => builder.stdout(true),
        OutputStream::Stderr => builder.stderr(true),
    };

    if state.docker.inspect_container(&container, Some(InspectContainerOptionsBuilder::new().build())).await.is_err() {
        return Err(StatusCode::NOT_FOUND);
    }

    let mut lines = Vec::new();
    let mut stream = state.docker.logs(&container, Some(builder.build())).take(max_lines);
    while let Some(output) = stream.next().await {
        let output = output.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        lines.push(String::from_utf8_lossy(&output.into_bytes()).trim().to_string());
    }
    Ok(Json(ContainerLogsResponse { lines }))
}
