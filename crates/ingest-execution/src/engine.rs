use crate::{
    concurrency::{ConcurrencyController, ExecutionPermit},
    context::ExecutionContext,
    error::{ExecutionError, Result},
    executor::StepExecutionCoordinator,
    retry::RetryManager,
    types::{ExecutionConfig, ExecutionRequest, ExecutionResult, ExecutionStatus},
};
use async_trait::async_trait;
use ingest_core::{Function, Id, StateManager, Step, StepOutput};
use ingest_queue::{
    CoreQueueManager, JobId, JobStatus, QueueJob, QueueStats, QueueStorage, TenantId,
    TenantManager, TenantStats,
};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

// Mock implementations for testing and benchmarking
#[derive(Debug)]
struct MockQueueStorage;

#[async_trait::async_trait]
impl QueueStorage for MockQueueStorage {
    async fn store_job(&self, _job: &QueueJob) -> ingest_core::Result<()> {
        Ok(())
    }

    async fn retrieve_job(&self, _job_id: &JobId) -> ingest_core::Result<Option<QueueJob>> {
        Ok(None)
    }

    async fn update_job(&self, _job: &QueueJob) -> ingest_core::Result<()> {
        Ok(())
    }

    async fn delete_job(&self, _job_id: &JobId) -> ingest_core::Result<()> {
        Ok(())
    }

    async fn list_jobs(
        &self,
        _tenant_id: Option<&TenantId>,
        _status: Option<JobStatus>,
        _limit: Option<usize>,
    ) -> ingest_core::Result<Vec<QueueJob>> {
        Ok(vec![])
    }

    async fn get_stats(&self) -> ingest_core::Result<QueueStats> {
        Ok(QueueStats::default())
    }
}

#[derive(Debug)]
struct MockTenantManager;

#[async_trait::async_trait]
impl TenantManager for MockTenantManager {
    async fn can_enqueue(&self, _tenant_id: &TenantId) -> ingest_core::Result<bool> {
        Ok(true)
    }

    async fn record_job(&self, _tenant_id: &TenantId, _job: &QueueJob) -> ingest_core::Result<()> {
        Ok(())
    }

    async fn get_next_tenant(&self) -> ingest_core::Result<Option<TenantId>> {
        Ok(None)
    }

    async fn get_tenant_stats(&self, _tenant_id: &TenantId) -> ingest_core::Result<TenantStats> {
        Ok(TenantStats::default())
    }
}

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
impl StateManager for MockStateManager {
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

/// Main execution engine trait
#[async_trait]
pub trait ExecutionEngine: Send + Sync {
    /// Execute a function with the given request
    async fn execute_function(&self, request: ExecutionRequest) -> Result<ExecutionResult>;

    /// Execute a single step
    async fn execute_step(&self, step: Step, context: &ExecutionContext) -> Result<StepOutput>;

    /// Pause an execution
    async fn pause_execution(&self, run_id: &Id) -> Result<()>;

    /// Resume a paused execution
    async fn resume_execution(&self, run_id: &Id) -> Result<()>;

    /// Cancel an execution
    async fn cancel_execution(&self, run_id: &Id) -> Result<()>;

    /// Get execution status
    async fn get_execution_status(&self, run_id: &Id) -> Result<ExecutionStatus>;
}

/// Default implementation of the execution engine
pub struct DefaultExecutionEngine {
    /// State manager for persistence
    state_manager: Arc<dyn StateManager>,
    /// Queue manager for scheduling
    #[allow(dead_code)]
    queue_manager: Arc<CoreQueueManager>,
    /// Step execution coordinator
    step_coordinator: StepExecutionCoordinator,
    /// Retry manager
    retry_manager: RetryManager,
    /// Concurrency controller
    concurrency_controller: ConcurrencyController,
    /// Active executions
    active_executions: Arc<RwLock<std::collections::HashMap<Id, ExecutionContext>>>,
}

impl DefaultExecutionEngine {
    /// Create a new execution engine
    pub async fn new(
        state_manager: Arc<dyn StateManager>,
        queue_manager: Arc<CoreQueueManager>,
    ) -> Result<Self> {
        Ok(Self {
            state_manager,
            queue_manager,
            step_coordinator: StepExecutionCoordinator::new(),
            retry_manager: RetryManager::new(),
            concurrency_controller: ConcurrencyController::with_defaults(),
            active_executions: Arc::new(RwLock::new(std::collections::HashMap::new())),
        })
    }

