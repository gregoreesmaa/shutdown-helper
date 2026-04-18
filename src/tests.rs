use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use std::sync::Arc;
use tower::ServiceExt; // for `oneshot`

use crate::config::Config;
use crate::server::{create_router, AppState, ShutdownProvider};

struct MockShutdownProvider;
impl ShutdownProvider for MockShutdownProvider {
    fn shutdown(&self) -> anyhow::Result<()> {
        // Just return OK for tests
        Ok(())
    }
}

#[tokio::test]
async fn test_shutdown_authorized() {
    let config = Config {
        port: 8080,
        bind_address: "127.0.0.1".to_string(),
        auth_token: "test-token".to_string(),
        log_dir: "test-logs".to_string(),
    };

    let state = Arc::new(AppState {
        config,
        shutdown_provider: Box::new(MockShutdownProvider),
    });

    let app = create_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/shutdown")
                .header("X-Auth-Token", "test-token")
                // Add ConnectInfo mock or similar if needed, but for oneshot it's usually not strictly required if not used in middleware that enforces it
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_shutdown_unauthorized() {
    let config = Config {
        port: 8080,
        bind_address: "127.0.0.1".to_string(),
        auth_token: "test-token".to_string(),
        log_dir: "test-logs".to_string(),
    };

    let state = Arc::new(AppState {
        config,
        shutdown_provider: Box::new(MockShutdownProvider),
    });

    let app = create_router(state);

    // Wrong token
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/shutdown")
                .header("X-Auth-Token", "wrong-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Missing token
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/shutdown")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[test]
fn test_config_serialization() {
    let config = Config {
        port: 1234,
        bind_address: "127.0.0.1".to_string(),
        auth_token: "abc".to_string(),
        log_dir: "logs".to_string(),
    };

    let toml = toml::to_string(&config).unwrap();
    assert!(toml.contains("port = 1234"));
    assert!(toml.contains("auth_token = \"abc\""));
}
