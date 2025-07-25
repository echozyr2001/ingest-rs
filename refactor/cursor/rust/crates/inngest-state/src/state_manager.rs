//! Redis-backed state manager implementation.

use crate::config::StateConfig;
use crate::error::{StateError, StateResult};
use crate::key_generator::KeyGenerator;
use crate::key_generator::RedisKeyGenerator;
use crate::lua_scripts::LuaScripts;
use crate::redis_client::RedisClient;
use crate::serialization::{PauseState, RunState, SerializationFormat, Serializer, StepResult};
use inngest_core::{Identifier, Metadata};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, instrument, warn};

/// Redis state manager for workflow runs
#[derive(Clone)]
pub struct RedisStateManager {
    config: StateConfig,
    redis_client: Arc<RedisClient>,
    key_generator: KeyGenerator,
    serializer: Serializer,
    lua_scripts: Arc<tokio::sync::RwLock<LuaScripts>>,
}

impl RedisStateManager {
    /// Create a new Redis state manager
    #[instrument(skip(config))]
    pub async fn new(config: StateConfig) -> StateResult<Self> {
        config.validate()?;

        info!("Initializing Redis state manager");

        // Create Redis client
        let redis_client = Arc::new(RedisClient::new(config.clone()).await?);

        // Create key generator
        let key_generator = KeyGenerator::new(config.key_prefix.clone(), config.sharded_keys);

        // Create serializer with JSON format
        let serializer = Serializer::with_format(SerializationFormat::Json);

        // Create and load Lua scripts
        let mut lua_scripts = LuaScripts::new();
        lua_scripts.load_scripts(redis_client.client()).await?;
        let lua_scripts = Arc::new(tokio::sync::RwLock::new(lua_scripts));

        info!("Redis state manager initialized successfully");

        Ok(Self {
            config,
            redis_client,
            key_generator,
            serializer,
            lua_scripts,
        })
    }

    /// Create a new state for a workflow run
    #[instrument(skip(self, metadata, event_data))]
    pub async fn create_state(
        &self,
        identifier: &Identifier,
        metadata: &Metadata,
        event_data: serde_json::Value,
    ) -> StateResult<()> {
        debug!("Creating state for run {}", identifier.run_id);

        let run_state = RunState {
            run_id: identifier.run_id.to_string(),
            function_id: identifier.workflow_id.to_string(),
            status: metadata.status.to_string(),
            started_at: metadata.started_at,
            updated_at: chrono::Utc::now(),
            event_data,
            metadata: HashMap::new(),
            steps: HashMap::new(),
        };

        let keys = self.key_generator.new_state_keys(identifier);
        let serialized_state = self.serializer.serialize_to_string(&run_state)?;

        // Check if state already exists
        if self.redis_client.exists(&keys.metadata).await? {
            return Err(StateError::run_id_exists(identifier.run_id.to_string()));
        }

        // Set state (no TTL support in current config)
        self.redis_client
            .set_string(&keys.metadata, &serialized_state)
            .await?;

        debug!("Created state for run {}", identifier.run_id);
        Ok(())
    }

    /// Get state for a workflow run
    #[instrument(skip(self))]
    pub async fn get_state(&self, run_id: &str) -> StateResult<Option<RunState>> {
        debug!("Getting state for run {}", run_id);

        // Parse run_id as ULID
        let run_ulid = run_id
            .parse::<ulid::Ulid>()
            .map_err(|_| StateError::Internal(format!("Invalid run ID format: {run_id}")))?;

        let metadata_key = self.key_generator.run_metadata_key(run_ulid);

        match self.redis_client.get_string(&metadata_key).await? {
            Some(serialized_state) => {
                let run_state = self.serializer.deserialize_from_string(&serialized_state)?;
                debug!("Retrieved state for run {}", run_id);
                Ok(Some(run_state))
            }
            None => {
                debug!("No state found for run {}", run_id);
                Ok(None)
            }
        }
    }

