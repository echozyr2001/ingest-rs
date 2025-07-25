//! Function definitions and configurations for Inngest

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
// use std::collections::HashMap; // Will be used later
use uuid::Uuid;

/// Function trigger types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TriggerType {
    Event,
    Cron,
}

/// Function trigger configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trigger {
    /// Type of trigger (event or cron)
    #[serde(rename = "type")]
    pub trigger_type: TriggerType,

    /// Event name for event triggers, cron expression for cron triggers
    pub value: String,

    /// Optional condition expression
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
}

impl Trigger {
    /// Create a new event trigger
    pub fn event(event_name: impl Into<String>) -> Self {
        Self {
            trigger_type: TriggerType::Event,
            value: event_name.into(),
            condition: None,
        }
    }

    /// Create a new event trigger with condition
    pub fn event_with_condition(
        event_name: impl Into<String>,
        condition: impl Into<String>,
    ) -> Self {
        Self {
            trigger_type: TriggerType::Event,
            value: event_name.into(),
            condition: Some(condition.into()),
        }
    }

    /// Create a new cron trigger
    pub fn cron(expression: impl Into<String>) -> Self {
        Self {
            trigger_type: TriggerType::Cron,
            value: expression.into(),
            condition: None,
        }
    }

    /// Check if this is an event trigger
    pub fn is_event(&self) -> bool {
        self.trigger_type == TriggerType::Event
    }

    /// Check if this is a cron trigger
    pub fn is_cron(&self) -> bool {
        self.trigger_type == TriggerType::Cron
    }
}

/// Retry configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retries
    pub attempts: u32,

    /// Whether this is the default retry configuration
    #[serde(default)]
    pub is_default: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            attempts: 3,
            is_default: true,
        }
    }
}

/// Concurrency scope
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConcurrencyScope {
    Account,
    Environment,
    Function,
}

/// Concurrency configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConcurrencyConfig {
    /// Scope of the concurrency limit
    pub scope: ConcurrencyScope,

    /// Maximum number of concurrent executions
    pub limit: u32,

    /// Optional key expression for custom concurrency grouping
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

