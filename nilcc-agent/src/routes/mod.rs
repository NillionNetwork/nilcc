#![allow(clippy::disallowed_types)]

use crate::auth::AuthLayer;
use crate::clients::cvm_agent::CvmAgentClient;
use crate::config::ResourceLimitsConfig;
use crate::services::upgrade::UpgradeService;
use crate::services::workload::WorkloadService;
use axum::Router;
use axum::extract::rejection::QueryRejection;
use axum::extract::{FromRequest, rejection::JsonRejection};
use axum::extract::{FromRequestParts, Request};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use nilcc_agent_models::errors::RequestHandlerError;
use serde::Serialize;
use std::ops::Deref;
use std::sync::Arc;
use tower::ServiceBuilder;
use validator::{Validate, ValidationErrors, ValidationErrorsKind};

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
}

pub fn build_router(state: AppState, token: String) -> Router {
    Router::new().route("/health", get(health)).nest(
        "/api/v1",
        Router::new()
            .nest(
                "/system",
                Router::new()
                    .route("/artifacts/install", post(system::artifacts::install::handler))
                    .route("/artifacts/versions", get(system::artifacts::versions::handler))
                    .route("/artifacts/changelog", get(system::artifacts::changelog::handler))
                    .route("/artifacts/cleanup", post(system::artifacts::cleanup::handler))
                    .route("/agent/upgrade", post(system::agent::upgrade::handler))
                    .route("/agent/version", get(system::agent::version::handler)),
            )
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
                let payload = RequestHandlerError::new(e.to_string_pretty(), "MALFORMED_REQUEST");
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
                let payload = RequestHandlerError::new(e.to_string_pretty(), "MALFORMED_REQUEST");
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

trait PrettyPrintError {
    fn to_string_pretty(&self) -> String {
        self.to_string_pretty_with_fields(vec![])
    }

    fn to_string_pretty_with_fields(&self, fields: Vec<&str>) -> String;
}

impl PrettyPrintError for ValidationErrors {
    fn to_string_pretty_with_fields(&self, fields: Vec<&str>) -> String {
        let mut output_errors = Vec::new();
        for (field, errors) in &self.0 {
            let mut fields = fields.clone();
            fields.push(field);
            output_errors.push(errors.to_string_pretty_with_fields(fields));
        }
        output_errors.join(", ")
    }
}

impl PrettyPrintError for ValidationErrorsKind {
    fn to_string_pretty_with_fields(&self, fields: Vec<&str>) -> String {
        match self {
            ValidationErrorsKind::Struct(errors) => errors.to_string_pretty_with_fields(fields),
            ValidationErrorsKind::List(errors) => {
                let mut output_errors = Vec::new();
                for (index, errors) in errors {
                    let mut fields = fields.clone();
                    let last = match fields.last() {
                        Some(last) => format!("{last}[{index}]"),
                        None => format!("[{index}]"),
                    };
                    fields.pop();
                    fields.push(&last);
                    output_errors.push(errors.to_string_pretty_with_fields(fields));
                }
                output_errors.join(", ")
            }
            ValidationErrorsKind::Field(errors) => {
                let field = fields.join(".");
                let errors = errors
                    .iter()
                    .map(|e| match e.code.as_ref() {
                        "range" => "value outside of expected range",
                        "regex" => "does not match expected format",
                        _ => e.code.as_ref(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("'{field}' {errors}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use validator::ValidationError;

    fn no_dots(value: &str) -> Result<(), ValidationError> {
        if value.contains(".") { Err(ValidationError::new("can't contain '.'")) } else { Ok(()) }
    }

    #[derive(Validate)]
    struct OuterModel {
        #[validate(range(min = 1))]
        number: u32,

        #[validate(nested)]
        list: Vec<InnerModel>,

        #[validate(nested)]
        inner: InnerModel,

        #[validate(custom(function = "no_dots"))]
        string: &'static str,
    }

    #[derive(Validate)]
    struct InnerModel {
        #[validate(range(min = 1))]
        number: u32,
    }

    #[rstest]
    #[case::number(
        OuterModel{ number: 0, list: vec![], inner: InnerModel { number: 1 }, string: "" },
        "'number' value outside of expected range"
    )]
    #[case::list(
        OuterModel{ number: 1, list: vec![InnerModel{ number: 0 }], inner: InnerModel { number: 1 }, string: "" },
        "'list[0].number' value outside of expected range"
    )]
    #[case::inner(
        OuterModel{ number: 1, list: vec![], inner: InnerModel { number: 0 }, string: "" },
        "'inner.number' value outside of expected range"
    )]
    #[case::custom(
        OuterModel{ number: 1, list: vec![], inner: InnerModel { number: 1 }, string: "a dot ." },
        "'string' can't contain '.'"
    )]
    fn validate_error_format(#[case] model: OuterModel, #[case] expected: &str) {
        let err = model.validate().expect_err("not an error");
        assert_eq!(err.to_string_pretty(), expected);
    }
}
