use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{Executor, HttpExecutor};
use inngest_core::Edge;
use inngest_queue::{
    Consumer, ProcessorFn, QueueError, QueueItem, QueueItemKind, RedisQueue, RunInfo, RunResult,
};
use inngest_state::StateManager;

/// Queue consumer that processes function executions
pub struct QueueConsumer {
    queue: Arc<RedisQueue>,
    executor: Arc<HttpExecutor>,
    state_manager: Arc<dyn StateManager>,
    consumer_id: String,
    max_concurrent_executions: usize,
    shutdown_signal: tokio::sync::watch::Receiver<bool>,
}

impl QueueConsumer {
    pub async fn new(
        queue: Arc<RedisQueue>,
        state_manager: Arc<dyn StateManager>,
        function_base_url: String,
        max_concurrent_executions: usize,
    ) -> Result<(Self, tokio::sync::watch::Sender<bool>), QueueError> {
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        // Use the provided state manager for HttpExecutor
        let executor = Arc::new(HttpExecutor::new_with_arc(
            Arc::clone(&state_manager),
            function_base_url,
        ));

        let consumer = Self {
            queue,
            executor,
            state_manager,
            consumer_id: format!("consumer-{}", Uuid::new_v4()),
            max_concurrent_executions,
            shutdown_signal: shutdown_rx,
        };

        Ok((consumer, shutdown_tx))
    }

    /// Start the queue consumer
    pub async fn start(&mut self) -> Result<(), QueueError> {
        info!(
            "Starting queue consumer {} with max {} concurrent executions",
            self.consumer_id, self.max_concurrent_executions
        );

        let executor = Arc::clone(&self.executor);
        let state_manager = Arc::clone(&self.state_manager);
        let consumer_id = self.consumer_id.clone();

        // Create processor function
        let processor: ProcessorFn = Arc::new(move |run_info: RunInfo, item: QueueItem| {
            let executor = Arc::clone(&executor);
            let state_manager = Arc::clone(&state_manager);
            let consumer_id = consumer_id.clone();

            // Process the item
            tokio::spawn(async move {
                Self::process_queue_item(executor, state_manager, consumer_id, run_info, item).await
            });

            Ok(RunResult {
                scheduled_immediate_job: false,
            })
        });

        // Start the consumer
        self.queue.run(processor).await
    }

    /// Process a single queue item
    async fn process_queue_item(
        executor: Arc<HttpExecutor>,
        _state_manager: Arc<dyn StateManager>,
        consumer_id: String,
        _run_info: RunInfo,
        item: QueueItem,
    ) -> Result<(), QueueError> {
        let item_id = item.id.to_string();

        info!(
            "Consumer {} processing item {} (attempt {}/{})",
            consumer_id,
            item_id,
            item.attempt + 1,
            item.max_attempts.unwrap_or(3)
        );

        // Execute the item based on its kind
        let execution_result = match item.kind {
            QueueItemKind::Start | QueueItemKind::Edge => {
                // Create edge for execution
                let edge = item.edge.clone().unwrap_or_else(|| Edge {
                    incoming: "trigger".to_string(),
                    outgoing: "function".to_string(),
                });

                // Execute the function
                executor
                    .execute(item.identifier.clone(), item.clone(), edge)
                    .await
            }
            QueueItemKind::Sleep => {
                warn!("Sleep operations not yet implemented for item {}", item_id);
                return Ok(());
            }
            QueueItemKind::EdgeError => {
                warn!(
                    "Edge error operations not yet implemented for item {}",
                    item_id
                );
                return Ok(());
            }
            QueueItemKind::Pause => {
                warn!("Pause operations not yet implemented for item {}", item_id);
                return Ok(());
            }
            QueueItemKind::Debounce => {
                warn!(
                    "Debounce operations not yet implemented for item {}",
                    item_id
                );
                return Ok(());
            }
            QueueItemKind::ScheduleBatch => {
                warn!(
                    "Schedule batch operations not yet implemented for item {}",
                    item_id
                );
                return Ok(());
            }
            QueueItemKind::Cancel => {
                warn!("Cancel operations not yet implemented for item {}", item_id);
                return Ok(());
            }
            QueueItemKind::QueueMigrate => {
                warn!(
                    "Queue migrate operations not yet implemented for item {}",
                    item_id
                );
                return Ok(());
            }
        };

        // Handle execution result
        match execution_result {
            Ok(response) => {
                info!(
                    "Successfully executed item {} with output: {:?}",
                    item_id, response.output
                );
            }
            Err(e) => {
                error!("Failed to execute item {}: {}", item_id, e);
                return Err(QueueError::Internal(format!("Execution failed: {}", e)));
            }
        }

        Ok(())
    }

    /// Graceful shutdown
    pub async fn shutdown(&mut self) {
        info!("Shutting down queue consumer {}", self.consumer_id);

        // The consumer will naturally stop when the queue.run() method returns
        // or when the shutdown signal is triggered
        if let Err(e) = self.queue.stop().await {
            error!("Error stopping queue consumer: {}", e);
        }

        info!("Queue consumer {} shutdown complete", self.consumer_id);
    }
}
