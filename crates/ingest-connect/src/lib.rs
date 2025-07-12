//! # ingest-connect
//!
//! Connection management and SDK integration for the Inngest platform.
//!
//! This crate provides WebSocket handling, SDK authentication protocols,
//! connection pooling and load balancing, and protocol negotiation.

pub mod auth;
pub mod config;
pub mod connection;
pub mod error;
pub mod manager;
pub mod protocol;
pub mod server;
pub mod types;

pub use auth::{AuthContext, AuthService, SdkCredentials};
pub use config::ConnectConfig;
pub use connection::{Connection, ConnectionInfo, ConnectionState};
pub use error::{ConnectError, Result};
pub use manager::{ConnectionManager, ConnectionPool};
pub use protocol::{Message, ProtocolNegotiator, ProtocolVersion};
pub use server::ConnectServer;
pub use types::{ClientInfo, ConnectionId, SessionId};

/// Re-export common types
pub use ingest_core::{Event, Function};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Test that all main types are properly exported
        let _config = ConnectConfig::default();
        let _error = ConnectError::Authentication("test".to_string());
    }
}
