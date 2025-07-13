//! GraphQL schema definitions
//!
//! This module contains the GraphQL schema types and their implementations.

use async_graphql::{
    Enum, ID, InputValueError, InputValueResult, Scalar, ScalarType, Schema, SimpleObject,
    Value as GqlValue,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// Custom DateTime scalar for GraphQL
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DateTimeUtc(pub DateTime<Utc>);

#[Scalar]
impl ScalarType for DateTimeUtc {
    fn parse(value: GqlValue) -> InputValueResult<Self> {
        match value {
            GqlValue::String(s) => {
                let dt = DateTime::parse_from_rfc3339(&s)
                    .map_err(|_| InputValueError::custom("Invalid datetime format"))?
                    .with_timezone(&Utc);
                Ok(DateTimeUtc(dt))
            }
            _ => Err(InputValueError::expected_type(value)),
        }
    }

    fn to_value(&self) -> GqlValue {
        GqlValue::String(self.0.to_rfc3339())
    }
}

impl From<DateTime<Utc>> for DateTimeUtc {
    fn from(dt: DateTime<Utc>) -> Self {
        DateTimeUtc(dt)
    }
}

impl From<DateTimeUtc> for DateTime<Utc> {
    fn from(dt: DateTimeUtc) -> Self {
        dt.0
    }
}

/// GraphQL representation of an Event
#[derive(SimpleObject, Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    /// Unique event identifier
    pub id: ID,
    /// Event name/type
    pub name: String,
    /// Event data payload
    pub data: Value,
    /// User identifier
    pub user: Option<String>,
    /// Timestamp when event was created
    pub timestamp: DateTimeUtc,
    /// Event version
    pub version: Option<String>,
}

/// GraphQL representation of a Function
#[derive(SimpleObject, Clone, Debug, Serialize, Deserialize)]
pub struct Function {
    /// Unique function identifier
    pub id: ID,
    /// Function name
    pub name: String,
    /// Function description
    pub description: Option<String>,
    /// Function configuration
    pub config: FunctionConfig,
    /// Function triggers
    pub triggers: Vec<Trigger>,
    /// Function status
    pub status: FunctionStatus,
    /// Creation timestamp
    pub created_at: DateTimeUtc,
    /// Last update timestamp
    pub updated_at: DateTimeUtc,
}

/// Function configuration
#[derive(SimpleObject, Clone, Debug, Serialize, Deserialize)]
pub struct FunctionConfig {
    /// Function timeout in seconds
    pub timeout: i32,
    /// Maximum retry attempts
    pub max_retries: i32,
    /// Retry delay in seconds
    pub retry_delay: i32,
    /// Function runtime environment
    pub runtime: String,
}

/// Function trigger configuration
#[derive(SimpleObject, Clone, Debug, Serialize, Deserialize)]
pub struct Trigger {
    /// Trigger type
    pub trigger_type: TriggerType,
    /// Event pattern to match
    pub event_pattern: String,
    /// Trigger configuration
    pub config: Value,
}

/// Trigger type enumeration
#[derive(Enum, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Copy)]
pub enum TriggerType {
    Event,
    Schedule,
    Http,
}

/// Function status enumeration
#[derive(Enum, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Copy)]
pub enum FunctionStatus {
    Active,
    Inactive,
    Error,
}

/// Function run representation
#[derive(SimpleObject, Clone, Debug, Serialize, Deserialize)]
pub struct FunctionRun {
    /// Unique run identifier
    pub id: ID,
    /// Function identifier
    pub function_id: ID,
    /// Event that triggered the run
    pub event_id: Option<ID>,
    /// Run status
    pub status: RunStatus,
    /// Run output
    pub output: Option<Value>,
    /// Error message if failed
    pub error: Option<String>,
    /// Run start time
    pub started_at: DateTimeUtc,
    /// Run completion time
    pub completed_at: Option<DateTimeUtc>,
    /// Run duration in milliseconds
    pub duration_ms: Option<i32>,
}

