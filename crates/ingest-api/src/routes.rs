//! API route definitions and setup

use crate::{
    config::ApiConfig,
    docs::create_docs_router,
    graphql::create_graphql_router,
    handlers::{AppState, create_event, get_event, get_function, health_check, list_functions},
    rate_limit::{RateLimitConfig, RateLimitState, rate_limit_middleware},
    security::{
        create_cors_layer, input_validation_middleware, security_headers_middleware,
        sql_injection_detection_middleware,
    },
};
use axum::{
    Router, middleware,
    routing::{get, post},
};

/// Create the main API router
pub fn create_router(state: AppState, _config: &ApiConfig) -> Router {
    // Create rate limiting state
    let rate_limit_config = RateLimitConfig::default();
    let rate_limit_state = RateLimitState::new(rate_limit_config);

    // Create CORS layer
    let cors_config = crate::security::CorsConfig::default();
    let cors_layer = create_cors_layer(&cors_config);

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
        // GraphQL API
        .merge(create_graphql_router(state.clone()))
        // API Documentation
        .merge(create_docs_router())
        // Apply middleware layers (order matters!)
        .layer(cors_layer)
        .layer(middleware::from_fn(security_headers_middleware))
        .layer(middleware::from_fn(input_validation_middleware))
        .layer(middleware::from_fn(sql_injection_detection_middleware))
        .layer(middleware::from_fn_with_state(
            rate_limit_state,
            rate_limit_middleware,
        ))
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
        "features": [
            "rest_api",
            "graphql",
            "rate_limiting",
            "security_headers",
            "cors",
            "documentation"
        ],
        "endpoints": {
            "rest": "/",
            "graphql": "/graphql",
            "docs": "/docs",
            "openapi": "/api-docs/openapi.json"
        }
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

    #[tokio::test]
    async fn test_graphql_endpoint_exists() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        let app = create_test_app();

        // Test GET /graphql (playground)
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/graphql")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_docs_endpoint_exists() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::ServiceExt;

        let app = create_test_app();

        // Test GET /docs
        let response = app
            .oneshot(Request::builder().uri("/docs").body(Body::empty()).unwrap())
            .await
            .unwrap();
        // Should redirect to /docs/
        assert!(response.status().is_redirection());
    }

    #[tokio::test]
    async fn test_openapi_endpoint_exists() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        let app = create_test_app();

        // Test GET /api-docs/openapi.json
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api-docs/openapi.json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