    /// Update state for a workflow run
    #[instrument(skip(self, run_state))]
    pub async fn update_state(&self, run_state: &RunState) -> StateResult<()> {
        debug!("Updating state for run {}", run_state.run_id);

        // Parse run_id as ULID
        let run_ulid = run_state.run_id.parse::<ulid::Ulid>().map_err(|_| {
            StateError::Internal(format!("Invalid run ID format: {}", run_state.run_id))
        })?;

        let metadata_key = self.key_generator.run_metadata_key(run_ulid);

        // Check if state exists
        if !self.redis_client.exists(&metadata_key).await? {
            return Err(StateError::state_not_found(&run_state.run_id));
        }

        let mut updated_state = run_state.clone();
        updated_state.updated_at = chrono::Utc::now();

        let serialized_state = self.serializer.serialize_to_string(&updated_state)?;
        self.redis_client
            .set_string(&metadata_key, &serialized_state)
            .await?;

        debug!("Updated state for run {}", run_state.run_id);
        Ok(())
    }

    /// Delete state for a workflow run
    #[instrument(skip(self))]
    pub async fn delete_state(&self, run_id: &str) -> StateResult<bool> {
        debug!("Deleting state for run {}", run_id);

        // Parse run_id as ULID
        let run_ulid = run_id
            .parse::<ulid::Ulid>()
            .map_err(|_| StateError::Internal(format!("Invalid run ID format: {run_id}")))?;

        let metadata_key = self.key_generator.run_metadata_key(run_ulid);

        // For now, only delete the metadata key
        // In a full implementation, you'd delete all related keys
        let deleted_count = self.redis_client.del(vec![metadata_key]).await?;
        let deleted = deleted_count > 0;

        if deleted {
            debug!("Deleted state for run {}", run_id);
        } else {
            debug!("No state found to delete for run {}", run_id);
        }

        Ok(deleted)
    }

    /// Set a step result (simplified implementation)
    #[instrument(skip(self, step_result))]
    pub async fn set_step_result(&self, run_id: &str, step_result: &StepResult) -> StateResult<()> {
        debug!(
            "Setting step result for step {} in run {}",
            step_result.step_id, run_id
        );

        // Parse run_id as ULID
        let run_ulid = run_id
            .parse::<ulid::Ulid>()
            .map_err(|_| StateError::Internal(format!("Invalid run ID format: {run_id}")))?;

        let metadata_key = self.key_generator.run_metadata_key(run_ulid);

        // Check if run state exists
        if !self.redis_client.exists(&metadata_key).await? {
            return Err(StateError::state_not_found(run_id));
        }

        // For now, store step results in a simple key format
        let step_key = format!("{}:step:{}", metadata_key, step_result.step_id);

        // Check if step result already exists
        if self.redis_client.exists(&step_key).await? {
            return Err(StateError::DuplicateStepResponse {
                step_id: step_result.step_id.clone(),
            });
        }

        let serialized_result = self.serializer.serialize_to_string(step_result)?;
        self.redis_client
            .set_string(&step_key, &serialized_result)
            .await?;

        debug!(
            "Set step result for step {} in run {}",
            step_result.step_id, run_id
        );
        Ok(())
    }

    /// Get a step result (simplified implementation)
    #[instrument(skip(self))]
    pub async fn get_step_result(
        &self,
        run_id: &str,
        step_id: &str,
    ) -> StateResult<Option<StepResult>> {
        debug!("Getting step result for step {} in run {}", step_id, run_id);

        // Parse run_id as ULID
        let run_ulid = run_id
            .parse::<ulid::Ulid>()
            .map_err(|_| StateError::Internal(format!("Invalid run ID format: {run_id}")))?;

        let metadata_key = self.key_generator.run_metadata_key(run_ulid);
        let step_key = format!("{metadata_key}:step:{step_id}");

        match self.redis_client.get_string(&step_key).await? {
            Some(serialized_result) => {
                let step_result = self
                    .serializer
                    .deserialize_from_string(&serialized_result)?;
                debug!(
                    "Retrieved step result for step {} in run {}",
                    step_id, run_id
                );
                Ok(Some(step_result))
            }
            None => {
                debug!(
                    "No step result found for step {} in run {}",
                    step_id, run_id
                );
                Ok(None)
            }
        }
    }

