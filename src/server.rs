use anyhow::Result;
use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};

use crate::config::Config;

/// Trait to allow mocking the shutdown behavior in tests
pub trait ShutdownProvider: Send + Sync {
    fn shutdown(&self) -> Result<()>;
}

pub struct RealShutdownProvider;

impl ShutdownProvider for RealShutdownProvider {
    fn shutdown(&self) -> Result<()> {
        system_shutdown::shutdown().map_err(|e| anyhow::anyhow!(e))
    }
}

pub struct AppState {
    pub config: Config,
    pub shutdown_provider: Box<dyn ShutdownProvider>,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/shutdown", post(shutdown_handler))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub async fn run_server(config: Config) -> Result<()> {
    let port = config.port;
    let bind_address = config.bind_address.clone();
    let state = Arc::new(AppState {
        config,
        shutdown_provider: Box::new(RealShutdownProvider),
    });

    let app = create_router(state);

    let addr_str = format!("{}:{}", bind_address, port);
    let addr: SocketAddr = addr_str.parse()?;
    info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

async fn shutdown_handler(
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let addr = connect_info.map(|ci| ci.0.ip().to_string()).unwrap_or_else(|| "unknown".to_string());
    let auth_header = headers.get("X-Auth-Token");

    match auth_header {
        Some(token) if token == state.config.auth_token.as_str() => {
            info!(
                "Shutdown requested from {}. Authorization successful.",
                addr
            );

            // Attempt shutdown using the provider
            match state.shutdown_provider.shutdown() {
                Ok(_) => {
                    info!("Shutdown signal sent successfully.");
                    (StatusCode::OK, "Shutdown initiated").into_response()
                }
                Err(e) => {
                    error!("Failed to initiate shutdown: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, "Failed to shutdown").into_response()
                }
            }
        }
        _ => {
            warn!(
                "Unauthorized shutdown attempt from {}. Invalid or missing token.",
                addr
            );
            (StatusCode::UNAUTHORIZED, "Unauthorized").into_response()
        }
    }
}
