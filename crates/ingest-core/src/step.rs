use crate::{DateTime, Id, Json, Result};
use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Step identifier type
pub type StepId = Id;

/// Step execution status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StepStatus {
    /// Step is pending execution
    Pending,
    /// Step is currently running
    Running,
    /// Step completed successfully
    Completed,
    /// Step failed with error
    Failed,
    /// Step was cancelled
    Cancelled,
    /// Step is waiting for an event
    Waiting,
    /// Step is sleeping/delayed
    Sleeping,
}

/// Step output data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepOutput {
    /// Output data
    pub data: Json,
    /// Output metadata
    pub metadata: HashMap<String, String>,
    /// Error information if step failed
    pub error: Option<String>,
}

impl StepOutput {
    /// Create successful output
    pub fn success(data: Json) -> Self {
        Self {
            data,
            metadata: HashMap::new(),
            error: None,
        }
    }

    /// Create failed output
    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            data: serde_json::Value::Null,
            metadata: HashMap::new(),
            error: Some(error.into()),
        }
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Check if step output indicates success
    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }

    /// Check if step output indicates failure
    pub fn is_failure(&self) -> bool {
        self.error.is_some()
    }
}

/// Step configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct StepConfig {
    /// Step timeout
    pub timeout: Option<std::time::Duration>,
    /// Maximum retries for this step
    pub max_retries: Option<u32>,
    /// Retry delay
    pub retry_delay: Option<std::time::Duration>,
    /// Step-specific configuration
    pub config: HashMap<String, Json>,
}

impl Default for StepConfig {
    fn default() -> Self {
        Self {
            timeout: Some(std::time::Duration::from_secs(60)),
            max_retries: Some(3),
            retry_delay: Some(std::time::Duration::from_secs(1)),
            config: HashMap::new(),
        }
    }
}

/// Step definition and execution state
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct Step {
    /// Unique step identifier
    pub id: StepId,
    /// Step name
    pub name: String,
    /// Function this step belongs to
    pub function_id: crate::FunctionId,
    /// Run this step belongs to
    pub run_id: Id,
    /// Step execution status
    pub status: StepStatus,
    /// Step configuration
    pub config: StepConfig,
    /// Step input data
    pub input: Option<Json>,
    /// Step output data
    pub output: Option<StepOutput>,
    /// Number of retry attempts
    pub retry_count: u32,
    /// Step creation timestamp
    pub created_at: DateTime,
    /// Step start timestamp
    pub started_at: Option<DateTime>,
    /// Step completion timestamp
    pub completed_at: Option<DateTime>,
    /// Next scheduled execution time
    pub scheduled_at: Option<DateTime>,
    /// Step metadata
    pub metadata: HashMap<String, String>,
}

impl Step {
    /// Create a new step
    pub fn new(name: impl Into<String>, function_id: crate::FunctionId, run_id: Id) -> Self {
        Self {
            id: crate::generate_id_with_prefix("step"),
            name: name.into(),
            function_id,
            run_id,
            status: StepStatus::Pending,
            config: StepConfig::default(),
            input: None,
            output: None,
            retry_count: 0,
            created_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            scheduled_at: None,
            metadata: HashMap::new(),
        }
    }

    /// Set step input
    pub fn with_input(mut self, input: Json) -> Self {
        self.input = Some(input);
        self
    }

    /// Mark step as started
    pub fn start(&mut self) {
        self.status = StepStatus::Running;
        self.started_at = Some(chrono::Utc::now());
    }

    /// Mark step as completed with output
    pub fn complete(&mut self, output: StepOutput) {
        self.status = if output.is_success() {
            StepStatus::Completed
        } else {
            StepStatus::Failed
        };
        self.output = Some(output);
        self.completed_at = Some(chrono::Utc::now());
    }

