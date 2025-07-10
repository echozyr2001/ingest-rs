use crate::types::ExecutionConfig;
use crate::{ExecutionError, Result};
use ingest_core::{DateTime, Id, Json, StateManager};
use std::collections::HashMap;
use std::sync::Arc;

/// Execution context containing runtime state and resources
#[derive(Clone)]
pub struct ExecutionContext {
    /// Execution run identifier
    pub run_id: Id,
    /// Function identifier
    pub function_id: Id,
    /// Execution configuration
    pub config: ExecutionConfig,
    /// State manager for persistence
    pub state_manager: Arc<dyn StateManager>,
    /// Execution start time
    pub started_at: DateTime,
    /// Execution metadata
    pub metadata: HashMap<String, String>,
    /// Shared execution data
    pub data: Arc<tokio::sync::RwLock<HashMap<String, Json>>>,
}

impl ExecutionContext {
    /// Create a new execution context
    pub fn new(
        run_id: Id,
        function_id: Id,
        config: ExecutionConfig,
        state_manager: Arc<dyn StateManager>,
    ) -> Self {
        Self {
            run_id,
            function_id,
            config,
            state_manager,
            started_at: chrono::Utc::now(),
            metadata: HashMap::new(),
            data: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Add metadata to the context
    pub fn add_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    /// Get metadata value
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    /// Set shared data
    pub async fn set_data(&self, key: impl Into<String>, value: Json) -> Result<()> {
        let mut data = self.data.write().await;
        data.insert(key.into(), value);
        Ok(())
    }

    /// Get shared data
    pub async fn get_data(&self, key: &str) -> Result<Option<Json>> {
        let data = self.data.read().await;
        Ok(data.get(key).cloned())
    }

    /// Save state to persistent storage
    pub async fn save_state(&self, key: &str, data: &Json) -> Result<()> {
        self.state_manager
            .save_state(key, data)
            .await
            .map_err(|e| ExecutionError::internal(format!("State save failed: {e}")))
    }

    /// Load state from persistent storage
    pub async fn load_state(&self, key: &str) -> Result<Option<Json>> {
        self.state_manager
            .load_state(key)
            .await
            .map_err(|e| ExecutionError::internal(format!("State load failed: {e}")))
    }

    /// Check if execution has timed out
    pub fn is_timed_out(&self) -> bool {
        if let Some(timeout) = self.config.timeout {
            let elapsed = chrono::Utc::now() - self.started_at;
            elapsed.to_std().unwrap_or_default() > timeout
        } else {
            false
        }
    }

    /// Get execution duration so far
    pub fn duration(&self) -> std::time::Duration {
        let elapsed = chrono::Utc::now() - self.started_at;
        elapsed.to_std().unwrap_or_default()
    }

    /// Create a scoped context for a step
    pub fn for_step(&self, step_id: Id) -> StepContext {
        StepContext {
            execution_context: self.clone(),
            step_id,
            step_data: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }
}

/// Step-specific execution context
#[derive(Clone)]
pub struct StepContext {
    /// Parent execution context
    pub execution_context: ExecutionContext,
    /// Step identifier
    pub step_id: Id,
    /// Step-specific data
    pub step_data: Arc<tokio::sync::RwLock<HashMap<String, Json>>>,
}

impl StepContext {
    /// Set step-specific data
    pub async fn set_step_data(&self, key: impl Into<String>, value: Json) -> Result<()> {
        let mut data = self.step_data.write().await;
        data.insert(key.into(), value);
        Ok(())
    }

    /// Get step-specific data
    pub async fn get_step_data(&self, key: &str) -> Result<Option<Json>> {
        let data = self.step_data.read().await;
        Ok(data.get(key).cloned())
    }

    /// Save step state to persistent storage
    pub async fn save_step_state(&self, data: &Json) -> Result<()> {
        let key = format!("step:{}:state", self.step_id.as_str());
        self.execution_context.save_state(&key, data).await
    }

    /// Load step state from persistent storage
    pub async fn load_step_state(&self) -> Result<Option<Json>> {
        let key = format!("step:{}:state", self.step_id.as_str());
        self.execution_context.load_state(&key).await
    }

    /// Access execution-level data
    pub async fn set_execution_data(&self, key: impl Into<String>, value: Json) -> Result<()> {
        self.execution_context.set_data(key, value).await
    }

    /// Get execution-level data
    pub async fn get_execution_data(&self, key: &str) -> Result<Option<Json>> {
        self.execution_context.get_data(key).await
    }
}

/// Context builder for creating execution contexts
pub struct ExecutionContextBuilder {
    run_id: Id,
    function_id: Id,
    config: ExecutionConfig,
    state_manager: Option<Arc<dyn StateManager>>,
    metadata: HashMap<String, String>,
}

impl ExecutionContextBuilder {
    /// Create a new context builder
    pub fn new(run_id: Id, function_id: Id) -> Self {
        Self {
            run_id,
            function_id,
            config: ExecutionConfig::default(),
            state_manager: None,
            metadata: HashMap::new(),
        }
    }

    /// Set execution configuration
    pub fn config(mut self, config: ExecutionConfig) -> Self {
        self.config = config;
        self
    }

    /// Set state manager
    pub fn state_manager(mut self, state_manager: Arc<dyn StateManager>) -> Self {
        self.state_manager = Some(state_manager);
        self
    }

    /// Add metadata
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Build the execution context
    pub fn build(self) -> Result<ExecutionContext> {
        let state_manager = self
            .state_manager
            .ok_or_else(|| crate::ExecutionError::configuration("State manager is required"))?;

        let mut context =
            ExecutionContext::new(self.run_id, self.function_id, self.config, state_manager);
        context.metadata = self.metadata;
        Ok(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingest_core::generate_id_with_prefix;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::sync::Arc;

    // Mock state manager for testing
    #[derive(Debug)]
    struct MockStateManager {
        data: Arc<tokio::sync::RwLock<HashMap<String, Json>>>,
    }

    impl MockStateManager {
        fn new() -> Self {
            Self {
                data: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            }
        }
    }

    #[async_trait::async_trait]
    impl StateManager for MockStateManager {
        async fn save_state(&self, key: &str, data: &Json) -> ingest_core::Result<()> {
            let mut storage = self.data.write().await;
            storage.insert(key.to_string(), data.clone());
            Ok(())
        }

        async fn load_state(&self, key: &str) -> ingest_core::Result<Option<Json>> {
            let storage = self.data.read().await;
            Ok(storage.get(key).cloned())
        }

        async fn delete_state(&self, key: &str) -> ingest_core::Result<()> {
            let mut storage = self.data.write().await;
            storage.remove(key);
            Ok(())
        }

        async fn state_exists(&self, key: &str) -> ingest_core::Result<bool> {
            let storage = self.data.read().await;
            Ok(storage.contains_key(key))
        }

        async fn list_state_keys(&self, prefix: Option<&str>) -> ingest_core::Result<Vec<String>> {
            let storage = self.data.read().await;
            let keys: Vec<String> = if let Some(prefix) = prefix {
                storage
                    .keys()
                    .filter(|k| k.starts_with(prefix))
                    .cloned()
                    .collect()
            } else {
                storage.keys().cloned().collect()
            };
            Ok(keys)
        }

        async fn clear_all_state(&self) -> ingest_core::Result<()> {
            let mut storage = self.data.write().await;
            storage.clear();
            Ok(())
        }
    }

    fn create_test_context() -> ExecutionContext {
        let run_id = generate_id_with_prefix("run");
        let function_id = generate_id_with_prefix("fn");
        let config = ExecutionConfig::default();
        let state_manager = Arc::new(MockStateManager::new());

        ExecutionContext::new(run_id, function_id, config, state_manager)
    }

    #[test]
    fn test_execution_context_creation() {
        let fixture_run_id = generate_id_with_prefix("run");
        let fixture_function_id = generate_id_with_prefix("fn");
        let fixture_config = ExecutionConfig::default();
        let fixture_state_manager = Arc::new(MockStateManager::new());

        let actual = ExecutionContext::new(
            fixture_run_id.clone(),
            fixture_function_id.clone(),
            fixture_config.clone(),
            fixture_state_manager,
        );

        assert_eq!(actual.run_id, fixture_run_id);
        assert_eq!(actual.function_id, fixture_function_id);
        assert_eq!(actual.config, fixture_config);
    }

    #[test]
    fn test_execution_context_metadata() {
        let mut fixture = create_test_context();

        fixture.add_metadata("source", "test");
        fixture.add_metadata("version", "1.0");

        let actual_source = fixture.get_metadata("source");
        let actual_version = fixture.get_metadata("version");

        assert_eq!(actual_source, Some(&"test".to_string()));
        assert_eq!(actual_version, Some(&"1.0".to_string()));
    }

    #[tokio::test]
    async fn test_execution_context_data() {
        let fixture = create_test_context();
        let test_data = json!({"key": "value"});

        fixture.set_data("test", test_data.clone()).await.unwrap();
        let actual = fixture.get_data("test").await.unwrap();

        assert_eq!(actual, Some(test_data));
    }

    #[tokio::test]
    async fn test_execution_context_state() {
        let fixture = create_test_context();
        let test_data = json!({"state": "value"});

        fixture.save_state("test-key", &test_data).await.unwrap();
        let actual = fixture.load_state("test-key").await.unwrap();

        assert_eq!(actual, Some(test_data));
    }

    #[test]
    fn test_execution_context_timeout() {
        let mut fixture = create_test_context();
        fixture.config.timeout = Some(std::time::Duration::from_millis(1));

        // Wait for timeout
        std::thread::sleep(std::time::Duration::from_millis(2));

        assert!(fixture.is_timed_out());
    }

    #[test]
    fn test_execution_context_duration() {
        let fixture = create_test_context();
        std::thread::sleep(std::time::Duration::from_millis(1));

        let actual = fixture.duration();
        assert!(actual.as_millis() >= 1);
    }

    #[test]
    fn test_step_context_creation() {
        let fixture_execution_context = create_test_context();
        let fixture_step_id = generate_id_with_prefix("step");

        let actual = fixture_execution_context.for_step(fixture_step_id.clone());

        assert_eq!(actual.step_id, fixture_step_id);
        assert_eq!(
            actual.execution_context.run_id,
            fixture_execution_context.run_id
        );
    }

    #[tokio::test]
    async fn test_step_context_step_data() {
        let fixture_execution_context = create_test_context();
        let fixture_step_context =
            fixture_execution_context.for_step(generate_id_with_prefix("step"));
        let test_data = json!({"step": "data"});

        fixture_step_context
            .set_step_data("test", test_data.clone())
            .await
            .unwrap();
        let actual = fixture_step_context.get_step_data("test").await.unwrap();

        assert_eq!(actual, Some(test_data));
    }

    #[tokio::test]
    async fn test_step_context_state() {
        let fixture_execution_context = create_test_context();
        let fixture_step_context =
            fixture_execution_context.for_step(generate_id_with_prefix("step"));
        let test_data = json!({"step": "state"});

        fixture_step_context
            .save_step_state(&test_data)
            .await
            .unwrap();
        let actual = fixture_step_context.load_step_state().await.unwrap();

        assert_eq!(actual, Some(test_data));
    }

    #[test]
    fn test_execution_context_builder() {
        let fixture_run_id = generate_id_with_prefix("run");
        let fixture_function_id = generate_id_with_prefix("fn");
        let fixture_state_manager = Arc::new(MockStateManager::new());

        let actual =
            ExecutionContextBuilder::new(fixture_run_id.clone(), fixture_function_id.clone())
                .state_manager(fixture_state_manager)
                .metadata("source", "test")
                .build()
                .unwrap();

        assert_eq!(actual.run_id, fixture_run_id);
        assert_eq!(actual.function_id, fixture_function_id);
        assert_eq!(actual.get_metadata("source"), Some(&"test".to_string()));
    }

    #[test]
    fn test_execution_context_builder_missing_state_manager() {
        let fixture_run_id = generate_id_with_prefix("run");
        let fixture_function_id = generate_id_with_prefix("fn");

        let actual = ExecutionContextBuilder::new(fixture_run_id, fixture_function_id).build();

        assert!(actual.is_err());
    }
}
