use derive_setters::Setters;
use ingest_core::{DateTime, Event, Function, Id, Json, Step, StepOutput};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Execution request containing function and trigger event
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct ExecutionRequest {
    /// Unique execution run identifier
    pub run_id: Id,
    /// Function to execute
    pub function: Function,
    /// Triggering event
    pub event: Event,
    /// Execution configuration overrides
    pub config: ExecutionConfig,
    /// Request metadata
    pub metadata: HashMap<String, String>,
    /// Request timestamp
    pub created_at: DateTime,
}

impl ExecutionRequest {
    /// Create a new execution request
    pub fn new(function: Function, event: Event) -> Self {
        Self {
            run_id: ingest_core::generate_id_with_prefix("run"),
            function,
            event,
            config: ExecutionConfig::default(),
            metadata: HashMap::new(),
            created_at: chrono::Utc::now(),
        }
    }

    /// Add metadata to the request
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Execution configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
#[derive(Default)]
pub struct ExecutionConfig {
    /// Override function timeout
    pub timeout: Option<std::time::Duration>,
    /// Override retry configuration
    pub max_retries: Option<u32>,
    /// Override concurrency limits
    pub concurrency_limit: Option<u32>,
    /// Execution priority
    pub priority: Option<i32>,
    /// Additional configuration
    pub config: HashMap<String, Json>,
}


/// Execution status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// Execution is pending
    Pending,
    /// Execution is running
    Running,
    /// Execution completed successfully
    Completed,
    /// Execution failed
    Failed,
    /// Execution was cancelled
    Cancelled,
    /// Execution is paused
    Paused,
    /// Execution status is unknown
    Unknown,
}

impl std::fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionStatus::Pending => write!(f, "pending"),
            ExecutionStatus::Running => write!(f, "running"),
            ExecutionStatus::Completed => write!(f, "completed"),
            ExecutionStatus::Failed => write!(f, "failed"),
            ExecutionStatus::Cancelled => write!(f, "cancelled"),
            ExecutionStatus::Paused => write!(f, "paused"),
            ExecutionStatus::Unknown => write!(f, "unknown"),
        }
    }
}

/// Execution result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct ExecutionResult {
    /// Execution run identifier
    pub run_id: Id,
    /// Function identifier
    pub function_id: Id,
    /// Execution status
    pub status: ExecutionStatus,
    /// Execution output data
    pub output: Option<Json>,
    /// Error information if failed
    pub error: Option<String>,
    /// Execution steps
    pub steps: Vec<Step>,
    /// Step outputs from execution
    pub step_outputs: Vec<StepOutput>,
    /// Execution start time
    pub started_at: Option<DateTime>,
    /// Execution completion time
    pub completed_at: Option<DateTime>,
    /// Execution duration
    pub duration: Option<std::time::Duration>,
    /// Result metadata
    pub metadata: HashMap<String, String>,
}

impl ExecutionResult {
    /// Create a new execution result
    pub fn new(run_id: Id, function_id: Id) -> Self {
        Self {
            run_id,
            function_id,
            status: ExecutionStatus::Pending,
            output: None,
            error: None,
            steps: Vec::new(),
            step_outputs: Vec::new(),
            started_at: None,
            completed_at: None,
            duration: None,
            metadata: HashMap::new(),
        }
    }

    /// Mark execution as started
    pub fn start(&mut self) {
        self.status = ExecutionStatus::Running;
        self.started_at = Some(chrono::Utc::now());
    }

    /// Mark execution as completed
    pub fn complete(&mut self, output: Option<Json>) {
        self.status = ExecutionStatus::Completed;
        self.output = output;
        self.completed_at = Some(chrono::Utc::now());
        self.calculate_duration();
    }

