//! Main execution orchestrator
//!
//! Coordinates function execution including batching, concurrency control,
//! driver delegation, and state management

use chrono::Utc;
use inngest_core::{
    Error, Event, ExecutionContext, ExecutionResult, ExecutionStatus, Function, Result,
    StateIdentifier,
};
use inngest_queue::QueueManager;
use inngest_state::StateManager;
use std::collections::HashMap;
use std::sync::Arc;
use url::Url;

use crate::batch::{BatchConfig, BatchItem, BatchManager};
use crate::concurrency::ConcurrencyManager;
use crate::driver::ExecutionRequest as DriverExecutionRequest;
use crate::driver::ExecutionStatus as DriverExecutionStatus;
use crate::driver::{ExecutionDriver, ExecutionResponse};

/// Configuration for the executor
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Maximum number of concurrent executions
    pub max_concurrency: usize,
    /// Default timeout for function execution
    pub default_timeout: std::time::Duration,
    /// Maximum number of retries
    pub max_retries: u32,
    /// Base URL for API callbacks
    pub api_base_url: String,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            max_concurrency: crate::defaults::DEFAULT_CONCURRENCY_LIMIT,
            default_timeout: crate::defaults::DEFAULT_STEP_TIMEOUT,
            max_retries: crate::defaults::DEFAULT_RETRY_COUNT,
            api_base_url: "http://localhost:8288".to_string(),
        }
    }
}

/// Request to execute a function
#[derive(Debug, Clone)]
pub struct ExecutionRequest {
    /// Function to execute
    pub function: Function,
    /// Events that triggered the function
    pub events: Vec<Event>,
    /// Run identifier
    pub run_id: String,
    /// Account and workspace context
    pub account_id: uuid::Uuid,
    pub workspace_id: uuid::Uuid,
    pub app_id: uuid::Uuid,
    /// Execution attempt number
    pub attempt: u32,
}

/// Main executor that orchestrates function execution
#[allow(dead_code)]
pub struct Executor {
    config: ExecutorConfig,
    driver: Arc<dyn ExecutionDriver>,
    state_manager: Arc<dyn StateManager>,
    queue_manager: Arc<dyn QueueManager>,
    batch_manager: Option<Arc<dyn BatchManager>>,
    concurrency_manager: Arc<dyn ConcurrencyManager>,
}

impl Executor {
    pub fn new(
        config: ExecutorConfig,
        driver: Arc<dyn ExecutionDriver>,
        state_manager: Arc<dyn StateManager>,
        queue_manager: Arc<dyn QueueManager>,
        concurrency_manager: Arc<dyn ConcurrencyManager>,
    ) -> Self {
        Self {
            config,
            driver,
            state_manager,
            queue_manager,
            batch_manager: None,
            concurrency_manager,
        }
    }

    pub fn with_batch_manager(mut self, batch_manager: Arc<dyn BatchManager>) -> Self {
        self.batch_manager = Some(batch_manager);
        self
    }

    /// Execute a single function
    pub async fn execute(&self, request: ExecutionRequest) -> Result<ExecutionResult> {
        tracing::info!(
            function_id = %request.function.id,
            run_id = %request.run_id,
            attempt = request.attempt,
            "Starting function execution"
        );

        // For now, always execute as single function
        // TODO: Implement batch processing when needed
        self.execute_single(&request).await
    }

    async fn execute_single(&self, request: &ExecutionRequest) -> Result<ExecutionResult> {
        let function_uri = self.get_function_uri(&request.function)?;

        // Create execution context
        let ctx = ExecutionContext {
            function_id: request.function.id.to_string(),
            run_id: request.run_id.clone(),
            step_id: "step".to_string(), // Default step name
            attempt: request.attempt,
            state: StateIdentifier::new(request.function.id.to_string(), request.run_id.clone()),
        };

        // Prepare the event data - for single events, use the first event
        let event_data = request
            .events
            .first()
            .map(|e| serde_json::to_value(e).unwrap_or_default())
            .unwrap_or_default();

        // Create execution request for the driver
        let exec_request = DriverExecutionRequest {
            function_id: request.function.id.to_string(),
            step_id: Some("step".to_string()),
            ctx: ctx.clone(),
            event: event_data,
            steps: HashMap::new(),
        };

        // Acquire concurrency slot if limits are configured
        let _concurrency_guard = if let Some(concurrency) = &request.function.concurrency {
            let queue_item = crate::concurrency::QueueItem {
                id: request.run_id.clone(),
                function_id: request.function.id,
                account_id: request.account_id,
                data: serde_json::json!({}),
            };

            // Convert ConcurrencyConfig to ConcurrencyLimits
            let concurrency_limits = crate::concurrency::ConcurrencyLimits::new().add_limit(
                crate::concurrency::ConcurrencyLimit::new(
                    concurrency.limit,
                    crate::concurrency::ConcurrencyScope::Function,
                ),
            );

            Some(
                self.concurrency_manager
                    .acquire(request.function.id, &concurrency_limits, &queue_item)
                    .await?,
            )
        } else {
            None
        };

        // Execute the function
        let response = self.driver.execute(&function_uri, exec_request).await?;

        // Process the response
        self.process_execution_response(&ctx, response).await
    }

