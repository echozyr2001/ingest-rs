//! WebSocket event streaming system
//!
//! This module provides real-time event streaming capabilities for live monitoring
//! and debugging of function executions and event processing.

use crate::types::{ConnectionId, SessionId};
use ingest_core::{Event, ExecutionId, FunctionId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, broadcast};

/// Stream event types for real-time monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum StreamEvent {
    /// Raw event received
    EventReceived {
        event_id: String,
        event: Box<Event>,
        timestamp: u64,
        source: String,
    },
    /// Event processing started
    EventProcessingStarted {
        event_id: String,
        execution_id: ExecutionId,
        function_id: FunctionId,
        timestamp: u64,
    },
    /// Event processing completed
    EventProcessingCompleted {
        event_id: String,
        execution_id: ExecutionId,
        function_id: FunctionId,
        duration_ms: u64,
        timestamp: u64,
    },
    /// Function execution log entry
    ExecutionLog {
        execution_id: ExecutionId,
        level: String,
        message: String,
        timestamp: u64,
    },
    /// Function execution output
    ExecutionOutput {
        execution_id: ExecutionId,
        step_id: Option<String>,
        output: serde_json::Value,
        timestamp: u64,
    },
    /// Error occurred during processing
    ProcessingError {
        event_id: Option<String>,
        execution_id: Option<ExecutionId>,
        error: String,
        timestamp: u64,
    },
    /// Heartbeat for connection health
    Heartbeat { timestamp: u64 },
}

impl StreamEvent {
    /// Get the timestamp of the event
    pub fn timestamp(&self) -> u64 {
        match self {
            StreamEvent::EventReceived { timestamp, .. }
            | StreamEvent::EventProcessingStarted { timestamp, .. }
            | StreamEvent::EventProcessingCompleted { timestamp, .. }
            | StreamEvent::ExecutionLog { timestamp, .. }
            | StreamEvent::ExecutionOutput { timestamp, .. }
            | StreamEvent::ProcessingError { timestamp, .. }
            | StreamEvent::Heartbeat { timestamp } => *timestamp,
        }
    }

    /// Get the execution ID if available
    pub fn execution_id(&self) -> Option<ExecutionId> {
        match self {
            StreamEvent::EventProcessingStarted { execution_id, .. }
            | StreamEvent::EventProcessingCompleted { execution_id, .. }
            | StreamEvent::ExecutionLog { execution_id, .. }
            | StreamEvent::ExecutionOutput { execution_id, .. } => Some(execution_id.clone()),
            StreamEvent::ProcessingError { execution_id, .. } => execution_id.clone(),
            _ => None,
        }
    }

    /// Create a heartbeat event
    pub fn heartbeat() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        StreamEvent::Heartbeat { timestamp }
    }
}

/// Stream filter for selective event streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamFilter {
    /// Filter by event types
    pub event_types: Option<Vec<String>>,
    /// Filter by function IDs
    pub function_ids: Option<Vec<FunctionId>>,
    /// Filter by execution IDs
    pub execution_ids: Option<Vec<ExecutionId>>,
    /// Filter by log levels
    pub log_levels: Option<Vec<String>>,
    /// Time range filter (start timestamp)
    pub since: Option<u64>,
    /// Time range filter (end timestamp)
    pub until: Option<u64>,
    /// Include heartbeat events
    #[serde(default = "default_include_heartbeat")]
    pub include_heartbeat: bool,
}

fn default_include_heartbeat() -> bool {
    true
}

impl Default for StreamFilter {
    fn default() -> Self {
        Self {
            event_types: None,
            function_ids: None,
            execution_ids: None,
            log_levels: None,
            since: None,
            until: None,
            include_heartbeat: true,
        }
    }
}

impl StreamFilter {
    /// Check if a stream event matches this filter
    pub fn matches(&self, event: &StreamEvent) -> bool {
        // Check time range
        if let Some(since) = self.since {
            if event.timestamp() < since {
                return false;
            }
        }

        if let Some(until) = self.until {
            if event.timestamp() > until {
                return false;
            }
        }

        match event {
            StreamEvent::EventReceived { event, .. } => self.matches_event_type(&event.name),
            StreamEvent::EventProcessingStarted {
                function_id,
                execution_id,
                ..
            }
            | StreamEvent::EventProcessingCompleted {
                function_id,
                execution_id,
                ..
            } => {
                self.matches_function(function_id.clone())
                    && self.matches_execution(execution_id.clone())
            }
            StreamEvent::ExecutionLog {
                execution_id,
                level,
                ..
            } => self.matches_execution(execution_id.clone()) && self.matches_log_level(level),
            StreamEvent::ExecutionOutput { execution_id, .. } => {
                self.matches_execution(execution_id.clone())
            }
            StreamEvent::ProcessingError { execution_id, .. } => execution_id
                .as_ref()
                .is_none_or(|id| self.matches_execution(id.clone())),
            StreamEvent::Heartbeat { .. } => self.include_heartbeat,
        }
    }

