use crate::context::StepContext;
use crate::{ExecutionContext, ExecutionError, Result};
use async_trait::async_trait;
use ingest_core::{Step, StepOutput};
use std::collections::HashMap;
use std::sync::Arc;

/// Trait for executing individual steps
#[async_trait]
pub trait StepExecutor: Send + Sync {
    /// Execute a step with the given context
    async fn execute(&self, step: &Step, context: &StepContext) -> Result<StepOutput>;

    /// Check if this executor can handle the given step
    fn can_execute(&self, step: &Step) -> bool;

    /// Get the step type this executor handles
    fn step_type(&self) -> &str;

    /// Get executor name for identification
    fn name(&self) -> &str;
}

/// Registry for managing step executors
pub struct StepExecutorRegistry {
    executors: HashMap<String, Arc<dyn StepExecutor>>,
}

impl StepExecutorRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            executors: HashMap::new(),
        }
    }

    /// Register a step executor
    pub fn register(&mut self, executor: Arc<dyn StepExecutor>) {
        self.executors
            .insert(executor.step_type().to_string(), executor);
    }

    /// Get an executor for a step type
    pub fn get_executor(&self, step_type: &str) -> Option<Arc<dyn StepExecutor>> {
        self.executors.get(step_type).cloned()
    }

    /// Find the best executor for a step
    pub fn find_executor(&self, step: &Step) -> Option<Arc<dyn StepExecutor>> {
        self.executors
            .values()
            .find(|executor| executor.can_execute(step))
            .cloned()
    }

    /// List all registered step types
    pub fn list_step_types(&self) -> Vec<String> {
        self.executors.keys().cloned().collect()
    }

    /// Get the number of registered executors
    pub fn len(&self) -> usize {
        self.executors.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.executors.is_empty()
    }
}

impl Default for StepExecutorRegistry {
    fn default() -> Self {
        let mut registry = Self::new();

        // Register default executors
        registry.register(Arc::new(DefaultStepExecutor));
        registry.register(Arc::new(SleepStepExecutor));
        registry.register(Arc::new(WaitStepExecutor));

        registry
    }
}

/// Default step executor for basic steps
#[derive(Debug)]
pub struct DefaultStepExecutor;

#[async_trait]
impl StepExecutor for DefaultStepExecutor {
    async fn execute(&self, step: &Step, context: &StepContext) -> Result<StepOutput> {
        tracing::info!("Executing default step: {}", step.name);

        // Save step state
        if let Some(input) = &step.input {
            context.save_step_state(input).await?;
        }

        // Simulate step execution
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Create success output
        let output_data = serde_json::json!({
            "step_id": step.id.as_str(),
            "step_name": step.name,
            "executed_at": chrono::Utc::now(),
            "input": step.input
        });

        Ok(StepOutput::success(output_data))
    }

    fn can_execute(&self, _step: &Step) -> bool {
        true // Default executor can handle any step
    }

    fn step_type(&self) -> &str {
        "default"
    }

    fn name(&self) -> &str {
        "DefaultStepExecutor"
    }
}

/// Step executor for sleep/delay steps
#[derive(Debug)]
pub struct SleepStepExecutor;

#[async_trait]
impl StepExecutor for SleepStepExecutor {
    async fn execute(&self, step: &Step, context: &StepContext) -> Result<StepOutput> {
        tracing::info!("Executing sleep step: {}", step.name);

        // Extract sleep duration from step input or config
        let duration = self.extract_sleep_duration(step)?;

        // Save sleep state
        let sleep_data = serde_json::json!({
            "duration_ms": duration.as_millis(),
            "started_at": chrono::Utc::now()
        });
        context.save_step_state(&sleep_data).await?;

        // Perform the sleep
        tokio::time::sleep(duration).await;

        // Create success output
        let output_data = serde_json::json!({
            "step_id": step.id.as_str(),
            "slept_for_ms": duration.as_millis(),
            "completed_at": chrono::Utc::now()
        });

        Ok(StepOutput::success(output_data))
    }

