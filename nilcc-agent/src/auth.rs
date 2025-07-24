use crate::routes::Json;
use axum::body::Body;
use axum::http::header::AUTHORIZATION;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{extract::Request, response::Response};
use nilcc_agent_models::errors::RequestHandlerError;
use std::convert::Infallible;
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tower::{Layer, Service};

#[derive(Clone)]
pub(crate) struct AuthLayer {
    token: Arc<String>,
}

impl AuthLayer {
    pub(crate) fn new(token: String) -> Self {
        Self { token: Arc::new(token) }
    }
}

impl<S> Layer<S> for AuthLayer {
    type Service = AuthMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthMiddleware { inner, token: self.token.clone() }
    }
}

#[derive(Clone)]
pub(crate) struct AuthMiddleware<S> {
    inner: S,
    token: Arc<String>,
}

impl<S> Service<Request<Body>> for AuthMiddleware<S>
where
    S: Service<Request<Body>, Response = Response, Error = Infallible> + Send + Clone + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let mut inner = self.inner.clone();
        let token = self.token.clone();
        Box::pin(async move {
            if let Some(header) = req.headers().get(AUTHORIZATION) {
                if let Ok(value) = header.to_str() {
                    if value.strip_prefix("Bearer ") == Some(&token) {
                        return inner.call(req).await;
                    }
                }
            }

            let response = RequestHandlerError {
                error_code: "UNAUTHORIZED".into(),
                message: "invalid or missing bearer token".into(),
            };
            Ok((StatusCode::UNAUTHORIZED, Json(response)).into_response())
        })
    }
}