    /// Mark step as failed
    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = StepStatus::Failed;
        self.output = Some(StepOutput::failure(error));
        self.completed_at = Some(chrono::Utc::now());
    }

    /// Mark step as cancelled
    pub fn cancel(&mut self) {
        self.status = StepStatus::Cancelled;
        self.completed_at = Some(chrono::Utc::now());
    }

    /// Mark step as waiting
    pub fn wait(&mut self) {
        self.status = StepStatus::Waiting;
    }

    /// Mark step as sleeping until a specific time
    pub fn sleep_until(&mut self, until: DateTime) {
        self.status = StepStatus::Sleeping;
        self.scheduled_at = Some(until);
    }

    /// Increment retry count
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }

    /// Check if step can be retried
    pub fn can_retry(&self) -> bool {
        if let Some(max_retries) = self.config.max_retries {
            self.retry_count < max_retries
        } else {
            false
        }
    }

    /// Check if step is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            StepStatus::Completed | StepStatus::Failed | StepStatus::Cancelled
        )
    }

    /// Check if step is active (running or waiting)
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            StepStatus::Running | StepStatus::Waiting | StepStatus::Sleeping
        )
    }

    /// Get step duration if completed
    pub fn duration(&self) -> Option<chrono::Duration> {
        match (self.started_at, self.completed_at) {
            (Some(start), Some(end)) => Some(end - start),
            _ => None,
        }
    }

    /// Validate the step
    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(crate::Error::step("Step name cannot be empty"));
        }

        if self.name.len() > 255 {
            return Err(crate::Error::step(
                "Step name too long (max 255 characters)",
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn create_test_step() -> Step {
        Step::new(
            "test-step",
            crate::generate_id_with_prefix("fn"),
            crate::generate_id_with_prefix("run"),
        )
    }

    #[test]
    fn test_step_output_success() {
        let fixture = json!({"result": "success"});
        let actual = StepOutput::success(fixture.clone());
        let expected = StepOutput {
            data: fixture,
            metadata: HashMap::new(),
            error: None,
        };
        assert_eq!(actual, expected);
        assert!(actual.is_success());
        assert!(!actual.is_failure());
    }

    #[test]
    fn test_step_output_failure() {
        let fixture = "Something went wrong";
        let actual = StepOutput::failure(fixture);
        assert!(actual.is_failure());
        assert!(!actual.is_success());
        assert_eq!(actual.error, Some("Something went wrong".to_string()));
    }

    #[test]
    fn test_step_output_with_metadata() {
        let fixture = StepOutput::success(json!({})).with_metadata("source", "test");

        let actual = fixture.metadata.get("source").unwrap();
        let expected = "test";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_step_config_default() {
        let actual = StepConfig::default();
        assert_eq!(actual.timeout, Some(std::time::Duration::from_secs(60)));
        assert_eq!(actual.max_retries, Some(3));
        assert!(actual.config.is_empty());
    }

    #[test]
    fn test_step_creation() {
        let fixture_name = "test-step";
        let fixture_fn_id = crate::generate_id_with_prefix("fn");
        let fixture_run_id = crate::generate_id_with_prefix("run");

        let actual = Step::new(fixture_name, fixture_fn_id.clone(), fixture_run_id.clone());

        assert_eq!(actual.name, "test-step");
        assert_eq!(actual.function_id, fixture_fn_id);
        assert_eq!(actual.run_id, fixture_run_id);
        assert_eq!(actual.status, StepStatus::Pending);
        assert!(actual.id.as_str().starts_with("step_"));
    }

    #[test]
    fn test_step_with_input() {
        let fixture_input = json!({"key": "value"});
        let actual = create_test_step().with_input(fixture_input.clone());
        assert_eq!(actual.input, Some(fixture_input));
    }

    #[test]
    fn test_step_start() {
        let mut fixture = create_test_step();
        fixture.start();

        assert_eq!(fixture.status, StepStatus::Running);
        assert!(fixture.started_at.is_some());
    }

    #[test]
    fn test_step_complete_success() {
        let mut fixture = create_test_step();
        let output = StepOutput::success(json!({"result": "ok"}));

        fixture.complete(output.clone());

        assert_eq!(fixture.status, StepStatus::Completed);
        assert_eq!(fixture.output, Some(output));
        assert!(fixture.completed_at.is_some());
    }

    #[test]
    fn test_step_complete_failure() {
        let mut fixture = create_test_step();
        let output = StepOutput::failure("error");

        fixture.complete(output.clone());

        assert_eq!(fixture.status, StepStatus::Failed);
        assert_eq!(fixture.output, Some(output));
        assert!(fixture.completed_at.is_some());
    }

    #[test]
    fn test_step_fail() {
        let mut fixture = create_test_step();
        fixture.fail("test error");

        assert_eq!(fixture.status, StepStatus::Failed);
        assert!(fixture.output.is_some());
        assert!(fixture.output.as_ref().unwrap().is_failure());
    }

    #[test]
    fn test_step_cancel() {
        let mut fixture = create_test_step();
        fixture.cancel();

        assert_eq!(fixture.status, StepStatus::Cancelled);
        assert!(fixture.completed_at.is_some());
    }

    #[test]
    fn test_step_wait() {
        let mut fixture = create_test_step();
        fixture.wait();

        assert_eq!(fixture.status, StepStatus::Waiting);
    }

    #[test]
    fn test_step_sleep_until() {
        let mut fixture = create_test_step();
        let until = chrono::Utc::now() + chrono::Duration::hours(1);

        fixture.sleep_until(until);

        assert_eq!(fixture.status, StepStatus::Sleeping);
        assert_eq!(fixture.scheduled_at, Some(until));
    }

    #[test]
    fn test_step_can_retry() {
        let mut fixture = create_test_step();

        assert!(fixture.can_retry());

        fixture.retry_count = 3;
        assert!(!fixture.can_retry());
    }

    #[test]
    fn test_step_is_terminal() {
        let mut fixture = create_test_step();

        assert!(!fixture.is_terminal());

        fixture.status = StepStatus::Completed;
        assert!(fixture.is_terminal());

        fixture.status = StepStatus::Failed;
        assert!(fixture.is_terminal());

        fixture.status = StepStatus::Cancelled;
        assert!(fixture.is_terminal());
    }

    #[test]
    fn test_step_is_active() {
        let mut fixture = create_test_step();

        assert!(!fixture.is_active());

        fixture.status = StepStatus::Running;
        assert!(fixture.is_active());

        fixture.status = StepStatus::Waiting;
        assert!(fixture.is_active());

        fixture.status = StepStatus::Sleeping;
        assert!(fixture.is_active());
    }

    #[test]
    fn test_step_duration() {
        let mut fixture = create_test_step();

        assert!(fixture.duration().is_none());

        let start = chrono::Utc::now();
        let end = start + chrono::Duration::seconds(5);

        fixture.started_at = Some(start);
        fixture.completed_at = Some(end);

        let actual = fixture.duration().unwrap();
        let expected = chrono::Duration::seconds(5);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_step_validation_success() {
        let fixture = create_test_step();
        let actual = fixture.validate();
        assert!(actual.is_ok());
    }

    #[test]
    fn test_step_validation_empty_name() {
        let mut fixture = create_test_step();
        fixture.name = String::new();

        let actual = fixture.validate();
        assert!(actual.is_err());
    }

    #[test]
    fn test_step_serialization() {
        let fixture = create_test_step();
        let actual = serde_json::to_string(&fixture);
        assert!(actual.is_ok());
    }

    #[test]
    fn test_step_deserialization() {
        let fixture = create_test_step();
        let serialized = serde_json::to_string(&fixture).unwrap();
        let actual: Step = serde_json::from_str(&serialized).unwrap();
        assert_eq!(actual.name, fixture.name);
        assert_eq!(actual.status, fixture.status);
    }
}
