//! Event filtering for routing

use crate::{Result, error::EventError};
use ingest_core::Event;
use serde_json::Value;
use std::collections::HashMap;

/// Event filter trait
pub trait EventFilter: Send + Sync {
    /// Filter an event
    fn filter(&self, event: &Event) -> Result<bool>;

    /// Get filter name
    fn name(&self) -> &str;
}

/// Field-based event filter
#[derive(Debug, Clone)]
pub struct FieldFilter {
    /// Field path (e.g., "data.user_id", "source")
    field_path: String,
    /// Expected value
    expected_value: Value,
    /// Filter operation
    operation: FilterOperation,
}

/// Filter operations
#[derive(Debug, Clone, PartialEq)]
pub enum FilterOperation {
    /// Equals
    Equals,
    /// Not equals
    NotEquals,
    /// Greater than
    GreaterThan,
    /// Greater than or equal
    GreaterThanOrEqual,
    /// Less than
    LessThan,
    /// Less than or equal
    LessThanOrEqual,
    /// Contains (for strings and arrays)
    Contains,
    /// Not contains
    NotContains,
    /// Starts with (for strings)
    StartsWith,
    /// Ends with (for strings)
    EndsWith,
    /// In list
    In,
    /// Not in list
    NotIn,
    /// Exists (field is present)
    Exists,
    /// Not exists (field is not present)
    NotExists,
}

impl FieldFilter {
    /// Create new field filter
    pub fn new(
        field_path: impl Into<String>,
        operation: FilterOperation,
        expected_value: Value,
    ) -> Self {
        Self {
            field_path: field_path.into(),
            operation,
            expected_value,
        }
    }

    /// Get field value from event
    fn get_field_value(&self, event: &Event) -> Option<Value> {
        let parts: Vec<&str> = self.field_path.split('.').collect();

        match parts.as_slice() {
            ["name"] => Some(Value::String(event.name().to_string())),
            ["source"] => event.source.as_ref().map(|s| Value::String(s.clone())),
            ["user"] => event.user.as_ref().map(|u| Value::String(u.clone())),
            ["version"] => event.version.as_ref().map(|v| Value::String(v.clone())),
            ["trace_id"] => event.trace_id.as_ref().map(|t| Value::String(t.clone())),
            ["data"] => Some(event.data().data.clone()),
            ["data", field] => event
                .data()
                .data
                .as_object()
                .and_then(|obj| obj.get(*field))
                .cloned(),
            ["data", field, subfield] => event
                .data()
                .data
                .as_object()
                .and_then(|obj| obj.get(*field))
                .and_then(|v| v.as_object())
                .and_then(|obj| obj.get(*subfield))
                .cloned(),
            ["metadata", key] => event
                .data()
                .metadata
                .get(*key)
                .map(|v| Value::String(v.clone())),
            _ => None,
        }
    }

    /// Apply filter operation
    fn apply_operation(&self, actual: &Value) -> bool {
        match self.operation {
            FilterOperation::Equals => actual == &self.expected_value,
            FilterOperation::NotEquals => actual != &self.expected_value,
            FilterOperation::GreaterThan => self.compare_values(actual, &self.expected_value) > 0,
            FilterOperation::GreaterThanOrEqual => {
                self.compare_values(actual, &self.expected_value) >= 0
            }
            FilterOperation::LessThan => self.compare_values(actual, &self.expected_value) < 0,
            FilterOperation::LessThanOrEqual => {
                self.compare_values(actual, &self.expected_value) <= 0
            }
            FilterOperation::Contains => self.value_contains(actual, &self.expected_value),
            FilterOperation::NotContains => !self.value_contains(actual, &self.expected_value),
            FilterOperation::StartsWith => self.value_starts_with(actual, &self.expected_value),
            FilterOperation::EndsWith => self.value_ends_with(actual, &self.expected_value),
            FilterOperation::In => self.value_in_list(actual, &self.expected_value),
            FilterOperation::NotIn => !self.value_in_list(actual, &self.expected_value),
            FilterOperation::Exists => true, // If we got here, field exists
            FilterOperation::NotExists => false, // If we got here, field exists
        }
    }

