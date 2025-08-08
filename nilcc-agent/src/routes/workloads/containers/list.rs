use crate::routes::{workloads::containers::CvmAgentHandlerError, AppState, Json};
use axum::extract::{Path, State};
use cvm_agent_models::container::Container;
use uuid::Uuid;

pub(crate) async fn handler(
    state: State<AppState>,
    path: Path<Uuid>,
) -> Result<Json<Vec<Container>>, CvmAgentHandlerError> {
    let port = state.services.workload.cvm_agent_port(path.0).await?;
    state
        .clients
        .cvm_agent
        .list_containers(port)
        .await
        .map(Json)
        .map_err(|e| CvmAgentHandlerError::Internal(format!("{e:#}")))
}
