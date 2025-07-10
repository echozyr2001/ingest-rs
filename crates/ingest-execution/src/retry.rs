use crate::{ExecutionError, Result};
use ingest_core::{DateTime, Id};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Retry strategy configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RetryStrategy {
    /// Fixed delay between retries
    Fixed { delay: Duration },
    /// Exponential backoff with optional jitter
    Exponential {
        initial_delay: Duration,
        multiplier: f64,
        max_delay: Duration,
        jitter: bool,
    },
    /// Linear backoff
    Linear {
        initial_delay: Duration,
        increment: Duration,
        max_delay: Duration,
    },
}

impl Default for RetryStrategy {
    fn default() -> Self {
        Self::Exponential {
            initial_delay: Duration::from_secs(1),
            multiplier: 2.0,
            max_delay: Duration::from_secs(300), // 5 minutes
            jitter: true,
        }
    }
}

/// Retry policy configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Retry strategy
    pub strategy: RetryStrategy,
    /// Maximum total retry duration
    pub max_duration: Option<Duration>,
    /// Custom retry conditions
    pub retry_conditions: Vec<RetryCondition>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            strategy: RetryStrategy::default(),
            max_duration: Some(Duration::from_secs(3600)), // 1 hour
            retry_conditions: vec![RetryCondition::default()],
        }
    }
}

/// Retry condition for determining if an error should be retried
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RetryCondition {
    /// Retry all retryable errors
    Always,
    /// Never retry
    Never,
    /// Retry specific error categories
    ErrorCategory(Vec<String>),
    /// Custom condition (not serializable, for runtime use)
    Custom,
}

impl Default for RetryCondition {
    fn default() -> Self {
        Self::Always
    }
}

/// Retry attempt information
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryAttempt {
    /// Attempt number (1-based)
    pub attempt: u32,
    /// Error that triggered this retry
    pub error: String,
    /// Error category
    pub error_category: String,
    /// Timestamp of the attempt
    pub attempted_at: DateTime,
    /// Delay before this attempt
    pub delay: Duration,
}

/// Retry state for tracking retry attempts
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryState {
    /// Execution or step identifier
    pub id: Id,
    /// Retry policy being used
    pub policy: RetryPolicy,
    /// List of retry attempts
    pub attempts: Vec<RetryAttempt>,
    /// First failure timestamp
    pub first_failure_at: DateTime,
    /// Next retry timestamp
    pub next_retry_at: Option<DateTime>,
    /// Whether retries are exhausted
    pub exhausted: bool,
}

impl RetryState {
    /// Create a new retry state
    pub fn new(id: Id, policy: RetryPolicy) -> Self {
        Self {
            id,
            policy,
            attempts: Vec::new(),
            first_failure_at: chrono::Utc::now(),
            next_retry_at: None,
            exhausted: false,
        }
    }

    /// Get the current attempt number
    pub fn current_attempt(&self) -> u32 {
        self.attempts.len() as u32
    }

    /// Check if more retries are available
    pub fn can_retry(&self) -> bool {
        !self.exhausted
            && self.current_attempt() < self.policy.max_attempts
            && !self.is_duration_exceeded()
    }

    /// Check if maximum retry duration has been exceeded
    pub fn is_duration_exceeded(&self) -> bool {
        if let Some(max_duration) = self.policy.max_duration {
            let elapsed = chrono::Utc::now() - self.first_failure_at;
            elapsed.to_std().unwrap_or_default() > max_duration
        } else {
            false
        }
    }

    /// Mark retries as exhausted
    pub fn exhaust(&mut self) {
        self.exhausted = true;
        self.next_retry_at = None;
    }
}

/// Retry manager for handling retry logic
pub struct RetryManager {
    /// Default retry policy
    default_policy: RetryPolicy,
}

impl RetryManager {
    /// Create a new retry manager
    pub fn new() -> Self {
        Self {
            default_policy: RetryPolicy::default(),
        }
    }

    /// Create a retry manager with custom default policy
    pub fn with_policy(policy: RetryPolicy) -> Self {
        Self {
            default_policy: policy,
        }
    }

    /// Calculate the next retry delay
    pub fn calculate_delay(&self, strategy: &RetryStrategy, attempt: u32) -> Duration {
        match strategy {
            RetryStrategy::Fixed { delay } => *delay,
            RetryStrategy::Exponential {
                initial_delay,
                multiplier,
                max_delay,
                jitter,
            } => {
                let base_delay = initial_delay.as_secs_f64() * multiplier.powi(attempt as i32 - 1);
                let delay = Duration::from_secs_f64(base_delay.min(max_delay.as_secs_f64()));

                if *jitter {
                    self.add_jitter(delay)
                } else {
                    delay
                }
            }
            RetryStrategy::Linear {
                initial_delay,
                increment,
                max_delay,
            } => {
                let delay_secs =
                    initial_delay.as_secs() + increment.as_secs() * (attempt as u64 - 1);
                Duration::from_secs(delay_secs.min(max_delay.as_secs()))
            }
        }
    }