    /// Mark execution as failed
    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = ExecutionStatus::Failed;
        self.error = Some(error.into());
        self.completed_at = Some(chrono::Utc::now());
        self.calculate_duration();
    }

    /// Mark execution as cancelled
    pub fn cancel(&mut self) {
        self.status = ExecutionStatus::Cancelled;
        self.completed_at = Some(chrono::Utc::now());
        self.calculate_duration();
    }

    /// Mark execution as paused
    pub fn pause(&mut self) {
        self.status = ExecutionStatus::Paused;
    }

    /// Add a step to the execution
    pub fn add_step(&mut self, step: Step) {
        self.steps.push(step);
    }

    /// Add a step output to the execution
    pub fn add_step_output(&mut self, output: StepOutput) {
        self.step_outputs.push(output);
    }

    /// Check if execution is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            ExecutionStatus::Completed | ExecutionStatus::Failed | ExecutionStatus::Cancelled
        )
    }

    /// Check if execution is active
    pub fn is_active(&self) -> bool {
        matches!(self.status, ExecutionStatus::Running)
    }

    /// Calculate execution duration
    fn calculate_duration(&mut self) {
        if let (Some(start), Some(end)) = (self.started_at, self.completed_at) {
            self.duration = Some((end - start).to_std().unwrap_or_default());
        }
    }
}

/// Step execution request
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepExecutionRequest {
    /// Step to execute
    pub step: Step,
    /// Execution context
    pub run_id: Id,
    /// Function identifier
    pub function_id: Id,
    /// Step input data
    pub input: Option<Json>,
    /// Step configuration
    pub config: HashMap<String, Json>,
}

impl StepExecutionRequest {
    /// Create a new step execution request
    pub fn new(step: Step, run_id: Id, function_id: Id) -> Self {
        Self {
            step,
            run_id,
            function_id,
            input: None,
            config: HashMap::new(),
        }
    }

    /// Set step input
    pub fn with_input(mut self, input: Json) -> Self {
        self.input = Some(input);
        self
    }

