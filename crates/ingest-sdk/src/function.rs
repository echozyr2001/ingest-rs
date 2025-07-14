//! Function definition and execution context

use crate::error::{Result, SdkError};
use async_trait::async_trait;
use ingest_core::{Event, Json, StepId, StepStatus};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;
use tracing::{debug, info, instrument};

/// Trait for defining Inngest functions
#[async_trait]
pub trait Function: Send + Sync {
    /// Get the function name
    fn name(&self) -> &str;

    /// Get the function configuration
    fn config(&self) -> &FunctionConfig;

    /// Execute the function
    async fn execute(&self, event: Event, step: &mut StepContextWrapper) -> Result<Json>;
}

/// Configuration for a function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionConfig {
    /// Function name
    pub name: String,
    /// Event triggers
    pub triggers: Vec<FunctionTrigger>,
    /// Concurrency settings
    pub concurrency: Option<ConcurrencyConfig>,
    /// Rate limiting settings
    pub rate_limit: Option<RateLimitConfig>,
    /// Retry settings
    pub retry: Option<RetryConfig>,
    /// Timeout settings
    pub timeout: Option<Duration>,
}

impl FunctionConfig {
    /// Create a new function configuration
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            triggers: Vec::new(),
            concurrency: None,
            rate_limit: None,
            retry: None,
            timeout: None,
        }
    }

    /// Add an event trigger
    pub fn trigger(mut self, trigger: FunctionTrigger) -> Self {
        self.triggers.push(trigger);
        self
    }

    /// Set concurrency configuration
    pub fn concurrency(mut self, concurrency: ConcurrencyConfig) -> Self {
        self.concurrency = Some(concurrency);
        self
    }

    /// Set rate limiting configuration
    pub fn rate_limit(mut self, rate_limit: RateLimitConfig) -> Self {
        self.rate_limit = Some(rate_limit);
        self
    }

    /// Set retry configuration
    pub fn retry(mut self, retry: RetryConfig) -> Self {
        self.retry = Some(retry);
        self
    }

    /// Set timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

/// Function trigger configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionTrigger {
    /// Event name pattern
    pub event: String,
    /// Optional expression filter
    pub expression: Option<String>,
}

impl FunctionTrigger {
    /// Create a new trigger for an event
    pub fn event(event: impl Into<String>) -> Self {
        Self {
            event: event.into(),
            expression: None,
        }
    }

    /// Add an expression filter
    pub fn expression(mut self, expression: impl Into<String>) -> Self {
        self.expression = Some(expression.into());
        self
    }
}

/// Concurrency configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrencyConfig {
    /// Maximum concurrent executions
    pub limit: u32,
    /// Concurrency key expression
    pub key: Option<String>,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Number of executions allowed
    pub limit: u32,
    /// Time period for the limit
    pub period: Duration,
    /// Rate limiting key expression
    pub key: Option<String>,
}

/// Retry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retries
    pub attempts: u32,
    /// Base delay between retries
    pub delay: Duration,
    /// Exponential backoff multiplier
    pub backoff: Option<f64>,
}

/// Step context wrapper for different implementations
pub enum StepContextWrapper {
    /// Standard step context
    Standard(StepContext),
    /// Mock step context for testing
    Mock(crate::testing::MockStepContext),
}

#[async_trait::async_trait]
impl StepContextTrait for StepContextWrapper {
    async fn run<F, Fut, T>(&mut self, name: &str, func: F) -> Result<T>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = Result<T>> + Send,
        T: Serialize + for<'de> Deserialize<'de> + Send + 'static,
    {
        match self {
            StepContextWrapper::Standard(ctx) => ctx.run(name, func).await,
            StepContextWrapper::Mock(ctx) => ctx.run(name, func).await,
        }
    }

    async fn sleep(&mut self, name: &str, duration: Duration) -> Result<()> {
        match self {
            StepContextWrapper::Standard(ctx) => ctx.sleep(name, duration).await,
            StepContextWrapper::Mock(ctx) => ctx.sleep(name, duration).await,
        }
    }