/// Function run status enumeration
#[derive(Enum, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Copy)]
pub enum RunStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Pagination information
#[derive(SimpleObject, Clone, Debug)]
pub struct PageInfo {
    /// Whether there are more pages after this one
    pub has_next_page: bool,
    /// Whether there are pages before this one
    pub has_previous_page: bool,
    /// Cursor for the first item in this page
    pub start_cursor: Option<String>,
    /// Cursor for the last item in this page
    pub end_cursor: Option<String>,
}

/// Event edge type for pagination
#[derive(SimpleObject, Clone, Debug)]
pub struct EventEdge {
    /// Cursor for this edge
    pub cursor: String,
    /// The actual node
    pub node: Event,
}

/// Function edge type for pagination
#[derive(SimpleObject, Clone, Debug)]
pub struct FunctionEdge {
    /// Cursor for this edge
    pub cursor: String,
    /// The actual node
    pub node: Function,
}

/// Function run edge type for pagination
#[derive(SimpleObject, Clone, Debug)]
pub struct FunctionRunEdge {
    /// Cursor for this edge
    pub cursor: String,
    /// The actual node
    pub node: FunctionRun,
}

/// Event connection type for pagination
#[derive(SimpleObject, Clone, Debug)]
pub struct EventConnection {
    /// List of edges
    pub edges: Vec<EventEdge>,
    /// Pagination information
    pub page_info: PageInfo,
    /// Total count of items
    pub total_count: i32,
}

/// Function connection type for pagination
#[derive(SimpleObject, Clone, Debug)]
pub struct FunctionConnection {
    /// List of edges
    pub edges: Vec<FunctionEdge>,
    /// Pagination information
    pub page_info: PageInfo,
    /// Total count of items
    pub total_count: i32,
}

/// Function run connection type for pagination
#[derive(SimpleObject, Clone, Debug)]
pub struct FunctionRunConnection {
    /// List of edges
    pub edges: Vec<FunctionRunEdge>,
    /// Pagination information
    pub page_info: PageInfo,
    /// Total count of items
    pub total_count: i32,
}

/// Input type for creating events
#[derive(async_graphql::InputObject, Clone, Debug)]
pub struct CreateEventInput {
    /// Event name
    pub name: String,
    /// Event data
    pub data: Value,
    /// User identifier
    pub user: Option<String>,
    /// Event version
    pub version: Option<String>,
}

/// Input type for creating functions
#[derive(async_graphql::InputObject, Clone, Debug)]
pub struct CreateFunctionInput {
    /// Function name
    pub name: String,
    /// Function description
    pub description: Option<String>,
    /// Function configuration
    pub config: FunctionConfigInput,
    /// Function triggers
    pub triggers: Vec<TriggerInput>,
}

/// Input type for function configuration
#[derive(async_graphql::InputObject, Clone, Debug)]
pub struct FunctionConfigInput {
    /// Function timeout in seconds
    pub timeout: i32,
    /// Maximum retry attempts
    pub max_retries: i32,
    /// Retry delay in seconds
    pub retry_delay: i32,
    /// Function runtime environment
    pub runtime: String,
}

/// Input type for triggers
#[derive(async_graphql::InputObject, Clone, Debug)]
pub struct TriggerInput {
    /// Trigger type
    pub trigger_type: TriggerType,
    /// Event pattern to match
    pub event_pattern: String,
    /// Trigger configuration
    pub config: Value,
}

// Conversion functions from core types to GraphQL types
impl From<&ingest_core::Event> for Event {
    fn from(event: &ingest_core::Event) -> Self {
        Event {
            id: ID(event.id.to_string()),
            name: event.name.clone(),
            data: event.data.data.clone(),
            user: event.user.clone(),
            timestamp: DateTimeUtc(event.timestamp),
            version: event.version.clone(),
        }
    }
}

impl From<CreateEventInput> for ingest_core::Event {
    fn from(input: CreateEventInput) -> Self {
        ingest_core::Event::new(input.name, input.data)
            .user(input.user.unwrap_or_default())
            .version(input.version.unwrap_or_default())
    }
}

/// GraphQL schema type
pub type ApiSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

/// Root query type
pub struct QueryRoot;

/// Root mutation type  
pub struct MutationRoot;

use super::subscriptions::SubscriptionRoot;

/// Create the GraphQL schema
pub fn create_schema() -> ApiSchema {
    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot).finish()
}