    fn matches_event_type(&self, event_type: &str) -> bool {
        self.event_types
            .as_ref()
            .is_none_or(|types| types.contains(&event_type.to_string()))
    }

    fn matches_function(&self, function_id: FunctionId) -> bool {
        self.function_ids
            .as_ref()
            .is_none_or(|ids| ids.contains(&function_id))
    }

    fn matches_execution(&self, execution_id: ExecutionId) -> bool {
        self.execution_ids
            .as_ref()
            .is_none_or(|ids| ids.contains(&execution_id))
    }

    fn matches_log_level(&self, level: &str) -> bool {
        self.log_levels
            .as_ref()
            .is_none_or(|levels| levels.contains(&level.to_string()))
    }
}

/// Stream connection configuration
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// Buffer size for events
    pub buffer_size: usize,
    /// Maximum number of concurrent connections
    pub max_connections: usize,
    /// Heartbeat interval
    pub heartbeat_interval: Duration,
    /// Enable compression
    pub compression: bool,
    /// Connection timeout
    pub connection_timeout: Duration,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            buffer_size: 10000,
            max_connections: 1000,
            heartbeat_interval: Duration::from_secs(30),
            compression: true,
            connection_timeout: Duration::from_secs(300),
        }
    }
}

/// Stream connection state
#[derive(Debug, Clone)]
pub struct StreamConnection {
    pub connection_id: ConnectionId,
    pub session_id: SessionId,
    pub filter: StreamFilter,
    pub buffer: VecDeque<StreamEvent>,
    pub created_at: Instant,
    pub last_heartbeat: Instant,
    pub bytes_sent: u64,
    pub events_sent: u64,
}

impl StreamConnection {
    /// Create a new stream connection
    pub fn new(connection_id: ConnectionId, session_id: SessionId, filter: StreamFilter) -> Self {
        let now = Instant::now();
        Self {
            connection_id,
            session_id,
            filter,
            buffer: VecDeque::new(),
            created_at: now,
            last_heartbeat: now,
            bytes_sent: 0,
            events_sent: 0,
        }
    }

    /// Add event to buffer
    pub fn buffer_event(&mut self, event: StreamEvent) {
        if self.filter.matches(&event) {
            self.buffer.push_back(event);

            // Limit buffer size
            if self.buffer.len() > 1000 {
                self.buffer.pop_front();
            }
        }
    }

    /// Update heartbeat timestamp
    pub fn update_heartbeat(&mut self) {
        self.last_heartbeat = Instant::now();
    }

    /// Check if connection is stale
    pub fn is_stale(&self, timeout: Duration) -> bool {
        self.last_heartbeat.elapsed() > timeout
    }

    /// Get connection statistics
    pub fn stats(&self) -> StreamConnectionStats {
        StreamConnectionStats {
            connection_id: self.connection_id,
            session_id: self.session_id,
            uptime: self.created_at.elapsed(),
            buffered_events: self.buffer.len(),
            bytes_sent: self.bytes_sent,
            events_sent: self.events_sent,
            last_heartbeat: self.last_heartbeat.elapsed(),
        }
    }
}

/// Stream connection statistics
#[derive(Debug, Clone, Serialize)]
pub struct StreamConnectionStats {
    pub connection_id: ConnectionId,
    pub session_id: SessionId,
    pub uptime: Duration,
    pub buffered_events: usize,
    pub bytes_sent: u64,
    pub events_sent: u64,
    pub last_heartbeat: Duration,
}

/// Event streamer for real-time event streaming
pub struct EventStreamer {
    /// Active stream connections
    connections: Arc<RwLock<HashMap<ConnectionId, StreamConnection>>>,
    /// Broadcast channel for stream events
    sender: broadcast::Sender<StreamEvent>,
    /// Stream configuration
    config: StreamConfig,
}

