use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Authentication required")]
    Unauthorized,

    #[error("Invalid API key")]
    InvalidApiKey,

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Blocked URL: {0}")]
    BlockedUrl(String),

    #[error("Request timeout after {0} seconds")]
    Timeout(u64),

    #[error("Browser error: {0}")]
    BrowserError(String),

    #[error("Scraping error: {0}")]
    ScrapingError(String),

    #[error("Content extraction error: {0}")]
    ExtractionError(String),

    #[error("Markdown conversion error: {0}")]
    MarkdownError(String),

    #[error("Screenshot error: {0}")]
    ScreenshotError(String),

    #[error("Rate limit exceeded for domain: {0}")]
    RateLimitExceeded(String),

    #[error("Circuit breaker open for domain: {0}")]
    CircuitBreakerOpen(String),

    #[error("Too many domains requested: {0}")]
    TooManyDomains(usize),

    #[error("Internal server error: {0}")]
    Internal(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            AppError::InvalidApiKey => (StatusCode::UNAUTHORIZED, self.to_string()),
            AppError::InvalidUrl(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            AppError::BlockedUrl(_) => (StatusCode::FORBIDDEN, self.to_string()),
            AppError::Timeout(_) => (StatusCode::GATEWAY_TIMEOUT, self.to_string()),
            AppError::BrowserError(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            AppError::ScrapingError(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            AppError::ExtractionError(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            AppError::MarkdownError(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            AppError::ScreenshotError(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            AppError::RateLimitExceeded(_) => (StatusCode::TOO_MANY_REQUESTS, self.to_string()),
            AppError::CircuitBreakerOpen(_) => (StatusCode::SERVICE_UNAVAILABLE, self.to_string()),
            AppError::TooManyDomains(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            AppError::ConfigError(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            AppError::IoError(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        let body = Json(json!({
            "error": error_message,
            "code": status.as_u16()
        }));

        (status, body).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
