use crate::auth::AuthLayer;
use crate::config::ResourceLimitsConfig;
use crate::services::workload::WorkloadService;
use axum::extract::Request;
use axum::extract::{rejection::JsonRejection, FromRequest};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::Router;
use convert_case::{Case, Casing};
use serde::Serialize;
use std::ops::Deref;
use std::sync::Arc;
use tower::ServiceBuilder;

pub(crate) mod workloads;

#[derive(Clone)]
pub struct Services {
    pub workload: Arc<dyn WorkloadService>,
}

#[derive(Clone)]
pub struct AppState {
    pub services: Services,
    pub resource_limits: ResourceLimitsConfig,
}

pub fn build_router(state: AppState, token: String) -> Router {
    Router::new().nest(
        "/api/v1",
        Router::new()
            .route("/workloads/create", post(workloads::create::handler))
            .route("/workloads/delete", post(workloads::delete::handler))
            .with_state(state)
            .layer(ServiceBuilder::new().layer(AuthLayer::new(token))),
    )
}

/// An error when handling a request.
#[derive(Debug, Serialize)]
pub struct RequestHandlerError {
    /// A descriptive message about the error that was encountered.
    pub(crate) message: String,

    /// The error code.
    pub(crate) error_code: String,
}

impl RequestHandlerError {
    pub(crate) fn new(message: impl Into<String>, error_code: impl AsRef<str>) -> Self {
        let error_code = error_code.as_ref().to_case(Case::UpperSnake);
        Self { message: message.into(), error_code }
    }
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
