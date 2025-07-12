//! Protocol handling and negotiation for WebSocket connections

use crate::{Result, error::ConnectError};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Protocol version identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProtocolVersion {
    /// Major version
    pub major: u32,

    /// Minor version
    pub minor: u32,

    /// Patch version
    pub patch: Option<u32>,
}

impl ProtocolVersion {
    /// Create a new protocol version
    pub fn new(major: u32, minor: u32, patch: Option<u32>) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Parse version from string (e.g., "1.0", "1.1.2")
    pub fn parse(version: &str) -> Result<Self> {
        let parts: Vec<&str> = version.split('.').collect();

        if parts.is_empty() || parts.len() > 3 {
            return Err(ConnectError::ProtocolNegotiation(format!(
                "Invalid version format: {version}"
            )));
        }

        let major = parts[0].parse::<u32>().map_err(|_| {
            ConnectError::ProtocolNegotiation(format!("Invalid major version: {}", parts[0]))
        })?;

        let minor = if parts.len() > 1 {
            parts[1].parse::<u32>().map_err(|_| {
                ConnectError::ProtocolNegotiation(format!("Invalid minor version: {}", parts[1]))
            })?
        } else {
            0
        };

        let patch = if parts.len() > 2 {
            Some(parts[2].parse::<u32>().map_err(|_| {
                ConnectError::ProtocolNegotiation(format!("Invalid patch version: {}", parts[2]))
            })?)
        } else {
            None
        };

        Ok(Self::new(major, minor, patch))
    }

    /// Check if this version is compatible with another
    pub fn is_compatible_with(&self, other: &Self) -> bool {
        // Major version must match
        if self.major != other.major {
            return false;
        }

        // Minor version compatibility (backward compatible)
        self.minor >= other.minor
    }

    /// Get the latest compatible version between two versions
    pub fn latest_compatible(&self, other: &Self) -> Option<Self> {
        if !self.is_compatible_with(other) && !other.is_compatible_with(self) {
            return None;
        }

        // Return the version with higher minor version
        if self.major == other.major {
            if self.minor >= other.minor {
                Some(self.clone())
            } else {
                Some(other.clone())
            }
        } else {
            None
        }
    }
}

impl fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(patch) = self.patch {
            write!(f, "{}.{}.{}", self.major, self.minor, patch)
        } else {
            write!(f, "{}.{}", self.major, self.minor)
        }
    }
}

/// Protocol negotiator for handling version negotiation
#[derive(Debug, Clone)]
pub struct ProtocolNegotiator {
    /// Supported versions (in order of preference)
    supported_versions: Vec<ProtocolVersion>,

    /// Default version to use
    default_version: ProtocolVersion,
}

impl ProtocolNegotiator {
    /// Create a new protocol negotiator
    pub fn new(supported_versions: Vec<ProtocolVersion>, default_version: ProtocolVersion) -> Self {
        Self {
            supported_versions,
            default_version,
        }
    }

    /// Negotiate protocol version with client
    pub fn negotiate(&self, client_versions: &[ProtocolVersion]) -> Result<ProtocolVersion> {
        let mut best_version: Option<ProtocolVersion> = None;

        // Find the best mutually supported version
        for server_version in &self.supported_versions {
            for client_version in client_versions {
                // Check if versions are exactly the same or if server can support client
                if server_version == client_version {
                    match &best_version {
                        None => best_version = Some(server_version.clone()),
                        Some(current_best) => {
                            if server_version.major == current_best.major
                                && server_version.minor > current_best.minor
                            {
                                best_version = Some(server_version.clone());
                            }
                        }
                    }
                } else if server_version.major == client_version.major
                    && server_version.minor >= client_version.minor
                {
                    // Server version is backward compatible with client
                    match &best_version {
                        None => best_version = Some(server_version.clone()),
                        Some(current_best) => {
                            if server_version.major == current_best.major
                                && server_version.minor > current_best.minor
                            {
                                best_version = Some(server_version.clone());
                            }
                        }
                    }
                }
            }
        }

        match best_version {
            Some(version) => Ok(version),
            None => Err(ConnectError::ProtocolNegotiation(
                "No compatible protocol version found".to_string(),
            )),
        }
    }

    /// Get supported versions
    pub fn supported_versions(&self) -> &[ProtocolVersion] {
        &self.supported_versions
    }