    /// Compare two values
    fn compare_values(&self, a: &Value, b: &Value) -> i8 {
        match (a, b) {
            (Value::Number(n1), Value::Number(n2)) => {
                if let (Some(f1), Some(f2)) = (n1.as_f64(), n2.as_f64()) {
                    f1.partial_cmp(&f2).map(|o| o as i8).unwrap_or(0)
                } else {
                    0
                }
            }
            (Value::String(s1), Value::String(s2)) => s1.cmp(s2) as i8,
            _ => 0,
        }
    }

    /// Check if value contains another value
    fn value_contains(&self, haystack: &Value, needle: &Value) -> bool {
        match (haystack, needle) {
            (Value::String(s), Value::String(n)) => s.contains(n),
            (Value::Array(arr), needle) => arr.contains(needle),
            _ => false,
        }
    }

    /// Check if string value starts with another string
    fn value_starts_with(&self, value: &Value, prefix: &Value) -> bool {
        match (value, prefix) {
            (Value::String(s), Value::String(p)) => s.starts_with(p),
            _ => false,
        }
    }

    /// Check if string value ends with another string
    fn value_ends_with(&self, value: &Value, suffix: &Value) -> bool {
        match (value, suffix) {
            (Value::String(s), Value::String(suf)) => s.ends_with(suf),
            _ => false,
        }
    }

    /// Check if value is in a list
    fn value_in_list(&self, value: &Value, list: &Value) -> bool {
        match list {
            Value::Array(arr) => arr.contains(value),
            _ => false,
        }
    }
}

impl EventFilter for FieldFilter {
    fn filter(&self, event: &Event) -> Result<bool> {
        match self.operation {
            FilterOperation::Exists => Ok(self.get_field_value(event).is_some()),
            FilterOperation::NotExists => Ok(self.get_field_value(event).is_none()),
            _ => {
                if let Some(actual_value) = self.get_field_value(event) {
                    Ok(self.apply_operation(&actual_value))
                } else {
                    Ok(false)
                }
            }
        }
    }

    fn name(&self) -> &str {
        "field_filter"
    }
}

/// Composite filter that combines multiple filters
pub struct CompositeFilter {
    /// Child filters
    filters: Vec<Box<dyn EventFilter>>,
    /// Combination operation
    operation: CompositeOperation,
}

/// Composite operations
#[derive(Debug, Clone, PartialEq)]
pub enum CompositeOperation {
    /// All filters must pass
    And,
    /// At least one filter must pass
    Or,
    /// No filters must pass
    Not,
}

impl CompositeFilter {
    /// Create new composite filter
    pub fn new(operation: CompositeOperation) -> Self {
        Self {
            filters: Vec::new(),
            operation,
        }
    }

    /// Add a filter
    pub fn add_filter(mut self, filter: Box<dyn EventFilter>) -> Self {
        self.filters.push(filter);
        self
    }
}