    async fn wait_for_event(
        &mut self,
        name: &str,
        event_name: &str,
        timeout: Option<Duration>,
    ) -> Result<Event> {
        match self {
            StepContextWrapper::Standard(ctx) => {
                ctx.wait_for_event(name, event_name, timeout).await
            }
            StepContextWrapper::Mock(ctx) => ctx.wait_for_event(name, event_name, timeout).await,
        }
    }

    async fn send_event(&mut self, name: &str, event: Event) -> Result<()> {
        match self {
            StepContextWrapper::Standard(ctx) => ctx.send_event(name, event).await,
            StepContextWrapper::Mock(ctx) => ctx.send_event(name, event).await,
        }
    }
}

impl StepContextWrapper {
    /// Get the list of executed steps (only available for Mock context)
    pub fn steps_executed(&self) -> Vec<String> {
        match self {
            StepContextWrapper::Standard(_) => Vec::new(),
            StepContextWrapper::Mock(ctx) => ctx.steps_executed(),
        }
    }
}

/// Trait for step execution context
#[async_trait::async_trait]
pub trait StepContextTrait: Send + Sync {
    /// Run a step with the given function
    async fn run<F, Fut, T>(&mut self, name: &str, func: F) -> Result<T>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = Result<T>> + Send,
        T: Serialize + for<'de> Deserialize<'de> + Send + 'static;

    /// Sleep for the specified duration
    async fn sleep(&mut self, name: &str, duration: Duration) -> Result<()>;

    /// Wait for an event
    async fn wait_for_event(
        &mut self,
        name: &str,
        event_name: &str,
        timeout: Option<Duration>,
    ) -> Result<Event>;

    /// Send an event
    async fn send_event(&mut self, name: &str, event: Event) -> Result<()>;
}

/// Step execution context
pub struct StepContext {
    /// Execution state
    state: HashMap<StepId, StepOutput>,
    /// Step counter for generating IDs
    step_counter: u32,
}

impl StepContext {
    /// Create a new step context
    pub fn new() -> Self {
        Self {
            state: HashMap::new(),
            step_counter: 0,
        }
    }

    /// Get the current step state
    pub fn state(&self) -> &HashMap<StepId, StepOutput> {
        &self.state
    }

    /// Generate a unique step ID
    fn generate_step_id(&mut self, name: &str) -> StepId {
        self.step_counter += 1;
        format!("{}_{}", name, self.step_counter).into()
    }
}

#[async_trait::async_trait]
impl StepContextTrait for StepContext {
    /// Run a step with the given function
    #[instrument(skip(self, func))]
    async fn run<F, Fut, T>(&mut self, name: &str, func: F) -> Result<T>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = Result<T>> + Send,
        T: Serialize + for<'de> Deserialize<'de> + Send + 'static,
    {
        let step_id = self.generate_step_id(name);

        debug!("Running step: {}", step_id);

        // Check if step has already been executed
        if let Some(output) = self.state.get(&step_id) {
            if output.status == StepStatus::Completed {
                info!(
                    "Step {} already completed, returning cached result",
                    step_id
                );
                return Ok(serde_json::from_value(output.data.clone())?);
            }
        }

        // Execute the step
        match func().await {
            Ok(result) => {
                let output = StepOutput {
                    id: step_id.clone(),
                    status: StepStatus::Completed,
                    data: serde_json::to_value(&result)?,
                    error: None,
                    started_at: chrono::Utc::now(),
                    completed_at: Some(chrono::Utc::now()),
                };

                self.state.insert(step_id, output);
                Ok(result)
            }
            Err(e) => {
                let output = StepOutput {
                    id: step_id.clone(),
                    status: StepStatus::Failed,
                    data: serde_json::Value::Null,
                    error: Some(e.to_string()),
                    started_at: chrono::Utc::now(),
                    completed_at: Some(chrono::Utc::now()),
                };

                self.state.insert(step_id, output);
                Err(e)
            }
        }
    }

    /// Sleep for the specified duration
    async fn sleep(&mut self, name: &str, duration: Duration) -> Result<()> {
        self.run(name, || async move {
            tokio::time::sleep(duration).await;
            Ok(())
        })
        .await
    }

