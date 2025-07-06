//! Core event processing functionality

use crate::{
    Result,
    error::EventError,
    routing::{RoutingEngine, RoutingResult},
    serialization::{EventSerializer, SerializationFormat},
    validation::EventValidator,
};
use async_trait::async_trait;
use ingest_core::{Event, EventHandler};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Event processing pipeline stage
#[async_trait]
pub trait ProcessingStage: Send + Sync {
    /// Process an event
    async fn process(&self, event: Event) -> Result<Event>;

    /// Get stage name
    fn name(&self) -> &str;

    /// Check if stage is enabled
    fn is_enabled(&self) -> bool {
        true
    }
}

/// Event processor configuration
#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    /// Enable validation
    pub enable_validation: bool,
    /// Enable routing
    pub enable_routing: bool,
    /// Enable serialization
    pub enable_serialization: bool,
    /// Default serialization format
    pub default_format: SerializationFormat,
    /// Maximum concurrent events
    pub max_concurrent_events: usize,
    /// Enable metrics collection
    pub enable_metrics: bool,
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            enable_validation: true,
            enable_routing: true,
            enable_serialization: true,
            default_format: SerializationFormat::Json,
            max_concurrent_events: 1000,
            enable_metrics: true,
        }
    }
}

/// Event processing statistics
#[derive(Debug, Clone, Default)]
pub struct ProcessingStats {
    /// Total events processed
    pub events_processed: u64,
    /// Events processed successfully
    pub events_success: u64,
    /// Events failed processing
    pub events_failed: u64,
    /// Events currently being processed
    pub events_in_progress: u64,
    /// Average processing time in milliseconds
    pub avg_processing_time_ms: f64,
    /// Total processing time
    pub total_processing_time_ms: u64,
}

impl ProcessingStats {
    /// Update statistics after processing
    pub fn update(&mut self, success: bool, processing_time_ms: u64) {
        self.events_processed += 1;
        self.total_processing_time_ms += processing_time_ms;
        self.avg_processing_time_ms =
            self.total_processing_time_ms as f64 / self.events_processed as f64;

        if success {
            self.events_success += 1;
        } else {
            self.events_failed += 1;
        }
    }

    /// Get success rate as percentage
    pub fn success_rate(&self) -> f64 {
        if self.events_processed == 0 {
            0.0
        } else {
            (self.events_success as f64 / self.events_processed as f64) * 100.0
        }
    }
}

/// Core event processor
pub struct EventProcessor {
    /// Configuration
    config: ProcessorConfig,
    /// Event validator
    validator: Arc<EventValidator>,
    /// Routing engine
    routing_engine: Arc<RwLock<RoutingEngine>>,
    /// Event serializer
    serializer: Arc<EventSerializer>,
    /// Processing stages
    stages: Vec<Arc<dyn ProcessingStage>>,
    /// Event handlers
    handlers: Vec<Arc<dyn EventHandler>>,
    /// Processing statistics
    stats: Arc<RwLock<ProcessingStats>>,
}

impl EventProcessor {
    /// Create new event processor
    pub fn new(config: ProcessorConfig) -> Self {
        Self {
            config,
            validator: Arc::new(EventValidator::new()),
            routing_engine: Arc::new(RwLock::new(RoutingEngine::new())),
            serializer: Arc::new(EventSerializer::new()),
            stages: Vec::new(),
            handlers: Vec::new(),
            stats: Arc::new(RwLock::new(ProcessingStats::default())),
        }
    }

    /// Set event validator
    pub fn with_validator(mut self, validator: EventValidator) -> Self {
        self.validator = Arc::new(validator);
        self
    }

    /// Set routing engine
    pub fn with_routing_engine(mut self, engine: RoutingEngine) -> Self {
        self.routing_engine = Arc::new(RwLock::new(engine));
        self
    }

    /// Set event serializer
    pub fn with_serializer(mut self, serializer: EventSerializer) -> Self {
        self.serializer = Arc::new(serializer);
        self
    }

    /// Add processing stage
    pub fn add_stage(mut self, stage: Arc<dyn ProcessingStage>) -> Self {
        self.stages.push(stage);
        self
    }

    /// Add event handler
    pub fn add_handler(mut self, handler: Arc<dyn EventHandler>) -> Self {
        self.handlers.push(handler);
        self
    }

