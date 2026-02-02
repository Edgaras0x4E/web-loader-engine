use axum::{extract::State, Json};
use crate::models::{BrowserPoolStatus, HealthResponse};
use crate::AppState;

pub async fn health_handler(
    State(state): State<AppState>,
) -> Json<HealthResponse> {
    let browser_status = BrowserPoolStatus {
        available: state.browser_pool.available_slots(),
        total: state.browser_pool.total_slots(),
    };

    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        browser_pool: browser_status,
    })
}
