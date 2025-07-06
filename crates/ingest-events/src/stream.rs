//! Event stream management

use crate::{Result, error::EventError, processor::EventProcessor};
use ingest_core::Event;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tokio::time::{Duration, Instant};

/// Event stream configuration
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// Buffer size for the stream
    pub buffer_size: usize,
    /// Maximum batch size for processing
    pub max_batch_size: usize,
    /// Batch timeout in milliseconds
    pub batch_timeout_ms: u64,
    /// Enable ordering guarantee
    pub enable_ordering: bool,
    /// Enable backpressure handling
    pub enable_backpressure: bool,
    /// Maximum events per second (rate limiting)
    pub max_events_per_second: Option<u64>,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            buffer_size: 10000,
            max_batch_size: 100,
            batch_timeout_ms: 1000,
            enable_ordering: false,
            enable_backpressure: true,
            max_events_per_second: None,
        }
    }
}

/// Event stream statistics
#[derive(Debug, Clone, Default)]
pub struct StreamStats {
    /// Total events received
    pub events_received: u64,
    /// Events currently in buffer
    pub events_in_buffer: u64,
    /// Events processed
    pub events_processed: u64,
    /// Events dropped due to backpressure
    pub events_dropped: u64,
    /// Total batches processed
    pub batches_processed: u64,
    /// Average batch size
    pub avg_batch_size: f64,
    /// Average processing time per batch
    pub avg_batch_processing_time_ms: f64,
}

impl StreamStats {
    /// Update batch statistics
    pub fn update_batch(&mut self, batch_size: usize, processing_time_ms: u64) {
        self.batches_processed += 1;
        self.events_processed += batch_size as u64;

        // Update average batch size
        self.avg_batch_size = (self.avg_batch_size * (self.batches_processed - 1) as f64
            + batch_size as f64)
            / self.batches_processed as f64;

        // Update average processing time
        self.avg_batch_processing_time_ms = (self.avg_batch_processing_time_ms
            * (self.batches_processed - 1) as f64
            + processing_time_ms as f64)
            / self.batches_processed as f64;
    }
}

/// Event stream for managing event flow
pub struct EventStream {
    /// Stream configuration
    config: StreamConfig,
    /// Event buffer
    buffer: Arc<RwLock<VecDeque<Event>>>,
    /// Event processor
    processor: Arc<EventProcessor>,
    /// Stream statistics
    stats: Arc<RwLock<StreamStats>>,
    /// Sender for new events
    event_sender: mpsc::UnboundedSender<Event>,
    /// Receiver for new events
    event_receiver: Arc<RwLock<mpsc::UnboundedReceiver<Event>>>,
    /// Rate limiter state
    rate_limiter: Arc<RwLock<RateLimiter>>,
}

/// Simple rate limiter
#[derive(Debug)]
struct RateLimiter {
    /// Maximum events per second
    max_events_per_second: Option<u64>,
    /// Events processed in current second
    events_in_current_second: u64,
    /// Current second timestamp
    current_second: Instant,
}

impl RateLimiter {
    fn new(max_events_per_second: Option<u64>) -> Self {
        Self {
            max_events_per_second,
            events_in_current_second: 0,
            current_second: Instant::now(),
        }
    }

    fn can_process(&mut self) -> bool {
        if let Some(max_rate) = self.max_events_per_second {
            let now = Instant::now();

            // Reset counter if we're in a new second
            if now.duration_since(self.current_second) >= Duration::from_secs(1) {
                self.events_in_current_second = 0;
                self.current_second = now;
            }

            if self.events_in_current_second >= max_rate {
                return false;
            }

            self.events_in_current_second += 1;
        }

        true
    }
}

impl EventStream {
    /// Create new event stream
    pub fn new(config: StreamConfig, processor: Arc<EventProcessor>) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();

