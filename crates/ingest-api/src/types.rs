//! API response types and utilities

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Standard API response wrapper
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    /// Response data
    pub data: T,

    /// Response metadata
    pub meta: ResponseMeta,
}

/// Response metadata
#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseMeta {
    /// Request ID for tracing
    pub request_id: Uuid,

    /// Response timestamp
    pub timestamp: DateTime<Utc>,

    /// API version
    pub version: String,
}

/// Paginated response wrapper
#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    /// Response data items
    pub data: Vec<T>,

    /// Pagination information
    pub pagination: Pagination,

    /// Response metadata
    pub meta: ResponseMeta,
}

/// Pagination information
#[derive(Debug, Serialize, Deserialize)]
pub struct Pagination {
    /// Current page number (1-based)
    pub page: u32,

    /// Number of items per page
    pub per_page: u32,

    /// Total number of items
    pub total: u64,

    /// Total number of pages
    pub total_pages: u32,

    /// Whether there is a next page
    pub has_next: bool,

    /// Whether there is a previous page
    pub has_prev: bool,
}

/// Error response (re-exported from error module)
pub use crate::error::ErrorResponse;

/// Event creation request
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateEventRequest {
    /// Event name
    pub name: String,

    /// Event data payload
    pub data: serde_json::Value,

    /// Optional user identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    /// Optional timestamp (defaults to now)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ts: Option<i64>,

    /// Optional event ID (auto-generated if not provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// Event response
#[derive(Debug, Serialize, Deserialize)]
pub struct EventResponse {
    /// Event ID
    pub id: String,

    /// Event name
    pub name: String,

    /// Event data
    pub data: serde_json::Value,

    /// User identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    /// Event timestamp
    pub timestamp: DateTime<Utc>,

    /// Event status
    pub status: String,
}

/// Function execution request
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecuteFunctionRequest {
    /// Function ID
    pub function_id: String,

    /// Event that triggered the execution
    pub event: serde_json::Value,

    /// Execution context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,

    /// Optional execution ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
}

/// Function execution response
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutionResponse {
    /// Execution ID
    pub id: String,

    /// Function ID
    pub function_id: String,

    /// Execution status
    pub status: String,

    /// Started timestamp
    pub started_at: DateTime<Utc>,

    /// Completed timestamp (if finished)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,

    /// Execution result (if completed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,

    /// Error information (if failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Function information response
#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionResponse {
    /// Function ID
    pub id: String,

    /// Function name
    pub name: String,

    /// Function configuration
    pub config: serde_json::Value,

    /// Function triggers
    pub triggers: Vec<serde_json::Value>,

    /// Created timestamp
    pub created_at: DateTime<Utc>,

    /// Updated timestamp
    pub updated_at: DateTime<Utc>,

    /// Function status
    pub status: String,
}

/// Health check response
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Service status
    pub status: String,

    /// Service version
    pub version: String,

    /// Uptime in seconds
    pub uptime: u64,

    /// Component health status
    pub components: std::collections::HashMap<String, ComponentHealth>,
}

/// Component health information
#[derive(Debug, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Component status
    pub status: String,

    /// Last check timestamp
    pub last_check: DateTime<Utc>,

    /// Additional details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl<T> ApiResponse<T> {
    /// Create a new API response
    pub fn new(data: T) -> Self {
        Self {
            data,
            meta: ResponseMeta::new(),
        }
    }

    /// Create a new API response with custom request ID
    pub fn with_request_id(data: T, request_id: Uuid) -> Self {
        Self {
            data,
            meta: ResponseMeta::with_request_id(request_id),
        }
    }
}

impl<T> PaginatedResponse<T> {
    /// Create a new paginated response
    pub fn new(data: Vec<T>, page: u32, per_page: u32, total: u64) -> Self {
        let total_pages = ((total as f64) / (per_page as f64)).ceil() as u32;
        let has_next = page < total_pages;
        let has_prev = page > 1;

        Self {
            data,
            pagination: Pagination {
                page,
                per_page,
                total,
                total_pages,
                has_next,
                has_prev,
            },
            meta: ResponseMeta::new(),
        }
    }
}

impl ResponseMeta {
    /// Create new response metadata
    pub fn new() -> Self {
        Self {
            request_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Create new response metadata with custom request ID
    pub fn with_request_id(request_id: Uuid) -> Self {
        Self {
            request_id,
            timestamp: Utc::now(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

impl Default for ResponseMeta {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_api_response_creation() {
        let data = "test data";
        let response = ApiResponse::new(data);

        assert_eq!(response.data, "test data");
        assert_eq!(response.meta.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn test_api_response_with_request_id() {
        let data = "test data";
        let request_id = Uuid::new_v4();
        let response = ApiResponse::with_request_id(data, request_id);

        assert_eq!(response.data, "test data");
        assert_eq!(response.meta.request_id, request_id);
    }

    #[test]
    fn test_paginated_response() {
        let data = vec!["item1", "item2", "item3"];
        let response = PaginatedResponse::new(data.clone(), 1, 10, 25);

        assert_eq!(response.data, data);
        assert_eq!(response.pagination.page, 1);
        assert_eq!(response.pagination.per_page, 10);
        assert_eq!(response.pagination.total, 25);
        assert_eq!(response.pagination.total_pages, 3);
        assert!(response.pagination.has_next);
        assert!(!response.pagination.has_prev);
    }

    #[test]
    fn test_pagination_calculations() {
        // Test last page
        let response = PaginatedResponse::new(vec!["item"], 3, 10, 25);
        assert!(!response.pagination.has_next);
        assert!(response.pagination.has_prev);

        // Test middle page
        let response = PaginatedResponse::new(vec!["item"], 2, 10, 25);
        assert!(response.pagination.has_next);
        assert!(response.pagination.has_prev);

        // Test single page
        let response = PaginatedResponse::new(vec!["item"], 1, 10, 5);
        assert!(!response.pagination.has_next);
        assert!(!response.pagination.has_prev);
        assert_eq!(response.pagination.total_pages, 1);
    }

    #[test]
    fn test_response_meta_default() {
        let meta = ResponseMeta::default();

        assert_eq!(meta.version, env!("CARGO_PKG_VERSION"));
        // request_id should be different each time
        let meta2 = ResponseMeta::default();
        assert_ne!(meta.request_id, meta2.request_id);
    }
}
