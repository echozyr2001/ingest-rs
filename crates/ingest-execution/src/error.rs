use ingest_core::Id;
use thiserror::Error;

/// Result type for execution operations
pub type Result<T> = std::result::Result<T, ExecutionError>;

/// Execution engine error types
#[derive(Error, Debug)]
pub enum ExecutionError {
    /// Function execution failed
    #[error("Function execution failed: {message}")]
    FunctionExecution { message: String },

    /// Step execution failed
    #[error("Step execution failed for step {step_id}: {message}")]
    StepExecution { step_id: Id, message: String },

    /// Retry limit exceeded
    #[error("Retry limit exceeded for execution {run_id}: {attempts} attempts")]
    RetryLimitExceeded { run_id: Id, attempts: u32 },

    /// Concurrency limit exceeded
    #[error("Concurrency limit exceeded: {current}/{limit}")]
    ConcurrencyLimitExceeded { current: u32, limit: u32 },

    /// Invalid execution state
    #[error("Invalid execution state for {run_id}: expected {expected}, got {actual}")]
    InvalidState {
        run_id: Id,
        expected: String,
        actual: String,
    },

    /// Execution timeout
    #[error("Execution timeout for {run_id} after {duration:?}")]
    Timeout {
        run_id: Id,
        duration: std::time::Duration,
    },

    /// Step not found
    #[error("Step not found: {step_id}")]
    StepNotFound { step_id: Id },

    /// Function not found
    #[error("Function not found: {function_id}")]
    FunctionNotFound { function_id: Id },

    /// State management error
    #[error("State error: {0}")]
    State(#[from] ingest_state::StateError),

    /// Queue operation error
    #[error("Queue error: {0}")]
    Queue(String),

    /// Event processing error
    #[error("Event error: {0}")]
    Event(String),

    /// Expression evaluation error
    #[error("Expression error: {0}")]
    Expression(String),

    /// Configuration error
    #[error("Configuration error: {message}")]
    Configuration { message: String },

    /// Internal error
    #[error("Internal error: {message}")]
    Internal { message: String },
}

impl ExecutionError {
    /// Create a function execution error
    pub fn function_execution(message: impl Into<String>) -> Self {
        Self::FunctionExecution {
            message: message.into(),
        }
    }

    /// Create a step execution error
    pub fn step_execution(step_id: Id, message: impl Into<String>) -> Self {
        Self::StepExecution {
            step_id,
            message: message.into(),
        }
    }

    /// Create a retry limit exceeded error
    pub fn retry_limit_exceeded(run_id: Id, attempts: u32) -> Self {
        Self::RetryLimitExceeded { run_id, attempts }
    }

    /// Create a concurrency limit exceeded error
    pub fn concurrency_limit_exceeded(current: u32, limit: u32) -> Self {
        Self::ConcurrencyLimitExceeded { current, limit }
    }

    /// Create an invalid state error
    pub fn invalid_state(
        run_id: Id,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        Self::InvalidState {
            run_id,
            expected: expected.into(),
            actual: actual.into(),
        }
    }

    /// Create a timeout error
    pub fn timeout(run_id: Id, duration: std::time::Duration) -> Self {
        Self::Timeout { run_id, duration }
    }

    /// Create a step not found error
    pub fn step_not_found(step_id: Id) -> Self {
        Self::StepNotFound { step_id }
    }

    /// Create a function not found error
    pub fn function_not_found(function_id: Id) -> Self {
        Self::FunctionNotFound { function_id }
    }

    /// Create a configuration error
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration {
            message: message.into(),
        }
    }

    /// Create an internal error
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            // Non-retryable errors
            ExecutionError::FunctionNotFound { .. }
            | ExecutionError::StepNotFound { .. }
            | ExecutionError::InvalidState { .. }
            | ExecutionError::Configuration { .. }
            | ExecutionError::RetryLimitExceeded { .. } => false,