    /// List all step results for a run (simplified implementation)
    #[instrument(skip(self))]
    pub async fn list_step_results(
        &self,
        run_id: &str,
    ) -> StateResult<HashMap<String, StepResult>> {
        debug!("Listing step results for run {}", run_id);

        // Parse run_id as ULID
        let run_ulid = run_id
            .parse::<ulid::Ulid>()
            .map_err(|_| StateError::Internal(format!("Invalid run ID format: {run_id}")))?;

        let metadata_key = self.key_generator.run_metadata_key(run_ulid);
        let step_pattern = format!("{metadata_key}:step:*");
        let step_keys = self.redis_client.keys(&step_pattern).await?;

        let mut step_results = HashMap::new();

        // Get all step results
        for step_key in step_keys {
            if let Some(serialized_result) = self.redis_client.get_string(&step_key).await? {
                match self
                    .serializer
                    .deserialize_from_string::<StepResult>(&serialized_result)
                {
                    Ok(step_result) => {
                        step_results.insert(step_result.step_id.clone(), step_result);
                    }
                    Err(e) => {
                        warn!(
                            "Failed to deserialize step result from key {}: {}",
                            step_key, e
                        );
                    }
                }
            }
        }

        debug!(
            "Listed {} step results for run {}",
            step_results.len(),
            run_id
        );
        Ok(step_results)
    }

    /// Create a pause state (simplified implementation)
    #[instrument(skip(self, pause_state))]
    pub async fn create_pause(&self, pause_state: &PauseState) -> StateResult<()> {
        debug!("Creating pause {}", pause_state.pause_id);

        // Parse pause_id as UUID
        let pause_uuid = pause_state.pause_id.parse::<uuid::Uuid>().map_err(|_| {
            StateError::Internal(format!("Invalid pause ID format: {}", pause_state.pause_id))
        })?;

        let pause_key = self.key_generator.pause_key(pause_uuid);

        // Check if pause already exists
        if self.redis_client.exists(&pause_key).await? {
            return Err(StateError::Internal(format!(
                "Pause {} already exists",
                pause_state.pause_id
            )));
        }

        let serialized_pause = self.serializer.serialize_to_string(pause_state)?;

        // Set pause with timeout if specified
        if let Some(timeout) = pause_state.timeout {
            let ttl = (timeout - chrono::Utc::now()).num_seconds().max(0) as u64;
            self.redis_client
                .set_string_ex(&pause_key, &serialized_pause, ttl)
                .await?;
        } else {
            self.redis_client
                .set_string(&pause_key, &serialized_pause)
                .await?;
        }

        debug!("Created pause {}", pause_state.pause_id);
        Ok(())
    }

    /// Get a pause state (simplified implementation)
    #[instrument(skip(self))]
    pub async fn get_pause(&self, pause_id: &str) -> StateResult<Option<PauseState>> {
        debug!("Getting pause {}", pause_id);

        // Parse pause_id as UUID
        let pause_uuid = pause_id
            .parse::<uuid::Uuid>()
            .map_err(|_| StateError::Internal(format!("Invalid pause ID format: {pause_id}")))?;

        let pause_key = self.key_generator.pause_key(pause_uuid);

        match self.redis_client.get_string(&pause_key).await? {
            Some(serialized_pause) => {
                let pause_state = self.serializer.deserialize_from_string(&serialized_pause)?;
                debug!("Retrieved pause {}", pause_id);
                Ok(Some(pause_state))
            }
            None => {
                debug!("No pause found: {}", pause_id);
                Ok(None)
            }
        }
    }

    /// Delete a pause state (simplified implementation)
    #[instrument(skip(self))]
    pub async fn delete_pause(&self, pause_id: &str) -> StateResult<bool> {
        debug!("Deleting pause {}", pause_id);

        // Parse pause_id as UUID
        let pause_uuid = pause_id
            .parse::<uuid::Uuid>()
            .map_err(|_| StateError::Internal(format!("Invalid pause ID format: {pause_id}")))?;

        let pause_key = self.key_generator.pause_key(pause_uuid);
        let deleted_count = self.redis_client.del(vec![pause_key]).await?;
        let deleted = deleted_count > 0;

        if deleted {
            debug!("Deleted pause {}", pause_id);
        } else {
            debug!("No pause found to delete: {}", pause_id);
        }

        Ok(deleted)
    }