    /// Create an execution engine with custom configuration
    pub async fn with_config(
        state_manager: Arc<dyn StateManager>,
        queue_manager: Arc<CoreQueueManager>,
        _config: ExecutionConfig,
    ) -> Result<Self> {
        Ok(Self {
            state_manager,
            queue_manager,
            step_coordinator: StepExecutionCoordinator::new(),
            retry_manager: RetryManager::new(),
            concurrency_controller: ConcurrencyController::with_defaults(),
            active_executions: Arc::new(RwLock::new(std::collections::HashMap::new())),
        })
    }

    /// Create an in-memory execution engine for benchmarking
    pub async fn new_in_memory() -> Result<Self> {
        // Create mock state manager and queue manager for benchmarking
        let state_manager: Arc<dyn StateManager> = Arc::new(MockStateManager::new());
        let queue_storage: Arc<dyn ingest_queue::QueueStorage> = Arc::new(MockQueueStorage);
        let tenant_manager: Arc<dyn ingest_queue::TenantManager> = Arc::new(MockTenantManager);
        let config = ingest_queue::QueueConfig::default();
        let queue_manager = Arc::new(CoreQueueManager::new(queue_storage, tenant_manager, config));

        Self::new(state_manager, queue_manager).await
    }

    /// Execute function steps in sequence
    async fn execute_function_steps(
        &self,
        function: &Function,
        context: &ExecutionContext,
    ) -> Result<ExecutionResult> {
        let mut result = ExecutionResult::new(context.run_id.clone(), function.id.clone());
        result.start();

        // Get function steps
        let steps = self.get_function_steps(function).await?;

        for step in steps {
            // Check if execution should be paused or cancelled
            if self.should_pause_execution(&context.run_id).await? {
                result.pause();
                return Ok(result);
            }

            if self.should_cancel_execution(&context.run_id).await? {
                result.cancel();
                return Ok(result);
            }

            // Execute step with retry logic
            match self.execute_step_with_retry(step, context).await {
                Ok(step_output) => {
                    result.add_step_output(step_output);

                    // Save intermediate state
                    self.save_execution_state(&result).await?;
                }
                Err(error) => {
                    error!("Step execution failed: {}", error);
                    result.fail(error.to_string());
                    return Ok(result);
                }
            }
        }

        result.complete(None);
        Ok(result)
    }

    /// Execute a step with retry logic
    async fn execute_step_with_retry(
        &self,
        step: Step,
        context: &ExecutionContext,
    ) -> Result<StepOutput> {
        let mut retry_state = self.retry_manager.create_state(step.id.clone());

        loop {
            match self.step_coordinator.execute_step(&step, context).await {
                Ok(output) => return Ok(output),
                Err(error) => {
                    // Check if we should retry
                    if let Some(_next_retry) =
                        self.retry_manager.plan_retry(&mut retry_state, &error)?
                    {
                        warn!("Step {} failed, retrying: {}", step.name, error);

                        // Wait for retry delay (in a real implementation, this would be scheduled)
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        continue;
                    } else {
                        return Err(error);
                    }
                }
            }
        }
    }

    /// Get function steps (placeholder implementation)
    async fn get_function_steps(&self, function: &Function) -> Result<Vec<Step>> {
        // In a real implementation, this would parse the function definition
        // For now, create a simple step
        let step = Step::new("main", function.id.clone(), function.id.clone());
        Ok(vec![step])
    }

    /// Check if execution should be paused
    async fn should_pause_execution(&self, run_id: &Id) -> Result<bool> {
        // Check state manager for pause signals
        let pause_key = format!("execution:{}:pause", run_id.as_str());
        Ok(self
            .state_manager
            .state_exists(&pause_key)
            .await
            .unwrap_or(false))
    }

    /// Check if execution should be cancelled
    async fn should_cancel_execution(&self, run_id: &Id) -> Result<bool> {
        // Check state manager for cancel signals
        let cancel_key = format!("execution:{}:cancel", run_id.as_str());
        Ok(self
            .state_manager
            .state_exists(&cancel_key)
            .await
            .unwrap_or(false))
    }

