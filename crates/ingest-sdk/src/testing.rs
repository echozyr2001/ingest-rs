//! Testing utilities for Inngest functions

use crate::error::{Result, SdkError};
use crate::function::{Function, StepContextTrait};
use ingest_core::{Event, EventId, Json};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, info, instrument};

/// Test harness for function testing
pub struct TestHarness {
    /// Mock step outputs
    mock_outputs: HashMap<String, Json>,
    /// Event fixtures
    event_fixtures: HashMap<String, Event>,
}

impl TestHarness {
    /// Create a new test harness
    pub fn new() -> Self {
        Self {
            mock_outputs: HashMap::new(),
            event_fixtures: HashMap::new(),
        }
    }

    /// Add a mock step output
    pub fn mock_step(mut self, step_name: impl Into<String>, output: Json) -> Self {
        self.mock_outputs.insert(step_name.into(), output);
        self
    }

    /// Add an event fixture
    pub fn event_fixture(mut self, name: impl Into<String>, event: Event) -> Self {
        self.event_fixtures.insert(name.into(), event);
        self
    }

    /// Run a function test
    #[instrument(skip(self, function, event))]
    pub async fn run_function_test<F: Function>(
        &self,
        function: F,
        event: Event,
    ) -> Result<TestResult> {
        info!("Running function test: {}", function.name());

        let mut step_context =
            crate::function::StepContextWrapper::Mock(MockStepContext::new(&self.mock_outputs));

        let start_time = std::time::Instant::now();
        let result = function.execute(event.clone(), &mut step_context).await;
        let duration = start_time.elapsed();

        let test_result = TestResult {
            function_name: function.name().to_string(),
            event_id: event.id.clone(),
            result,
            duration,
            steps_executed: step_context.steps_executed(),
        };

        debug!("Function test completed in {:?}", duration);
        Ok(test_result)
    }

    /// Get an event fixture by name
    pub fn get_event_fixture(&self, name: &str) -> Option<&Event> {
        self.event_fixtures.get(name)
    }
}

impl Default for TestHarness {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock step context for testing
pub struct MockStepContext {
    mock_outputs: HashMap<String, Json>,
    executed_steps: Vec<String>,
}

impl MockStepContext {
    /// Create a new mock step context
    pub fn new(mock_outputs: &HashMap<String, Json>) -> Self {
        Self {
            mock_outputs: mock_outputs.clone(),
            executed_steps: Vec::new(),
        }
    }

    /// Get the list of executed steps
    pub fn steps_executed(&self) -> Vec<String> {
        self.executed_steps.clone()
    }
}

#[async_trait::async_trait]
impl StepContextTrait for MockStepContext {
    async fn run<F, Fut, T>(&mut self, name: &str, _func: F) -> Result<T>
    where
        F: FnOnce() -> Fut + Send,
        Fut: std::future::Future<Output = Result<T>> + Send,
        T: serde::Serialize + for<'de> serde::Deserialize<'de> + Send + 'static,
    {
        self.executed_steps.push(name.to_string());

        if let Some(output) = self.mock_outputs.get(name) {
            debug!("Using mock output for step: {}", name);
            Ok(serde_json::from_value(output.clone())?)
        } else {
            debug!("No mock output for step: {}, executing function", name);
            _func().await
        }
    }

    async fn sleep(&mut self, name: &str, _duration: std::time::Duration) -> Result<()> {
        self.executed_steps.push(format!("sleep_{name}"));
        debug!("Mock sleep: {}", name);
        Ok(())
    }

    async fn wait_for_event(
        &mut self,
        name: &str,
        _event_name: &str,
        _timeout: Option<std::time::Duration>,
    ) -> Result<Event> {
        self.executed_steps.push(format!("wait_{name}"));

        if let Some(output) = self.mock_outputs.get(name) {
            debug!("Using mock event for wait: {}", name);
            Ok(serde_json::from_value(output.clone())?)
        } else {
            Err(SdkError::execution(
                "No mock event provided for wait_for_event",
            ))
        }
    }

    async fn send_event(&mut self, name: &str, _event: Event) -> Result<()> {
        self.executed_steps.push(format!("send_{name}"));
        debug!("Mock send event: {}", name);
        Ok(())
    }
}

/// Result of a function test
#[derive(Debug)]
pub struct TestResult {
    /// Function name
    pub function_name: String,
    /// Event ID that triggered the test
    pub event_id: EventId,
    /// Test result
    pub result: Result<Json>,
    /// Test duration
    pub duration: std::time::Duration,
    /// Steps that were executed
    pub steps_executed: Vec<String>,
}

impl TestResult {
    /// Check if the test was successful
    pub fn is_success(&self) -> bool {
        self.result.is_ok()
    }

    /// Get the test output if successful
    pub fn output(&self) -> Option<&Json> {
        self.result.as_ref().ok()
    }

