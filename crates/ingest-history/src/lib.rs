//! # Inngest History
//!
//! Comprehensive audit system with historical data queries and retention policies
//! for the Inngest durable functions platform.
//!
//! This crate provides a robust audit trail system that tracks all platform activities,
//! enables efficient historical data queries, and implements configurable data retention
//! policies with automated cleanup.
//!
//! ## Features
//!
//! - **Audit Trail**: Comprehensive tracking of all platform activities
//! - **Historical Queries**: Efficient querying of historical data with filtering and pagination
//! - **Retention Policies**: Configurable data lifecycle management with automated cleanup
//! - **Archive System**: Data archival and export capabilities
//! - **Performance Optimized**: Efficient indexing and query optimization
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use ingest_history::{AuditTracker, RetentionManager};
//! use ingest_history::audit::{AuditEventType, ResourceType, Actor};
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Note: This is a simplified example for documentation
//!     // In practice, you would initialize storage with proper configuration
//!     
//!     // Example of tracking an audit event
//!     // let tracker = AuditTracker::new(storage).await?;
//!     // tracker.track_event(
//!     //     AuditEventType::Created,
//!     //     ResourceType::Function,
//!     //     "func_123",
//!     //     Some(Actor::User("user_456".to_string())),
//!     //     serde_json::json!({"name": "welcome_user"}),
//!     //     None,
//!     // ).await?;
//!     
//!     Ok(())
//! }
//! ```

pub mod archive;
pub mod audit;
pub mod error;
pub mod query;
pub mod retention;
pub mod storage;

// Re-export public API
pub use archive::ArchiveManager;
pub use audit::{Actor, AuditEvent, AuditEventType, AuditTracker, ChangeSet, ResourceType};
pub use error::{HistoryError, Result};
pub use query::AuditQueryService;
pub use retention::RetentionManager;

// Re-export core types for convenience
pub use ingest_core::{DateTime, Duration, Json};

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_history_exports() {
        // Test that all main exports are available
        let _event_type = AuditEventType::Created;
        let _resource_type = ResourceType::Function;
        assert_eq!(_event_type.to_string(), "Created");
    }
}
