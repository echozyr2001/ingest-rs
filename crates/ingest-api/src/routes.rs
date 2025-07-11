//! API route definitions and setup

use crate::{
    config::ApiConfig,
    handlers::{AppState, create_event, get_event, get_function, health_check, list_functions},
};
use axum::{
    Router,
    routing::{get, post},
};

/// Create the main API router
pub fn create_router(state: AppState, _config: &ApiConfig) -> Router {
    Router::new()
        // Health and status endpoints
        .route("/health", get(health_check))
        .route("/status", get(health_check)) // Alias for health
        // Event endpoints
        .route("/events", post(create_event))
        .route("/events/{event_id}", get(get_event))
        // Function endpoints
        .route("/functions", get(list_functions))
        .route("/functions/{function_id}", get(get_function))
        // Utility endpoints
        .route("/ping", get(ping))
        .route("/version", get(version))
        // State for all routes
        .with_state(state)
}

/// Simple ping endpoint
async fn ping() -> &'static str {
    "pong"
}

/// Version information endpoint
async fn version() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "name": env!("CARGO_PKG_NAME"),
        "description": env!("CARGO_PKG_DESCRIPTION"),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn create_test_app() -> axum::Router {
        let app_state = AppState {
            start_time: Instant::now(),
        };

        let config = ApiConfig::default();
        create_router(app_state, &config)
    }

    #[tokio::test]
    async fn test_ping_endpoint() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        let app = create_test_app();
        let response = app
            .oneshot(Request::builder().uri("/ping").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_version_endpoint() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        let app = create_test_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/version")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        let app = create_test_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_function_endpoints_exist() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        let app = create_test_app();

        // Test GET /functions
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/functions")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
