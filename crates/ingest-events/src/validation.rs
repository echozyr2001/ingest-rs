//! Event validation framework

use crate::{Result, error::EventError};
use ingest_core::Event;
use regex::Regex;
use serde_json::{Value, json};
use std::collections::HashMap;

/// Event validation rules
#[derive(Debug)]
pub struct ValidationRules {
    /// Schema for event validation
    pub schema: Option<String>, // Store as string instead of JSONSchema
    /// Required fields
    pub required_fields: Vec<String>,
    /// Field patterns (regex)
    pub field_patterns: HashMap<String, Regex>,
}

impl ValidationRules {
    /// Create new validation rules
    pub fn new() -> Self {
        Self {
            schema: None,
            required_fields: Vec::new(),
            field_patterns: HashMap::new(),
        }
    }

    /// Set JSON schema for validation
    pub fn with_schema(mut self, schema: Value) -> Result<Self> {
        self.schema = Some(serde_json::to_string(&schema)?);
        Ok(self)
    }

    /// Add required field
    pub fn require_field(mut self, field: impl Into<String>) -> Self {
        self.required_fields.push(field.into());
        self
    }

    /// Add field pattern validation
    pub fn with_pattern(mut self, field: impl Into<String>, pattern: &str) -> Result<Self> {
        let regex = Regex::new(pattern)
            .map_err(|e| EventError::validation(format!("Invalid regex pattern: {e}")))?;
        self.field_patterns.insert(field.into(), regex);
        Ok(self)
    }
}

impl Default for ValidationRules {
    fn default() -> Self {
        Self::new()
    }
}

/// Event validator
#[derive(Debug)]
pub struct EventValidator {
    /// Validation rules by event name pattern
    rules: HashMap<String, ValidationRules>,
    /// Default validation rules
    default_rules: ValidationRules,
}

impl EventValidator {
    /// Create new event validator
    pub fn new() -> Self {
        Self {
            rules: HashMap::new(),
            default_rules: ValidationRules::default(),
        }
    }

    /// Add validation rules for event pattern
    pub fn add_rules(mut self, pattern: impl Into<String>, rules: ValidationRules) -> Self {
        self.rules.insert(pattern.into(), rules);
        self
    }

    /// Set default validation rules
    pub fn with_default_rules(mut self, rules: ValidationRules) -> Self {
        self.default_rules = rules;
        self
    }

    /// Validate an event
    pub fn validate(&self, event: &Event) -> Result<()> {
        // First, validate the event itself
        event
            .validate()
            .map_err(|e| EventError::Core { source: e })?;

        // Find matching validation rules
        let rules = self.find_rules(event);

        // Validate against schema if present
        if let Some(ref schema_str) = rules.schema {
            self.validate_schema(event, schema_str)?;
        }

        // Validate required fields
        self.validate_required_fields(event, &rules.required_fields)?;

        // Validate field patterns
        self.validate_field_patterns(event, &rules.field_patterns)?;

        Ok(())
    }

    /// Find validation rules for an event
    fn find_rules(&self, event: &Event) -> &ValidationRules {
        for (pattern, rules) in &self.rules {
            if event.matches_pattern(pattern) {
                return rules;
            }
        }
        &self.default_rules
    }

    /// Validate event against JSON schema
    fn validate_schema(&self, event: &Event, schema_str: &str) -> Result<()> {
        let schema_value: Value = serde_json::from_str(schema_str)?;
        let validator = jsonschema::validator_for(&schema_value)
            .map_err(|e| EventError::validation(format!("Invalid schema: {e}")))?;

        let event_json = serde_json::to_value(event)?;

        // Check if validation passes
        if !validator.is_valid(&event_json) {
            let error_messages: Vec<String> = validator
                .iter_errors(&event_json)
                .map(|e| format!("{}: {}", e.instance_path, e))
                .collect();
            return Err(EventError::schema(error_messages));
        }

        Ok(())
    }