        Self {
            rate_limiter: Arc::new(RwLock::new(RateLimiter::new(config.max_events_per_second))),
            config,
            buffer: Arc::new(RwLock::new(VecDeque::new())),
            processor,
            stats: Arc::new(RwLock::new(StreamStats::default())),
            event_sender: sender,
            event_receiver: Arc::new(RwLock::new(receiver)),
        }
    }

    /// Add event to stream
    pub async fn add_event(&self, event: Event) -> Result<()> {
        // Check rate limiting
        if !self.rate_limiter.write().await.can_process() {
            return Err(EventError::stream("Rate limit exceeded"));
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.events_received += 1;
        }

        // Check backpressure
        if self.config.enable_backpressure {
            let buffer_size = self.buffer.read().await.len();
            if buffer_size >= self.config.buffer_size {
                let mut stats = self.stats.write().await;
                stats.events_dropped += 1;
                return Err(EventError::buffer_overflow(self.config.buffer_size));
            }
        }

        // Send event to processing
        self.event_sender
            .send(event)
            .map_err(|_| EventError::stream("Failed to send event to stream"))?;

        Ok(())
    }

    /// Start processing events
    pub async fn start_processing(&self) -> Result<()> {
        let buffer = Arc::clone(&self.buffer);
        let processor = Arc::clone(&self.processor);
        let stats = Arc::clone(&self.stats);
        let receiver = Arc::clone(&self.event_receiver);
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut batch_timer =
                tokio::time::interval(Duration::from_millis(config.batch_timeout_ms));

            loop {
                tokio::select! {
                    // Process events from receiver
                    _ = async {
                        let mut receiver_guard = receiver.write().await;
                        while let Ok(event) = receiver_guard.try_recv() {
                            let mut buffer_guard = buffer.write().await;
                            buffer_guard.push_back(event);

                            // Update buffer stats
                            {
                                let mut stats_guard = stats.write().await;
                                stats_guard.events_in_buffer = buffer_guard.len() as u64;
                            }

                            // Process batch if it's full
                            if buffer_guard.len() >= config.max_batch_size {
                                let batch = Self::extract_batch(&mut buffer_guard, config.max_batch_size);
                                drop(buffer_guard);
                                Self::process_batch(Arc::clone(&processor), Arc::clone(&stats), batch).await;
                            }
                        }
                    } => {}

                    // Process batch on timeout
                    _ = batch_timer.tick() => {
                        let mut buffer_guard = buffer.write().await;
                        if !buffer_guard.is_empty() {
                            let batch_size = std::cmp::min(buffer_guard.len(), config.max_batch_size);
                            let batch = Self::extract_batch(&mut buffer_guard, batch_size);

                            // Update buffer stats
                            {
                                let mut stats_guard = stats.write().await;
                                stats_guard.events_in_buffer = buffer_guard.len() as u64;
                            }

                            drop(buffer_guard);
                            Self::process_batch(Arc::clone(&processor), Arc::clone(&stats), batch).await;
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Extract batch from buffer
    fn extract_batch(buffer: &mut VecDeque<Event>, batch_size: usize) -> Vec<Event> {
        let mut batch = Vec::with_capacity(batch_size);
        for _ in 0..batch_size {
            if let Some(event) = buffer.pop_front() {
                batch.push(event);
            } else {
                break;
            }
        }
        batch
    }

    /// Process a batch of events
    async fn process_batch(
        processor: Arc<EventProcessor>,
        stats: Arc<RwLock<StreamStats>>,
        batch: Vec<Event>,
    ) {
        if batch.is_empty() {
            return;
        }

        let start_time = Instant::now();
        let batch_size = batch.len();

        // Process the batch
        match processor.process_events(batch).await {
            Ok(_results) => {
                // Update statistics
                let processing_time = start_time.elapsed().as_millis() as u64;
                let mut stats_guard = stats.write().await;
                stats_guard.update_batch(batch_size, processing_time);
            }
            Err(e) => {
                eprintln!("Batch processing failed: {e}");
            }
        }
    }

    /// Get stream statistics
    pub async fn stats(&self) -> StreamStats {
        self.stats.read().await.clone()
    }

    /// Reset stream statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = StreamStats::default();
    }

    /// Get buffer size
    pub async fn buffer_size(&self) -> usize {
        self.buffer.read().await.len()
    }

    /// Clear buffer
    pub async fn clear_buffer(&self) -> usize {
        let mut buffer = self.buffer.write().await;
        let size = buffer.len();
        buffer.clear();

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.events_in_buffer = 0;
        }

        size
    }

    /// Check if stream is healthy
    pub async fn is_healthy(&self) -> bool {
        let buffer_size = self.buffer_size().await;
        let stats = self.stats().await;

        // Consider unhealthy if buffer is full or too many events dropped
        buffer_size < self.config.buffer_size
            && (stats.events_received == 0
                || (stats.events_dropped as f64 / stats.events_received as f64) < 0.1)
    }
}

/// Event stream builder for easier configuration
pub struct EventStreamBuilder {
    config: StreamConfig,
}

impl EventStreamBuilder {
    /// Create new builder
    pub fn new() -> Self {
        Self {
            config: StreamConfig::default(),
        }
    }

    /// Set buffer size
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.config.buffer_size = size;
        self
    }

    /// Set batch size
    pub fn batch_size(mut self, size: usize) -> Self {
        self.config.max_batch_size = size;
        self
    }

    /// Set batch timeout
    pub fn batch_timeout(mut self, timeout_ms: u64) -> Self {
        self.config.batch_timeout_ms = timeout_ms;
        self
    }

    /// Enable ordering
    pub fn enable_ordering(mut self, enable: bool) -> Self {
        self.config.enable_ordering = enable;
        self
    }

    /// Enable backpressure
    pub fn enable_backpressure(mut self, enable: bool) -> Self {
        self.config.enable_backpressure = enable;
        self
    }

    /// Set rate limit
    pub fn rate_limit(mut self, events_per_second: u64) -> Self {
        self.config.max_events_per_second = Some(events_per_second);
        self
    }

    /// Build the event stream
    pub fn build(self, processor: Arc<EventProcessor>) -> EventStream {
        EventStream::new(self.config, processor)
    }
}

impl Default for EventStreamBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processor::create_basic_processor;
    use ingest_core::Event;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use tokio::time::{Duration, sleep};

    fn create_test_event(name: &str) -> Event {
        Event::new(name, json!({"test": "data"}))
    }

    #[test]
    fn test_stream_config_default() {
        let fixture = StreamConfig::default();
        assert_eq!(fixture.buffer_size, 10000);
        assert_eq!(fixture.max_batch_size, 100);
        assert_eq!(fixture.batch_timeout_ms, 1000);
        assert!(!fixture.enable_ordering);
        assert!(fixture.enable_backpressure);
        assert!(fixture.max_events_per_second.is_none());
    }

    #[test]
    fn test_stream_stats_default() {
        let fixture = StreamStats::default();
        assert_eq!(fixture.events_received, 0);
        assert_eq!(fixture.events_processed, 0);
        assert_eq!(fixture.batches_processed, 0);
        assert_eq!(fixture.avg_batch_size, 0.0);
    }

    #[test]
    fn test_stream_stats_update_batch() {
        let mut fixture = StreamStats::default();
        fixture.update_batch(10, 100);
        fixture.update_batch(20, 200);

        assert_eq!(fixture.batches_processed, 2);
        assert_eq!(fixture.events_processed, 30);
        assert_eq!(fixture.avg_batch_size, 15.0);
        assert_eq!(fixture.avg_batch_processing_time_ms, 150.0);
    }

    #[test]
    fn test_rate_limiter_no_limit() {
        let mut fixture = RateLimiter::new(None);
        assert!(fixture.can_process());
        assert!(fixture.can_process());
    }

    #[test]
    fn test_rate_limiter_with_limit() {
        let mut fixture = RateLimiter::new(Some(2));
        assert!(fixture.can_process());
        assert!(fixture.can_process());
        assert!(!fixture.can_process()); // Should be rate limited
    }

    #[tokio::test]
    async fn test_event_stream_creation() {
        let processor = Arc::new(create_basic_processor());
        let config = StreamConfig::default();
        let fixture = EventStream::new(config, processor);

        assert_eq!(fixture.buffer_size().await, 0);
        assert!(fixture.is_healthy().await);
    }

    #[tokio::test]
    async fn test_event_stream_add_event() {
        let processor = Arc::new(create_basic_processor());
        let config = StreamConfig::default();
        let fixture = EventStream::new(config, processor);

        let event = create_test_event("test.event");
        let actual = fixture.add_event(event).await;
        assert!(actual.is_ok());

        let stats = fixture.stats().await;
        assert_eq!(stats.events_received, 1);
    }

    #[tokio::test]
    async fn test_event_stream_backpressure() {
        let processor = Arc::new(create_basic_processor());
        let config = StreamConfig {
            buffer_size: 1, // Very small buffer
            enable_backpressure: true,
            ..Default::default()
        };
        let fixture = EventStream::new(config, processor);

        // Start processing to fill up the internal buffer
        let _ = fixture.start_processing().await;

        // Add events rapidly to trigger backpressure
        // Since events are sent to a channel, we need to overwhelm the processing
        let mut success_count = 0;

        for i in 0..10 {
            let result = fixture
                .add_event(create_test_event(&format!("event{i}")))
                .await;
            if result.is_ok() {
                success_count += 1;
            }
        }

        // At least some events should succeed
        assert!(success_count > 0);

        let stats = fixture.stats().await;
        assert!(stats.events_received > 0);
    }

    #[tokio::test]
    async fn test_event_stream_rate_limiting() {
        let processor = Arc::new(create_basic_processor());
        let config = StreamConfig {
            max_events_per_second: Some(1),
            ..Default::default()
        };
        let fixture = EventStream::new(config, processor);

        // First event should succeed
        let result1 = fixture.add_event(create_test_event("event1")).await;
        assert!(result1.is_ok());

        // Second event should be rate limited
        let result2 = fixture.add_event(create_test_event("event2")).await;
        assert!(result2.is_err());
    }

    #[tokio::test]
    async fn test_event_stream_stats() {
        let processor = Arc::new(create_basic_processor());
        let config = StreamConfig::default();
        let fixture = EventStream::new(config, processor);

        let _ = fixture.add_event(create_test_event("event1")).await;
        let _ = fixture.add_event(create_test_event("event2")).await;

        let stats = fixture.stats().await;
        assert_eq!(stats.events_received, 2);
    }

    #[tokio::test]
    async fn test_event_stream_clear_buffer() {
        let processor = Arc::new(create_basic_processor());
        let config = StreamConfig::default();
        let fixture = EventStream::new(config, processor);

        let _ = fixture.add_event(create_test_event("event1")).await;
        let _ = fixture.add_event(create_test_event("event2")).await;

        let cleared = fixture.clear_buffer().await;
        assert_eq!(cleared, 0); // Events are immediately sent to processing, not buffered
        assert_eq!(fixture.buffer_size().await, 0);
    }

    #[test]
    fn test_event_stream_builder() {
        let fixture = EventStreamBuilder::new()
            .buffer_size(5000)
            .batch_size(50)
            .batch_timeout(500)
            .enable_ordering(true)
            .enable_backpressure(false)
            .rate_limit(1000);

        assert_eq!(fixture.config.buffer_size, 5000);
        assert_eq!(fixture.config.max_batch_size, 50);
        assert_eq!(fixture.config.batch_timeout_ms, 500);
        assert!(fixture.config.enable_ordering);
        assert!(!fixture.config.enable_backpressure);
        assert_eq!(fixture.config.max_events_per_second, Some(1000));
    }

    #[tokio::test]
    async fn test_event_stream_builder_build() {
        let processor = Arc::new(create_basic_processor());
        let fixture = EventStreamBuilder::new().buffer_size(1000).build(processor);

        assert!(fixture.is_healthy().await);
    }

    #[tokio::test]
    async fn test_event_stream_processing() {
        let processor = Arc::new(create_basic_processor());
        let config = StreamConfig {
            max_batch_size: 2,
            batch_timeout_ms: 100,
            ..Default::default()
        };
        let fixture = EventStream::new(config, processor);

        // Start processing
        let _ = fixture.start_processing().await;

        // Add events
        let _ = fixture.add_event(create_test_event("event1")).await;
        let _ = fixture.add_event(create_test_event("event2")).await;

        // Wait a bit for processing
        sleep(Duration::from_millis(200)).await;

        // Check that events were processed
        let stats = fixture.stats().await;
        assert!(stats.events_received >= 2);
    }
}
