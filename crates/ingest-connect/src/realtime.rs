//! Real-time status broadcasting system
//!
//! This module provides real-time status updates for function executions,
//! event processing, and system health monitoring.

use crate::types::{ConnectionId, SessionId};
use ingest_core::{ExecutionId, FunctionId, StepId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, broadcast};

/// Real-time status update types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum StatusUpdate {
    /// Function execution started
    ExecutionStarted {
        execution_id: ExecutionId,
        function_id: FunctionId,
        timestamp: u64,
    },
    /// Function execution completed successfully
    ExecutionCompleted {
        execution_id: ExecutionId,
        function_id: FunctionId,
        duration_ms: u64,
        timestamp: u64,
    },
    /// Function execution failed
    ExecutionFailed {
        execution_id: ExecutionId,
        function_id: FunctionId,
        error: String,
        timestamp: u64,
    },
    /// Step execution started
    StepStarted {
        execution_id: ExecutionId,
        step_id: StepId,
        step_name: String,
        timestamp: u64,
    },
    /// Step execution completed
    StepCompleted {
        execution_id: ExecutionId,
        step_id: StepId,
        step_name: String,
        duration_ms: u64,
        timestamp: u64,
    },
    /// Event received and queued
    EventReceived {
        event_id: String,
        event_type: String,
        timestamp: u64,
    },
    /// Event processed successfully
    EventProcessed {
        event_id: String,
        event_type: String,
        processing_time_ms: u64,
        timestamp: u64,
    },
    /// System health update
    SystemHealth {
        active_executions: u64,
        queue_depth: u64,
        error_rate: f64,
        timestamp: u64,
    },
}

/// Status filter for selective subscriptions
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StatusFilter {
    /// Filter by execution IDs
    pub execution_ids: Option<Vec<ExecutionId>>,
    /// Filter by function IDs
    pub function_ids: Option<Vec<FunctionId>>,
    /// Filter by event types
    pub event_types: Option<Vec<String>>,
    /// Filter by update types
    pub update_types: Option<Vec<String>>,
    /// Time range filter
    pub since: Option<u64>,
}

impl StatusFilter {
    /// Check if a status update matches this filter
    pub fn matches(&self, update: &StatusUpdate) -> bool {
        match update {
            StatusUpdate::ExecutionStarted {
                execution_id,
                function_id,
                ..
            }
            | StatusUpdate::ExecutionCompleted {
                execution_id,
                function_id,
                ..
            }
            | StatusUpdate::ExecutionFailed {
                execution_id,
                function_id,
                ..
            } => {
                self.matches_execution(execution_id.clone())
                    && self.matches_function(function_id.clone())
            }
            StatusUpdate::StepStarted { execution_id, .. }
            | StatusUpdate::StepCompleted { execution_id, .. } => {
                self.matches_execution(execution_id.clone())
            }
            StatusUpdate::EventReceived { event_type, .. }
            | StatusUpdate::EventProcessed { event_type, .. } => {
                self.matches_event_type(event_type)
            }
            StatusUpdate::SystemHealth { .. } => true, // System health always matches
        }
    }

    fn matches_execution(&self, execution_id: ExecutionId) -> bool {
        self.execution_ids
            .as_ref()
            .is_none_or(|ids| ids.contains(&execution_id))
    }

    fn matches_function(&self, function_id: FunctionId) -> bool {
        self.function_ids
            .as_ref()
            .is_none_or(|ids| ids.contains(&function_id))
    }

    fn matches_event_type(&self, event_type: &str) -> bool {
        self.event_types
            .as_ref()
            .is_none_or(|types| types.contains(&event_type.to_string()))
    }
}

/// Status subscription information
#[derive(Debug, Clone)]
pub struct StatusSubscription {
    pub connection_id: ConnectionId,
    pub session_id: SessionId,
    pub filter: StatusFilter,
    pub created_at: Instant,
    pub last_activity: Instant,
}

impl StatusSubscription {
    /// Create a new status subscription
    pub fn new(connection_id: ConnectionId, session_id: SessionId, filter: StatusFilter) -> Self {
        let now = Instant::now();
        Self {
            connection_id,
            session_id,
            filter,
            created_at: now,
            last_activity: now,
        }
    }

    /// Update last activity timestamp
    pub fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Check if subscription is stale
    pub fn is_stale(&self, timeout: Duration) -> bool {
        self.last_activity.elapsed() > timeout
    }
}

/// Status broadcaster for real-time updates
pub struct StatusBroadcaster {
    /// Active subscriptions
    subscriptions: Arc<RwLock<HashMap<ConnectionId, StatusSubscription>>>,
    /// Broadcast channel for status updates
    sender: broadcast::Sender<StatusUpdate>,
    /// Subscription timeout
    subscription_timeout: Duration,
}

impl StatusBroadcaster {
    /// Create a new status broadcaster
    pub fn new(buffer_size: usize, subscription_timeout: Duration) -> Self {
        let (sender, _) = broadcast::channel(buffer_size);
        Self {
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            sender,
            subscription_timeout,
        }
    }

