//! Audit event tracking system

use crate::error::Result;
use derive_setters::Setters;
use ingest_core::{DateTime, Json};
use ingest_storage::Storage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

/// Unique identifier for audit events
pub type AuditEventId = String;

/// Main audit event structure
#[derive(Debug, Clone, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct AuditEvent {
    /// Unique event identifier
    pub id: AuditEventId,
    /// Event timestamp
    pub timestamp: DateTime,
    /// Type of audit event
    pub event_type: AuditEventType,
    /// Type of resource being audited
    pub resource_type: ResourceType,
    /// Identifier of the resource
    pub resource_id: String,
    /// Actor who performed the action
    pub actor: Option<Actor>,
    /// Event metadata
    pub metadata: Json,
    /// Changes made (for update events)
    pub changes: Option<ChangeSet>,
    /// Creation timestamp
    pub created_at: DateTime,
}

impl AuditEvent {
    /// Create a new audit event
    pub fn new(
        event_type: AuditEventType,
        resource_type: ResourceType,
        resource_id: impl Into<String>,
        actor: Option<Actor>,
        metadata: Json,
        changes: Option<ChangeSet>,
    ) -> Self {
        let now = chrono::Utc::now();

        Self {
            id: generate_audit_event_id(),
            timestamp: now,
            event_type,
            resource_type,
            resource_id: resource_id.into(),
            actor,
            metadata,
            changes,
            created_at: now,
        }
    }

    /// Create a creation event
    pub fn created(
        resource_type: ResourceType,
        resource_id: impl Into<String>,
        actor: Option<Actor>,
        metadata: Json,
    ) -> Self {
        Self::new(
            AuditEventType::Created,
            resource_type,
            resource_id,
            actor,
            metadata,
            None,
        )
    }

    /// Create an update event
    pub fn updated(
        resource_type: ResourceType,
        resource_id: impl Into<String>,
        actor: Option<Actor>,
        metadata: Json,
        changes: ChangeSet,
    ) -> Self {
        Self::new(
            AuditEventType::Updated,
            resource_type,
            resource_id,
            actor,
            metadata,
            Some(changes),
        )
    }

    /// Create a deletion event
    pub fn deleted(
        resource_type: ResourceType,
        resource_id: impl Into<String>,
        actor: Option<Actor>,
        metadata: Json,
    ) -> Self {
        Self::new(
            AuditEventType::Deleted,
            resource_type,
            resource_id,
            actor,
            metadata,
            None,
        )
    }

    /// Create an execution event
    pub fn executed(
        resource_type: ResourceType,
        resource_id: impl Into<String>,
        actor: Option<Actor>,
        metadata: Json,
    ) -> Self {
        Self::new(
            AuditEventType::Executed,
            resource_type,
            resource_id,
            actor,
            metadata,
            None,
        )
    }
}

/// Types of audit events
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuditEventType {
    /// Resource was created
    Created,
    /// Resource was updated
    Updated,
    /// Resource was deleted
    Deleted,
    /// Resource was executed
    Executed,
    /// Resource execution failed
    Failed,
    /// Resource execution was cancelled
    Cancelled,
    /// Resource was paused
    Paused,
    /// Resource was resumed
    Resumed,
    /// Custom event type
    Custom(String),
}

impl fmt::Display for AuditEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuditEventType::Created => write!(f, "Created"),
            AuditEventType::Updated => write!(f, "Updated"),
            AuditEventType::Deleted => write!(f, "Deleted"),
            AuditEventType::Executed => write!(f, "Executed"),
            AuditEventType::Failed => write!(f, "Failed"),
            AuditEventType::Cancelled => write!(f, "Cancelled"),
            AuditEventType::Paused => write!(f, "Paused"),
            AuditEventType::Resumed => write!(f, "Resumed"),
            AuditEventType::Custom(name) => write!(f, "{name}"),
        }
    }
}

/// Types of resources that can be audited
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    /// Function resource
    Function,
    /// Event resource
    Event,
    /// Execution resource
    Execution,
    /// Step resource
    Step,
    /// User resource
    User,
    /// API key resource
    ApiKey,
    /// Environment resource
    Environment,
    /// Configuration resource
    Configuration,
    /// System resource
    System,
    /// Custom resource type
    Custom(String),
}

impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourceType::Function => write!(f, "Function"),
            ResourceType::Event => write!(f, "Event"),
            ResourceType::Execution => write!(f, "Execution"),
            ResourceType::Step => write!(f, "Step"),
            ResourceType::User => write!(f, "User"),
            ResourceType::ApiKey => write!(f, "ApiKey"),
            ResourceType::Environment => write!(f, "Environment"),
            ResourceType::Configuration => write!(f, "Configuration"),
            ResourceType::System => write!(f, "System"),
            ResourceType::Custom(name) => write!(f, "{name}"),
        }
    }
}

/// Actor who performed an action
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Actor {
    /// User actor
    User(String),
    /// System actor
    System(String),
    /// API key actor
    ApiKey(String),
    /// Service actor
    Service(String),
    /// Anonymous actor
    Anonymous,
}

impl fmt::Display for Actor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Actor::User(id) => write!(f, "User({id})"),
            Actor::System(id) => write!(f, "System({id})"),
            Actor::ApiKey(id) => write!(f, "ApiKey({id})"),
            Actor::Service(id) => write!(f, "Service({id})"),
            Actor::Anonymous => write!(f, "Anonymous"),
        }
    }
}

/// Set of changes made to a resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeSet {
    /// Previous values
    pub previous: HashMap<String, Json>,
    /// Current values
    pub current: HashMap<String, Json>,
}

impl ChangeSet {
    /// Create a new change set
    pub fn new() -> Self {
        Self {
            previous: HashMap::new(),
            current: HashMap::new(),
        }
    }

    /// Add a field change
    pub fn add_change(mut self, field: impl Into<String>, previous: Json, current: Json) -> Self {
        let field = field.into();
        self.previous.insert(field.clone(), previous);
        self.current.insert(field, current);
        self
    }

    /// Get changed fields
    pub fn changed_fields(&self) -> Vec<&String> {
        self.current.keys().collect()
    }

    /// Check if a field was changed
    pub fn has_change(&self, field: &str) -> bool {
        self.current.contains_key(field)
    }
}

impl Default for ChangeSet {
    fn default() -> Self {
        Self::new()
    }
}

/// Audit tracker for recording events
pub struct AuditTracker {
    #[allow(dead_code)]
    storage: Arc<dyn Storage<Transaction = ingest_storage::postgres::PostgresTransaction>>,
}

impl AuditTracker {
    /// Create a new audit tracker
    pub async fn new(
        storage: Arc<dyn Storage<Transaction = ingest_storage::postgres::PostgresTransaction>>,
    ) -> Result<Self> {
        info!("Initializing audit tracker");
        Ok(Self { storage })
    }

    /// Track an audit event
    #[instrument(skip(self, metadata, changes))]
    pub async fn track_event(
        &self,
        event_type: AuditEventType,
        resource_type: ResourceType,
        resource_id: impl Into<String> + std::fmt::Debug,
        actor: Option<Actor>,
        metadata: Json,
        changes: Option<ChangeSet>,
    ) -> Result<AuditEventId> {
        let event = AuditEvent::new(
            event_type,
            resource_type,
            resource_id,
            actor,
            metadata,
            changes,
        );

        debug!(
            "Tracking audit event: {} {} {}",
            event.event_type, event.resource_type, event.resource_id
        );

        self.store_event(&event).await?;

        info!("Audit event tracked: {}", event.id);
        Ok(event.id)
    }

    /// Track a creation event
    #[instrument(skip(self, metadata))]
    pub async fn track_creation(
        &self,
        resource_type: ResourceType,
        resource_id: impl Into<String> + std::fmt::Debug,
        actor: Option<Actor>,
        metadata: Json,
    ) -> Result<AuditEventId> {
        self.track_event(
            AuditEventType::Created,
            resource_type,
            resource_id,
            actor,
            metadata,
            None,
        )
        .await
    }

    /// Track an update event
    #[instrument(skip(self, metadata, changes))]
    pub async fn track_update(
        &self,
        resource_type: ResourceType,
        resource_id: impl Into<String> + std::fmt::Debug,
        actor: Option<Actor>,
        metadata: Json,
        changes: ChangeSet,
    ) -> Result<AuditEventId> {
        self.track_event(
            AuditEventType::Updated,
            resource_type,
            resource_id,
            actor,
            metadata,
            Some(changes),
        )
        .await
    }

    /// Track a deletion event
    #[instrument(skip(self, metadata))]
    pub async fn track_deletion(
        &self,
        resource_type: ResourceType,
        resource_id: impl Into<String> + std::fmt::Debug,
        actor: Option<Actor>,
        metadata: Json,
    ) -> Result<AuditEventId> {
        self.track_event(
            AuditEventType::Deleted,
            resource_type,
            resource_id,
            actor,
            metadata,
            None,
        )
        .await
    }

