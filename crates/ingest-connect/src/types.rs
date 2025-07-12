//! Common types for the connect system

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Unique identifier for a connection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConnectionId(pub Uuid);

impl ConnectionId {
    /// Generate a new connection ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ConnectionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for ConnectionId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<ConnectionId> for Uuid {
    fn from(id: ConnectionId) -> Self {
        id.0
    }
}

/// Unique identifier for a session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub Uuid);

impl SessionId {
    /// Generate a new session ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for SessionId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<SessionId> for Uuid {
    fn from(id: SessionId) -> Self {
        id.0
    }
}

/// Information about a connected client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    /// Client identifier (SDK name, version, etc.)
    pub client_id: String,

    /// SDK version
    pub sdk_version: String,

    /// Programming language/runtime
    pub runtime: String,

    /// Platform information
    pub platform: String,

    /// User agent string
    pub user_agent: Option<String>,

    /// Client IP address
    pub ip_address: String,

    /// Additional metadata
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

impl ClientInfo {
    /// Create a new client info instance
    pub fn new(
        client_id: String,
        sdk_version: String,
        runtime: String,
        platform: String,
        ip_address: String,
    ) -> Self {
        Self {
            client_id,
            sdk_version,
            runtime,
            platform,
            user_agent: None,
            ip_address,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Add metadata to the client info
    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Set user agent
    pub fn with_user_agent(mut self, user_agent: String) -> Self {
        self.user_agent = Some(user_agent);
        self
    }
}

/// Connection statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStats {
    /// When the connection was established
    pub connected_at: DateTime<Utc>,

    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,

    /// Number of messages sent
    pub messages_sent: u64,

    /// Number of messages received
    pub messages_received: u64,

    /// Total bytes sent
    pub bytes_sent: u64,

    /// Total bytes received
    pub bytes_received: u64,

    /// Number of errors encountered
    pub error_count: u64,
}

impl ConnectionStats {
    /// Create new connection statistics
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            connected_at: now,
            last_activity: now,
            messages_sent: 0,
            messages_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
            error_count: 0,
        }
    }

    /// Update activity timestamp
    pub fn update_activity(&mut self) {
        self.last_activity = Utc::now();
    }

    /// Record a sent message
    pub fn record_sent_message(&mut self, bytes: u64) {
        self.messages_sent += 1;
        self.bytes_sent += bytes;
        self.update_activity();
    }

    /// Record a received message
    pub fn record_received_message(&mut self, bytes: u64) {
        self.messages_received += 1;
        self.bytes_received += bytes;
        self.update_activity();
    }

    /// Record an error
    pub fn record_error(&mut self) {
        self.error_count += 1;
        self.update_activity();
    }

    /// Get connection duration
    pub fn duration(&self) -> chrono::Duration {
        Utc::now() - self.connected_at
    }

    /// Get idle duration
    pub fn idle_duration(&self) -> chrono::Duration {
        Utc::now() - self.last_activity
    }
}

impl Default for ConnectionStats {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_connection_id() {
        let id1 = ConnectionId::new();
        let id2 = ConnectionId::new();

        assert_ne!(id1, id2);
        assert_eq!(id1.to_string().len(), 36); // UUID string length
    }

    #[test]
    fn test_session_id() {
        let id1 = SessionId::new();
        let id2 = SessionId::new();

        assert_ne!(id1, id2);
        assert_eq!(id1.to_string().len(), 36); // UUID string length
    }

    #[test]
    fn test_client_info() {
        let client_info = ClientInfo::new(
            "test-client".to_string(),
            "1.0.0".to_string(),
            "rust".to_string(),
            "linux".to_string(),
            "127.0.0.1".to_string(),
        )
        .with_user_agent("test-agent".to_string())
        .with_metadata(
            "key".to_string(),
            serde_json::Value::String("value".to_string()),
        );

        assert_eq!(client_info.client_id, "test-client");
        assert_eq!(client_info.sdk_version, "1.0.0");
        assert_eq!(client_info.user_agent, Some("test-agent".to_string()));
        assert_eq!(client_info.metadata.len(), 1);
    }

    #[test]
    fn test_connection_stats() {
        let mut stats = ConnectionStats::new();

        // Test initial state
        assert_eq!(stats.messages_sent, 0);
        assert_eq!(stats.messages_received, 0);
        assert_eq!(stats.error_count, 0);

        // Test recording messages
        stats.record_sent_message(100);
        assert_eq!(stats.messages_sent, 1);
        assert_eq!(stats.bytes_sent, 100);

        stats.record_received_message(200);
        assert_eq!(stats.messages_received, 1);
        assert_eq!(stats.bytes_received, 200);

        // Test recording errors
        stats.record_error();
        assert_eq!(stats.error_count, 1);

        // Test durations
        assert!(stats.duration().num_milliseconds() >= 0);
        assert!(stats.idle_duration().num_milliseconds() >= 0);
    }

    #[test]
    fn test_id_conversions() {
        let uuid = Uuid::new_v4();
        let conn_id = ConnectionId::from(uuid);
        let converted_uuid: Uuid = conn_id.into();

        assert_eq!(uuid, converted_uuid);
    }
}
