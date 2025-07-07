//! Storage layer abstraction for state management
//!
//! This module provides storage abstractions and implementations for persisting
//! execution state, supporting both PostgreSQL for primary storage and Redis
//! for caching.

use crate::{ExecutionState, Result, StateError, StateSnapshot, StateTransition};
use async_trait::async_trait;
use ingest_core::Id;
use ingest_storage::{CacheStorage, PostgresStorage, RedisStorage};
use serde_json::Value as Json;
use sqlx::Row;

/// Trait for state storage operations
#[async_trait]
pub trait StateStorage: Send + Sync {
    /// Save execution state
    async fn save_state(&self, state: &ExecutionState) -> Result<()>;

    /// Get execution state by run ID
    async fn get_state(&self, run_id: &Id) -> Result<Option<ExecutionState>>;

    /// Update execution state with optimistic locking
    async fn update_state(&self, state: &ExecutionState) -> Result<()>;

    /// Delete execution state
    async fn delete_state(&self, run_id: &Id) -> Result<()>;

    /// Get states by function ID
    async fn get_states_by_function(&self, function_id: &Id) -> Result<Vec<ExecutionState>>;

    /// Get states by status
    async fn get_states_by_status(&self, status: &str) -> Result<Vec<ExecutionState>>;

    /// Save state snapshot
    async fn save_snapshot(&self, snapshot: &StateSnapshot) -> Result<()>;

    /// Get snapshots for a run
    async fn get_snapshots(&self, run_id: &Id) -> Result<Vec<StateSnapshot>>;

    /// Save state transition
    async fn save_transition(&self, transition: &StateTransition) -> Result<()>;

    /// Get transitions for a run
    async fn get_transitions(&self, run_id: &Id) -> Result<Vec<StateTransition>>;
}

/// PostgreSQL state storage implementation
pub struct PostgresStateStorage {
    storage: PostgresStorage,
}

impl PostgresStateStorage {
    pub fn new(storage: PostgresStorage) -> Self {
        Self { storage }
    }
}