    /// Track an execution event
    #[instrument(skip(self, metadata))]
    pub async fn track_execution(
        &self,
        resource_type: ResourceType,
        resource_id: impl Into<String> + std::fmt::Debug,
        actor: Option<Actor>,
        metadata: Json,
    ) -> Result<AuditEventId> {
        self.track_event(
            AuditEventType::Executed,
            resource_type,
            resource_id,
            actor,
            metadata,
            None,
        )
        .await
    }

    /// Get an audit event by ID
    #[instrument(skip(self))]
    pub async fn get_event(&self, id: &AuditEventId) -> Result<Option<AuditEvent>> {
        debug!("Getting audit event: {}", id);

        // In a real implementation, this would query the database
        // For now, we'll return None
        warn!("get_event not yet implemented");
        Ok(None)
    }

    /// Store an audit event in the database
    async fn store_event(&self, event: &AuditEvent) -> Result<()> {
        // In a real implementation, this would insert into the audit_events table
        // For now, we'll just log it
        debug!("Storing audit event: {}", serde_json::to_string(event)?);
        Ok(())
    }
}

/// Generate a unique audit event ID
fn generate_audit_event_id() -> AuditEventId {
    format!("audit_{}", Uuid::new_v4())
}

/// Middleware for automatic audit tracking
pub struct AuditMiddleware {
    tracker: Arc<AuditTracker>,
}

impl AuditMiddleware {
    /// Create new audit middleware
    pub fn new(tracker: Arc<AuditTracker>) -> Self {
        Self { tracker }
    }

    /// Track a request
    pub async fn track_request(
        &self,
        method: &str,
        path: &str,
        actor: Option<Actor>,
        metadata: Json,
    ) -> Result<AuditEventId> {
        self.tracker
            .track_event(
                AuditEventType::Custom("api_request".to_string()),
                ResourceType::Custom("api".to_string()),
                format!("{method} {path}"),
                actor,
                metadata,
                None,
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_audit_event_new() {
        let fixture = AuditEvent::new(
            AuditEventType::Created,
            ResourceType::Function,
            "func_123",
            Some(Actor::User("user_456".to_string())),
            serde_json::json!({"name": "test_function"}),
            None,
        );

        assert_eq!(fixture.event_type, AuditEventType::Created);
        assert_eq!(fixture.resource_type, ResourceType::Function);
        assert_eq!(fixture.resource_id, "func_123");
    }

    #[test]
    fn test_audit_event_created() {
        let fixture = AuditEvent::created(
            ResourceType::Function,
            "func_123",
            Some(Actor::User("user_456".to_string())),
            serde_json::json!({"name": "test_function"}),
        );

        assert_eq!(fixture.event_type, AuditEventType::Created);
        assert_eq!(fixture.resource_type, ResourceType::Function);
    }

    #[test]
    fn test_audit_event_type_display() {
        let fixture = AuditEventType::Created;
        let actual = fixture.to_string();
        let expected = "Created";
        assert_eq!(actual, expected);

        let fixture = AuditEventType::Custom("custom_event".to_string());
        let actual = fixture.to_string();
        let expected = "custom_event";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_resource_type_display() {
        let fixture = ResourceType::Function;
        let actual = fixture.to_string();
        let expected = "Function";
        assert_eq!(actual, expected);

        let fixture = ResourceType::Custom("custom_resource".to_string());
        let actual = fixture.to_string();
        let expected = "custom_resource";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_actor_display() {
        let fixture = Actor::User("user_123".to_string());
        let actual = fixture.to_string();
        let expected = "User(user_123)";
        assert_eq!(actual, expected);

        let fixture = Actor::Anonymous;
        let actual = fixture.to_string();
        let expected = "Anonymous";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_change_set() {
        let fixture = ChangeSet::new()
            .add_change(
                "name",
                serde_json::json!("old_name"),
                serde_json::json!("new_name"),
            )
            .add_change(
                "status",
                serde_json::json!("active"),
                serde_json::json!("inactive"),
            );

        assert_eq!(fixture.changed_fields().len(), 2);
        assert!(fixture.has_change("name"));
        assert!(fixture.has_change("status"));
        assert!(!fixture.has_change("description"));
    }

    #[test]
    fn test_generate_audit_event_id() {
        let fixture = generate_audit_event_id();
        assert!(fixture.starts_with("audit_"));
        assert_eq!(fixture.len(), 42); // "audit_" + UUID length
    }
}
