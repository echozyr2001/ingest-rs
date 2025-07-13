//! GraphQL type definitions and utilities
//!
//! This module contains additional GraphQL types and utility functions.

use crate::graphql::schema::DateTimeUtc;
use async_graphql::{Enum, ID, InputObject, SimpleObject};

/// GraphQL scalar for JSON values
pub type JSON = serde_json::Value;

/// Pagination arguments for GraphQL connections
#[derive(InputObject, Clone, Debug)]
pub struct PaginationArgs {
    /// Number of items to return
    pub first: Option<i32>,
    /// Cursor to start after
    pub after: Option<String>,
    /// Number of items to return (from end)
    pub last: Option<i32>,
    /// Cursor to end before
    pub before: Option<String>,
}

/// Sort direction enumeration
#[derive(Enum, Clone, Debug, PartialEq, Eq, Copy)]
pub enum SortDirection {
    Asc,
    Desc,
}

/// Event sort fields
#[derive(Enum, Clone, Debug, PartialEq, Eq, Copy)]
pub enum EventSortField {
    Timestamp,
    Name,
    User,
}

/// Event sort input
#[derive(InputObject, Clone, Debug)]
pub struct EventSort {
    /// Field to sort by
    pub field: EventSortField,
    /// Sort direction
    pub direction: SortDirection,
}

/// Function sort fields
#[derive(Enum, Clone, Debug, PartialEq, Eq, Copy)]
pub enum FunctionSortField {
    Name,
    CreatedAt,
    UpdatedAt,
    Status,
}

/// Function sort input
#[derive(InputObject, Clone, Debug)]
pub struct FunctionSort {
    /// Field to sort by
    pub field: FunctionSortField,
    /// Sort direction
    pub direction: SortDirection,
}

/// Date range filter
#[derive(InputObject, Clone, Debug)]
pub struct DateRange {
    /// Start date (inclusive)
    pub start: DateTimeUtc,
    /// End date (inclusive)
    pub end: DateTimeUtc,
}

/// Event filter input
#[derive(InputObject, Clone, Debug)]
pub struct EventFilter {
    /// Filter by event name pattern
    pub name_pattern: Option<String>,
    /// Filter by user
    pub user: Option<String>,
    /// Filter by date range
    pub date_range: Option<DateRange>,
    /// Filter by version
    pub version: Option<String>,
}

/// Function filter input
#[derive(InputObject, Clone, Debug)]
pub struct FunctionFilter {
    /// Filter by function name pattern
    pub name_pattern: Option<String>,
    /// Filter by status
    pub status: Option<crate::graphql::schema::FunctionStatus>,
    /// Filter by date range
    pub date_range: Option<DateRange>,
}

/// Function run filter input
#[derive(InputObject, Clone, Debug)]
pub struct FunctionRunFilter {
    /// Filter by function ID
    pub function_id: Option<ID>,
    /// Filter by event ID
    pub event_id: Option<ID>,
    /// Filter by status
    pub status: Option<crate::graphql::schema::RunStatus>,
    /// Filter by date range
    pub date_range: Option<DateRange>,
}

/// API usage statistics
#[derive(SimpleObject, Clone, Debug)]
pub struct ApiUsageStats {
    /// Total number of events processed
    pub total_events: i64,
    /// Total number of function runs
    pub total_runs: i64,
    /// Number of successful runs
    pub successful_runs: i64,
    /// Number of failed runs
    pub failed_runs: i64,
    /// Average run duration in milliseconds
    pub avg_duration_ms: f64,
    /// Statistics timestamp
    pub timestamp: DateTimeUtc,
}

/// System health status
#[derive(Enum, Clone, Debug, PartialEq, Eq, Copy)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// System health information
#[derive(SimpleObject, Clone, Debug)]
pub struct SystemHealth {
    /// Overall health status
    pub status: HealthStatus,
    /// Health check timestamp
    pub timestamp: DateTimeUtc,
    /// Additional health details
    pub details: JSON,
}

/// Audit log entry
#[derive(SimpleObject, Clone, Debug)]
pub struct AuditLogEntry {
    /// Entry ID
    pub id: ID,
    /// Action performed
    pub action: String,
    /// User who performed the action
    pub user: Option<String>,
    /// Resource affected
    pub resource: String,
    /// Resource ID
    pub resource_id: String,
    /// Action timestamp
    pub timestamp: DateTimeUtc,
    /// Additional metadata
    pub metadata: JSON,
}

/// Webhook delivery status
#[derive(Enum, Clone, Debug, PartialEq, Eq, Copy)]
pub enum WebhookStatus {
    Pending,
    Delivered,
    Failed,
    Retrying,
}

/// Webhook delivery attempt
#[derive(SimpleObject, Clone, Debug)]
pub struct WebhookDelivery {
    /// Delivery ID
    pub id: ID,
    /// Webhook URL
    pub url: String,
    /// HTTP status code
    pub status_code: Option<i32>,
    /// Delivery status
    pub status: WebhookStatus,
    /// Response body
    pub response: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
    /// Attempt timestamp
    pub timestamp: DateTimeUtc,
    /// Retry count
    pub retry_count: i32,
}

/// Rate limit information
#[derive(SimpleObject, Clone, Debug)]
pub struct RateLimitInfo {
    /// Current limit
    pub limit: i32,
    /// Remaining requests
    pub remaining: i32,
    /// Reset timestamp
    pub reset_at: DateTimeUtc,
    /// Rate limit window in seconds
    pub window_seconds: i32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_pagination_args() {
        let args = PaginationArgs {
            first: Some(10),
            after: Some("cursor_123".to_string()),
            last: None,
            before: None,
        };

        assert_eq!(args.first, Some(10));
        assert_eq!(args.after, Some("cursor_123".to_string()));
    }

    #[test]
    fn test_event_sort() {
        let sort = EventSort {
            field: EventSortField::Timestamp,
            direction: SortDirection::Desc,
        };

        assert_eq!(sort.field, EventSortField::Timestamp);
        assert_eq!(sort.direction, SortDirection::Desc);
    }

    #[test]
    fn test_health_status() {
        let health = SystemHealth {
            status: HealthStatus::Healthy,
            timestamp: DateTimeUtc(chrono::Utc::now()),
            details: serde_json::json!({"uptime": "99.9%"}),
        };

        assert_eq!(health.status, HealthStatus::Healthy);
    }
}
