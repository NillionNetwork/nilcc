use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use bollard::{query_parameters::LogsOptionsBuilder, Docker};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use validator::Validate;

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContainersLogsRequest {
    container: String,
    #[serde(default)]
    tail: bool,
    stream: OutputStream,
    #[validate(range(max = 1000))]
    max_lines: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum OutputStream {
    Stdout,
    Stderr,
}

#[derive(Serialize)]
pub(crate) struct ContainersLogsResponse {
    lines: Vec<String>,
}

pub(crate) async fn handler(
    docker: State<Arc<Docker>>,
    request: Query<ContainersLogsRequest>,
) -> Result<Json<ContainersLogsResponse>, StatusCode> {
    let ContainersLogsRequest { container, tail, stream, max_lines } = request.0;
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
    Ok(Json(ContainersLogsResponse { lines }))
}
