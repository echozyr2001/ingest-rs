//! HTTP handlers for API endpoints

use crate::{
    Result,
    types::{ApiResponse, ComponentHealth, CreateEventRequest, EventResponse, HealthResponse},
};
use axum::{
    extract::{Path, State},
    response::Json,
};
use chrono::Utc;
use ingest_core::Event;
use serde::Deserialize;
use std::{collections::HashMap, time::Instant};
use tracing::info;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub start_time: Instant,
}

/// Query parameters for pagination
#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_per_page")]
    pub per_page: u32,
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    20
}

/// Health check endpoint
pub async fn health_check(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<HealthResponse>>> {
    let uptime = state.start_time.elapsed().as_secs();

    // Check component health
    let mut components = HashMap::new();

    // Check event processor health
    components.insert(
        "event_processor".to_string(),
        ComponentHealth {
            status: "healthy".to_string(),
            last_check: Utc::now(),
            details: None,
        },
    );

    // Check execution engine health
    components.insert(
        "execution_engine".to_string(),
        ComponentHealth {
            status: "healthy".to_string(),
            last_check: Utc::now(),
            details: None,
        },
    );

    let health = HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime,
        components,
    };

    Ok(Json(ApiResponse::new(health)))
}

/// Create a new event
pub async fn create_event(
    Json(request): Json<CreateEventRequest>,
) -> Result<Json<ApiResponse<EventResponse>>> {
    info!(
        event_name = %request.name,
        "Creating new event"
    );

    // Create event
    let event = Event::new(&request.name, request.data.clone());

    // Convert to response format
    let response = EventResponse {
        id: event.id.to_string(),
        name: event.name.clone(),
        data: request.data,
        user: request.user,
        timestamp: event.timestamp,
        status: "processed".to_string(),
    };

    Ok(Json(ApiResponse::new(response)))
}

/// Get an event by ID
pub async fn get_event(Path(event_id): Path<String>) -> Result<Json<ApiResponse<EventResponse>>> {
    info!(event_id = %event_id, "Getting event");

    // Return a mock response
    let response = EventResponse {
        id: event_id.clone(),
        name: "mock.event".to_string(),
        data: serde_json::json!({"mock": true}),
        user: Some("mock-user".to_string()),
        timestamp: Utc::now(),
        status: "processed".to_string(),
    };

    Ok(Json(ApiResponse::new(response)))
}

/// List functions
pub async fn list_functions() -> Result<Json<ApiResponse<Vec<serde_json::Value>>>> {
    info!("Listing functions");

    // Return mock data
    let functions = vec![
        serde_json::json!({
            "id": "email-sender",
            "name": "Email Sender",
            "status": "active"
        }),
        serde_json::json!({
            "id": "notification-handler",
            "name": "Notification Handler",
            "status": "active"
        }),
    ];

    Ok(Json(ApiResponse::new(functions)))
}

/// Get a function by ID
pub async fn get_function(
    Path(function_id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>> {
    info!(function_id = %function_id, "Getting function");

    // Return mock data
    let response = serde_json::json!({
        "id": function_id,
        "name": "Mock Function",
        "status": "active"
    });

    Ok(Json(ApiResponse::new(response)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_default_pagination() {
        assert_eq!(default_page(), 1);
        assert_eq!(default_per_page(), 20);
    }

    #[test]
    fn test_pagination_query_defaults() {
        let query: PaginationQuery = serde_json::from_str("{}").unwrap();
        assert_eq!(query.page, 1);
        assert_eq!(query.per_page, 20);
    }
}