    /// Process a single event
    pub async fn process_event(&self, mut event: Event) -> Result<ProcessingResult> {
        let start_time = std::time::Instant::now();
        let mut result = ProcessingResult::new(event.id.to_string());

        // Update in-progress counter
        {
            let mut stats = self.stats.write().await;
            stats.events_in_progress += 1;
        }

        // Validate event if enabled
        if self.config.enable_validation {
            if let Err(e) = self.validator.validate(&event) {
                result.validation_error = Some(e.to_string());
                self.update_stats(false, start_time.elapsed().as_millis() as u64)
                    .await;
                return Ok(result);
            }
            result.validation_passed = true;
        }

        // Run through processing stages
        for stage in &self.stages {
            if stage.is_enabled() {
                match stage.process(event).await {
                    Ok(processed_event) => {
                        event = processed_event;
                        result.stages_completed.push(stage.name().to_string());
                    }
                    Err(e) => {
                        result.stage_error =
                            Some(format!("Stage '{}' failed: {}", stage.name(), e));
                        self.update_stats(false, start_time.elapsed().as_millis() as u64)
                            .await;
                        return Ok(result);
                    }
                }
            }
        }

        // Route event if enabled
        if self.config.enable_routing {
            match self.routing_engine.write().await.route(&event) {
                Ok(routing_result) => {
                    result.routing_result = Some(routing_result);
                }
                Err(e) => {
                    result.routing_error = Some(e.to_string());
                    self.update_stats(false, start_time.elapsed().as_millis() as u64)
                        .await;
                    return Ok(result);
                }
            }
        }

        // Serialize event if enabled
        if self.config.enable_serialization {
            match self
                .serializer
                .serialize(&event, Some(self.config.default_format))
            {
                Ok(serialized_data) => {
                    result.serialized_data = Some(serialized_data);
                }
                Err(e) => {
                    result.serialization_error = Some(e.to_string());
                    self.update_stats(false, start_time.elapsed().as_millis() as u64)
                        .await;
                    return Ok(result);
                }
            }
        }

        // Handle event with registered handlers
        for handler in &self.handlers {
            if handler.can_handle(&event) {
                if let Err(e) = handler.handle_event(event.clone()).await {
                    result.handler_errors.push(format!(
                        "Handler '{}' failed: {}",
                        handler.name(),
                        e
                    ));
                } else {
                    result.handlers_executed.push(handler.name().to_string());
                }
            }
        }

        result.processed_event = Some(event);
        result.success = true;

        self.update_stats(true, start_time.elapsed().as_millis() as u64)
            .await;
        Ok(result)
    }

    /// Process multiple events concurrently
    pub async fn process_events(&self, events: Vec<Event>) -> Result<Vec<ProcessingResult>> {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(
            self.config.max_concurrent_events,
        ));
        let mut tasks = Vec::new();

        for event in events {
            let processor = self.clone_refs();
            let permit = semaphore.clone();

            let task = tokio::spawn(async move {
                let _permit = permit.acquire().await.unwrap();
                processor.process_event(event).await
            });

            tasks.push(task);
        }

        let mut results = Vec::new();
        for task in tasks {
            match task.await {
                Ok(Ok(result)) => results.push(result),
                Ok(Err(e)) => return Err(e),
                Err(e) => return Err(EventError::processing(format!("Task failed: {e}"))),
            }
        }

        Ok(results)
    }

    /// Get processing statistics
    pub async fn stats(&self) -> ProcessingStats {
        self.stats.read().await.clone()
    }

    /// Reset statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = ProcessingStats::default();
    }

    /// Update statistics
    async fn update_stats(&self, success: bool, processing_time_ms: u64) {
        let mut stats = self.stats.write().await;
        stats.events_in_progress = stats.events_in_progress.saturating_sub(1);
        stats.update(success, processing_time_ms);
    }

    /// Clone references for async tasks
    fn clone_refs(&self) -> EventProcessorRefs {
        EventProcessorRefs {
            config: self.config.clone(),
            validator: Arc::clone(&self.validator),
            routing_engine: Arc::clone(&self.routing_engine),
            serializer: Arc::clone(&self.serializer),
            stages: self.stages.clone(),
            handlers: self.handlers.clone(),
            stats: Arc::clone(&self.stats),
        }
    }
}

/// References for async processing
#[derive(Clone)]
struct EventProcessorRefs {
    config: ProcessorConfig,
    validator: Arc<EventValidator>,
    routing_engine: Arc<RwLock<RoutingEngine>>,
    serializer: Arc<EventSerializer>,
    stages: Vec<Arc<dyn ProcessingStage>>,
    handlers: Vec<Arc<dyn EventHandler>>,
    stats: Arc<RwLock<ProcessingStats>>,
}