    /// Add jitter to a delay to avoid thundering herd
    fn add_jitter(&self, delay: Duration) -> Duration {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        std::thread::current().id().hash(&mut hasher);
        chrono::Utc::now()
            .timestamp_nanos_opt()
            .unwrap_or(0)
            .hash(&mut hasher);

        let jitter_factor = (hasher.finish() % 1000) as f64 / 1000.0; // 0.0 to 0.999
        let jitter_range = delay.as_secs_f64() * 0.1; // 10% jitter
        let jitter = jitter_range * jitter_factor;

        Duration::from_secs_f64(delay.as_secs_f64() + jitter)
    }

    /// Check if an error should be retried
    pub fn should_retry(&self, error: &ExecutionError, conditions: &[RetryCondition]) -> bool {
        // First check if the error is inherently retryable
        if !error.is_retryable() {
            return false;
        }

        // Then check retry conditions
        for condition in conditions {
            match condition {
                RetryCondition::Always => return true,
                RetryCondition::Never => return false,
                RetryCondition::ErrorCategory(categories) => {
                    if categories.contains(&error.category().to_string()) {
                        return true;
                    }
                }
                RetryCondition::Custom => {
                    // Custom conditions would be handled by caller
                    continue;
                }
            }
        }

        // Default to retrying if no specific condition matched
        true
    }

    /// Plan the next retry attempt
    pub fn plan_retry(
        &self,
        state: &mut RetryState,
        error: &ExecutionError,
    ) -> Result<Option<DateTime>> {
        // Check if we can retry
        if !state.can_retry() {
            state.exhaust();
            return Ok(None);
        }

        // Check retry conditions
        if !self.should_retry(error, &state.policy.retry_conditions) {
            state.exhaust();
            return Ok(None);
        }

        // Calculate delay for next attempt
        let next_attempt = state.current_attempt() + 1;
        let delay = self.calculate_delay(&state.policy.strategy, next_attempt);

        // Schedule next retry
        let next_retry_at = chrono::Utc::now() + chrono::Duration::from_std(delay).unwrap();
        state.next_retry_at = Some(next_retry_at);

        // Record this attempt
        let attempt = RetryAttempt {
            attempt: next_attempt,
            error: error.to_string(),
            error_category: error.category().to_string(),
            attempted_at: chrono::Utc::now(),
            delay,
        };
        state.attempts.push(attempt);

        Ok(Some(next_retry_at))
    }

    /// Check if it's time to retry
    pub fn is_retry_ready(&self, state: &RetryState) -> bool {
        if let Some(next_retry_at) = state.next_retry_at {
            chrono::Utc::now() >= next_retry_at
        } else {
            false
        }
    }

    /// Create retry state with default policy
    pub fn create_state(&self, id: Id) -> RetryState {
        RetryState::new(id, self.default_policy.clone())
    }

    /// Create retry state with custom policy
    pub fn create_state_with_policy(&self, id: Id, policy: RetryPolicy) -> RetryState {
        RetryState::new(id, policy)
    }
}

