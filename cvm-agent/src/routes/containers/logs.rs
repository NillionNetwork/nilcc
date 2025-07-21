use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use axum_valid::Valid;
use bollard::{query_parameters::LogsOptionsBuilder, Docker};
use cvm_agent_models::logs::{ContainerLogsRequest, ContainerLogsResponse, OutputStream};
use futures::StreamExt;
use std::sync::Arc;

pub(crate) async fn handler(
    docker: State<Arc<Docker>>,
    request: Valid<Query<ContainerLogsRequest>>,
) -> Result<Json<ContainerLogsResponse>, StatusCode> {
    let ContainerLogsRequest { container, tail, stream, max_lines } = request.0 .0;
    let mut builder = LogsOptionsBuilder::new();
    if tail {
        builder = builder.tail(&max_lines.to_string());
    }
    let builder = match stream {
        OutputStream::Stdout => builder.stdout(true),
        OutputStream::Stderr => builder.stderr(true),
    };

    let mut lines = Vec::new();
    let mut stream = docker.logs(&container, Some(builder.build())).take(max_lines);
    while let Some(output) = stream.next().await {
        let output = output.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        lines.push(String::from_utf8_lossy(&output.into_bytes()).trim().to_string());
    }
    Ok(Json(ContainerLogsResponse { lines }))
}