    /// Get default version
    pub fn default_version(&self) -> &ProtocolVersion {
        &self.default_version
    }
}

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Message {
    /// Protocol handshake message
    Handshake {
        /// Client protocol versions
        versions: Vec<ProtocolVersion>,

        /// Client information
        client_info: crate::types::ClientInfo,
    },

    /// Handshake response
    HandshakeResponse {
        /// Negotiated protocol version
        version: ProtocolVersion,

        /// Session ID
        session_id: crate::types::SessionId,

        /// Server capabilities
        capabilities: ServerCapabilities,
    },

    /// Authentication message
    Auth {
        /// Authentication token
        token: String,

        /// Authentication method
        method: AuthMethod,
    },

    /// Authentication response
    AuthResponse {
        /// Whether authentication was successful
        success: bool,

        /// Error message if authentication failed
        error: Option<String>,
    },

    /// Event message
    Event {
        /// Event data
        event: ingest_core::Event,

        /// Message ID for acknowledgment
        message_id: String,
    },

    /// Event acknowledgment
    EventAck {
        /// Message ID being acknowledged
        message_id: String,

        /// Whether processing was successful
        success: bool,

        /// Error message if processing failed
        error: Option<String>,
    },

    /// Function execution request
    FunctionExecution {
        /// Function to execute
        function: ingest_core::Function,

        /// Execution context
        context: serde_json::Value,

        /// Message ID for response
        message_id: String,
    },

    /// Function execution response
    FunctionExecutionResponse {
        /// Message ID being responded to
        message_id: String,

        /// Execution result
        result: ExecutionResult,
    },

    /// Heartbeat/ping message
    Ping {
        /// Timestamp
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// Heartbeat/pong response
    Pong {
        /// Original timestamp from ping
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// Error message
    Error {
        /// Error code
        code: String,

        /// Error message
        message: String,

        /// Additional error details
        details: Option<serde_json::Value>,
    },

    /// Connection close message
    Close {
        /// Close reason
        reason: String,

        /// Close code
        code: u16,
    },
}

/// Authentication methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    /// JWT token authentication
    Jwt,

    /// API key authentication
    ApiKey,

    /// SDK signature authentication
    SdkSignature {
        /// SDK identifier
        sdk_id: String,

        /// Signature
        signature: String,
    },
}

/// Server capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilities {
    /// Supported features
    pub features: Vec<String>,

    /// Maximum message size
    pub max_message_size: usize,

    /// Heartbeat interval in seconds
    pub heartbeat_interval: u64,

    /// Compression support
    pub compression: bool,
}

/// Function execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionResult {
    /// Successful execution
    Success {
        /// Result data
        data: serde_json::Value,

        /// Execution duration in milliseconds
        duration_ms: u64,
    },

    /// Failed execution
    Error {
        /// Error message
        error: String,

        /// Error code
        code: String,

        /// Whether the error is retryable
        retryable: bool,
    },

    /// Execution timeout
    Timeout {
        /// Timeout duration in milliseconds
        timeout_ms: u64,
    },
}

impl Message {
    /// Get message type as string
    pub fn message_type(&self) -> &'static str {
        match self {
            Message::Handshake { .. } => "handshake",
            Message::HandshakeResponse { .. } => "handshake_response",
            Message::Auth { .. } => "auth",
            Message::AuthResponse { .. } => "auth_response",
            Message::Event { .. } => "event",
            Message::EventAck { .. } => "event_ack",
            Message::FunctionExecution { .. } => "function_execution",
            Message::FunctionExecutionResponse { .. } => "function_execution_response",
            Message::Ping { .. } => "ping",
            Message::Pong { .. } => "pong",
            Message::Error { .. } => "error",
            Message::Close { .. } => "close",
        }
    }

    /// Serialize message to JSON
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(ConnectError::from)
    }

    /// Deserialize message from JSON
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(ConnectError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_protocol_version_parsing() {
        let version = ProtocolVersion::parse("1.0").unwrap();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, None);

        let version = ProtocolVersion::parse("2.1.3").unwrap();
        assert_eq!(version.major, 2);
        assert_eq!(version.minor, 1);
        assert_eq!(version.patch, Some(3));

        assert!(ProtocolVersion::parse("invalid").is_err());
    }

    #[test]
    fn test_protocol_version_display() {
        let version = ProtocolVersion::new(1, 0, None);
        assert_eq!(version.to_string(), "1.0");

        let version = ProtocolVersion::new(2, 1, Some(3));
        assert_eq!(version.to_string(), "2.1.3");
    }

    #[test]
    fn test_protocol_compatibility() {
        let v1_0 = ProtocolVersion::new(1, 0, None);
        let v1_1 = ProtocolVersion::new(1, 1, None);
        let v2_0 = ProtocolVersion::new(2, 0, None);

        assert!(v1_1.is_compatible_with(&v1_0));
        assert!(!v1_0.is_compatible_with(&v1_1));
        assert!(!v1_0.is_compatible_with(&v2_0));
        assert!(!v2_0.is_compatible_with(&v1_0));
    }

    #[test]
    fn test_protocol_negotiation() {
        let supported = vec![
            ProtocolVersion::new(1, 0, None),
            ProtocolVersion::new(1, 1, None),
        ];
        let default = ProtocolVersion::new(1, 1, None);
        let negotiator = ProtocolNegotiator::new(supported, default);

        let client_versions = vec![
            ProtocolVersion::new(1, 0, None),
            ProtocolVersion::new(1, 2, None),
        ];

        let result = negotiator.negotiate(&client_versions).unwrap();
        assert_eq!(result.major, 1);
        assert_eq!(result.minor, 1);
    }

    #[test]
    fn test_message_serialization() {
        let message = Message::Ping {
            timestamp: chrono::Utc::now(),
        };

        let json = message.to_json().unwrap();
        let deserialized = Message::from_json(&json).unwrap();

        assert_eq!(message.message_type(), deserialized.message_type());
    }

    #[test]
    fn test_message_types() {
        let ping = Message::Ping {
            timestamp: chrono::Utc::now(),
        };
        assert_eq!(ping.message_type(), "ping");

        let error = Message::Error {
            code: "ERR001".to_string(),
            message: "Test error".to_string(),
            details: None,
        };
        assert_eq!(error.message_type(), "error");
    }
}