    /// Save execution state
    async fn save_execution_state(&self, result: &ExecutionResult) -> Result<()> {
        let state_key = format!("execution:{}", result.run_id.as_str());
        let state_data = serde_json::to_value(result)
            .map_err(|e| ExecutionError::internal(format!("Failed to serialize state: {e}")))?;

        self.state_manager
            .save_state(&state_key, &state_data)
            .await
            .map_err(|e| ExecutionError::internal(format!("Failed to save state: {e}")))?;

        Ok(())
    }

    /// Load execution state
    async fn load_execution_state(&self, run_id: &Id) -> Result<Option<ExecutionResult>> {
        let state_key = format!("execution:{}", run_id.as_str());

        match self.state_manager.load_state(&state_key).await {
            Ok(Some(state_data)) => {
                let result: ExecutionResult = serde_json::from_value(state_data).map_err(|e| {
                    ExecutionError::internal(format!("Failed to deserialize state: {e}"))
                })?;
                Ok(Some(result))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(ExecutionError::internal(format!(
                "Failed to load state: {e}"
            ))),
        }
    }

    /// Create execution context
    async fn create_execution_context(
        &self,
        request: &ExecutionRequest,
        _permit: ExecutionPermit,
    ) -> Result<ExecutionContext> {
        let context = ExecutionContext::new(
            request.run_id.clone(),
            request.function.id.clone(),
            request.config.clone(),
            self.state_manager.clone(),
        );

        // Store in active executions
        let mut active = self.active_executions.write().await;
        active.insert(request.run_id.clone(), context.clone());

        Ok(context)
    }

    /// Remove execution context
    async fn remove_execution_context(&self, run_id: &Id) {
        let mut active = self.active_executions.write().await;
        active.remove(run_id);
    }
}

#[async_trait]
impl ExecutionEngine for DefaultExecutionEngine {
    async fn execute_function(&self, request: ExecutionRequest) -> Result<ExecutionResult> {
        info!("Starting function execution: {}", request.function.name);

        // Acquire execution permit
        let permit = self
            .concurrency_controller
            .acquire_permit(request.run_id.clone(), request.function.id.clone(), None)
            .await?;

        // Create execution context
        let context = self.create_execution_context(&request, permit).await?;

        // Execute function
        let result = self
            .execute_function_steps(&request.function, &context)
            .await;

        // Clean up
        self.remove_execution_context(&request.run_id).await;
        self.concurrency_controller
            .release_permit(&request.run_id)
            .await?;

        match &result {
            Ok(exec_result) => {
                info!(
                    "Function execution completed: {} - {}",
                    request.function.name, exec_result.status
                );
            }
            Err(error) => {
                error!(
                    "Function execution failed: {} - {}",
                    request.function.name, error
                );
            }
        }

        result
    }

    async fn execute_step(&self, step: Step, context: &ExecutionContext) -> Result<StepOutput> {
        debug!("Executing step: {}", step.name);
        self.step_coordinator.execute_step(&step, context).await
    }

    async fn pause_execution(&self, run_id: &Id) -> Result<()> {
        info!("Pausing execution: {}", run_id.as_str());

        let pause_key = format!("execution:{}:pause", run_id.as_str());
        let pause_data = serde_json::json!({
            "paused_at": chrono::Utc::now(),
            "reason": "user_requested"
        });

        self.state_manager
            .save_state(&pause_key, &pause_data)
            .await
            .map_err(|e| ExecutionError::internal(format!("Failed to pause execution: {e}")))?;

        Ok(())
    }

    async fn resume_execution(&self, run_id: &Id) -> Result<()> {
        info!("Resuming execution: {}", run_id.as_str());

        let pause_key = format!("execution:{}:pause", run_id.as_str());
        self.state_manager
            .delete_state(&pause_key)
            .await
            .map_err(|e| ExecutionError::internal(format!("Failed to resume execution: {e}")))?;

        // In a real implementation, this would re-queue the execution
        Ok(())
    }

