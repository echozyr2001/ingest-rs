use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{Consumer, ProcessorFn, Producer, Queue, QueueError, QueueItem, QueueReader, RunInfo};

/// In-memory queue implementation for development
pub struct MemoryQueue {
    items: Arc<Mutex<VecDeque<QueueItem>>>,
    running: Arc<Mutex<bool>>,
}

impl MemoryQueue {
    pub fn new() -> Self {
        Self {
            items: Arc::new(Mutex::new(VecDeque::new())),
            running: Arc::new(Mutex::new(false)),
        }
    }
}

#[async_trait]
impl Producer for MemoryQueue {
    async fn enqueue(&self, item: QueueItem) -> Result<(), QueueError> {
        let mut items = self.items.lock().await;
        items.push_back(item);
        Ok(())
    }

    async fn enqueue_batch(&self, items: Vec<QueueItem>) -> Result<(), QueueError> {
        let mut queue_items = self.items.lock().await;
        for item in items {
            queue_items.push_back(item);
        }
        Ok(())
    }

    async fn schedule(
        &self,
        item: QueueItem,
        _delay: std::time::Duration,
    ) -> Result<(), QueueError> {
        // For simplicity, just enqueue immediately in dev mode
        self.enqueue(item).await
    }
}

#[async_trait]
impl Consumer for MemoryQueue {
    async fn run(&self, processor: ProcessorFn) -> Result<(), QueueError> {
        let mut running = self.running.lock().await;
        *running = true;

        // Simple consumer loop - in production this would be more sophisticated
        tokio::spawn({
            let items = self.items.clone();
            let running = self.running.clone();
            async move {
                while *running.lock().await {
                    let item = {
                        let mut queue = items.lock().await;
                        queue.pop_front()
                    };

                    if let Some(queue_item) = item {
                        let run_info = RunInfo {
                            latency: std::time::Duration::from_millis(0),
                            sojourn_delay: std::time::Duration::from_millis(0),
                            priority: inngest_core::Priority::Low,
                            queue_shard_name: "memory".to_string(),
                            continue_count: 0,
                            refilled_from_backlog: None,
                        };

                        // Process the item
                        if let Err(e) = processor(run_info, queue_item) {
                            eprintln!("Error processing queue item: {}", e);
                        }
                    } else {
                        // No items, sleep briefly
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
            }
        });

        Ok(())
    }

    async fn stop(&self) -> Result<(), QueueError> {
        let mut running = self.running.lock().await;
        *running = false;
        Ok(())
    }
}

#[async_trait]
impl QueueReader for MemoryQueue {
    async fn len(&self) -> Result<usize, QueueError> {
        let items = self.items.lock().await;
        Ok(items.len())
    }

    async fn is_empty(&self) -> Result<bool, QueueError> {
        let items = self.items.lock().await;
        Ok(items.is_empty())
    }

    async fn get_items_by_function(
        &self,
        _function_id: Uuid,
    ) -> Result<Vec<QueueItem>, QueueError> {
        // TODO: Filter by function ID
        let items = self.items.lock().await;
        Ok(items.iter().cloned().collect())
    }
}

impl Queue for MemoryQueue {
    async fn set_function_paused(
        &self,
        _account_id: Uuid,
        _fn_id: Uuid,
        _paused: bool,
    ) -> Result<(), QueueError> {
        // TODO: Implement function pausing for memory queue
        // For now, just return success
        Ok(())
    }
}

impl Default for MemoryQueue {
    fn default() -> Self {
        Self::new()
    }
}
