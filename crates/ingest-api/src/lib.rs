//! # ingest-api
//!
//! REST API layer for the Inngest durable functions platform.
//!
//! This crate provides a comprehensive HTTP API for interacting with the Inngest platform,
//! including event ingestion, function management, execution control, and monitoring.
//!
//! ## Features
//!
//! - **Event Ingestion**: REST endpoints for submitting and retrieving events
//! - **Function Management**: CRUD operations for functions and their configurations
//! - **Execution Control**: Start, stop, and monitor function executions
//! - **Authentication**: JWT-based authentication and authorization
//! - **Rate Limiting**: Built-in rate limiting and throttling
//! - **Monitoring**: Health checks and metrics endpoints
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use ingest_api::{ApiServer, ApiConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = ApiConfig::default();
//!     let server = ApiServer::new(config).await?;
//!     server.serve("0.0.0.0:3000").await?;
//!     Ok(())
//! }
//! ```

pub mod api_server;
pub mod config;
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod realtime;
pub mod routes;
pub mod types;

// Re-export public API
pub use api_server::ApiServer;
pub use config::ApiConfig;
pub use error::{ApiError, Result};
pub use types::{ApiResponse, ErrorResponse, PaginatedResponse};

// Re-export commonly used types
pub use ingest_core::{Event, Function, Step};
pub use ingest_execution::{ExecutionRequest, ExecutionResult};