impl EventFilter for CompositeFilter {
    fn filter(&self, event: &Event) -> Result<bool> {
        match self.operation {
            CompositeOperation::And => {
                for filter in &self.filters {
                    if !filter.filter(event)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            CompositeOperation::Or => {
                for filter in &self.filters {
                    if filter.filter(event)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            CompositeOperation::Not => {
                for filter in &self.filters {
                    if filter.filter(event)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
        }
    }

    fn name(&self) -> &str {
        "composite_filter"
    }
}

/// Type alias for closure-based event filter
type EventFilterFn = dyn Fn(&Event) -> Result<bool> + Send + Sync;

/// Custom filter using a closure
pub struct ClosureFilter {
    /// Filter name
    name: String,
    /// Filter function
    filter_fn: Box<EventFilterFn>,
}

impl ClosureFilter {
    /// Create new closure filter
    pub fn new<F>(name: impl Into<String>, filter_fn: F) -> Self
    where
        F: Fn(&Event) -> Result<bool> + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            filter_fn: Box::new(filter_fn),
        }
    }
}

impl EventFilter for ClosureFilter {
    fn filter(&self, event: &Event) -> Result<bool> {
        (self.filter_fn)(event)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Filter manager for organizing and applying filters
pub struct FilterManager {
    /// Named filters
    filters: HashMap<String, Box<dyn EventFilter>>,
}

impl FilterManager {
    /// Create new filter manager
    pub fn new() -> Self {
        Self {
            filters: HashMap::new(),
        }
    }

    /// Add a named filter
    pub fn add_filter(mut self, name: impl Into<String>, filter: Box<dyn EventFilter>) -> Self {
        self.filters.insert(name.into(), filter);
        self
    }

    /// Apply a filter by name
    pub fn apply_filter(&self, name: &str, event: &Event) -> Result<bool> {
        self.filters
            .get(name)
            .ok_or_else(|| EventError::routing(format!("Filter '{name}' not found")))?
            .filter(event)
    }

    /// Apply all filters
    pub fn apply_all_filters(&self, event: &Event) -> Result<HashMap<String, bool>> {
        let mut results = HashMap::new();

        for (name, filter) in &self.filters {
            let result = filter.filter(event)?;
            results.insert(name.clone(), result);
        }

        Ok(results)
    }

    /// Get filter names
    pub fn filter_names(&self) -> Vec<&String> {
        self.filters.keys().collect()
    }
}

impl Default for FilterManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Create common event filters
pub fn create_common_filters() -> FilterManager {
    let user_filter = FieldFilter::new("data.user_id", FilterOperation::Exists, Value::Null);

    let admin_filter = FieldFilter::new(
        "source",
        FilterOperation::Equals,
        Value::String("admin".to_string()),
    );

    let error_filter = FieldFilter::new(
        "name",
        FilterOperation::EndsWith,
        Value::String(".error".to_string()),
    );

    FilterManager::new()
        .add_filter("has_user", Box::new(user_filter))
        .add_filter("is_admin", Box::new(admin_filter))
        .add_filter("is_error", Box::new(error_filter))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingest_core::Event;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn create_test_event() -> Event {
        Event::new("user.login", json!({"user_id": "123", "score": 85})).source("api")
    }

    #[test]
    fn test_field_filter_equals() {
        let fixture = FieldFilter::new(
            "data.user_id",
            FilterOperation::Equals,
            Value::String("123".to_string()),
        );
        let event = create_test_event();
        let actual = fixture.filter(&event).unwrap();
        assert!(actual);
    }

    #[test]
    fn test_field_filter_not_equals() {
        let fixture = FieldFilter::new(
            "data.user_id",
            FilterOperation::NotEquals,
            Value::String("456".to_string()),
        );
        let event = create_test_event();
        let actual = fixture.filter(&event).unwrap();
        assert!(actual);
    }

    #[test]
    fn test_field_filter_greater_than() {
        let fixture = FieldFilter::new(
            "data.score",
            FilterOperation::GreaterThan,
            Value::Number(serde_json::Number::from(80)),
        );
        let event = create_test_event();
        let actual = fixture.filter(&event).unwrap();
        assert!(actual);
    }

    #[test]
    fn test_field_filter_contains() {
        let fixture = FieldFilter::new(
            "name",
            FilterOperation::Contains,
            Value::String("login".to_string()),
        );
        let event = create_test_event();
        let actual = fixture.filter(&event).unwrap();
        assert!(actual);
    }

    #[test]
    fn test_field_filter_starts_with() {
        let fixture = FieldFilter::new(
            "name",
            FilterOperation::StartsWith,
            Value::String("user".to_string()),
        );
        let event = create_test_event();
        let actual = fixture.filter(&event).unwrap();
        assert!(actual);
    }

    #[test]
    fn test_field_filter_ends_with() {
        let fixture = FieldFilter::new(
            "name",
            FilterOperation::EndsWith,
            Value::String("login".to_string()),
        );
        let event = create_test_event();
        let actual = fixture.filter(&event).unwrap();
        assert!(actual);
    }

    #[test]
    fn test_field_filter_exists() {
        let fixture = FieldFilter::new("data.user_id", FilterOperation::Exists, Value::Null);
        let event = create_test_event();
        let actual = fixture.filter(&event).unwrap();
        assert!(actual);
    }

    #[test]
    fn test_field_filter_not_exists() {
        let fixture = FieldFilter::new(
            "data.missing_field",
            FilterOperation::NotExists,
            Value::Null,
        );
        let event = create_test_event();
        let actual = fixture.filter(&event).unwrap();
        assert!(actual);
    }

    #[test]
    fn test_field_filter_in_list() {
        let fixture = FieldFilter::new(
            "data.user_id",
            FilterOperation::In,
            json!(["123", "456", "789"]),
        );
        let event = create_test_event();
        let actual = fixture.filter(&event).unwrap();
        assert!(actual);
    }

    #[test]
    fn test_field_filter_source() {
        let fixture = FieldFilter::new(
            "source",
            FilterOperation::Equals,
            Value::String("api".to_string()),
        );
        let event = create_test_event();
        let actual = fixture.filter(&event).unwrap();
        assert!(actual);
    }

    #[test]
    fn test_composite_filter_and() {
        let filter1 = FieldFilter::new("data.user_id", FilterOperation::Exists, Value::Null);
        let filter2 = FieldFilter::new(
            "source",
            FilterOperation::Equals,
            Value::String("api".to_string()),
        );

        let fixture = CompositeFilter::new(CompositeOperation::And)
            .add_filter(Box::new(filter1))
            .add_filter(Box::new(filter2));

        let event = create_test_event();
        let actual = fixture.filter(&event).unwrap();
        assert!(actual);
    }

    #[test]
    fn test_composite_filter_or() {
        let filter1 = FieldFilter::new("data.missing_field", FilterOperation::Exists, Value::Null);
        let filter2 = FieldFilter::new(
            "source",
            FilterOperation::Equals,
            Value::String("api".to_string()),
        );

        let fixture = CompositeFilter::new(CompositeOperation::Or)
            .add_filter(Box::new(filter1))
            .add_filter(Box::new(filter2));

        let event = create_test_event();
        let actual = fixture.filter(&event).unwrap();
        assert!(actual);
    }

    #[test]
    fn test_closure_filter() {
        let fixture = ClosureFilter::new("custom", |event| Ok(event.name().contains("login")));

        let event = create_test_event();
        let actual = fixture.filter(&event).unwrap();
        assert!(actual);
        assert_eq!(fixture.name(), "custom");
    }

    #[test]
    fn test_filter_manager() {
        let filter = FieldFilter::new("data.user_id", FilterOperation::Exists, Value::Null);

        let fixture = FilterManager::new().add_filter("has_user", Box::new(filter));

        let event = create_test_event();
        let actual = fixture.apply_filter("has_user", &event).unwrap();
        assert!(actual);
    }

    #[test]
    fn test_filter_manager_missing_filter() {
        let fixture = FilterManager::new();
        let event = create_test_event();
        let actual = fixture.apply_filter("missing", &event);
        assert!(actual.is_err());
    }

    #[test]
    fn test_filter_manager_apply_all() {
        let filter1 = FieldFilter::new("data.user_id", FilterOperation::Exists, Value::Null);
        let filter2 = FieldFilter::new(
            "source",
            FilterOperation::Equals,
            Value::String("api".to_string()),
        );

        let fixture = FilterManager::new()
            .add_filter("has_user", Box::new(filter1))
            .add_filter("is_api", Box::new(filter2));

        let event = create_test_event();
        let actual = fixture.apply_all_filters(&event).unwrap();

        assert_eq!(actual.len(), 2);
        assert_eq!(actual.get("has_user"), Some(&true));
        assert_eq!(actual.get("is_api"), Some(&true));
    }

    #[test]
    fn test_create_common_filters() {
        let fixture = create_common_filters();
        let names = fixture.filter_names();
        assert!(names.contains(&&"has_user".to_string()));
        assert!(names.contains(&&"is_admin".to_string()));
        assert!(names.contains(&&"is_error".to_string()));
    }
}