/// Batch configuration for event batching
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchConfig {
    /// Maximum number of events in a batch
    pub max_size: u32,

    /// Maximum time to wait before processing a partial batch (in milliseconds)
    pub timeout_ms: u64,

    /// Optional key expression for batch grouping
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

/// Rate limiting configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Number of executions allowed per period
    pub limit: u32,

    /// Time period in milliseconds
    pub period_ms: u64,

    /// Optional key expression for rate limit grouping
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

/// Function configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FunctionConfig {
    /// Retry configuration
    #[serde(default)]
    pub retries: RetryConfig,

    /// Concurrency limits
    #[serde(default)]
    pub concurrency: Vec<ConcurrencyConfig>,

    /// Batch configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch: Option<BatchConfig>,

    /// Rate limiting configuration
    #[serde(rename = "rateLimit", skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimitConfig>,

    /// Priority (higher numbers = higher priority)
    #[serde(default)]
    pub priority: i32,

    /// Function timeout in milliseconds
    #[serde(rename = "timeoutMs", skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

/// Function definition
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Function {
    /// Unique function ID
    pub id: Uuid,

    /// Function name
    pub name: String,

    /// Function slug (URL-safe identifier)
    pub slug: String,

    /// App ID this function belongs to
    pub app_id: Uuid,

    /// Function triggers
    pub triggers: Vec<Trigger>,

    /// Function configuration
    #[serde(default)]
    pub config: FunctionConfig,

    /// Function version
    #[serde(default = "default_version")]
    pub version: i32,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Optional archive timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<DateTime<Utc>>,

    /// Raw configuration JSON
    #[serde(default)]
    pub raw_config: String,
}

fn default_version() -> i32 {
    1
}

impl Function {
    /// Create a new function
    pub fn new(
        id: Uuid,
        name: impl Into<String>,
        slug: impl Into<String>,
        app_id: Uuid,
        triggers: Vec<Trigger>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            slug: slug.into(),
            app_id,
            triggers,
            config: FunctionConfig::default(),
            version: 1,
            created_at: Utc::now(),
            archived_at: None,
            raw_config: String::new(),
        }
    }

    /// Set the function configuration
    pub fn with_config(mut self, config: FunctionConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the raw configuration JSON
    pub fn with_raw_config(mut self, raw_config: impl Into<String>) -> Self {
        self.raw_config = raw_config.into();
        self
    }

    /// Check if the function is archived
    pub fn is_archived(&self) -> bool {
        self.archived_at.is_some()
    }

    /// Archive the function
    pub fn archive(&mut self) {
        self.archived_at = Some(Utc::now());
    }

    /// Get event triggers only
    pub fn event_triggers(&self) -> Vec<&Trigger> {
        self.triggers.iter().filter(|t| t.is_event()).collect()
    }

    /// Get cron triggers only
    pub fn cron_triggers(&self) -> Vec<&Trigger> {
        self.triggers.iter().filter(|t| t.is_cron()).collect()
    }

    /// Check if function matches an event name
    pub fn matches_event(&self, event_name: &str) -> bool {
        self.event_triggers().iter().any(|trigger| {
            // Support exact match
            if trigger.value == event_name {
                return true;
            }

            // Support wildcard matching
            self.matches_wildcard(&trigger.value, event_name)
        })
    }

    /// Check if a trigger pattern matches an event name with wildcard support
    fn matches_wildcard(&self, pattern: &str, event_name: &str) -> bool {
        // Support path-style wildcards (users/created matches users/*)
        if let Some(prefix) = pattern.strip_suffix("/*") {
            if event_name.starts_with(prefix) && event_name.chars().nth(prefix.len()) == Some('/') {
                return true;
            }
        }

        // Support dot-style wildcards (payment.processed matches payment.*)
        if let Some(prefix) = pattern.strip_suffix(".*") {
            if event_name.starts_with(prefix) && event_name.chars().nth(prefix.len()) == Some('.') {
                return true;
            }
        }

        false
    }
}

impl std::fmt::Display for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Function[{}: {}]", self.slug, self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trigger_creation() {
        let event_trigger = Trigger::event("user.created");
        assert_eq!(event_trigger.trigger_type, TriggerType::Event);
        assert_eq!(event_trigger.value, "user.created");
        assert!(event_trigger.is_event());
        assert!(!event_trigger.is_cron());

        let cron_trigger = Trigger::cron("0 0 * * *");
        assert_eq!(cron_trigger.trigger_type, TriggerType::Cron);
        assert_eq!(cron_trigger.value, "0 0 * * *");
        assert!(cron_trigger.is_cron());
        assert!(!cron_trigger.is_event());
    }

    #[test]
    fn test_function_creation() {
        let function_id = Uuid::new_v4();
        let app_id = Uuid::new_v4();
        let triggers = vec![Trigger::event("user.created")];

        let function = Function::new(
            function_id,
            "Process User",
            "process-user",
            app_id,
            triggers.clone(),
        );

        assert_eq!(function.id, function_id);
        assert_eq!(function.name, "Process User");
        assert_eq!(function.slug, "process-user");
        assert_eq!(function.app_id, app_id);
        assert_eq!(function.triggers, triggers);
        assert!(!function.is_archived());
    }

    #[test]
    fn test_event_matching() {
        let function = Function::new(
            Uuid::new_v4(),
            "Test",
            "test",
            Uuid::new_v4(),
            vec![
                Trigger::event("user.created"),
                Trigger::event("users/*"),
                Trigger::event("payment.*"),
            ],
        );

        // Exact matches
        assert!(function.matches_event("user.created"));

        // Wildcard matches
        assert!(function.matches_event("users/updated"));
        assert!(function.matches_event("users/deleted"));
        assert!(function.matches_event("payment.processed"));
        assert!(function.matches_event("payment.failed"));

        // Non-matches
        assert!(!function.matches_event("other.event"));
        assert!(!function.matches_event("user.updated"));
        assert!(!function.matches_event("users"));
        assert!(!function.matches_event("payment"));
    }

    #[test]
    fn test_function_serialization() {
        let function = Function::new(
            Uuid::new_v4(),
            "Test Function",
            "test-function",
            Uuid::new_v4(),
            vec![Trigger::event("test.event")],
        );

        let json = serde_json::to_string(&function).unwrap();
        let deserialized: Function = serde_json::from_str(&json).unwrap();

        assert_eq!(function.id, deserialized.id);
        assert_eq!(function.name, deserialized.name);
        assert_eq!(function.triggers, deserialized.triggers);
    }
}
