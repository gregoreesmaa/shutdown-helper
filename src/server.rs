use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::config::Config;

pub async fn run_server(config: Arc<Config>) {
    let app = Router::new()
        .route("/shutdown", post(shutdown_handler))
        .with_state(config.clone());

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

async fn shutdown_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    State(config): State<Arc<Config>>,
) -> impl IntoResponse {
    let auth_header = headers.get("X-Auth-Token");

    match auth_header {
        Some(token) if token == config.auth_token.as_str() => {
            info!(
                "Shutdown requested from {}. Authorization successful.",
                addr.ip()
            );

            // Graceful shutdown attempt
            match system_shutdown::shutdown() {
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
                addr.ip()
            );
            (StatusCode::UNAUTHORIZED, "Unauthorized").into_response()
        }
    }
}
