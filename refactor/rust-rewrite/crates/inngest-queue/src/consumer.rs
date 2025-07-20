use crate::{Consumer, Producer, QueueItem, QueueError, RunInfo, RunResult, Priority};
use inngest_core::{Event, Function, EventRouter, Executor, ExecutionContext, ExecutionResult};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc};
use tokio::time::{interval, timeout};
use tracing::{info, warn, error, debug};
use uuid::Uuid;

/// Queue consumer that processes items and executes functions
pub struct QueueConsumer {
    /// Event router for matching events to functions
    router: Arc<RwLock<EventRouter>>,
    /// Function executor
    executor: Arc<dyn Executor>,
    /// Queue for processing items
    queue: Arc<dyn Producer + Consumer + Send + Sync>,
    /// Consumer configuration
    config: ConsumerConfig,
    /// Shutdown signal receiver
    shutdown_rx: Option<mpsc::Receiver<()>>,
    /// Consumer state
    state: ConsumerState,
}

/// Configuration for queue consumer
#[derive(Debug, Clone)]
pub struct ConsumerConfig {
    /// Maximum number of concurrent executions
    pub max_concurrency: usize,
    /// Consumer poll interval
    pub poll_interval: Duration,
    /// Execution timeout
    pub execution_timeout: Duration,
    /// Retry configuration
    pub retry_config: RetryConfig,
    /// Consumer name for logging
    pub consumer_name: String,
}

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retries
    pub max_retries: u32,
    /// Base retry delay
    pub base_delay: Duration,
    /// Maximum retry delay
    pub max_delay: Duration,
    /// Exponential backoff multiplier
    pub backoff_multiplier: f64,
}

/// Consumer execution state
#[derive(Debug, Clone)]
enum ConsumerState {
    Stopped,
    Starting,
    Running,
    Stopping,
}

/// Result of processing a queue item
#[derive(Debug)]
pub struct ProcessingResult {
    /// Queue item that was processed
    pub item: QueueItem,
    /// Execution result
    pub execution_result: ExecutionResult,
    /// Processing duration
    pub duration: Duration,
    /// Whether the item should be retried
    pub should_retry: bool,
}

impl QueueConsumer {
    /// Create a new queue consumer
    pub fn new(
        router: Arc<RwLock<EventRouter>>,
        executor: Arc<dyn Executor>,
        queue: Arc<dyn Producer + Consumer + Send + Sync>,
        config: ConsumerConfig,
    ) -> Self {
        Self {
            router,
            executor,
            queue,
            config,
            shutdown_rx: None,
            state: ConsumerState::Stopped,
        }
    }

    /// Start the consumer
    pub async fn start(&mut self) -> Result<(), QueueError> {
        info!(
            consumer = %self.config.consumer_name,
            "Starting queue consumer"
        );

        self.state = ConsumerState::Starting;

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
        self.shutdown_rx = Some(shutdown_rx);

        // Start consumer loop
        self.state = ConsumerState::Running;
        
        let consumer_clone = self.clone_for_processing();
        tokio::spawn(async move {
            consumer_clone.run_consumer_loop().await;
        });

        info!(
            consumer = %self.config.consumer_name,
            "Queue consumer started successfully"
        );

        Ok(())
    }

    /// Stop the consumer
    pub async fn stop(&mut self) -> Result<(), QueueError> {
        info!(
            consumer = %self.config.consumer_name,
            "Stopping queue consumer"
        );

        self.state = ConsumerState::Stopping;
        
        if let Some(mut shutdown_rx) = self.shutdown_rx.take() {
            // Send shutdown signal and wait for completion
            let _ = shutdown_rx.recv().await;
        }

        self.state = ConsumerState::Stopped;
        
        info!(
            consumer = %self.config.consumer_name,
            "Queue consumer stopped"
        );

        Ok(())
    }

    /// Clone consumer for async processing
    fn clone_for_processing(&self) -> ConsumerClone {
        ConsumerClone {
            router: Arc::clone(&self.router),
            executor: Arc::clone(&self.executor),
            queue: Arc::clone(&self.queue),
            config: self.config.clone(),
        }
    }

    /// Main consumer processing loop
    async fn run_consumer_loop(&self) -> Result<(), QueueError> {
        let mut poll_interval = interval(self.config.poll_interval);
        let (work_tx, mut work_rx) = mpsc::channel(self.config.max_concurrency);

        loop {
            tokio::select! {
                // Check for shutdown signal
                _ = async {
                    if let Some(ref mut shutdown_rx) = self.shutdown_rx {
                        shutdown_rx.recv().await
                    } else {
                        std::future::pending().await
                    }
                } => {
                    info!("Received shutdown signal, stopping consumer");
                    break;
                }

                // Poll for new work
                _ = poll_interval.tick() => {
                    if matches!(self.state, ConsumerState::Running) {
                        if let Err(e) = self.poll_and_process(&work_tx).await {
                            error!(error = %e, "Error during polling");
                        }
                    }
                }

                // Handle completed work
                Some(result) = work_rx.recv() => {
                    self.handle_processing_result(result).await;
                }
            }
        }

        Ok(())
    }