    async fn cancel_execution(&self, run_id: &Id) -> Result<()> {
        info!("Cancelling execution: {}", run_id.as_str());

        let cancel_key = format!("execution:{}:cancel", run_id.as_str());
        let cancel_data = serde_json::json!({
            "cancelled_at": chrono::Utc::now(),
            "reason": "user_requested"
        });

        self.state_manager
            .save_state(&cancel_key, &cancel_data)
            .await
            .map_err(|e| ExecutionError::internal(format!("Failed to cancel execution: {e}")))?;

        // Remove from active executions
        self.remove_execution_context(run_id).await;

        Ok(())
    }

    async fn get_execution_status(&self, run_id: &Id) -> Result<ExecutionStatus> {
        // Check if execution is active
        let active = self.active_executions.read().await;
        if active.contains_key(run_id) {
            return Ok(ExecutionStatus::Running);
        }

        // Check saved state
        if let Some(result) = self.load_execution_state(run_id).await? {
            return Ok(result.status);
        }

        // Check for pause/cancel signals
        let pause_key = format!("execution:{}:pause", run_id.as_str());
        let cancel_key = format!("execution:{}:cancel", run_id.as_str());

        if self
            .state_manager
            .state_exists(&cancel_key)
            .await
            .unwrap_or(false)
        {
            return Ok(ExecutionStatus::Cancelled);
        }

        if self
            .state_manager
            .state_exists(&pause_key)
            .await
            .unwrap_or(false)
        {
            return Ok(ExecutionStatus::Paused);
        }

        Ok(ExecutionStatus::Unknown)
    }
}

// Convenience type alias
pub type Engine = DefaultExecutionEngine;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Event;
    use ingest_core::generate_id_with_prefix;
    use ingest_queue::QueueConfig;
    use pretty_assertions::assert_eq;

    async fn create_test_engine() -> DefaultExecutionEngine {
        let state_manager = Arc::new(MockStateManager::new());
        let storage = Arc::new(MockQueueStorage);
        let tenant_manager = Arc::new(MockTenantManager);
        let config = QueueConfig::default();
        let queue_manager = Arc::new(CoreQueueManager::new(storage, tenant_manager, config));
        DefaultExecutionEngine::new(state_manager, queue_manager)
            .await
            .unwrap()
    }

    fn create_test_function() -> Function {
        Function::new("test-function")
    }

    fn create_test_request() -> ExecutionRequest {
        let function = create_test_function();
        let event = Event::new("test.event", serde_json::json!({"key": "value"}));
        ExecutionRequest::new(function, event)
    }

    #[tokio::test]
    async fn test_execution_engine_creation() {
        let actual = create_test_engine().await;
        // Just verify it was created successfully
        assert_eq!(actual.active_executions.read().await.len(), 0);
    }

    #[tokio::test]
    async fn test_execute_function() {
        let fixture_engine = create_test_engine().await;
        let fixture_request = create_test_request();

        let actual = fixture_engine.execute_function(fixture_request).await;

        assert!(actual.is_ok());
        let result = actual.unwrap();
        assert_eq!(result.status, ExecutionStatus::Completed);
    }

    #[tokio::test]
    async fn test_pause_and_resume_execution() {
        let fixture_engine = create_test_engine().await;
        let fixture_run_id = generate_id_with_prefix("run");

        // Pause execution
        let actual_pause = fixture_engine.pause_execution(&fixture_run_id).await;
        assert!(actual_pause.is_ok());

        // Check status
        let status = fixture_engine
            .get_execution_status(&fixture_run_id)
            .await
            .unwrap();
        assert_eq!(status, ExecutionStatus::Paused);

        // Resume execution
        let actual_resume = fixture_engine.resume_execution(&fixture_run_id).await;
        assert!(actual_resume.is_ok());
    }

    #[tokio::test]
    async fn test_cancel_execution() {
        let fixture_engine = create_test_engine().await;
        let fixture_run_id = generate_id_with_prefix("run");

        let actual = fixture_engine.cancel_execution(&fixture_run_id).await;
        assert!(actual.is_ok());

        let status = fixture_engine
            .get_execution_status(&fixture_run_id)
            .await
            .unwrap();
        assert_eq!(status, ExecutionStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_get_execution_status_unknown() {
        let fixture_engine = create_test_engine().await;
        let fixture_run_id = generate_id_with_prefix("run");

        let actual = fixture_engine.get_execution_status(&fixture_run_id).await;
        assert!(actual.is_ok());
        assert_eq!(actual.unwrap(), ExecutionStatus::Unknown);
    }
}