    /// Add configuration
    pub fn with_config(mut self, key: impl Into<String>, value: Json) -> Self {
        self.config.insert(key.into(), value);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingest_core::{FunctionTrigger, generate_id_with_prefix};
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn create_test_function() -> Function {
        ingest_core::Function::new("test-function")
            .add_trigger(FunctionTrigger::event("user.created"))
    }

    fn create_test_event() -> Event {
        ingest_core::Event::new("user.created", json!({"user_id": 123}))
    }

    #[test]
    fn test_execution_request_creation() {
        let fixture_function = create_test_function();
        let fixture_event = create_test_event();

        let actual = ExecutionRequest::new(fixture_function.clone(), fixture_event.clone());

        assert_eq!(actual.function, fixture_function);
        assert_eq!(actual.event, fixture_event);
        assert!(actual.run_id.as_str().starts_with("run_"));
        assert!(actual.metadata.is_empty());
    }

    #[test]
    fn test_execution_request_with_metadata() {
        let fixture = ExecutionRequest::new(create_test_function(), create_test_event())
            .with_metadata("source", "api")
            .with_metadata("version", "1.0");

        assert_eq!(fixture.metadata.get("source"), Some(&"api".to_string()));
        assert_eq!(fixture.metadata.get("version"), Some(&"1.0".to_string()));
    }

    #[test]
    fn test_execution_config_default() {
        let actual = ExecutionConfig::default();
        assert!(actual.timeout.is_none());
        assert!(actual.max_retries.is_none());
        assert!(actual.concurrency_limit.is_none());
        assert!(actual.priority.is_none());
        assert!(actual.config.is_empty());
    }

    #[test]
    fn test_execution_result_creation() {
        let fixture_run_id = generate_id_with_prefix("run");
        let fixture_function_id = generate_id_with_prefix("fn");

        let actual = ExecutionResult::new(fixture_run_id.clone(), fixture_function_id.clone());

        assert_eq!(actual.run_id, fixture_run_id);
        assert_eq!(actual.function_id, fixture_function_id);
        assert_eq!(actual.status, ExecutionStatus::Pending);
        assert!(actual.steps.is_empty());
    }

    #[test]
    fn test_execution_result_start() {
        let mut fixture = ExecutionResult::new(
            generate_id_with_prefix("run"),
            generate_id_with_prefix("fn"),
        );

        fixture.start();

        assert_eq!(fixture.status, ExecutionStatus::Running);
        assert!(fixture.started_at.is_some());
    }

    #[test]
    fn test_execution_result_complete() {
        let mut fixture = ExecutionResult::new(
            generate_id_with_prefix("run"),
            generate_id_with_prefix("fn"),
        );
        fixture.start();

        let output = json!({"result": "success"});
        fixture.complete(Some(output.clone()));

        assert_eq!(fixture.status, ExecutionStatus::Completed);
        assert_eq!(fixture.output, Some(output));
        assert!(fixture.completed_at.is_some());
        assert!(fixture.duration.is_some());
    }

    #[test]
    fn test_execution_result_fail() {
        let mut fixture = ExecutionResult::new(
            generate_id_with_prefix("run"),
            generate_id_with_prefix("fn"),
        );
        fixture.start();

        fixture.fail("Something went wrong");

        assert_eq!(fixture.status, ExecutionStatus::Failed);
        assert_eq!(fixture.error, Some("Something went wrong".to_string()));
        assert!(fixture.completed_at.is_some());
    }

    #[test]
    fn test_execution_result_is_terminal() {
        let mut fixture = ExecutionResult::new(
            generate_id_with_prefix("run"),
            generate_id_with_prefix("fn"),
        );

        assert!(!fixture.is_terminal());

        fixture.status = ExecutionStatus::Completed;
        assert!(fixture.is_terminal());

        fixture.status = ExecutionStatus::Failed;
        assert!(fixture.is_terminal());

        fixture.status = ExecutionStatus::Cancelled;
        assert!(fixture.is_terminal());
    }

    #[test]
    fn test_execution_result_is_active() {
        let mut fixture = ExecutionResult::new(
            generate_id_with_prefix("run"),
            generate_id_with_prefix("fn"),
        );

        assert!(!fixture.is_active());

        fixture.status = ExecutionStatus::Running;
        assert!(fixture.is_active());
    }

    #[test]
    fn test_step_execution_request() {
        let fixture_step = ingest_core::Step::new(
            "test-step",
            generate_id_with_prefix("fn"),
            generate_id_with_prefix("run"),
        );
        let fixture_run_id = generate_id_with_prefix("run");
        let fixture_function_id = generate_id_with_prefix("fn");

        let actual = StepExecutionRequest::new(
            fixture_step.clone(),
            fixture_run_id.clone(),
            fixture_function_id.clone(),
        );

        assert_eq!(actual.step, fixture_step);
        assert_eq!(actual.run_id, fixture_run_id);
        assert_eq!(actual.function_id, fixture_function_id);
    }

    #[test]
    fn test_step_execution_request_with_input() {
        let fixture_step = ingest_core::Step::new(
            "test-step",
            generate_id_with_prefix("fn"),
            generate_id_with_prefix("run"),
        );
        let fixture_input = json!({"key": "value"});

        let actual = StepExecutionRequest::new(
            fixture_step,
            generate_id_with_prefix("run"),
            generate_id_with_prefix("fn"),
        )
        .with_input(fixture_input.clone());

        assert_eq!(actual.input, Some(fixture_input));
    }

    #[test]
    fn test_execution_result_serialization() {
        let fixture = ExecutionResult::new(
            generate_id_with_prefix("run"),
            generate_id_with_prefix("fn"),
        );

        let actual = serde_json::to_string(&fixture);
        assert!(actual.is_ok());
    }

    #[test]
    fn test_execution_request_serialization() {
        let fixture = ExecutionRequest::new(create_test_function(), create_test_event());

        let actual = serde_json::to_string(&fixture);
        assert!(actual.is_ok());
    }
}
