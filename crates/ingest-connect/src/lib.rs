//! # ingest-connect
//!
//! Connection management and SDK integration for the Inngest platform.
//!
//! This crate provides WebSocket handling, SDK authentication protocols,
//! connection pooling and load balancing, and protocol negotiation.//!
//! ## Real-time Features
//!
//! This crate now includes real-time status updates and event streaming:
//! - Live execution status broadcasting
//! - WebSocket event streaming
//! - Real-time monitoring capabilities
//! - Performance dashboard integration

pub mod auth;
pub mod config;
pub mod connection;
pub mod error;
pub mod manager;
pub mod protocol;
pub mod server;
pub mod types;

// Real-time features
pub mod monitoring;
pub mod realtime;
pub mod streaming;

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

// Real-time features exports
pub use monitoring::{
    AlertLevel, DashboardUpdate, LiveMetrics, LiveMonitor, MemoryUsage, NetworkMetrics,
    PerformanceAnalytics, SystemLoad, SystemStatus,
};
pub use realtime::{StatusBroadcaster, StatusFilter, StatusSubscription, StatusUpdate};
pub use streaming::{EventStreamer, StreamConfig, StreamConnection, StreamEvent, StreamFilter};

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
