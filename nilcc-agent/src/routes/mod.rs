#![allow(clippy::disallowed_types)]

use crate::auth::AuthLayer;
use crate::clients::cvm_agent::CvmAgentClient;
use crate::config::ResourceLimitsConfig;
use crate::services::upgrade::UpgradeService;
use crate::services::workload::WorkloadService;
use axum::extract::rejection::QueryRejection;
use axum::extract::{rejection::JsonRejection, FromRequest};
use axum::extract::{FromRequestParts, Request};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use nilcc_agent_models::errors::RequestHandlerError;
use nilcc_artifacts::VmType;
use serde::Serialize;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceBuilder;
use validator::Validate;

pub(crate) mod system;
pub(crate) mod workloads;

#[derive(Clone)]
pub struct Services {
    pub workload: Arc<dyn WorkloadService>,
    pub upgrade: Arc<dyn UpgradeService>,
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
    pub agent_domain: String,
    pub vm_types: Vec<VmType>,
    pub cvm_artifacts_path: PathBuf,
}

pub fn build_router(state: AppState, token: String) -> Router {
    Router::new().route("/health", get(health)).nest(
        "/api/v1",
        Router::new()
            .route("/system/artifacts/upgrade", post(system::artifacts::upgrade::handler))
            .route("/system/artifacts/version", get(system::artifacts::version::handler))
            .nest(
                "/workloads",
                Router::new()
                    .route("/create", post(workloads::create::handler))
                    .route("/delete", post(workloads::delete::handler))
                    .route("/restart", post(workloads::restart::handler))
                    .route("/stop", post(workloads::stop::handler))
                    .route("/start", post(workloads::start::handler))
                    .route("/list", get(workloads::list::handler))
                    .route("/{workload_id}/health", get(workloads::health::handler))
                    .route("/{workload_id}/containers/list", get(workloads::containers::list::handler))
                    .route("/{workload_id}/containers/logs", get(workloads::containers::logs::handler))
                    .route("/{workload_id}/system/logs", get(workloads::system::logs::handler))
                    .route("/{workload_id}/system/stats", get(workloads::system::stats::handler)),
            )
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
    T: Validate,
    S: Send + Sync,
{
    type Rejection = (StatusCode, axum::Json<RequestHandlerError>);

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let (parts, body) = req.into_parts();
        let req = Request::from_parts(parts, body);

        let inner = match axum::Json::<T>::from_request(req, state).await {
            Ok(value) => value.0,
            Err(rejection) => {
                let payload = RequestHandlerError::new(rejection.body_text(), "MALFORMED_REQUEST");
                return Err((rejection.status(), axum::Json(payload)));
            }
        };
        match inner.validate() {
            Ok(_) => Ok(Self(inner)),
            Err(e) => {
                let payload = RequestHandlerError::new(e.to_string(), "MALFORMED_REQUEST");
                Err((StatusCode::BAD_REQUEST, axum::Json(payload)))
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

/// A type that behaves like `axum::Query` but provides JSON structured errors when parsing fails.
#[derive(Debug)]
pub struct Query<T>(pub T);

impl<S, T> FromRequestParts<S> for Query<T>
where
    axum::extract::Query<T>: FromRequestParts<S, Rejection = QueryRejection>,
    T: Validate,
    S: Send + Sync,
{
    type Rejection = (StatusCode, axum::Json<RequestHandlerError>);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let inner = match axum::extract::Query::<T>::from_request_parts(parts, state).await {
            Ok(value) => value.0,
            Err(rejection) => {
                let payload = RequestHandlerError::new(rejection.body_text(), "MALFORMED_REQUEST");
                return Err((rejection.status(), axum::Json(payload)));
            }
        };
        match inner.validate() {
            Ok(_) => Ok(Self(inner)),
            Err(e) => {
                let payload = RequestHandlerError::new(e.to_string(), "MALFORMED_REQUEST");
                Err((StatusCode::BAD_REQUEST, axum::Json(payload)))
            }
        }
    }
}

impl<T> Deref for Query<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> IntoResponse for Query<T>
where
    T: Serialize,
{
    fn into_response(self) -> axum::response::Response {
        axum::Json(self.0).into_response()
    }
}