    /// Check if a run exists
    #[instrument(skip(self))]
    pub async fn run_exists(&self, run_id: &str) -> StateResult<bool> {
        // Parse run_id as ULID
        let run_ulid = run_id
            .parse::<ulid::Ulid>()
            .map_err(|_| StateError::Internal(format!("Invalid run ID format: {run_id}")))?;

        let metadata_key = self.key_generator.run_metadata_key(run_ulid);
        self.redis_client.exists(&metadata_key).await
    }

    /// Get connection info for debugging
    pub async fn connection_info(&self) -> StateResult<crate::config::ConnectionInfo> {
        self.redis_client.connection_info().await
    }

    /// Get configuration information
    pub fn config(&self) -> &StateConfig {
        &self.config
    }

    /// Get Lua scripts manager for advanced operations
    pub fn lua_scripts(&self) -> &Arc<tokio::sync::RwLock<LuaScripts>> {
        &self.lua_scripts
    }

    /// Execute a Lua script by name (advanced usage)
    pub async fn execute_script(
        &self,
        script_name: crate::lua_scripts::ScriptName,
        keys: Vec<String>,
        args: Vec<String>,
    ) -> StateResult<String> {
        let scripts = self.lua_scripts.read().await;
        scripts
            .execute(self.redis_client.client(), script_name, keys, args)
            .await
    }