impl EventProcessorRefs {
    async fn process_event(&self, event: Event) -> Result<ProcessingResult> {
        let start_time = std::time::Instant::now();
        let mut result = ProcessingResult::new(event.id.to_string());
        let mut event = event;

        // Validate event
        if self.config.enable_validation {
            if let Err(e) = self.validator.validate(&event) {
                result.validation_error = Some(e.to_string());
                return Ok(result);
            }
            result.validation_passed = true;
        }

        // Run through processing stages
        for stage in &self.stages {
            match stage.process(event).await {
                Ok(processed_event) => {
                    event = processed_event;
                    result.stages_completed.push(stage.name().to_string());
                }
                Err(e) => {
                    result.stage_error = Some(e.to_string());
                    result.success = false;
                    return Ok(result);
                }
            }
        }

        // Route the event
        match self.routing_engine.write().await.route(&event) {
            Ok(routing_result) => {
                result.routing_result = Some(routing_result);
            }
            Err(e) => {
                result.routing_error = Some(e.to_string());
                result.success = false;
                return Ok(result);
            }
        }

        // Serialize the event
        if let Some(routing_result) = &result.routing_result {
            if !routing_result.destinations.is_empty() {
                match self
                    .serializer
                    .serialize(&event, Some(self.config.default_format))
                {
                    Ok(serialized) => {
                        result.serialized_data = Some(serialized);
                    }
                    Err(e) => {
                        result.serialization_error = Some(e.to_string());
                        result.success = false;
                        return Ok(result);
                    }
                }
            }
        }

        // Handle event with registered handlers
        for handler in &self.handlers {
            match handler.handle_event(event.clone()).await {
                Ok(_) => {
                    result.handlers_executed.push(handler.name().to_string());
                }
                Err(e) => {
                    result.handler_errors.push(e.to_string());
                    // Continue with other handlers even if one fails
                }
            }
        }

        result.processed_event = Some(event);
        result.success = true;

        // Update stats
        let mut stats = self.stats.write().await;
        stats.update(true, start_time.elapsed().as_millis() as u64);

        Ok(result)
    }
}

/// Event processing result
#[derive(Debug, Clone)]
pub struct ProcessingResult {
    /// Event ID
    pub event_id: String,
    /// Whether processing was successful
    pub success: bool,
    /// Validation passed
    pub validation_passed: bool,
    /// Validation error
    pub validation_error: Option<String>,
    /// Completed stages
    pub stages_completed: Vec<String>,
    /// Stage error
    pub stage_error: Option<String>,
    /// Routing result
    pub routing_result: Option<RoutingResult>,
    /// Routing error
    pub routing_error: Option<String>,
    /// Serialized event data
    pub serialized_data: Option<Vec<u8>>,
    /// Serialization error
    pub serialization_error: Option<String>,
    /// Handlers that executed successfully
    pub handlers_executed: Vec<String>,
    /// Handler errors
    pub handler_errors: Vec<String>,
    /// Final processed event
    pub processed_event: Option<Event>,
}

impl ProcessingResult {
    /// Create new processing result
    pub fn new(event_id: String) -> Self {
        Self {
            event_id,
            success: false,
            validation_passed: false,
            validation_error: None,
            stages_completed: Vec::new(),
            stage_error: None,
            routing_result: None,
            routing_error: None,
            serialized_data: None,
            serialization_error: None,
            handlers_executed: Vec::new(),
            handler_errors: Vec::new(),
            processed_event: None,
        }
    }

    /// Check if processing had any errors
    pub fn has_errors(&self) -> bool {
        self.validation_error.is_some()
            || self.stage_error.is_some()
            || self.routing_error.is_some()
            || self.serialization_error.is_some()
            || !self.handler_errors.is_empty()
    }

    /// Get all error messages
    pub fn error_messages(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if let Some(ref e) = self.validation_error {
            errors.push(format!("Validation: {e}"));
        }
        if let Some(ref e) = self.stage_error {
            errors.push(format!("Stage: {e}"));
        }
        if let Some(ref e) = self.routing_error {
            errors.push(format!("Routing: {e}"));
        }
        if let Some(ref e) = self.serialization_error {
            errors.push(format!("Serialization: {e}"));
        }
        for error in &self.handler_errors {
            errors.push(format!("Handler: {error}"));
        }

        errors
    }
}

/// Create a basic event processor with default configuration
pub fn create_basic_processor() -> EventProcessor {
    EventProcessor::new(ProcessorConfig::default())
}