    fn can_execute(&self, step: &Step) -> bool {
        step.name.contains("sleep") || step.name.contains("delay")
    }

    fn step_type(&self) -> &str {
        "sleep"
    }

    fn name(&self) -> &str {
        "SleepStepExecutor"
    }
}

impl SleepStepExecutor {
    fn extract_sleep_duration(&self, step: &Step) -> Result<tokio::time::Duration> {
        // Try to get duration from step input
        if let Some(input) = &step.input {
            if let Some(duration_ms) = input.get("duration_ms").and_then(|v| v.as_u64()) {
                return Ok(tokio::time::Duration::from_millis(duration_ms));
            }
            if let Some(duration_secs) = input.get("duration_secs").and_then(|v| v.as_u64()) {
                return Ok(tokio::time::Duration::from_secs(duration_secs));
            }
        }

        // Try to get duration from step config
        if let Some(duration_ms) = step
            .config
            .config
            .get("duration_ms")
            .and_then(|v| v.as_u64())
        {
            return Ok(tokio::time::Duration::from_millis(duration_ms));
        }

        // Default to 1 second
        Ok(tokio::time::Duration::from_secs(1))
    }
}

/// Step executor for wait/event steps
#[derive(Debug)]
pub struct WaitStepExecutor;

#[async_trait]
impl StepExecutor for WaitStepExecutor {
    async fn execute(&self, step: &Step, context: &StepContext) -> Result<StepOutput> {
        tracing::info!("Executing wait step: {}", step.name);

        // Save wait state
        let wait_data = serde_json::json!({
            "waiting_for": self.extract_wait_event(step),
            "started_at": chrono::Utc::now()
        });
        context.save_step_state(&wait_data).await?;

        // For now, simulate waiting by sleeping
        // In a real implementation, this would set up event listeners
        let wait_duration = tokio::time::Duration::from_millis(100);
        tokio::time::sleep(wait_duration).await;

        // Create success output
        let output_data = serde_json::json!({
            "step_id": step.id.as_str(),
            "waited_for": self.extract_wait_event(step),
            "completed_at": chrono::Utc::now()
        });

        Ok(StepOutput::success(output_data))
    }

    fn can_execute(&self, step: &Step) -> bool {
        step.name.contains("wait") || step.name.contains("event")
    }

    fn step_type(&self) -> &str {
        "wait"
    }

    fn name(&self) -> &str {
        "WaitStepExecutor"
    }
}

impl WaitStepExecutor {
    fn extract_wait_event(&self, step: &Step) -> String {
        // Try to get event name from step input
        if let Some(input) = &step.input {
            if let Some(event) = input.get("event").and_then(|v| v.as_str()) {
                return event.to_string();
            }
        }

        // Try to get event name from step config
        if let Some(event) = step.config.config.get("event").and_then(|v| v.as_str()) {
            return event.to_string();
        }

        // Default event name
        "unknown.event".to_string()
    }
}

/// Coordinator for step execution
pub struct StepExecutionCoordinator {
    registry: StepExecutorRegistry,
}

impl StepExecutionCoordinator {
    /// Create a new step execution coordinator
    pub fn new() -> Self {
        Self {
            registry: StepExecutorRegistry::default(),
        }
    }

    /// Create a coordinator with a custom registry
    pub fn with_registry(registry: StepExecutorRegistry) -> Self {
        Self { registry }
    }

    /// Register a custom step executor
    pub fn register_executor(&mut self, executor: Arc<dyn StepExecutor>) {
        self.registry.register(executor);
    }