impl EventStreamer {
    /// Create a new event streamer
    pub fn new(config: StreamConfig) -> Self {
        let (sender, _) = broadcast::channel(config.buffer_size);
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            sender,
            config,
        }
    }

    /// Create a new stream connection
    pub async fn create_stream(
        &self,
        connection_id: ConnectionId,
        session_id: SessionId,
        filter: StreamFilter,
    ) -> crate::Result<broadcast::Receiver<StreamEvent>> {
        // Check connection limit
        {
            let connections = self.connections.read().await;
            if connections.len() >= self.config.max_connections {
                return Err(crate::ConnectError::TooManyConnections);
            }
        }

        let connection = StreamConnection::new(connection_id, session_id, filter);

        let mut connections = self.connections.write().await;
        connections.insert(connection_id, connection);

        Ok(self.sender.subscribe())
    }

    /// Remove a stream connection
    pub async fn remove_stream(&self, connection_id: ConnectionId) {
        let mut connections = self.connections.write().await;
        connections.remove(&connection_id);
    }

    /// Broadcast an event to all matching streams
    pub async fn broadcast_event(&self, event: StreamEvent) -> crate::Result<usize> {
        // Clean up stale connections
        self.cleanup_stale_connections().await;

        // Count matching connections
        let connections = self.connections.read().await;
        let matching_count = connections
            .values()
            .filter(|conn| conn.filter.matches(&event))
            .count();

        // Broadcast to all streams (filtering happens on receiver side)
        if self.sender.send(event).is_err() {
            // No active receivers, which is fine
        }

        Ok(matching_count)
    }

    /// Send heartbeat to all connections
    pub async fn send_heartbeat(&self) -> crate::Result<()> {
        let heartbeat = StreamEvent::heartbeat();
        self.broadcast_event(heartbeat).await?;

        // Update heartbeat timestamps
        let mut connections = self.connections.write().await;
        for connection in connections.values_mut() {
            connection.update_heartbeat();
        }

        Ok(())
    }

    /// Get current connection count
    pub async fn connection_count(&self) -> usize {
        let connections = self.connections.read().await;
        connections.len()
    }

    /// Get connection statistics
    pub async fn connection_stats(&self) -> Vec<StreamConnectionStats> {
        let connections = self.connections.read().await;
        connections.values().map(|conn| conn.stats()).collect()
    }

    /// Clean up stale connections
    async fn cleanup_stale_connections(&self) {
        let mut connections = self.connections.write().await;
        connections.retain(|_, conn| !conn.is_stale(self.config.connection_timeout));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingest_core::Event;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_stream_filter_matches_event() {
        let filter = StreamFilter {
            event_types: Some(vec!["user.created".to_string()]),
            ..Default::default()
        };

        let event = Event::new("user.created", serde_json::json!({}));
        let stream_event = StreamEvent::EventReceived {
            event_id: "test".to_string(),
            event: Box::new(event),
            timestamp: 1234567890,
            source: "api".to_string(),
        };

        assert!(filter.matches(&stream_event));
    }

    #[test]
    fn test_stream_filter_no_match() {
        let filter = StreamFilter {
            event_types: Some(vec!["user.created".to_string()]),
            ..Default::default()
        };

        let event = Event::new("user.deleted", serde_json::json!({}));
        let stream_event = StreamEvent::EventReceived {
            event_id: "test".to_string(),
            event: Box::new(event),
            timestamp: 1234567890,
            source: "api".to_string(),
        };

        assert!(!filter.matches(&stream_event));
    }

    #[test]
    fn test_stream_connection_creation() {
        let connection_id = ConnectionId::new();
        let session_id = SessionId::new();
        let filter = StreamFilter::default();

        let connection = StreamConnection::new(connection_id, session_id, filter);

        assert_eq!(connection.connection_id, connection_id);
        assert_eq!(connection.session_id, session_id);
        assert!(!connection.is_stale(Duration::from_secs(60)));
    }

    #[tokio::test]
    async fn test_event_streamer_creation() {
        let config = StreamConfig::default();
        let streamer = EventStreamer::new(config);

        assert_eq!(streamer.connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_event_streamer_stream_creation() {
        let config = StreamConfig::default();
        let streamer = EventStreamer::new(config);

        let connection_id = ConnectionId::new();
        let session_id = SessionId::new();
        let filter = StreamFilter::default();

        let _receiver = streamer
            .create_stream(connection_id, session_id, filter)
            .await
            .unwrap();

        assert_eq!(streamer.connection_count().await, 1);

        streamer.remove_stream(connection_id).await;
        assert_eq!(streamer.connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_event_streamer_broadcast() {
        let config = StreamConfig::default();
        let streamer = EventStreamer::new(config);

        let connection_id = ConnectionId::new();
        let session_id = SessionId::new();
        let filter = StreamFilter::default();

        let mut receiver = streamer
            .create_stream(connection_id, session_id, filter)
            .await
            .unwrap();

        let event = StreamEvent::heartbeat();
        let count = streamer.broadcast_event(event.clone()).await.unwrap();
        assert_eq!(count, 1);

        // Receive the event
        let received = receiver.recv().await.unwrap();
        match received {
            StreamEvent::Heartbeat { .. } => {
                // Expected
            }
            _ => panic!("Unexpected event type"),
        }
    }
}