    /// Subscribe to status updates
    pub async fn subscribe(
        &self,
        connection_id: ConnectionId,
        session_id: SessionId,
        filter: StatusFilter,
    ) -> broadcast::Receiver<StatusUpdate> {
        let subscription = StatusSubscription::new(connection_id, session_id, filter);

        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.insert(connection_id, subscription);

        self.sender.subscribe()
    }

    /// Unsubscribe from status updates
    pub async fn unsubscribe(&self, connection_id: ConnectionId) {
        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.remove(&connection_id);
    }

    /// Broadcast a status update to all matching subscribers
    pub async fn broadcast(&self, update: StatusUpdate) -> crate::Result<usize> {
        // Clean up stale subscriptions
        self.cleanup_stale_subscriptions().await;

        // Count matching subscribers
        let subscriptions = self.subscriptions.read().await;
        let matching_count = subscriptions
            .values()
            .filter(|sub| sub.filter.matches(&update))
            .count();

        // Broadcast to all subscribers (filtering happens on receiver side)
        if self.sender.send(update).is_err() {
            // No active receivers, which is fine
        }

        Ok(matching_count)
    }

    /// Get current subscription count
    pub async fn subscription_count(&self) -> usize {
        let subscriptions = self.subscriptions.read().await;
        subscriptions.len()
    }

    /// Clean up stale subscriptions
    async fn cleanup_stale_subscriptions(&self) {
        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.retain(|_, sub| !sub.is_stale(self.subscription_timeout));
    }

    /// Update subscription activity
    pub async fn update_activity(&self, connection_id: ConnectionId) {
        let mut subscriptions = self.subscriptions.write().await;
        if let Some(subscription) = subscriptions.get_mut(&connection_id) {
            subscription.update_activity();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_status_filter_matches_execution() {
        let filter = StatusFilter {
            execution_ids: Some(vec![ExecutionId::new("test-execution")]),
            function_ids: None,
            event_types: None,
            update_types: None,
            since: None,
        };

        let execution_id = filter.execution_ids.as_ref().unwrap()[0].clone();
        let update = StatusUpdate::ExecutionStarted {
            execution_id,
            function_id: FunctionId::new("test-function"),
            timestamp: 1234567890,
        };

        assert!(filter.matches(&update));
    }

    #[test]
    fn test_status_filter_no_match() {
        let filter = StatusFilter {
            execution_ids: Some(vec![ExecutionId::new("test-execution")]),
            function_ids: None,
            event_types: None,
            update_types: None,
            since: None,
        };

        let update = StatusUpdate::ExecutionStarted {
            execution_id: ExecutionId::new("different-execution"), // Different execution ID
            function_id: FunctionId::new("test-function"),
            timestamp: 1234567890,
        };

        assert!(!filter.matches(&update));
    }

    #[test]
    fn test_status_subscription_creation() {
        let connection_id = ConnectionId::new();
        let session_id = SessionId::new();
        let filter = StatusFilter {
            execution_ids: None,
            function_ids: None,
            event_types: None,
            update_types: None,
            since: None,
        };

        let subscription = StatusSubscription::new(connection_id, session_id, filter);

        assert_eq!(subscription.connection_id, connection_id);
        assert_eq!(subscription.session_id, session_id);
        assert!(!subscription.is_stale(Duration::from_secs(60)));
    }

    #[tokio::test]
    async fn test_status_broadcaster_subscription() {
        let broadcaster = StatusBroadcaster::new(1000, Duration::from_secs(300));
        let connection_id = ConnectionId::new();
        let session_id = SessionId::new();
        let filter = StatusFilter {
            execution_ids: None,
            function_ids: None,
            event_types: None,
            update_types: None,
            since: None,
        };

        let _receiver = broadcaster
            .subscribe(connection_id, session_id, filter)
            .await;

        assert_eq!(broadcaster.subscription_count().await, 1);

        broadcaster.unsubscribe(connection_id).await;
        assert_eq!(broadcaster.subscription_count().await, 0);
    }

    #[tokio::test]
    async fn test_status_broadcaster_broadcast() {
        let broadcaster = StatusBroadcaster::new(1000, Duration::from_secs(300));
        let connection_id = ConnectionId::new();
        let session_id = SessionId::new();
        let filter = StatusFilter {
            execution_ids: None,
            function_ids: None,
            event_types: None,
            update_types: None,
            since: None,
        };

        let mut receiver = broadcaster
            .subscribe(connection_id, session_id, filter)
            .await;

        let update = StatusUpdate::SystemHealth {
            active_executions: 5,
            queue_depth: 10,
            error_rate: 0.01,
            timestamp: 1234567890,
        };

        let count = broadcaster.broadcast(update.clone()).await.unwrap();
        assert_eq!(count, 1);

        // Receive the update
        let received = receiver.recv().await.unwrap();
        match received {
            StatusUpdate::SystemHealth {
                active_executions, ..
            } => {
                assert_eq!(active_executions, 5);
            }
            _ => panic!("Unexpected update type"),
        }
    }
}
