//! Storage interface for historical audit data
//!
//! This module provides storage abstractions specifically for audit data,
//! including specialized queries and optimizations for historical data access.

use crate::audit::{Actor, AuditEvent, AuditEventId, ResourceType};
use crate::error::Result;
use crate::query::{AuditQuery, AuditQueryResult};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, instrument};

/// Specialized storage interface for audit data
#[async_trait]
pub trait AuditStorage: Send + Sync {
    /// Store a new audit event
    async fn store_event(&self, event: &AuditEvent) -> Result<()>;

    /// Store multiple audit events in a batch
    async fn store_events_batch(&self, events: &[AuditEvent]) -> Result<()>;

    /// Get an audit event by ID
    async fn get_event(&self, id: &AuditEventId) -> Result<Option<AuditEvent>>;

    /// Query audit events with filters and pagination
    async fn query_events(&self, query: &AuditQuery) -> Result<AuditQueryResult>;

    /// Count events matching a query
    async fn count_events(&self, query: &AuditQuery) -> Result<u64>;

    /// Get events for a specific resource
    async fn get_events_by_resource(
        &self,
        resource_type: ResourceType,
        resource_id: &str,
    ) -> Result<Vec<AuditEvent>>;

    /// Get events by actor
    async fn get_events_by_actor(&self, actor_id: &str) -> Result<Vec<AuditEvent>>;

    /// Delete events older than a specific date
    async fn delete_events_before(&self, cutoff_date: DateTime<Utc>) -> Result<u64>;

    /// Get aggregated statistics
    async fn get_aggregations(&self, query: &AuditQuery) -> Result<HashMap<String, u64>>;

    /// Check storage health
    async fn health_check(&self) -> Result<bool>;
}

/// PostgreSQL implementation of audit storage
pub struct PostgresAuditStorage {
    storage: Arc<
        dyn ingest_storage::Storage<Transaction = ingest_storage::postgres::PostgresTransaction>,
    >,
}

impl PostgresAuditStorage {
    /// Create a new PostgreSQL audit storage
    pub fn new(
        storage: Arc<
            dyn ingest_storage::Storage<Transaction = ingest_storage::postgres::PostgresTransaction>,
        >,
    ) -> Self {
        Self { storage }
    }
}

#[async_trait]
impl AuditStorage for PostgresAuditStorage {
    #[instrument(skip(self, event))]
    async fn store_event(&self, event: &AuditEvent) -> Result<()> {
        debug!("Storing audit event: {}", event.id);

        // In a real implementation, this would execute an INSERT query
        // For now, just return success
        Ok(())
    }

    #[instrument(skip(self, events))]
    async fn store_events_batch(&self, events: &[AuditEvent]) -> Result<()> {
        debug!("Storing batch of {} audit events", events.len());

        // In a real implementation, this would use batch INSERT
        for event in events {
            self.store_event(event).await?;
        }

        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_event(&self, id: &AuditEventId) -> Result<Option<AuditEvent>> {
        debug!("Getting audit event: {}", id);

        // Placeholder implementation
        Ok(None)
    }

    #[instrument(skip(self, query))]
    async fn query_events(&self, query: &AuditQuery) -> Result<AuditQueryResult> {
        debug!("Querying audit events with filters");

        // Placeholder implementation
        Ok(AuditQueryResult {
            events: Vec::new(),
            total_count: 0,
            page: query.page.unwrap_or(0),
            page_size: query.page_size.unwrap_or(50),
            total_pages: 0,
            has_next_page: false,
            has_previous_page: false,
        })
    }

    #[instrument(skip(self, _query))]
    async fn count_events(&self, _query: &AuditQuery) -> Result<u64> {
        debug!("Counting audit events");

        // Placeholder implementation
        Ok(0)
    }

    #[instrument(skip(self))]
    async fn get_events_by_resource(
        &self,
        resource_type: ResourceType,
        resource_id: &str,
    ) -> Result<Vec<AuditEvent>> {
        debug!(
            "Getting events for resource: {:?} {}",
            resource_type, resource_id
        );

        // Placeholder implementation
        Ok(Vec::new())
    }

    #[instrument(skip(self))]
    async fn get_events_by_actor(&self, actor_id: &str) -> Result<Vec<AuditEvent>> {
        debug!("Getting events for actor: {}", actor_id);

        // Placeholder implementation
        Ok(Vec::new())
    }

    #[instrument(skip(self))]
    async fn delete_events_before(&self, cutoff_date: DateTime<Utc>) -> Result<u64> {
        debug!("Deleting events before: {}", cutoff_date);

        // Placeholder implementation
        Ok(0)
    }

    #[instrument(skip(self, _query))]
    async fn get_aggregations(&self, _query: &AuditQuery) -> Result<HashMap<String, u64>> {
        debug!("Getting aggregations for query");

        // Placeholder implementation
        Ok(HashMap::new())
    }

    #[instrument(skip(self))]
    async fn health_check(&self) -> Result<bool> {
        debug!("Checking audit storage health");

        // Use the underlying storage health check
        match self.storage.health_check().await {
            Ok(health) => Ok(health.healthy),
            Err(_) => Ok(false),
        }
    }
}

/// In-memory audit storage for testing
pub struct InMemoryAuditStorage {
    events: Arc<tokio::sync::RwLock<Vec<AuditEvent>>>,
}

impl InMemoryAuditStorage {
    /// Create a new in-memory audit storage
    pub fn new() -> Self {
        Self {
            events: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }

    /// Clear all stored events
    pub async fn clear(&self) {
        let mut events = self.events.write().await;
        events.clear();
    }

    /// Get the number of stored events
    pub async fn len(&self) -> usize {
        let events = self.events.read().await;
        events.len()
    }

    /// Check if storage is empty
    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }
}

impl Default for InMemoryAuditStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AuditStorage for InMemoryAuditStorage {
    async fn store_event(&self, event: &AuditEvent) -> Result<()> {
        let mut events = self.events.write().await;
        events.push(event.clone());
        Ok(())
    }

