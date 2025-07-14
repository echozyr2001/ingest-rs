//! # Inngest SDK
//!
//! The official Rust SDK for the Inngest durable functions platform.
//!
//! This SDK provides a comprehensive, type-safe interface for building durable functions
//! and interacting with the Inngest platform. It includes tools for function definition,
//! event publishing, local development, and testing.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use ingest_sdk::{IngestClient, ClientConfig, Event};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = ClientConfig::new("your-api-key");
//!     let client = IngestClient::new(config).await?;
//!     
//!     let event = Event::new("user.created")
//!         .data(serde_json::json!({"user_id": "123", "email": "user@example.com"}));
//!     
//!     let event_id = client.publish_event(event).await?;
//!     println!("Published event: {}", event_id);
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Function Definition
//!
//! ```rust,no_run
//! use ingest_sdk::{function, Event, StepContext, Result};
//!
//! #[function(trigger = "user.created")]
//! async fn welcome_user(event: Event, step: &mut StepContext) -> Result<()> {
//!     let user_id = event.data["user_id"].as_str().unwrap();
//!     
//!     step.run("send_email", || async {
//!         send_welcome_email(user_id).await
//!     }).await?;
//!     
//!     step.sleep("wait_24h", std::time::Duration::from_secs(24 * 60 * 60)).await?;
//!     
//!     step.run("send_followup", || async {
//!         send_followup_email(user_id).await
//!     }).await?;
//!     
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod error;
pub mod event;
pub mod function;
pub mod local;
pub mod testing;

// Re-export public API
pub use client::{ClientConfig, IngestClient};
pub use error::{Result, SdkError};
pub use event::{EventBuilder, EventPublisher};
pub use function::{Function, StepContext, StepContextTrait, StepOutput};
pub use local::LocalServer;
pub use testing::TestHarness;

// Re-export core types for convenience
pub use ingest_core::{Event, EventData, EventId, ExecutionId, FunctionId, Json};

// Note: Function macro will be implemented in a separate ingest-sdk-macros crate
// pub use ingest_sdk_macros::function;

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_sdk_exports() {
        // Test that all main exports are available
        let _config = ClientConfig::new("test-key");
        assert_eq!("test-key", _config.api_key());
    }
}
