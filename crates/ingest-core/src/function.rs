use crate::{DateTime, Id, Json, Result};
use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Function identifier type
pub type FunctionId = Id;

/// Function trigger configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionTrigger {
    /// Event pattern to match
    pub event: String,
    /// Optional expression for filtering
    pub expression: Option<String>,
    /// Trigger configuration
    pub config: HashMap<String, Json>,
}

impl FunctionTrigger {
    /// Create a new trigger for an event
    pub fn event(event: impl Into<String>) -> Self {
        Self {
            event: event.into(),
            expression: None,
            config: HashMap::new(),
        }
    }

    /// Add an expression filter
    pub fn with_expression(mut self, expression: impl Into<String>) -> Self {
        self.expression = Some(expression.into());
        self
    }

    /// Add configuration
    pub fn with_config(mut self, key: impl Into<String>, value: Json) -> Self {
        self.config.insert(key.into(), value);
        self
    }
}

/// Function configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct FunctionConfig {
    /// Maximum number of retries
    pub max_retries: Option<u32>,
    /// Retry delay configuration
    pub retry_delay: Option<std::time::Duration>,
    /// Function timeout
    pub timeout: Option<std::time::Duration>,
    /// Concurrency limit
    pub concurrency: Option<u32>,
    /// Rate limiting configuration
    pub rate_limit: Option<HashMap<String, Json>>,
    /// Priority level (higher = more priority)
    pub priority: Option<i32>,
    /// Batch configuration
    pub batch: Option<HashMap<String, Json>>,
}

impl Default for FunctionConfig {
    fn default() -> Self {
        Self {
            max_retries: Some(3),
            retry_delay: Some(std::time::Duration::from_secs(1)),
            timeout: Some(std::time::Duration::from_secs(300)), // 5 minutes
            concurrency: Some(10),
            rate_limit: None,
            priority: Some(0),
            batch: None,
        }
    }
}

/// Function definition
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Setters)]
#[setters(strip_option, into)]
pub struct Function {
    /// Unique function identifier
    pub id: FunctionId,
    /// Function name
    pub name: String,
    /// Function description
    pub description: Option<String>,
    /// Function triggers
    pub triggers: Vec<FunctionTrigger>,
    /// Function configuration
    pub config: FunctionConfig,
    /// Function metadata
    pub metadata: HashMap<String, String>,
    /// Function version
    pub version: String,
    /// Creation timestamp
    pub created_at: DateTime,
    /// Last update timestamp
    pub updated_at: DateTime,
    /// Whether function is enabled
    pub enabled: bool,
}