    /// Get the test error if failed
    pub fn error(&self) -> Option<&SdkError> {
        self.result.as_ref().err()
    }
}

/// Create a test event
pub fn create_test_event(event_type: &str, data: Json) -> Event {
    Event::new(event_type, data)
}

/// Create a test event with user context
pub fn create_user_event(event_type: &str, user_id: &str, data: Json) -> Event {
    let mut event = Event::new(event_type, data);
    event.user = Some(user_id.to_string());
    event
}

/// Create test data from a struct
pub fn create_test_data<T: serde::Serialize>(data: T) -> Result<Json> {
    Ok(serde_json::to_value(data)?)
}

/// Event builder for tests
pub struct TestEventBuilder {
    name: String,
    data: Json,
    user: Option<Json>,
    id: Option<EventId>,
}

impl TestEventBuilder {
    /// Create a new test event builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            data: Value::Object(serde_json::Map::new()),
            user: None,
            id: None,
        }
    }

    /// Set event data
    pub fn data(mut self, data: Json) -> Self {
        self.data = data;
        self
    }

    /// Set user context
    pub fn user(mut self, user: Json) -> Self {
        self.user = Some(user);
        self
    }

    /// Set custom event ID
    pub fn id(mut self, id: EventId) -> Self {
        self.id = Some(id);
        self
    }

    /// Build the test event
    pub fn build(self) -> Event {
        let mut event = Event::new(&self.name, self.data);

        if let Some(user) = self.user {
            event.user = Some(serde_json::to_string(&user).unwrap_or_default());
        }

        if let Some(id) = self.id {
            event.id = id;
        }

        event
    }
}

/// Assert that a function test was successful
pub fn assert_function_success(result: &TestResult) {
    assert!(
        result.is_success(),
        "Function test failed: {:?}",
        result.error()
    );
}

/// Assert that a function test failed
pub fn assert_function_failure(result: &TestResult) {
    assert!(
        !result.is_success(),
        "Function test should have failed but succeeded"
    );
}

/// Assert that specific steps were executed
pub fn assert_steps_executed(result: &TestResult, expected_steps: &[&str]) {
    let actual_steps: Vec<&str> = result.steps_executed.iter().map(|s| s.as_str()).collect();
    assert_eq!(
        actual_steps, expected_steps,
        "Expected steps {expected_steps:?}, but got {actual_steps:?}"
    );
}

/// Assert that the function output matches expected value
pub fn assert_output_equals(result: &TestResult, expected: &Json) {
    assert_function_success(result);
    assert_eq!(
        result.output().unwrap(),
        expected,
        "Function output does not match expected value"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::function::{FunctionConfig, FunctionTrigger};
    use async_trait::async_trait;
    use pretty_assertions::assert_eq;

    struct TestFunction {
        config: FunctionConfig,
    }

    impl TestFunction {
        fn new() -> Self {
            Self {
                config: FunctionConfig::new("test-function")
                    .trigger(FunctionTrigger::event("test.event")),
            }
        }
    }

    #[async_trait]
    impl Function for TestFunction {
        fn name(&self) -> &str {
            "test-function"
        }

        fn config(&self) -> &crate::function::FunctionConfig {
            &self.config
        }

        async fn execute(
            &self,
            _event: Event,
            step: &mut crate::function::StepContextWrapper,
        ) -> Result<Json> {
            let result: String = step
                .run("test_step", || async { Ok("test_result".to_string()) })
                .await?;
            Ok(serde_json::json!({"result": result}))
        }
    }

    #[test]
    fn test_harness_new() {
        let fixture = TestHarness::new();
        assert!(fixture.mock_outputs.is_empty());
        assert!(fixture.event_fixtures.is_empty());
    }

    #[test]
    fn test_harness_mock_step() {
        let fixture = TestHarness::new().mock_step("test_step", serde_json::json!("mocked_result"));
        assert!(fixture.mock_outputs.contains_key("test_step"));
    }

    #[tokio::test]
    async fn test_run_function_test() {
        let fixture = TestHarness::new().mock_step("test_step", serde_json::json!("mocked_result"));

        let function = TestFunction::new();
        let event = create_test_event("test.event", serde_json::json!({}));

        let actual = fixture.run_function_test(function, event).await.unwrap();
        assert!(actual.is_success());
        assert_eq!(actual.function_name, "test-function");
    }

    #[test]
    fn test_create_test_event() {
        let fixture = create_test_event("test.event", serde_json::json!({"key": "value"}));
        assert_eq!(fixture.name(), "test.event");
    }

    #[test]
    fn test_create_user_event() {
        let fixture = create_user_event("user.created", "user123", serde_json::json!({}));
        assert_eq!(fixture.name(), "user.created");
        assert!(fixture.user.is_some());
    }

    #[test]
    fn test_event_builder() {
        let fixture = TestEventBuilder::new("test.event")
            .data(serde_json::json!({"key": "value"}))
            .user(serde_json::json!({"id": "user123"}))
            .build();

        assert_eq!(fixture.name(), "test.event");
        assert!(fixture.user.is_some());
    }

    #[test]
    fn test_assert_function_success() {
        let fixture = TestResult {
            function_name: "test".to_string(),
            event_id: "event123".to_string().into(),
            result: Ok(serde_json::json!({})),
            duration: std::time::Duration::from_millis(100),
            steps_executed: vec![],
        };

        assert_function_success(&fixture);
    }

    #[test]
    fn test_assert_steps_executed() {
        let fixture = TestResult {
            function_name: "test".to_string(),
            event_id: "event123".to_string().into(),
            result: Ok(serde_json::json!({})),
            duration: std::time::Duration::from_millis(100),
            steps_executed: vec!["step1".to_string(), "step2".to_string()],
        };

        assert_steps_executed(&fixture, &["step1", "step2"]);
    }
}