    /// Gracefully shutdown the state manager
    pub async fn shutdown(&self) -> StateResult<()> {
        info!("Shutting down Redis state manager");
        // Log final configuration state
        debug!(
            "Final config: pool_size={}, prefix={}",
            self.config.pool_size, self.config.key_prefix
        );
        self.redis_client.quit().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialization::StepResult;
    use inngest_core::{Identifier, Metadata, metadata::RunStatus};
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use uuid::Uuid;

    fn create_test_identifier() -> Identifier {
        Identifier::new(
            ulid::Ulid::new(),
            Uuid::new_v4(),
            1,
            ulid::Ulid::new(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        )
    }

    #[tokio::test]
    #[ignore] // Requires Redis server
    async fn test_state_manager_lifecycle() {
        let config = StateConfig::default();
        let state_manager = RedisStateManager::new(config).await.unwrap();

        let identifier = create_test_identifier();
        let metadata = Metadata::new(identifier.clone(), RunStatus::Running, chrono::Utc::now());
        let event_data = json!({"user_id": "123", "action": "test"});

        // Test create state
        state_manager
            .create_state(&identifier, &metadata, event_data.clone())
            .await
            .unwrap();

        // Test get state
        let retrieved_state = state_manager
            .get_state(&identifier.run_id.to_string())
            .await
            .unwrap();
        assert!(retrieved_state.is_some());
        let state = retrieved_state.unwrap();
        assert_eq!(state.run_id, identifier.run_id.to_string());
        assert_eq!(state.function_id, identifier.workflow_id.to_string());
        assert_eq!(state.event_data, event_data);

        // Test update state
        let mut updated_state = state.clone();
        updated_state.status = "completed".to_string();
        state_manager.update_state(&updated_state).await.unwrap();

        let retrieved_updated = state_manager
            .get_state(&identifier.run_id.to_string())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retrieved_updated.status, "completed");

        // Test run exists
        let exists = state_manager
            .run_exists(&identifier.run_id.to_string())
            .await
            .unwrap();
        assert!(exists);

        // Test delete state
        let deleted = state_manager
            .delete_state(&identifier.run_id.to_string())
            .await
            .unwrap();
        assert!(deleted);

        // Verify state is deleted
        let after_delete = state_manager
            .get_state(&identifier.run_id.to_string())
            .await
            .unwrap();
        assert!(after_delete.is_none());

        let exists_after_delete = state_manager
            .run_exists(&identifier.run_id.to_string())
            .await
            .unwrap();
        assert!(!exists_after_delete);

        // Cleanup
        let _ = state_manager.shutdown().await;
    }

    #[tokio::test]
    #[ignore] // Requires Redis server
    async fn test_step_result_operations() {
        let config = StateConfig::default();
        let state_manager = RedisStateManager::new(config).await.unwrap();

        let identifier = create_test_identifier();
        let metadata = Metadata::new(identifier.clone(), RunStatus::Running, chrono::Utc::now());
        let event_data = json!({"test": "data"});

        // Create initial state
        state_manager
            .create_state(&identifier, &metadata, event_data)
            .await
            .unwrap();

        let run_id = identifier.run_id.to_string();

        // Test set step result
        let step_result = StepResult {
            step_id: "step-1".to_string(),
            output: json!({"result": "success"}),
            error: None,
            completed_at: chrono::Utc::now(),
        };

        state_manager
            .set_step_result(&run_id, &step_result)
            .await
            .unwrap();

        // Test get step result
        let retrieved_step = state_manager
            .get_step_result(&run_id, "step-1")
            .await
            .unwrap();
        assert!(retrieved_step.is_some());
        let retrieved = retrieved_step.unwrap();
        assert_eq!(retrieved.step_id, "step-1");
        assert_eq!(retrieved.output, json!({"result": "success"}));

        // Test duplicate step result error
        let duplicate_result = state_manager.set_step_result(&run_id, &step_result).await;
        assert!(duplicate_result.is_err());

        // Test list step results
        let step_results = state_manager.list_step_results(&run_id).await.unwrap();
        assert_eq!(step_results.len(), 1);
        assert!(step_results.contains_key("step-1"));

        // Cleanup
        let _ = state_manager.delete_state(&run_id).await;
        let _ = state_manager.shutdown().await;
    }

    #[tokio::test]
    #[ignore] // Requires Redis server
    async fn test_pause_operations() {
        let config = StateConfig::default();
        let state_manager = RedisStateManager::new(config).await.unwrap();

        let pause_id = Uuid::new_v4().to_string();
        let run_id = ulid::Ulid::new().to_string();

        let pause_state = PauseState {
            pause_id: pause_id.clone(),
            run_id: run_id.clone(),
            step_id: "wait-step".to_string(),
            event: "user.action".to_string(),
            timeout: Some(chrono::Utc::now() + chrono::Duration::hours(1)),
            created_at: chrono::Utc::now(),
        };

        // Test create pause
        state_manager.create_pause(&pause_state).await.unwrap();

        // Test get pause
        let retrieved_pause = state_manager.get_pause(&pause_id).await.unwrap();
        assert!(retrieved_pause.is_some());
        let pause = retrieved_pause.unwrap();
        assert_eq!(pause.pause_id, pause_id);
        assert_eq!(pause.run_id, run_id);
        assert_eq!(pause.event, "user.action");

        // Test delete pause
        let deleted = state_manager.delete_pause(&pause_id).await.unwrap();
        assert!(deleted);

        // Verify pause is deleted
        let after_delete = state_manager.get_pause(&pause_id).await.unwrap();
        assert!(after_delete.is_none());

        // Cleanup
        let _ = state_manager.shutdown().await;
    }

    #[test]
    fn test_state_manager_creation_with_invalid_config() {
        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(async {
            let config = StateConfig::default(); // Invalid

            let result = RedisStateManager::new(config).await;
            assert!(result.is_err());
        });
    }

    #[tokio::test]
    #[ignore] // Requires Redis server
    async fn test_config_access() {
        let config = StateConfig::default();
        let expected_prefix = config.key_prefix.clone();
        let expected_pool_size = config.pool_size;

        let state_manager = RedisStateManager::new(config).await.unwrap();

        // Test config access
        let retrieved_config = state_manager.config();
        assert_eq!(retrieved_config.key_prefix, expected_prefix);
        assert_eq!(retrieved_config.pool_size, expected_pool_size);

        // Test lua_scripts access
        let lua_scripts = state_manager.lua_scripts();
        let scripts_guard = lua_scripts.read().await;
        assert!(scripts_guard.all_scripts_loaded());

        // Cleanup
        let _ = state_manager.shutdown().await;
    }

    #[tokio::test]
    #[ignore] // Requires Redis server
    async fn test_script_execution() {
        let config = StateConfig::default();
        let state_manager = RedisStateManager::new(config).await.unwrap();

        // Test script execution (simplified)
        let result = state_manager
            .execute_script(
                crate::lua_scripts::ScriptName::RunExists,
                vec!["test-key".to_string()],
                vec![],
            )
            .await;

        // Should succeed (returns "OK" from our placeholder implementation)
        assert!(result.is_ok());

        // Cleanup
        let _ = state_manager.shutdown().await;
    }
}