    async fn store_events_batch(&self, events: &[AuditEvent]) -> Result<()> {
        let mut stored_events = self.events.write().await;
        stored_events.extend_from_slice(events);
        Ok(())
    }

    async fn get_event(&self, id: &AuditEventId) -> Result<Option<AuditEvent>> {
        let events = self.events.read().await;
        Ok(events.iter().find(|e| e.id == *id).cloned())
    }

    async fn query_events(&self, query: &AuditQuery) -> Result<AuditQueryResult> {
        let events = self.events.read().await;
        let mut filtered_events: Vec<_> = events.iter().cloned().collect();

        // Apply filters
        if let Some(ref event_types) = query.event_types {
            filtered_events.retain(|e| event_types.contains(&e.event_type));
        }

        if let Some(ref resource_types) = query.resource_types {
            filtered_events.retain(|e| resource_types.contains(&e.resource_type));
        }

        if let Some(ref start_date) = query.start_date {
            filtered_events.retain(|e| e.timestamp >= *start_date);
        }

        if let Some(ref end_date) = query.end_date {
            filtered_events.retain(|e| e.timestamp <= *end_date);
        }

        let total_count = filtered_events.len() as u64;
        let page = query.page.unwrap_or(0);
        let page_size = query.page_size.unwrap_or(50) as usize;
        let start_idx = (page as usize) * page_size;
        let end_idx = std::cmp::min(start_idx + page_size, filtered_events.len());

        let paginated_events = if start_idx < filtered_events.len() {
            filtered_events[start_idx..end_idx].to_vec()
        } else {
            Vec::new()
        };

        let total_pages = total_count.div_ceil(page_size as u64);

        Ok(AuditQueryResult {
            events: paginated_events,
            total_count,
            page,
            page_size: page_size as u32,
            total_pages: total_pages as u32,
            has_next_page: page < total_pages as u32 - 1,
            has_previous_page: page > 0,
        })
    }

    async fn count_events(&self, query: &AuditQuery) -> Result<u64> {
        let result = self.query_events(query).await?;
        Ok(result.total_count)
    }

    async fn get_events_by_resource(
        &self,
        resource_type: ResourceType,
        resource_id: &str,
    ) -> Result<Vec<AuditEvent>> {
        let events = self.events.read().await;
        Ok(events
            .iter()
            .filter(|e| e.resource_type == resource_type && e.resource_id == resource_id)
            .cloned()
            .collect())
    }

    async fn get_events_by_actor(&self, actor_id: &str) -> Result<Vec<AuditEvent>> {
        let events = self.events.read().await;
        Ok(events
            .iter()
            .filter(|e| {
                if let Some(ref actor) = e.actor {
                    match actor {
                        Actor::User(id) => id == actor_id,
                        Actor::System(id) => id == actor_id,
                        Actor::ApiKey(id) => id == actor_id,
                        Actor::Service(id) => id == actor_id,
                        Actor::Anonymous => actor_id == "anonymous",
                    }
                } else {
                    false
                }
            })
            .cloned()
            .collect())
    }