#[async_trait]
impl StateStorage for PostgresStateStorage {
    async fn save_state(&self, state: &ExecutionState) -> Result<()> {
        let query = r#"
            INSERT INTO execution_states (
                run_id, function_id, status, current_step, variables, 
                context, version, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        "#;

        let pool = self.storage.pool();
        sqlx::query(query)
            .bind(state.run_id.as_str())
            .bind(state.function_id.as_str())
            .bind(state.status.as_str())
            .bind(state.current_step.as_ref().map(|id| id.as_str()))
            .bind(serde_json::to_value(&state.variables).unwrap())
            .bind(serde_json::to_value(&state.context).unwrap())
            .bind(state.version as i64)
            .bind(state.created_at)
            .bind(state.updated_at)
            .execute(pool)
            .await
            .map_err(|e| StateError::storage(format!("Failed to save state: {e}")))?;

        Ok(())
    }

    async fn get_state(&self, run_id: &Id) -> Result<Option<ExecutionState>> {
        let query = r#"
            SELECT run_id, function_id, status, current_step, variables,
                   context, version, created_at, updated_at
            FROM execution_states WHERE run_id = $1
        "#;

        let pool = self.storage.pool();
        let row = sqlx::query(query)
            .bind(run_id.as_str())
            .fetch_optional(pool)
            .await
            .map_err(|e| StateError::storage(format!("Failed to get state: {e}")))?;

        if let Some(row) = row {
            let state = ExecutionState {
                run_id: Id::from(row.get::<String, _>("run_id")),
                function_id: Id::from(row.get::<String, _>("function_id")),
                status: row
                    .get::<String, _>("status")
                    .parse()
                    .map_err(|e| StateError::invalid_status(format!("Invalid status: {e}")))?,
                current_step: row.get::<Option<String>, _>("current_step").map(Id::from),
                variables: serde_json::from_str(&row.get::<String, _>("variables"))
                    .unwrap_or_default(),
                context: serde_json::from_str(&row.get::<String, _>("context")).unwrap_or_default(),
                version: row.get::<i64, _>("version") as u64,
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                scheduled_at: row.get("scheduled_at"),
                completed_at: row.get("completed_at"), // Will be set based on status
            };
            Ok(Some(state))
        } else {
            Ok(None)
        }
    }

    async fn update_state(&self, state: &ExecutionState) -> Result<()> {
        let query = r#"
            UPDATE execution_states 
            SET status = $2, current_step = $3, variables = $4, context = $5,
                version = $6, updated_at = $7
            WHERE run_id = $1 AND version = $8
        "#;

        let pool = self.storage.pool();
        let result = sqlx::query(query)
            .bind(state.run_id.as_str())
            .bind(state.status.as_str())
            .bind(state.current_step.as_ref().map(|id| id.as_str()))
            .bind(serde_json::to_value(&state.variables).unwrap())
            .bind(serde_json::to_value(&state.context).unwrap())
            .bind(state.version as i64)
            .bind(state.updated_at)
            .bind((state.version - 1) as i64) // Previous version for optimistic locking
            .execute(pool)
            .await
            .map_err(|e| StateError::storage(format!("Failed to update state: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(StateError::optimistic_lock_failure(
                "State was modified by another process".to_string(),
            ));
        }

        Ok(())
    }

    async fn delete_state(&self, run_id: &Id) -> Result<()> {
        let query = "DELETE FROM execution_states WHERE run_id = $1";

        let pool = self.storage.pool();
        sqlx::query(query)
            .bind(run_id.as_str())
            .execute(pool)
            .await
            .map_err(|e| StateError::storage(format!("Failed to delete state: {e}")))?;

        Ok(())
    }

    async fn get_states_by_function(&self, function_id: &Id) -> Result<Vec<ExecutionState>> {
        let query = r#"
            SELECT run_id, function_id, status, current_step, variables,
                   context, version, created_at, updated_at
            FROM execution_states WHERE function_id = $1
            ORDER BY created_at DESC
        "#;

        let pool = self.storage.pool();
        let rows = sqlx::query(query)
            .bind(function_id.as_str())
            .fetch_all(pool)
            .await
            .map_err(|e| StateError::storage(format!("Failed to get states by function: {e}")))?;

        let mut states = Vec::new();
        for row in rows {
            let state = ExecutionState {
                run_id: Id::from(row.get::<String, _>("run_id")),
                function_id: Id::from(row.get::<String, _>("function_id")),
                status: row
                    .get::<String, _>("status")
                    .parse()
                    .map_err(|e| StateError::invalid_status(format!("Invalid status: {e}")))?,
                current_step: row.get::<Option<String>, _>("current_step").map(Id::from),
                variables: serde_json::from_str(&row.get::<String, _>("variables"))
                    .unwrap_or_default(),
                context: serde_json::from_str(&row.get::<String, _>("context")).unwrap_or_default(),
                version: row.get::<i64, _>("version") as u64,
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                scheduled_at: row.get("scheduled_at"),
                completed_at: row.get("completed_at"),
            };
            states.push(state);
        }

        Ok(states)
    }

    async fn get_states_by_status(&self, status: &str) -> Result<Vec<ExecutionState>> {
        let query = r#"
            SELECT run_id, function_id, status, current_step, variables,
                   context, version, created_at, updated_at
            FROM execution_states WHERE status = $1
            ORDER BY created_at DESC
        "#;

        let pool = self.storage.pool();
        let rows = sqlx::query(query)
            .bind(status)
            .fetch_all(pool)
            .await
            .map_err(|e| StateError::storage(format!("Failed to get states by status: {e}")))?;

        let mut states = Vec::new();
        for row in rows {
            let state = ExecutionState {
                run_id: Id::from(row.get::<String, _>("run_id")),
                function_id: Id::from(row.get::<String, _>("function_id")),
                status: row
                    .get::<String, _>("status")
                    .parse()
                    .map_err(|e| StateError::invalid_status(format!("Invalid status: {e}")))?,
                current_step: row.get::<Option<String>, _>("current_step").map(Id::from),
                variables: serde_json::from_str(&row.get::<String, _>("variables"))
                    .unwrap_or_default(),
                context: serde_json::from_str(&row.get::<String, _>("context")).unwrap_or_default(),
                version: row.get::<i64, _>("version") as u64,
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                scheduled_at: row.get("scheduled_at"),
                completed_at: row.get("completed_at"),
            };
            states.push(state);
        }

        Ok(states)
    }

    async fn save_snapshot(&self, snapshot: &StateSnapshot) -> Result<()> {
        let query = r#"
            INSERT INTO state_snapshots (id, run_id, state_data, checkpoint_name, created_at)
            VALUES ($1, $2, $3, $4, $5)
        "#;

        let state_json = serde_json::to_value(&snapshot.state)
            .map_err(|e| StateError::serialization(format!("Failed to serialize state: {e}")))?;

        let pool = self.storage.pool();
        sqlx::query(query)
            .bind(snapshot.id.as_str())
            .bind(snapshot.run_id.as_str())
            .bind(&state_json)
            .bind(&snapshot.checkpoint_name)
            .bind(snapshot.created_at)
            .execute(pool)
            .await
            .map_err(|e| StateError::storage(format!("Failed to save snapshot: {e}")))?;

        Ok(())
    }

    async fn get_snapshots(&self, run_id: &Id) -> Result<Vec<StateSnapshot>> {
        let query = r#"
            SELECT id, run_id, state_data, checkpoint_name, created_at
            FROM state_snapshots WHERE run_id = $1
            ORDER BY created_at DESC
        "#;

        let pool = self.storage.pool();
        let rows = sqlx::query(query)
            .bind(run_id.as_str())
            .fetch_all(pool)
            .await
            .map_err(|e| StateError::storage(format!("Failed to get snapshots: {e}")))?;

        let mut snapshots = Vec::new();
        for row in rows {
            let state_json: Json = row.get("state_data");
            let state: ExecutionState = serde_json::from_value(state_json).map_err(|e| {
                StateError::serialization(format!("Failed to deserialize state: {e}"))
            })?;

            let snapshot = StateSnapshot {
                id: Id::from(row.get::<String, _>("id")),
                run_id: Id::from(row.get::<String, _>("run_id")),
                state,
                checkpoint_name: row.get("checkpoint_name"),
                metadata: std::collections::HashMap::new(), // Add missing field
                created_at: row.get("created_at"),
            };
            snapshots.push(snapshot);
        }

        Ok(snapshots)
    }

    async fn save_transition(&self, transition: &StateTransition) -> Result<()> {
        let query = r#"
            INSERT INTO state_transitions (id, run_id, from_status, to_status, reason, metadata, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#;

        let pool = self.storage.pool();
        sqlx::query(query)
            .bind(transition.id.as_str())
            .bind(transition.run_id.as_str())
            .bind(transition.from_status.as_str())
            .bind(transition.to_status.as_str())
            .bind(&transition.reason)
            .bind(serde_json::to_string(&transition.metadata).unwrap_or_default())
            .bind(transition.created_at)
            .execute(pool)
            .await
            .map_err(|e| StateError::storage(format!("Failed to save transition: {e}")))?;

        Ok(())
    }

    async fn get_transitions(&self, run_id: &Id) -> Result<Vec<StateTransition>> {
        let query = r#"
            SELECT id, run_id, from_status, to_status, reason, metadata, created_at
            FROM state_transitions WHERE run_id = $1
            ORDER BY created_at ASC
        "#;

        let pool = self.storage.pool();
        let rows = sqlx::query(query)
            .bind(run_id.as_str())
            .fetch_all(pool)
            .await
            .map_err(|e| StateError::storage(format!("Failed to get transitions: {e}")))?;

        let mut transitions = Vec::new();
        for row in rows {
            let transition = StateTransition {
                id: Id::from(row.get::<String, _>("id")),
                run_id: Id::from(row.get::<String, _>("run_id")),
                from_status: row.get::<String, _>("from_status").parse().unwrap(),
                to_status: row.get::<String, _>("to_status").parse().unwrap(),
                reason: row.get("reason"),
                metadata: serde_json::from_str(&row.get::<String, _>("metadata"))
                    .unwrap_or_default(),
                created_at: row.get("created_at"),
                step_id: row.get::<Option<String>, _>("step_id").map(Id::from),
                from_version: row.get::<i64, _>("from_version") as u64,
                to_version: row.get::<i64, _>("to_version") as u64,
            };
            transitions.push(transition);
        }

        Ok(transitions)
    }
}

/// Redis cache storage implementation
pub struct CacheStateStorage {
    storage: RedisStorage,
    ttl_seconds: u64,
}

impl CacheStateStorage {
    pub fn new(storage: RedisStorage, ttl_seconds: u64) -> Self {
        Self {
            storage,
            ttl_seconds,
        }
    }

    fn state_key(&self, run_id: &Id) -> String {
        format!("state:{run_id}")
    }
}

#[async_trait]
impl StateStorage for CacheStateStorage {
    async fn save_state(&self, state: &ExecutionState) -> Result<()> {
        let key = self.state_key(&state.run_id);
        let value = serde_json::to_string(state)
            .map_err(|e| StateError::serialization(format!("Failed to serialize state: {e}")))?;

        self.storage
            .set(
                &key,
                value.as_bytes(),
                Some(std::time::Duration::from_secs(self.ttl_seconds)),
            )
            .await
            .map_err(|e| StateError::cache(format!("Failed to cache state: {e}")))?;

        Ok(())
    }

    async fn get_state(&self, run_id: &Id) -> Result<Option<ExecutionState>> {
        let key = self.state_key(run_id);

        if let Some(value_bytes) = self
            .storage
            .get(&key)
            .await
            .map_err(|e| StateError::cache(format!("Failed to get cached state: {e}")))?
        {
            let value = String::from_utf8(value_bytes).map_err(|e| {
                StateError::serialization(format!("Failed to decode cached state: {e}"))
            })?;
            let state: ExecutionState = serde_json::from_str(&value).map_err(|e| {
                StateError::serialization(format!("Failed to deserialize cached state: {e}"))
            })?;
            Ok(Some(state))
        } else {
            Ok(None)
        }
    }

    async fn update_state(&self, state: &ExecutionState) -> Result<()> {
        // For cache, we just overwrite
        self.save_state(state).await
    }

    async fn delete_state(&self, run_id: &Id) -> Result<()> {
        let key = self.state_key(run_id);
        self.storage
            .delete(&key)
            .await
            .map_err(|e| StateError::cache(format!("Failed to delete cached state: {e}")))?;
        Ok(())
    }

    async fn get_states_by_function(&self, _function_id: &Id) -> Result<Vec<ExecutionState>> {
        // Cache doesn't support complex queries, return empty
        Ok(Vec::new())
    }

    async fn get_states_by_status(&self, _status: &str) -> Result<Vec<ExecutionState>> {
        // Cache doesn't support complex queries, return empty
        Ok(Vec::new())
    }

    async fn save_snapshot(&self, _snapshot: &StateSnapshot) -> Result<()> {
        // Snapshots are not cached
        Ok(())
    }

    async fn get_snapshots(&self, _run_id: &Id) -> Result<Vec<StateSnapshot>> {
        // Snapshots are not cached
        Ok(Vec::new())
    }

    async fn save_transition(&self, _transition: &StateTransition) -> Result<()> {
        // Transitions are not cached
        Ok(())
    }

    async fn get_transitions(&self, _run_id: &Id) -> Result<Vec<StateTransition>> {
        // Transitions are not cached
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn test_state_storage_operations() {
        // This would require a test database setup
        // Implementation would go here
    }
}
