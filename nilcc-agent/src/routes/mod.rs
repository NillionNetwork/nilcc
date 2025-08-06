use crate::auth::AuthLayer;
use crate::clients::cvm_agent::CvmAgentClient;
use crate::config::ResourceLimitsConfig;
use crate::services::workload::WorkloadService;
use axum::extract::Request;
use axum::extract::{rejection::JsonRejection, FromRequest};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use axum_valid::{HasValidate, HasValidateArgs};
use nilcc_agent_models::errors::RequestHandlerError;
use serde::Serialize;
use std::ops::Deref;
use std::sync::Arc;
use tower::ServiceBuilder;
use validator::ValidateArgs;

pub(crate) mod workloads;

#[derive(Clone)]
pub struct Services {
    pub workload: Arc<dyn WorkloadService>,
}

#[derive(Clone)]
pub struct Clients {
    pub cvm_agent: Arc<dyn CvmAgentClient>,
}

#[derive(Clone)]
pub struct AppState {
    pub services: Services,
    pub clients: Clients,
    pub resource_limits: ResourceLimitsConfig,
}

pub fn build_router(state: AppState, token: String) -> Router {
    Router::new().route("/health", get(health)).nest(
        "/api/v1",
        Router::new()
            .route("/workloads/create", post(workloads::create::handler))
            .route("/workloads/delete", post(workloads::delete::handler))
            .route("/workloads/restart", post(workloads::restart::handler))
            .route("/workloads/stop", post(workloads::stop::handler))
            .route("/workloads/start", post(workloads::start::handler))
            .route("/workloads/list", get(workloads::list::handler))
            .route("/workloads/{workload_id}/containers/list", get(workloads::containers::list::handler))
            .route("/workloads/{workload_id}/containers/logs", get(workloads::containers::logs::handler))
            .with_state(state)
            .layer(ServiceBuilder::new().layer(AuthLayer::new(token))),
    )
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

/// A type that behaves like `axum::Json` but provides JSON structured errors when parsing fails.
#[derive(Debug)]
pub struct Json<T>(pub T);

impl<S, T> FromRequest<S> for Json<T>
where
    axum::Json<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync,
{
    type Rejection = (StatusCode, axum::Json<RequestHandlerError>);

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let (parts, body) = req.into_parts();
        let req = Request::from_parts(parts, body);

        match axum::Json::<T>::from_request(req, state).await {
            Ok(value) => Ok(Self(value.0)),
            Err(rejection) => {
                let payload = RequestHandlerError::new(rejection.body_text(), "MALFORMED_REQUEST");
                Err((rejection.status(), axum::Json(payload)))
            }
        }
    }
}

impl<T> Deref for Json<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> IntoResponse for Json<T>
where
    T: Serialize,
{
    fn into_response(self) -> axum::response::Response {
        axum::Json(self.0).into_response()
    }
}

// `axum_valid` support for `Json`/`validator`

impl<T> HasValidate for Json<T> {
    type Validate = T;
    fn get_validate(&self) -> &T {
        &self.0
    }
}

impl<'v, T: ValidateArgs<'v>> HasValidateArgs<'v> for Json<T> {
    type ValidateArgs = T;
    fn get_validate_args(&self) -> &Self::ValidateArgs {
        &self.0
    }
}
