use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::sync::Arc;
use tracing::warn;

#[derive(Clone)]
pub struct AuthLayer {
    api_key: Option<String>,
}

impl AuthLayer {
    pub fn new(api_key: Option<String>) -> Self {
        Self { api_key }
    }
}

pub async fn auth_middleware(
    auth: axum::extract::Extension<Arc<AuthLayer>>,
    request: Request,
    next: Next,
) -> Response {
    if request.uri().path() == "/health" {
        return next.run(request).await;
    }

    let Some(ref expected_key) = auth.api_key else {
        return next.run(request).await;
    };

    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());

    match auth_header {
        Some(header_value) => {
            let provided_key = header_value
                .strip_prefix("Bearer ")
                .unwrap_or(header_value);

            if provided_key == expected_key {
                next.run(request).await
            } else {
                warn!("Invalid API key provided");
                (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({
                        "error": "Invalid API key",
                        "code": 401
                    })),
                )
                    .into_response()
            }
        }
        None => {
            warn!("Missing Authorization header");
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": "Authorization header required",
                    "code": 401
                })),
            )
                .into_response()
        }
    }
}