    /// Execute a step using the appropriate executor
    pub async fn execute_step(
        &self,
        step: &Step,
        context: &ExecutionContext,
    ) -> Result<StepOutput> {
        // Create step context
        let step_context = context.for_step(step.id.clone());

        // Find appropriate executor
        let executor = self.registry.find_executor(step).ok_or_else(|| {
            ExecutionError::step_execution(step.id.clone(), "No suitable executor found")
        })?;

        tracing::debug!(
            "Executing step {} with executor {}",
            step.name,
            executor.name()
        );

        // Execute the step
        let result = executor.execute(step, &step_context).await;

        match &result {
            Ok(output) => {
                tracing::info!(
                    "Step {} executed successfully: {}",
                    step.name,
                    output.is_success()
                );
            }
            Err(error) => {
                tracing::error!("Step {} execution failed: {}", step.name, error);
            }
        }

        result
    }

    /// Get the registry for inspection
    pub fn registry(&self) -> &StepExecutorRegistry {
        &self.registry
    }
}

impl Default for StepExecutionCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ExecutionConfig;
    use ingest_core::{StateManager as StateManagerTrait, generate_id_with_prefix};
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::collections::HashMap;

    // Mock state manager for testing
    #[derive(Debug)]
    struct MockStateManager {
        data: Arc<tokio::sync::RwLock<HashMap<String, serde_json::Value>>>,
    }

    impl MockStateManager {
        fn new() -> Self {
            Self {
                data: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            }
        }
    }

    #[async_trait::async_trait]
    impl StateManagerTrait for MockStateManager {
        async fn save_state(&self, key: &str, data: &serde_json::Value) -> ingest_core::Result<()> {
            let mut storage = self.data.write().await;
            storage.insert(key.to_string(), data.clone());
            Ok(())
        }

        async fn load_state(&self, key: &str) -> ingest_core::Result<Option<serde_json::Value>> {
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

    fn create_test_step(name: &str) -> Step {
        ingest_core::Step::new(
            name,
            generate_id_with_prefix("fn"),
            generate_id_with_prefix("run"),
        )
    }

    fn create_test_context() -> ExecutionContext {
        ExecutionContext::new(
            generate_id_with_prefix("run"),
            generate_id_with_prefix("fn"),
            ExecutionConfig::default(),
            Arc::new(MockStateManager::new()),
        )
    }

    #[test]
    fn test_step_executor_registry_creation() {
        let actual = StepExecutorRegistry::new();
        assert!(actual.is_empty());
        assert_eq!(actual.len(), 0);
    }

    #[test]
    fn test_step_executor_registry_register() {
        let mut fixture = StepExecutorRegistry::new();
        let executor = Arc::new(DefaultStepExecutor);

        fixture.register(executor);

        assert_eq!(fixture.len(), 1);
        assert!(!fixture.is_empty());
        assert!(fixture.get_executor("default").is_some());
    }

    #[test]
    fn test_step_executor_registry_default() {
        let actual = StepExecutorRegistry::default();
        assert!(!actual.is_empty());
        assert!(actual.get_executor("default").is_some());
        assert!(actual.get_executor("sleep").is_some());
        assert!(actual.get_executor("wait").is_some());
    }

    #[test]
    fn test_default_step_executor() {
        let fixture = DefaultStepExecutor;
        let test_step = create_test_step("test-step");

        assert!(fixture.can_execute(&test_step));
        assert_eq!(fixture.step_type(), "default");
        assert_eq!(fixture.name(), "DefaultStepExecutor");
    }

    #[tokio::test]
    async fn test_default_step_executor_execute() {
        let fixture_executor = DefaultStepExecutor;
        let fixture_step = create_test_step("test-step");
        let fixture_context = create_test_context();
        let fixture_step_context = fixture_context.for_step(fixture_step.id.clone());

        let actual = fixture_executor
            .execute(&fixture_step, &fixture_step_context)
            .await;

        assert!(actual.is_ok());
        let output = actual.unwrap();
        assert!(output.is_success());
        assert!(output.data.get("step_id").is_some());
    }

    #[test]
    fn test_sleep_step_executor() {
        let fixture = SleepStepExecutor;
        let test_step = create_test_step("sleep-step");

        assert!(fixture.can_execute(&test_step));
        assert_eq!(fixture.step_type(), "sleep");
        assert_eq!(fixture.name(), "SleepStepExecutor");
    }

    #[tokio::test]
    async fn test_sleep_step_executor_execute() {
        let fixture_executor = SleepStepExecutor;
        let mut fixture_step = create_test_step("sleep-step");
        fixture_step.input = Some(json!({"duration_ms": 1}));
        let fixture_context = create_test_context();
        let fixture_step_context = fixture_context.for_step(fixture_step.id.clone());

        let actual = fixture_executor
            .execute(&fixture_step, &fixture_step_context)
            .await;

        assert!(actual.is_ok());
        let output = actual.unwrap();
        assert!(output.is_success());
        assert!(output.data.get("slept_for_ms").is_some());
    }

    #[test]
    fn test_wait_step_executor() {
        let fixture = WaitStepExecutor;
        let test_step = create_test_step("wait-step");

        assert!(fixture.can_execute(&test_step));
        assert_eq!(fixture.step_type(), "wait");
        assert_eq!(fixture.name(), "WaitStepExecutor");
    }

    #[tokio::test]
    async fn test_wait_step_executor_execute() {
        let fixture_executor = WaitStepExecutor;
        let mut fixture_step = create_test_step("wait-step");
        fixture_step.input = Some(json!({"event": "user.created"}));
        let fixture_context = create_test_context();
        let fixture_step_context = fixture_context.for_step(fixture_step.id.clone());

        let actual = fixture_executor
            .execute(&fixture_step, &fixture_step_context)
            .await;

        assert!(actual.is_ok());
        let output = actual.unwrap();
        assert!(output.is_success());
        assert!(output.data.get("waited_for").is_some());
    }

    #[test]
    fn test_step_execution_coordinator_creation() {
        let actual = StepExecutionCoordinator::new();
        assert!(!actual.registry().is_empty());
    }

    #[tokio::test]
    async fn test_step_execution_coordinator_execute() {
        let fixture_coordinator = StepExecutionCoordinator::new();
        let fixture_step = create_test_step("test-step");
        let fixture_context = create_test_context();

        let actual = fixture_coordinator
            .execute_step(&fixture_step, &fixture_context)
            .await;

        assert!(actual.is_ok());
        let output = actual.unwrap();
        assert!(output.is_success());
    }

    #[tokio::test]
    async fn test_step_execution_coordinator_no_executor() {
        let fixture_coordinator =
            StepExecutionCoordinator::with_registry(StepExecutorRegistry::new());
        let fixture_step = create_test_step("unknown-step");
        let fixture_context = create_test_context();

        let actual = fixture_coordinator
            .execute_step(&fixture_step, &fixture_context)
            .await;

        assert!(actual.is_err());
    }

    #[test]
    fn test_sleep_step_executor_extract_duration() {
        let fixture_executor = SleepStepExecutor;

        // Test with duration_ms in input
        let mut step = create_test_step("sleep-step");
        step.input = Some(json!({"duration_ms": 5000}));
        let actual = fixture_executor.extract_sleep_duration(&step).unwrap();
        assert_eq!(actual, tokio::time::Duration::from_millis(5000));

        // Test with duration_secs in input
        step.input = Some(json!({"duration_secs": 3}));
        let actual = fixture_executor.extract_sleep_duration(&step).unwrap();
        assert_eq!(actual, tokio::time::Duration::from_secs(3));

        // Test default
        step.input = None;
        let actual = fixture_executor.extract_sleep_duration(&step).unwrap();
        assert_eq!(actual, tokio::time::Duration::from_secs(1));
    }

    #[test]
    fn test_wait_step_executor_extract_event() {
        let fixture_executor = WaitStepExecutor;

        // Test with event in input
        let mut step = create_test_step("wait-step");
        step.input = Some(json!({"event": "user.created"}));
        let actual = fixture_executor.extract_wait_event(&step);
        assert_eq!(actual, "user.created");

        // Test default
        step.input = None;
        let actual = fixture_executor.extract_wait_event(&step);
        assert_eq!(actual, "unknown.event");
    }
}
