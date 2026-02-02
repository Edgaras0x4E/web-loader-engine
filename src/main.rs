mod config;
mod error;
mod middleware;
mod models;
mod routes;
mod services;

use axum::{
    middleware as axum_middleware,
    routing::{get, post},
    Extension, Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use config::Config;
use middleware::{auth_middleware, AuthLayer};
use routes::{batch_load_handler, health_handler, load_handler, openwebui_handler};
use services::{
    BrowserPool, CacheService, ConverterService, ScreenshotService, SecurityService,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub browser_pool: Arc<BrowserPool>,
    pub converter: Arc<ConverterService>,
    pub cache: Arc<CacheService>,
    pub security: Arc<SecurityService>,
    pub screenshot_service: Arc<ScreenshotService>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,tower_http=debug")),
        )
        .init();

    info!("Starting Web Loader Engine v{}", env!("CARGO_PKG_VERSION"));

    let config = Config::from_env()?;
    info!("Configuration loaded");
    info!("  Port: {}", config.api_port);
    info!("  Browser pool size: {}", config.browser_pool_size);

    info!("Initializing services...");

    let browser_pool = Arc::new(BrowserPool::new(config.clone()).await?);
    info!("Browser pool initialized");

    let converter = Arc::new(ConverterService::new(config.clone()));
    info!("Converter service initialized");

    let cache = Arc::new(CacheService::new(config.cache_ttl));
    info!("Cache service initialized");

    let security = Arc::new(SecurityService::new(config.clone()));
    info!("Security service initialized");

    let screenshot_service = Arc::new(ScreenshotService::new(&config));
    screenshot_service.initialize().await?;
    info!("Screenshot service initialized");

    let state = AppState {
        config: config.clone(),
        browser_pool,
        converter,
        cache,
        security,
        screenshot_service,
    };

    let auth_layer = Arc::new(AuthLayer::new(config.api_key.clone()));

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/load", post(load_handler))
        .route("/load/batch", post(batch_load_handler))
        .route("/", post(openwebui_handler))
        .with_state(state)
        .layer(axum_middleware::from_fn(auth_middleware))
        .layer(Extension(auth_layer))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], config.api_port));
    info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("Server shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, initiating shutdown...");
        },
        _ = terminate => {
            info!("Received terminate signal, initiating shutdown...");
        },
    }
}