    #[allow(dead_code)]
    async fn execute_batch(
        &self,
        request: &ExecutionRequest,
        batch_config: &BatchConfig,
    ) -> Result<ExecutionResult> {
        let batch_manager = self.batch_manager.as_ref().ok_or_else(|| {
            Error::InvalidConfiguration("Batch manager not configured".to_string())
        })?;

        // Convert events to batch items
        let mut batch_results = Vec::new();

        for event in &request.events {
            let batch_item = BatchItem {
                account_id: request.account_id,
                workspace_id: request.workspace_id,
                app_id: request.app_id,
                function_id: request.function.id,
                function_version: 1, // Default version since Function struct doesn't have this field yet
                event_id: event.ulid,
                event: event.clone(),
                version: 1,
            };

            let result = batch_manager.append(batch_item, batch_config).await?;
            batch_results.push(result);
        }

        // If any batch is ready for execution, process it
        for result in batch_results {
            if matches!(result.status, crate::batch::BatchStatus::Full) {
                if let Some(batch_id) = result.batch_id {
                    // Schedule batch execution
                    let schedule_opts = crate::batch::ScheduleBatchOpts {
                        batch_id,
                        batch_pointer: result.batch_pointer_key,
                        account_id: request.account_id,
                        workspace_id: request.workspace_id,
                        app_id: request.app_id,
                        function_id: request.function.id,
                        function_version: 1, // Default version
                        at: Utc::now(),
                    };

                    batch_manager.schedule_execution(schedule_opts).await?;
                }
            }
        }

        Ok(ExecutionResult {
            status: ExecutionStatus::Completed,
            output: Some(serde_json::json!({"batched": true})),
            error: None,
            retry_at: None,
        })
    }

    async fn process_execution_response(
        &self,
        ctx: &ExecutionContext,
        response: ExecutionResponse,
    ) -> Result<ExecutionResult> {
        let status = match response.status {
            DriverExecutionStatus::Completed => ExecutionStatus::Completed,
            DriverExecutionStatus::Failed => ExecutionStatus::Failed,
            DriverExecutionStatus::Waiting => ExecutionStatus::Waiting,
            DriverExecutionStatus::Cancelled => ExecutionStatus::Cancelled,
        };

        let retry_at = if response.no_retry {
            None
        } else {
            response
                .retry_after
                .map(|delay| Utc::now() + chrono::Duration::from_std(delay).unwrap_or_default())
        };

        // Save execution state
        let output = response.body.clone();
        if let Err(e) = self
            .state_manager
            .save_execution_state(&ctx.state, &response.body.unwrap_or_default())
            .await
        {
            tracing::warn!("Failed to save execution state: {}", e);
        }

        // Process any additional steps
        if !response.steps.is_empty() {
            tracing::info!(
                "Function returned {} additional steps",
                response.steps.len()
            );
            // In a full implementation, these would be queued for execution
        }

        Ok(ExecutionResult {
            status,
            output,
            error: if status == ExecutionStatus::Failed {
                Some("Function execution failed".to_string())
            } else {
                None
            },
            retry_at,
        })
    }

    fn get_function_uri(&self, function: &Function) -> Result<Url> {
        if function.steps.is_empty() {
            return Err(Error::InvalidConfiguration(
                "Function has no steps".to_string(),
            ));
        }

        let step = &function.steps[0]; // Use first step
        Url::parse(&step.uri)
            .map_err(|e| Error::InvalidConfiguration(format!("Invalid function URI: {}", e)))
    }