    /// Wait for an event
    async fn wait_for_event(
        &mut self,
        name: &str,
        event_name: &str,
        timeout: Option<Duration>,
    ) -> Result<Event> {
        self.run(name, || async move {
            // This is a placeholder implementation
            // In a real implementation, this would wait for an actual event
            tokio::time::sleep(timeout.unwrap_or(Duration::from_secs(30))).await;
            Err(SdkError::timeout(format!(
                "Timeout waiting for event: {event_name}"
            )))
        })
        .await
    }

    /// Send an event
    async fn send_event(&mut self, name: &str, event: Event) -> Result<()> {
        self.run(name, || async move {
            // This is a placeholder implementation
            // In a real implementation, this would send the event
            debug!("Sending event: {}", event.name());
            Ok(())
        })
        .await
    }
}

impl Default for StepContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Step execution output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepOutput {
    /// Step ID
    pub id: StepId,
    /// Execution status
    pub status: StepStatus,
    /// Output data
    pub data: Json,
    /// Error message if failed
    pub error: Option<String>,
    /// When the step started
    pub started_at: chrono::DateTime<chrono::Utc>,
    /// When the step completed
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Function builder for creating function instances
pub struct FunctionBuilder {
    config: FunctionConfig,
}

impl FunctionBuilder {
    /// Create a new function builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            config: FunctionConfig::new(name),
        }
    }

    /// Add a trigger
    pub fn trigger(mut self, trigger: FunctionTrigger) -> Self {
        self.config = self.config.trigger(trigger);
        self
    }

    /// Set concurrency
    pub fn concurrency(mut self, limit: u32, key: Option<String>) -> Self {
        self.config = self.config.concurrency(ConcurrencyConfig { limit, key });
        self
    }

    /// Set rate limiting
    pub fn rate_limit(mut self, limit: u32, period: Duration, key: Option<String>) -> Self {
        self.config = self
            .config
            .rate_limit(RateLimitConfig { limit, period, key });
        self
    }

    /// Set retry configuration
    pub fn retry(mut self, attempts: u32, delay: Duration, backoff: Option<f64>) -> Self {
        self.config = self.config.retry(RetryConfig {
            attempts,
            delay,
            backoff,
        });
        self
    }

    /// Set timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config = self.config.timeout(timeout);
        self
    }

    /// Build the function configuration
    pub fn build(self) -> FunctionConfig {
        self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_function_config_new() {
        let fixture = FunctionConfig::new("test-function");
        let actual = fixture.name;
        let expected = "test-function";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_function_trigger() {
        let fixture = FunctionTrigger::event("user.created");
        let actual = fixture.event;
        let expected = "user.created";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_function_trigger_with_expression() {
        let fixture = FunctionTrigger::event("user.created").expression("event.data.email != null");
        assert!(fixture.expression.is_some());
    }

    #[test]
    fn test_step_context_new() {
        let fixture = StepContext::new();
        assert!(fixture.state.is_empty());
        assert_eq!(fixture.step_counter, 0);
    }

    #[tokio::test]
    async fn test_step_context_run() {
        let mut fixture = StepContext::new();
        let actual = fixture
            .run("test_step", || async { Ok("result".to_string()) })
            .await;
        assert!(actual.is_ok());
        assert_eq!(actual.unwrap(), "result");
    }

    #[tokio::test]
    async fn test_step_context_sleep() {
        let mut fixture = StepContext::new();
        let actual = fixture.sleep("test_sleep", Duration::from_millis(10)).await;
        assert!(actual.is_ok());
    }

    #[test]
    fn test_function_builder() {
        let fixture = FunctionBuilder::new("test-function")
            .trigger(FunctionTrigger::event("user.created"))
            .concurrency(10, None)
            .timeout(Duration::from_secs(30))
            .build();

        assert_eq!(fixture.name, "test-function");
        assert_eq!(fixture.triggers.len(), 1);
        assert!(fixture.concurrency.is_some());
        assert!(fixture.timeout.is_some());
    }
}