    async fn delete_events_before(&self, cutoff_date: DateTime<Utc>) -> Result<u64> {
        let mut events = self.events.write().await;
        let initial_len = events.len();
        events.retain(|e| e.timestamp >= cutoff_date);
        Ok((initial_len - events.len()) as u64)
    }

    async fn get_aggregations(&self, _query: &AuditQuery) -> Result<HashMap<String, u64>> {
        let events = self.events.read().await;
        let mut aggregations = HashMap::new();

        // Count by event type
        for event in events.iter() {
            let key = format!("event_type_{:?}", event.event_type);
            *aggregations.entry(key).or_insert(0) += 1;
        }

        // Count by resource type
        for event in events.iter() {
            let key = format!("resource_type_{:?}", event.resource_type);
            *aggregations.entry(key).or_insert(0) += 1;
        }

        Ok(aggregations)
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AuditEventType;
    use crate::audit::Actor;
    use pretty_assertions::assert_eq;

    fn create_test_event() -> AuditEvent {
        AuditEvent {
            id: "test_event_123".to_string(),
            timestamp: Utc::now(),
            event_type: AuditEventType::Created,
            resource_type: ResourceType::Function,
            resource_id: "test-function".to_string(),
            actor: Some(Actor::User("user123".to_string())),
            metadata: serde_json::json!({"action": "create"}),
            changes: None,
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_in_memory_storage_store_event() {
        let fixture = InMemoryAuditStorage::new();
        let event = create_test_event();

        let actual = fixture.store_event(&event).await;
        assert!(actual.is_ok());

        let stored_count = fixture.len().await;
        assert_eq!(stored_count, 1);
    }

    #[tokio::test]
    async fn test_in_memory_storage_get_event() {
        let fixture = InMemoryAuditStorage::new();
        let event = create_test_event();
        let event_id = event.id.clone();

        fixture.store_event(&event).await.unwrap();

        let actual = fixture.get_event(&event_id).await.unwrap();
        assert!(actual.is_some());
        assert_eq!(actual.unwrap().id, event_id);
    }

    #[tokio::test]
    async fn test_in_memory_storage_query_events() {
        let fixture = InMemoryAuditStorage::new();
        let event = create_test_event();

        fixture.store_event(&event).await.unwrap();

        let query = AuditQuery::default();
        let actual = fixture.query_events(&query).await.unwrap();

        assert_eq!(actual.total_count, 1);
        assert_eq!(actual.events.len(), 1);
    }

    #[tokio::test]
    async fn test_in_memory_storage_batch_store() {
        let fixture = InMemoryAuditStorage::new();
        let events = vec![create_test_event(), create_test_event()];

        let actual = fixture.store_events_batch(&events).await;
        assert!(actual.is_ok());

        let stored_count = fixture.len().await;
        assert_eq!(stored_count, 2);
    }

    #[tokio::test]
    async fn test_in_memory_storage_clear() {
        let fixture = InMemoryAuditStorage::new();
        let event = create_test_event();

        fixture.store_event(&event).await.unwrap();
        assert_eq!(fixture.len().await, 1);

        fixture.clear().await;
        assert_eq!(fixture.len().await, 0);
        assert!(fixture.is_empty().await);
    }

    #[tokio::test]
    async fn test_in_memory_storage_health_check() {
        let fixture = InMemoryAuditStorage::new();
        let actual = fixture.health_check().await.unwrap();
        assert!(actual);
    }

    #[tokio::test]
    async fn test_in_memory_storage_get_events_by_resource() {
        let fixture = InMemoryAuditStorage::new();
        let event = create_test_event();

        fixture.store_event(&event).await.unwrap();

        let actual = fixture
            .get_events_by_resource(ResourceType::Function, "test-function")
            .await
            .unwrap();

        assert_eq!(actual.len(), 1);
        assert_eq!(actual[0].resource_id, "test-function");
    }

    #[tokio::test]
    async fn test_in_memory_storage_get_events_by_actor() {
        let fixture = InMemoryAuditStorage::new();
        let event = create_test_event();

        fixture.store_event(&event).await.unwrap();

        let actual = fixture.get_events_by_actor("user123").await.unwrap();

        assert_eq!(actual.len(), 1);
        assert_eq!(
            actual[0].actor.as_ref().unwrap(),
            &Actor::User("user123".to_string())
        );
    }
}
