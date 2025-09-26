use crate::routes::SharedState;
use axum::{Json, extract::Query, http::StatusCode};
use axum_valid::Valid;
use cvm_agent_models::logs::{SystemLogsRequest, SystemLogsResponse, SystemLogsSource};
use std::io;
use tokio::io::{AsyncBufRead, AsyncBufReadExt};
use tokio::{fs::File, io::BufReader};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::LinesStream;
use tracing::error;

pub(crate) async fn handler(
    state: SharedState,
    request: Valid<Query<SystemLogsRequest>>,
) -> Result<Json<SystemLogsResponse>, StatusCode> {
    let SystemLogsRequest { source, tail, max_lines } = request.0.0;
    let path = match source {
        SystemLogsSource::CvmAgent => &state.log_path,
    };
    let reader = match File::open(path).await {
        Ok(file) => file,
        Err(e) => {
            error!("Failed to open log file {}: {e}", path.display());
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    let reader = BufReader::new(reader);
    let result = match tail {
        true => fetch_tail_lines(reader, max_lines).await,
        false => fetch_head_lines(reader, max_lines).await,
    };
    match result {
        Ok(lines) => Ok(Json(SystemLogsResponse { lines })),
        Err(e) => {
            error!("Failed to read logs: {e}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn fetch_head_lines<R: AsyncBufRead + Unpin>(reader: R, max_lines: usize) -> io::Result<Vec<String>> {
    let mut lines = Vec::new();
    let mut reader = LinesStream::new(reader.lines()).take(max_lines);
    while let Some(line) = reader.next().await {
        let line = line?;
        lines.push(line);
    }
    Ok(lines)
}

async fn fetch_tail_lines<R: AsyncBufRead + Unpin>(reader: R, max_lines: usize) -> io::Result<Vec<String>> {
    // This is very inefficient but for now we don't care
    let mut lines = Vec::new();
    let mut reader = reader.lines();
    while let Some(line) = reader.next_line().await? {
        lines.push(line);
    }
    let extra = lines.len().saturating_sub(max_lines);
    let lines = lines.into_iter().skip(extra).collect();
    Ok(lines)
}