    /// Start the executor event loop
    pub async fn run(&self) -> Result<()> {
        tracing::info!(
            "Starting executor with max_concurrency: {}",
            self.config.max_concurrency
        );

        // This would typically run in a loop, processing items from the queue
        loop {
            match self.process_queue_items().await {
                Ok(_) => {}
                Err(e) => {
                    tracing::error!("Error processing queue items: {}", e);
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }
    }

    async fn process_queue_items(&self) -> Result<()> {
        // In a real implementation, this would:
        // 1. Poll the queue for new items
        // 2. Check concurrency limits
        // 3. Execute functions
        // 4. Handle retries and failures

        tracing::debug!("Processing queue items (placeholder)");
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use inngest_core::{Function, Step, Trigger, Ulid, Uuid};
    use inngest_queue::{EnqueueOptions, QueueItem, QueueManager, RunFunction};
    use inngest_state::{
        CreateStateInput, Metadata, MetadataUpdate, RunStatus, State, StateId, StateManager,
    };
    use mockall::mock;
    use mockall::predicate::*;

    mock! {
        TestDriver {}

        #[async_trait]
        impl ExecutionDriver for TestDriver {
            async fn execute(&self, url: &Url, request: crate::driver::ExecutionRequest) -> Result<ExecutionResponse>;
            fn name(&self) -> &'static str;
            fn supports_scheme(&self, scheme: &str) -> bool;
        }
    }

    mock! {
        TestStateManager {}

        #[async_trait]
        impl StateManager for TestStateManager {
            async fn create(&self, input: CreateStateInput) -> Result<State>;
            async fn load(&self, id: &StateId) -> Result<Option<State>>;
            async fn load_metadata(&self, id: &StateId) -> Result<Option<Metadata>>;
            async fn update_metadata(&self, id: &StateId, update: MetadataUpdate) -> Result<()>;
            async fn save_step(&self, id: &StateId, step_id: &str, data: serde_json::Value) -> Result<bool>;
            async fn set_status(&self, id: &StateId, status: RunStatus) -> Result<()>;
            async fn delete(&self, id: &StateId) -> Result<bool>;
            async fn exists(&self, id: &StateId) -> Result<bool>;
            async fn save_execution_state(&self, state_id: &StateIdentifier, data: &serde_json::Value) -> Result<()>;
            async fn load_execution_state(&self, state_id: &StateIdentifier) -> Result<Option<serde_json::Value>>;
            async fn delete_execution_state(&self, state_id: &StateIdentifier) -> Result<()>;
        }
    }

    mock! {
        TestQueueManager {}

        #[async_trait]
        impl QueueManager for TestQueueManager {
            async fn enqueue(&self, item: QueueItem, at: chrono::DateTime<Utc>, options: EnqueueOptions) -> Result<()>;
            async fn run(&self, run_fn: RunFunction) -> Result<()>;
            async fn outstanding_job_count(&self, env_id: Uuid, function_id: Uuid, run_id: Ulid) -> Result<i32>;
            async fn running_count(&self, function_id: Uuid) -> Result<i64>;
            async fn run_jobs(&self, shard_name: &str, workspace_id: Uuid, function_id: Uuid, run_id: Ulid, limit: i64, offset: i64) -> Result<Vec<QueueItem>>;
            async fn load_queue_item(&self, shard: &str, item_id: &str) -> Result<Option<QueueItem>>;
            async fn remove_queue_item(&self, shard: &str, partition_key: &str, item_id: &str) -> Result<()>;
            async fn set_function_paused(&self, account_id: Uuid, function_id: Uuid, paused: bool) -> Result<()>;
        }
    }

    mock! {
        TestConcurrencyManager {}

        #[async_trait]
        impl ConcurrencyManager for TestConcurrencyManager {
            async fn add(&self, function_id: Uuid, item: &crate::concurrency::QueueItem) -> Result<()>;
            async fn done(&self, function_id: Uuid, item: &crate::concurrency::QueueItem) -> Result<()>;
            async fn check(&self, function_id: Uuid, limit: u32) -> Result<()>;
            async fn acquire(&self, function_id: Uuid, limits: &crate::ConcurrencyLimits, item: &crate::concurrency::QueueItem) -> Result<crate::concurrency::ConcurrencyGuard>;
        }
    }

    fn create_test_function() -> Function {
        Function {
            id: Uuid::new_v4(),
            name: "test-function".to_string(),
            triggers: vec![Trigger {
                event: "test.event".to_string(),
                expression: None,
            }],
            steps: vec![Step {
                id: "step-1".to_string(),
                name: "Test Step".to_string(),
                uri: "https://example.com/webhook".to_string(),
            }],
            concurrency: None,
        }
    }

    #[tokio::test]
    async fn test_executor_creation() {
        let config = ExecutorConfig::default();
        let driver = Arc::new(MockTestDriver::new());
        let state_manager = Arc::new(MockTestStateManager::new());
        let queue_manager = Arc::new(MockTestQueueManager::new());
        let concurrency_manager = Arc::new(MockTestConcurrencyManager::new());

        let executor = Executor::new(
            config,
            driver,
            state_manager,
            queue_manager,
            concurrency_manager,
        );

        assert_eq!(
            executor.config.max_concurrency,
            crate::defaults::DEFAULT_CONCURRENCY_LIMIT
        );
    }

    #[test]
    fn test_function_uri_extraction() {
        let function = create_test_function();
        let config = ExecutorConfig::default();
        let driver = Arc::new(MockTestDriver::new());
        let state_manager = Arc::new(MockTestStateManager::new());
        let queue_manager = Arc::new(MockTestQueueManager::new());
        let concurrency_manager = Arc::new(MockTestConcurrencyManager::new());

        let executor = Executor::new(
            config,
            driver,
            state_manager,
            queue_manager,
            concurrency_manager,
        );

        let uri = executor.get_function_uri(&function).unwrap();
        assert_eq!(uri.as_str(), "https://example.com/webhook");
    }

    #[test]
    fn test_function_without_steps() {
        let mut function = create_test_function();
        function.steps.clear();

        let config = ExecutorConfig::default();
        let driver = Arc::new(MockTestDriver::new());
        let state_manager = Arc::new(MockTestStateManager::new());
        let queue_manager = Arc::new(MockTestQueueManager::new());
        let concurrency_manager = Arc::new(MockTestConcurrencyManager::new());

        let executor = Executor::new(
            config,
            driver,
            state_manager,
            queue_manager,
            concurrency_manager,
        );

        let result = executor.get_function_uri(&function);
        assert!(result.is_err());
    }
}