    /// Validate required fields
    fn validate_required_fields(&self, event: &Event, required: &[String]) -> Result<()> {
        for field in required {
            match field.as_str() {
                "name" => {
                    if event.name().is_empty() {
                        return Err(EventError::validation("Event name is required"));
                    }
                }
                "data" => {
                    if event.data().data.is_null() {
                        return Err(EventError::validation("Event data is required"));
                    }
                }
                "source" => {
                    if event.source.is_none() {
                        return Err(EventError::validation("Event source is required"));
                    }
                }
                "user" => {
                    if event.user.is_none() {
                        return Err(EventError::validation("Event user is required"));
                    }
                }
                _ => {
                    // Check in event data
                    if !event
                        .data()
                        .data
                        .as_object()
                        .map(|obj| obj.contains_key(field))
                        .unwrap_or(false)
                    {
                        return Err(EventError::validation(format!(
                            "Required field '{field}' is missing"
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    /// Validate field patterns
    fn validate_field_patterns(
        &self,
        event: &Event,
        patterns: &HashMap<String, Regex>,
    ) -> Result<()> {
        for (field, pattern) in patterns {
            let value = match field.as_str() {
                "name" => Some(event.name().to_string()),
                "source" => event.source.clone(),
                "user" => event.user.clone(),
                _ => {
                    // Check in event data
                    event
                        .data()
                        .data
                        .as_object()
                        .and_then(|obj| obj.get(field))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                }
            };

            if let Some(val) = value {
                if !pattern.is_match(&val) {
                    return Err(EventError::validation(format!(
                        "Field '{}' value '{}' does not match pattern '{}'",
                        field,
                        val,
                        pattern.as_str()
                    )));
                }
            }
        }
        Ok(())
    }
}

impl Default for EventValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a basic event validator with common rules
pub fn create_basic_validator() -> EventValidator {
    let basic_rules = ValidationRules::new()
        .require_field("name")
        .require_field("data");

    EventValidator::new().with_default_rules(basic_rules)
}

/// Create a strict event validator with comprehensive rules
pub fn create_strict_validator() -> Result<EventValidator> {
    let schema = json!({
        "type": "object",
        "properties": {
            "id": {"type": "string"},
            "name": {"type": "string", "minLength": 1, "maxLength": 255},
            "data": {"type": "object"},
            "timestamp": {"type": "string", "format": "date-time"},
            "source": {"type": "string"},
            "version": {"type": "string"},
            "user": {"type": "string"},
            "trace_id": {"type": "string"}
        },
        "required": ["id", "name", "data", "timestamp"]
    });

    let strict_rules = ValidationRules::new()
        .with_schema(schema)?
        .require_field("name")
        .require_field("data")
        .with_pattern("name", r"^[a-zA-Z][a-zA-Z0-9._-]*$")?;

    Ok(EventValidator::new().with_default_rules(strict_rules))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingest_core::Event;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn create_test_event() -> Event {
        Event::new(
            "user.login",
            json!({"user_id": "123", "email": "test@example.com"}),
        )
    }

    #[test]
    fn test_validation_rules_creation() {
        let fixture = ValidationRules::new()
            .require_field("name")
            .require_field("data");

        assert_eq!(fixture.required_fields.len(), 2);
        assert!(fixture.required_fields.contains(&"name".to_string()));
        assert!(fixture.required_fields.contains(&"data".to_string()));
    }

    #[test]
    fn test_validation_rules_with_pattern() {
        let fixture = ValidationRules::new().with_pattern("name", r"^user\.");

        assert!(fixture.is_ok());
        let rules = fixture.unwrap();
        assert!(rules.field_patterns.contains_key("name"));
    }

    #[test]
    fn test_validation_rules_invalid_pattern() {
        let fixture = ValidationRules::new().with_pattern("name", "[invalid");

        assert!(fixture.is_err());
    }

    #[test]
    fn test_event_validator_creation() {
        let fixture = EventValidator::new();
        assert_eq!(fixture.rules.len(), 0);
    }

    #[test]
    fn test_event_validator_add_rules() {
        let rules = ValidationRules::new().require_field("user_id");
        let fixture = EventValidator::new().add_rules("user.*", rules);

        assert_eq!(fixture.rules.len(), 1);
        assert!(fixture.rules.contains_key("user.*"));
    }

    #[test]
    fn test_basic_validator_creation() {
        let fixture = create_basic_validator();
        assert_eq!(fixture.default_rules.required_fields.len(), 2);
    }

    #[test]
    fn test_strict_validator_creation() {
        let fixture = create_strict_validator();
        assert!(fixture.is_ok());
    }

    #[test]
    fn test_validate_valid_event() {
        let validator = create_basic_validator();
        let fixture = create_test_event();
        let actual = validator.validate(&fixture);
        assert!(actual.is_ok());
    }

    #[test]
    fn test_validate_event_missing_required_field() {
        let rules = ValidationRules::new().require_field("user_id");
        let validator = EventValidator::new().with_default_rules(rules);
        let fixture = Event::new("test.event", json!({"other_field": "value"}));
        let actual = validator.validate(&fixture);
        assert!(actual.is_err());
    }

    #[test]
    fn test_validate_event_pattern_mismatch() {
        let rules = ValidationRules::new()
            .with_pattern("name", r"^user\.")
            .unwrap();
        let validator = EventValidator::new().with_default_rules(rules);
        let fixture = Event::new("admin.login", json!({}));
        let actual = validator.validate(&fixture);
        assert!(actual.is_err());
    }

    #[test]
    fn test_validate_event_pattern_match() {
        let rules = ValidationRules::new()
            .with_pattern("name", r"^user\.")
            .unwrap();
        let validator = EventValidator::new().with_default_rules(rules);
        let fixture = Event::new("user.login", json!({}));
        let actual = validator.validate(&fixture);
        assert!(actual.is_ok());
    }

    #[test]
    fn test_validate_required_source() {
        let rules = ValidationRules::new().require_field("source");
        let validator = EventValidator::new().with_default_rules(rules);
        let fixture = Event::new("test.event", json!({}));
        let actual = validator.validate(&fixture);
        assert!(actual.is_err());
    }

    #[test]
    fn test_validate_with_source() {
        let rules = ValidationRules::new().require_field("source");
        let validator = EventValidator::new().with_default_rules(rules);
        let fixture = Event::new("test.event", json!({})).source("api");
        let actual = validator.validate(&fixture);
        assert!(actual.is_ok());
    }

    #[test]
    fn test_find_rules_pattern_match() {
        let user_rules = ValidationRules::new().require_field("user_id");
        let validator = EventValidator::new().add_rules("user.*", user_rules);

        let fixture = Event::new("user.login", json!({}));
        let actual = validator.find_rules(&fixture);
        assert_eq!(actual.required_fields.len(), 1);
    }

    #[test]
    fn test_find_rules_default() {
        let default_rules = ValidationRules::new().require_field("name");
        let validator = EventValidator::new().with_default_rules(default_rules);

        let fixture = Event::new("admin.login", json!({}));
        let actual = validator.find_rules(&fixture);
        assert_eq!(actual.required_fields.len(), 1);
    }
}