impl Function {
    /// Create a new function
    pub fn new(name: impl Into<String>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: crate::generate_id_with_prefix("fn"),
            name: name.into(),
            description: None,
            triggers: Vec::new(),
            config: FunctionConfig::default(),
            metadata: HashMap::new(),
            version: "1.0.0".to_string(),
            created_at: now,
            updated_at: now,
            enabled: true,
        }
    }

    /// Add a trigger to the function
    pub fn add_trigger(mut self, trigger: FunctionTrigger) -> Self {
        self.triggers.push(trigger);
        self
    }

    /// Add a trigger for an event
    pub fn trigger_on(self, event: impl Into<String>) -> Self {
        self.add_trigger(FunctionTrigger::event(event))
    }

    /// Check if function matches an event
    pub fn matches_event(&self, event_name: &str) -> bool {
        self.triggers
            .iter()
            .any(|trigger| self.matches_pattern(&trigger.event, event_name))
    }

    /// Check if a pattern matches an event name
    fn matches_pattern(&self, pattern: &str, event_name: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if let Some(prefix) = pattern.strip_suffix('*') {
            return event_name.starts_with(prefix);
        }

        pattern == event_name
    }

    /// Validate the function
    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(crate::Error::function("Function name cannot be empty"));
        }

        if self.name.len() > 255 {
            return Err(crate::Error::function(
                "Function name too long (max 255 characters)",
            ));
        }

        if self.triggers.is_empty() {
            return Err(crate::Error::function(
                "Function must have at least one trigger",
            ));
        }

        for trigger in &self.triggers {
            if trigger.event.is_empty() {
                return Err(crate::Error::function("Trigger event cannot be empty"));
            }
        }

        Ok(())
    }

    /// Update the function's updated_at timestamp
    pub fn touch(&mut self) {
        self.updated_at = chrono::Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn create_test_trigger() -> FunctionTrigger {
        FunctionTrigger::event("user.created").with_expression("event.data.email != null")
    }

    #[test]
    fn test_function_trigger_creation() {
        let fixture = "user.created";
        let actual = FunctionTrigger::event(fixture);
        let expected = FunctionTrigger {
            event: "user.created".to_string(),
            expression: None,
            config: HashMap::new(),
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_function_trigger_with_expression() {
        let fixture_event = "user.created";
        let fixture_expression = "event.data.email != null";

        let actual = FunctionTrigger::event(fixture_event).with_expression(fixture_expression);

        assert_eq!(actual.event, "user.created");
        assert_eq!(
            actual.expression,
            Some("event.data.email != null".to_string())
        );
    }

    #[test]
    fn test_function_trigger_with_config() {
        let fixture = FunctionTrigger::event("user.created").with_config("delay", json!("5s"));

        let actual = fixture.config.get("delay").unwrap();
        let expected = &json!("5s");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_function_config_default() {
        let actual = FunctionConfig::default();
        assert_eq!(actual.max_retries, Some(3));
        assert_eq!(actual.timeout, Some(std::time::Duration::from_secs(300)));
        assert_eq!(actual.priority, Some(0));
    }

    #[test]
    fn test_function_creation() {
        let fixture = "send-welcome-email";
        let actual = Function::new(fixture);

        assert_eq!(actual.name, "send-welcome-email");
        assert!(actual.id.as_str().starts_with("fn_"));
        assert_eq!(actual.version, "1.0.0");
        assert!(actual.enabled);
        assert!(actual.triggers.is_empty());
    }

    #[test]
    fn test_function_add_trigger() {
        let fixture_function = Function::new("test-function");
        let fixture_trigger = create_test_trigger();

        let actual = fixture_function.add_trigger(fixture_trigger.clone());
        assert_eq!(actual.triggers.len(), 1);
        assert_eq!(actual.triggers[0], fixture_trigger);
    }

    #[test]
    fn test_function_trigger_on() {
        let fixture = Function::new("test-function").trigger_on("user.created");

        let actual = fixture.triggers.len();
        let expected = 1;
        assert_eq!(actual, expected);
        assert_eq!(fixture.triggers[0].event, "user.created");
    }

    #[test]
    fn test_function_matches_event() {
        let fixture = Function::new("test-function")
            .trigger_on("user.*")
            .trigger_on("order.created");

        assert!(fixture.matches_event("user.created"));
        assert!(fixture.matches_event("user.updated"));
        assert!(fixture.matches_event("order.created"));
        assert!(!fixture.matches_event("product.created"));
    }

    #[test]
    fn test_function_setters() {
        let fixture = Function::new("test-function")
            .description("A test function")
            .version("2.0.0")
            .enabled(false);

        assert_eq!(fixture.description, Some("A test function".to_string()));
        assert_eq!(fixture.version, "2.0.0");
        assert!(!fixture.enabled);
    }

    #[test]
    fn test_function_validation_success() {
        let fixture = Function::new("valid-function").trigger_on("user.created");

        let actual = fixture.validate();
        assert!(actual.is_ok());
    }

    #[test]
    fn test_function_validation_empty_name() {
        let fixture = Function::new("");
        let actual = fixture.validate();
        assert!(actual.is_err());
    }

    #[test]
    fn test_function_validation_no_triggers() {
        let fixture = Function::new("test-function");
        let actual = fixture.validate();
        assert!(actual.is_err());
    }

    #[test]
    fn test_function_touch() {
        let mut fixture = Function::new("test-function");
        let original_time = fixture.updated_at;

        // Wait a bit to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(1));
        fixture.touch();

        assert!(fixture.updated_at > original_time);
    }

    #[test]
    fn test_function_serialization() {
        let fixture = Function::new("test-function").trigger_on("user.created");

        let actual = serde_json::to_string(&fixture);
        assert!(actual.is_ok());
    }

    #[test]
    fn test_function_deserialization() {
        let fixture = Function::new("test-function").trigger_on("user.created");

        let serialized = serde_json::to_string(&fixture).unwrap();
        let actual: Function = serde_json::from_str(&serialized).unwrap();
        assert_eq!(actual.name, fixture.name);
        assert_eq!(actual.triggers, fixture.triggers);
    }
}