            // Retryable errors
            ExecutionError::FunctionExecution { .. }
            | ExecutionError::StepExecution { .. }
            | ExecutionError::ConcurrencyLimitExceeded { .. }
            | ExecutionError::Timeout { .. }
            | ExecutionError::State(_)
            | ExecutionError::Queue(_)
            | ExecutionError::Event(_)
            | ExecutionError::Expression(_)
            | ExecutionError::Internal { .. } => true,
        }
    }

    /// Get error category for metrics and monitoring
    pub fn category(&self) -> &'static str {
        match self {
            ExecutionError::FunctionExecution { .. } => "function_execution",
            ExecutionError::StepExecution { .. } => "step_execution",
            ExecutionError::RetryLimitExceeded { .. } => "retry_limit",
            ExecutionError::ConcurrencyLimitExceeded { .. } => "concurrency_limit",
            ExecutionError::InvalidState { .. } => "invalid_state",
            ExecutionError::Timeout { .. } => "timeout",
            ExecutionError::StepNotFound { .. } => "step_not_found",
            ExecutionError::FunctionNotFound { .. } => "function_not_found",
            ExecutionError::State(_) => "state",
            ExecutionError::Queue(_) => "queue",
            ExecutionError::Event(_) => "event",
            ExecutionError::Expression(_) => "expression",
            ExecutionError::Configuration { .. } => "configuration",
            ExecutionError::Internal { .. } => "internal",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingest_core::generate_id_with_prefix;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_function_execution_error() {
        let fixture = "Function failed to execute";
        let actual = ExecutionError::function_execution(fixture);

        match actual {
            ExecutionError::FunctionExecution { message } => {
                assert_eq!(message, "Function failed to execute");
            }
            _ => panic!("Expected FunctionExecution error"),
        }
    }

    #[test]
    fn test_step_execution_error() {
        let fixture_step_id = generate_id_with_prefix("step");
        let fixture_message = "Step failed";
        let actual = ExecutionError::step_execution(fixture_step_id.clone(), fixture_message);

        match actual {
            ExecutionError::StepExecution { step_id, message } => {
                assert_eq!(step_id, fixture_step_id);
                assert_eq!(message, "Step failed");
            }
            _ => panic!("Expected StepExecution error"),
        }
    }

    #[test]
    fn test_retry_limit_exceeded_error() {
        let fixture_run_id = generate_id_with_prefix("run");
        let fixture_attempts = 5;
        let actual = ExecutionError::retry_limit_exceeded(fixture_run_id.clone(), fixture_attempts);

        match actual {
            ExecutionError::RetryLimitExceeded { run_id, attempts } => {
                assert_eq!(run_id, fixture_run_id);
                assert_eq!(attempts, 5);
            }
            _ => panic!("Expected RetryLimitExceeded error"),
        }
    }

    #[test]
    fn test_error_is_retryable() {
        let fixture_retryable = ExecutionError::function_execution("temp failure");
        let fixture_non_retryable =
            ExecutionError::function_not_found(generate_id_with_prefix("fn"));

        assert!(fixture_retryable.is_retryable());
        assert!(!fixture_non_retryable.is_retryable());
    }

    #[test]
    fn test_error_category() {
        let fixture = ExecutionError::function_execution("test");
        let actual = fixture.category();
        let expected = "function_execution";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_timeout_error() {
        let fixture_run_id = generate_id_with_prefix("run");
        let fixture_duration = std::time::Duration::from_secs(30);
        let actual = ExecutionError::timeout(fixture_run_id.clone(), fixture_duration);

        match actual {
            ExecutionError::Timeout { run_id, duration } => {
                assert_eq!(run_id, fixture_run_id);
                assert_eq!(duration, fixture_duration);
            }
            _ => panic!("Expected Timeout error"),
        }
    }

    #[test]
    fn test_invalid_state_error() {
        let fixture_run_id = generate_id_with_prefix("run");
        let fixture_expected = "running";
        let fixture_actual = "completed";
        let actual =
            ExecutionError::invalid_state(fixture_run_id.clone(), fixture_expected, fixture_actual);

        match actual {
            ExecutionError::InvalidState {
                run_id,
                expected,
                actual: actual_state,
            } => {
                assert_eq!(run_id, fixture_run_id);
                assert_eq!(expected, "running");
                assert_eq!(actual_state, "completed");
            }
            _ => panic!("Expected InvalidState error"),
        }
    }

    #[test]
    fn test_concurrency_limit_error() {
        let fixture_current = 10;
        let fixture_limit = 5;
        let actual = ExecutionError::concurrency_limit_exceeded(fixture_current, fixture_limit);

        match actual {
            ExecutionError::ConcurrencyLimitExceeded { current, limit } => {
                assert_eq!(current, 10);
                assert_eq!(limit, 5);
            }
            _ => panic!("Expected ConcurrencyLimitExceeded error"),
        }
    }
}