    /// Poll queue and process items
    async fn poll_and_process(
        &self, 
        work_tx: &mpsc::Sender<ProcessingResult>
    ) -> Result<(), QueueError> {
        // This would need to be implemented based on the specific queue implementation
        // For now, we'll define the interface
        debug!("Polling queue for new items");

        // TODO: Actual queue polling implementation
        // This would typically:
        // 1. Poll the queue for available items
        // 2. For each item, spawn a processing task
        // 3. Send results back through work_tx

        Ok(())
    }

    /// Handle processing result
    async fn handle_processing_result(&self, result: ProcessingResult) {
        debug!(
            item_id = %result.item.id,
            success = result.execution_result.success,
            duration_ms = result.duration.as_millis(),
            "Processing completed"
        );

        // Handle retry logic
        if !result.execution_result.success && result.should_retry {
            if let Some(retry_after) = result.execution_result.retry_after {
                info!(
                    item_id = %result.item.id,
                    retry_after_seconds = retry_after.num_seconds(),
                    "Scheduling retry"
                );

                // Schedule retry
                let retry_item = QueueItem {
                    retry_count: result.item.retry_count + 1,
                    ..result.item
                };

                if let Err(e) = self.queue.schedule(retry_item, retry_after.to_std().unwrap_or(Duration::from_secs(30))).await {
                    error!(error = %e, "Failed to schedule retry");
                }
            }
        }

        // TODO: Update metrics, logging, etc.
    }
}

/// Cloneable parts of consumer for async processing
#[derive(Clone)]
struct ConsumerClone {
    router: Arc<RwLock<EventRouter>>,
    executor: Arc<dyn Executor>,
    queue: Arc<dyn Producer + Consumer + Send + Sync>,
    config: ConsumerConfig,
}

impl ConsumerClone {
    /// Process a single queue item
    async fn process_item(&self, item: QueueItem) -> ProcessingResult {
        let start_time = std::time::Instant::now();
        let should_retry = item.retry_count < self.config.retry_config.max_retries;

        debug!(
            item_id = %item.id,
            function_id = %item.function_id,
            attempt = item.retry_count + 1,
            "Processing queue item"
        );

        // Route event to get matching functions
        let router = self.router.read().await;
        let routing_result = match router.route_event(&item.event) {
            Ok(result) => result,
            Err(e) => {
                error!(error = %e, "Failed to route event");
                return ProcessingResult {
                    item,
                    execution_result: ExecutionResult {
                        success: false,
                        output: None,
                        error: Some(format!("Routing error: {}", e)),
                        retry_after: None,
                    },
                    duration: start_time.elapsed(),
                    should_retry: false,
                };
            }
        };

        // Find the specific function to execute
        let function = match routing_result.matched_functions
            .iter()
            .find(|f| f.id == item.function_id) {
            Some(func) => func,
            None => {
                warn!(
                    function_id = %item.function_id,
                    "Function not found in routing results"
                );
                return ProcessingResult {
                    item,
                    execution_result: ExecutionResult {
                        success: false,
                        output: None,
                        error: Some("Function not found".to_string()),
                        retry_after: None,
                    },
                    duration: start_time.elapsed(),
                    should_retry: false,
                };
            }
        };

        // Create execution context
        let context = ExecutionContext {
            run_id: item.run_id,
            function_id: function.id.to_string(),
            event: item.event.clone(),
            attempt: item.retry_count + 1,
            metadata: item.metadata.clone(),
        };

        // Execute function with timeout
        let execution_result = match timeout(
            self.config.execution_timeout,
            self.executor.execute(function, context)
        ).await {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => ExecutionResult {
                success: false,
                output: None,
                error: Some(e.to_string()),
                retry_after: Some(chrono::Duration::seconds(30)),
            },
            Err(_) => ExecutionResult {
                success: false,
                output: None,
                error: Some("Execution timeout".to_string()),
                retry_after: Some(chrono::Duration::seconds(60)),
            },
        };

        ProcessingResult {
            item,
            execution_result,
            duration: start_time.elapsed(),
            should_retry,
        }
    }
}

impl Default for ConsumerConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 10,
            poll_interval: Duration::from_millis(100),
            execution_timeout: Duration::from_secs(30),
            retry_config: RetryConfig::default(),
            consumer_name: "default-consumer".to_string(),
        }
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(300),
            backoff_multiplier: 2.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use inngest_core::{HttpExecutor, Event};

    #[tokio::test]
    async fn test_consumer_config_defaults() {
        let config = ConsumerConfig::default();
        assert_eq!(config.max_concurrency, 10);
        assert_eq!(config.poll_interval, Duration::from_millis(100));
        assert_eq!(config.execution_timeout, Duration::from_secs(30));
    }

    #[tokio::test]
    async fn test_retry_config_defaults() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.base_delay, Duration::from_secs(1));
        assert_eq!(config.max_delay, Duration::from_secs(300));
        assert_eq!(config.backoff_multiplier, 2.0);
    }
}