impl Default for RetryManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingest_core::generate_id_with_prefix;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_retry_strategy_default() {
        let actual = RetryStrategy::default();
        match actual {
            RetryStrategy::Exponential {
                initial_delay,
                multiplier,
                max_delay,
                jitter,
            } => {
                assert_eq!(initial_delay, Duration::from_secs(1));
                assert_eq!(multiplier, 2.0);
                assert_eq!(max_delay, Duration::from_secs(300));
                assert!(jitter);
            }
            _ => panic!("Expected exponential strategy"),
        }
    }

    #[test]
    fn test_retry_policy_default() {
        let actual = RetryPolicy::default();
        assert_eq!(actual.max_attempts, 3);
        assert_eq!(actual.max_duration, Some(Duration::from_secs(3600)));
        assert!(!actual.retry_conditions.is_empty());
    }

    #[test]
    fn test_retry_state_creation() {
        let fixture_id = generate_id_with_prefix("test");
        let fixture_policy = RetryPolicy::default();

        let actual = RetryState::new(fixture_id.clone(), fixture_policy.clone());

        assert_eq!(actual.id, fixture_id);
        assert_eq!(actual.policy, fixture_policy);
        assert_eq!(actual.current_attempt(), 0);
        assert!(actual.can_retry());
        assert!(!actual.exhausted);
    }

    #[test]
    fn test_retry_state_can_retry() {
        let mut fixture = RetryState::new(generate_id_with_prefix("test"), RetryPolicy::default());

        assert!(fixture.can_retry());

        // Exhaust retries
        fixture.exhaust();
        assert!(!fixture.can_retry());
    }

    #[test]
    fn test_retry_manager_creation() {
        let actual = RetryManager::new();
        assert_eq!(actual.default_policy.max_attempts, 3);
    }

    #[test]
    fn test_retry_manager_with_policy() {
        let fixture_policy = RetryPolicy {
            max_attempts: 5,
            strategy: RetryStrategy::Fixed {
                delay: Duration::from_secs(2),
            },
            max_duration: None,
            retry_conditions: vec![RetryCondition::Never],
        };

        let actual = RetryManager::with_policy(fixture_policy.clone());
        assert_eq!(actual.default_policy, fixture_policy);
    }

    #[test]
    fn test_calculate_delay_fixed() {
        let fixture_manager = RetryManager::new();
        let fixture_strategy = RetryStrategy::Fixed {
            delay: Duration::from_secs(5),
        };

        let actual = fixture_manager.calculate_delay(&fixture_strategy, 1);
        let expected = Duration::from_secs(5);
        assert_eq!(actual, expected);

        let actual = fixture_manager.calculate_delay(&fixture_strategy, 3);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_calculate_delay_exponential() {
        let fixture_manager = RetryManager::new();
        let fixture_strategy = RetryStrategy::Exponential {
            initial_delay: Duration::from_secs(1),
            multiplier: 2.0,
            max_delay: Duration::from_secs(60),
            jitter: false,
        };

        let actual_1 = fixture_manager.calculate_delay(&fixture_strategy, 1);
        let actual_2 = fixture_manager.calculate_delay(&fixture_strategy, 2);
        let actual_3 = fixture_manager.calculate_delay(&fixture_strategy, 3);

        assert_eq!(actual_1, Duration::from_secs(1));
        assert_eq!(actual_2, Duration::from_secs(2));
        assert_eq!(actual_3, Duration::from_secs(4));
    }

    #[test]
    fn test_calculate_delay_linear() {
        let fixture_manager = RetryManager::new();
        let fixture_strategy = RetryStrategy::Linear {
            initial_delay: Duration::from_secs(1),
            increment: Duration::from_secs(2),
            max_delay: Duration::from_secs(10),
        };

        let actual_1 = fixture_manager.calculate_delay(&fixture_strategy, 1);
        let actual_2 = fixture_manager.calculate_delay(&fixture_strategy, 2);
        let actual_3 = fixture_manager.calculate_delay(&fixture_strategy, 3);

        assert_eq!(actual_1, Duration::from_secs(1));
        assert_eq!(actual_2, Duration::from_secs(3));
        assert_eq!(actual_3, Duration::from_secs(5));
    }

    #[test]
    fn test_should_retry() {
        let fixture_manager = RetryManager::new();
        let fixture_retryable_error = ExecutionError::function_execution("temp failure");
        let fixture_non_retryable_error =
            ExecutionError::function_not_found(generate_id_with_prefix("fn"));

        let fixture_always_conditions = vec![RetryCondition::Always];
        let fixture_never_conditions = vec![RetryCondition::Never];

        assert!(fixture_manager.should_retry(&fixture_retryable_error, &fixture_always_conditions));
        assert!(!fixture_manager.should_retry(&fixture_retryable_error, &fixture_never_conditions));
        assert!(
            !fixture_manager.should_retry(&fixture_non_retryable_error, &fixture_always_conditions)
        );
    }

    #[test]
    fn test_plan_retry() {
        let fixture_manager = RetryManager::new();
        let mut fixture_state = fixture_manager.create_state(generate_id_with_prefix("test"));
        let fixture_error = ExecutionError::function_execution("temp failure");

        let actual = fixture_manager.plan_retry(&mut fixture_state, &fixture_error);

        assert!(actual.is_ok());
        assert!(actual.unwrap().is_some());
        assert_eq!(fixture_state.current_attempt(), 1);
        assert!(fixture_state.next_retry_at.is_some());
    }

    #[test]
    fn test_plan_retry_exhausted() {
        let fixture_manager = RetryManager::new();
        let mut fixture_state = fixture_manager.create_state(generate_id_with_prefix("test"));
        fixture_state.exhaust();
        let fixture_error = ExecutionError::function_execution("temp failure");

        let actual = fixture_manager.plan_retry(&mut fixture_state, &fixture_error);

        assert!(actual.is_ok());
        assert!(actual.unwrap().is_none());
    }

    #[test]
    fn test_is_retry_ready() {
        let fixture_manager = RetryManager::new();
        let mut fixture_state = fixture_manager.create_state(generate_id_with_prefix("test"));

        // No retry scheduled
        assert!(!fixture_manager.is_retry_ready(&fixture_state));

        // Schedule retry in the past
        fixture_state.next_retry_at = Some(chrono::Utc::now() - chrono::Duration::seconds(1));
        assert!(fixture_manager.is_retry_ready(&fixture_state));

        // Schedule retry in the future
        fixture_state.next_retry_at = Some(chrono::Utc::now() + chrono::Duration::seconds(60));
        assert!(!fixture_manager.is_retry_ready(&fixture_state));
    }

    #[test]
    fn test_retry_state_serialization() {
        let fixture = RetryState::new(generate_id_with_prefix("test"), RetryPolicy::default());

        let actual = serde_json::to_string(&fixture);
        assert!(actual.is_ok());
    }

    #[test]
    fn test_retry_policy_serialization() {
        let fixture = RetryPolicy::default();

        let actual = serde_json::to_string(&fixture);
        assert!(actual.is_ok());
    }
}