/// Create a high-performance event processor
pub fn create_high_performance_processor() -> EventProcessor {
    let config = ProcessorConfig {
        enable_validation: true,
        enable_routing: true,
        enable_serialization: false, // Disable for performance
        default_format: SerializationFormat::JsonCompact,
        max_concurrent_events: 10000,
        enable_metrics: false, // Disable for performance
    };

    EventProcessor::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation::create_basic_validator;
    use ingest_core::Event;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn create_test_event() -> Event {
        Event::new("user.login", json!({"user_id": "123"}))
    }

    #[test]
    fn test_processor_config_default() {
        let fixture = ProcessorConfig::default();
        assert!(fixture.enable_validation);
        assert!(fixture.enable_routing);
        assert!(fixture.enable_serialization);
        assert_eq!(fixture.max_concurrent_events, 1000);
    }

    #[test]
    fn test_processing_stats_default() {
        let fixture = ProcessingStats::default();
        assert_eq!(fixture.events_processed, 0);
        assert_eq!(fixture.events_success, 0);
        assert_eq!(fixture.events_failed, 0);
        assert_eq!(fixture.success_rate(), 0.0);
    }

    #[test]
    fn test_processing_stats_update() {
        let mut fixture = ProcessingStats::default();
        fixture.update(true, 100);
        fixture.update(false, 200);

        assert_eq!(fixture.events_processed, 2);
        assert_eq!(fixture.events_success, 1);
        assert_eq!(fixture.events_failed, 1);
        assert_eq!(fixture.success_rate(), 50.0);
        assert_eq!(fixture.avg_processing_time_ms, 150.0);
    }

    #[test]
    fn test_event_processor_creation() {
        let _fixture = EventProcessor::new(ProcessorConfig::default());
        // Just test that it can be created
    }

    #[test]
    fn test_event_processor_with_validator() {
        let validator = create_basic_validator();
        let _fixture = EventProcessor::new(ProcessorConfig::default()).with_validator(validator);
        // Just test that it can be created
    }

    #[tokio::test]
    async fn test_process_event_basic() {
        let processor = create_basic_processor();
        let fixture = create_test_event();
        let actual = processor.process_event(fixture).await;
        assert!(actual.is_ok());
        let result = actual.unwrap();
        assert!(result.success);
        assert!(result.validation_passed);
    }

    #[tokio::test]
    async fn test_process_event_validation_disabled() {
        let config = ProcessorConfig {
            enable_validation: false,
            ..Default::default()
        };
        let processor = EventProcessor::new(config);
        let fixture = create_test_event();
        let actual = processor.process_event(fixture).await;
        assert!(actual.is_ok());
        let result = actual.unwrap();
        assert!(result.success);
        assert!(!result.validation_passed); // Should be false when disabled
    }

    #[tokio::test]
    async fn test_process_events_multiple() {
        let processor = create_basic_processor();
        let events = vec![
            create_test_event(),
            Event::new("user.logout", json!({"user_id": "456"})),
        ];
        let actual = processor.process_events(events).await;
        assert!(actual.is_ok());
        let results = actual.unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.success));
    }

    #[tokio::test]
    async fn test_processor_stats() {
        let processor = create_basic_processor();
        let fixture = create_test_event();
        let _ = processor.process_event(fixture).await;

        let actual = processor.stats().await;
        assert_eq!(actual.events_processed, 1);
        assert_eq!(actual.events_success, 1);
        assert_eq!(actual.events_failed, 0);
    }

    #[tokio::test]
    async fn test_processor_reset_stats() {
        let processor = create_basic_processor();
        let fixture = create_test_event();
        let _ = processor.process_event(fixture).await;

        processor.reset_stats().await;
        let actual = processor.stats().await;
        assert_eq!(actual.events_processed, 0);
    }

    #[test]
    fn test_processing_result_creation() {
        let fixture = ProcessingResult::new("test-id".to_string());
        assert_eq!(fixture.event_id, "test-id");
        assert!(!fixture.success);
        assert!(!fixture.has_errors());
    }

    #[test]
    fn test_processing_result_with_errors() {
        let mut fixture = ProcessingResult::new("test-id".to_string());
        fixture.validation_error = Some("Validation failed".to_string());
        fixture.handler_errors.push("Handler failed".to_string());

        assert!(fixture.has_errors());
        let errors = fixture.error_messages();
        assert_eq!(errors.len(), 2);
        assert!(errors[0].contains("Validation"));
        assert!(errors[1].contains("Handler"));
    }

    #[test]
    fn test_create_basic_processor() {
        let _fixture = create_basic_processor();
        // Just test that it can be created
    }

    #[test]
    fn test_create_high_performance_processor() {
        let _fixture = create_high_performance_processor();
        // Just test that it can be created
    }
}
