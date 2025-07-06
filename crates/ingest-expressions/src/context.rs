//! Evaluation context for expression execution

use crate::error::{ExpressionError, Result};
use crate::types::ExpressionValue;
use ingest_core::Event;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Function signature for custom functions
pub type ExpressionFunction =
    Arc<dyn Fn(&[ExpressionValue]) -> Result<ExpressionValue> + Send + Sync>;

/// Registry for variables in the evaluation context
#[derive(Debug, Clone, Default)]
pub struct VariableRegistry {
    variables: HashMap<String, ExpressionValue>,
}

impl VariableRegistry {
    /// Create a new variable registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a variable value
    pub fn set(&mut self, name: String, value: ExpressionValue) {
        self.variables.insert(name, value);
    }

    /// Get a variable value
    pub fn get(&self, name: &str) -> Option<&ExpressionValue> {
        self.variables.get(name)
    }

    /// Check if a variable exists
    pub fn has(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }

    /// Remove a variable
    pub fn remove(&mut self, name: &str) -> Option<ExpressionValue> {
        self.variables.remove(name)
    }

    /// Get all variable names
    pub fn variable_names(&self) -> Vec<&String> {
        self.variables.keys().collect()
    }

    /// Clear all variables
    pub fn clear(&mut self) {
        self.variables.clear();
    }

    /// Merge another registry into this one
    pub fn merge(&mut self, other: &VariableRegistry) {
        for (name, value) in &other.variables {
            self.variables.insert(name.clone(), value.clone());
        }
    }

    /// Create a scoped registry with additional variables
    pub fn with_scope(&self, scope_vars: HashMap<String, ExpressionValue>) -> Self {
        let mut new_registry = self.clone();
        for (name, value) in scope_vars {
            new_registry.set(name, value);
        }
        new_registry
    }
}

/// Registry for functions in the evaluation context
#[derive(Clone, Default)]
pub struct FunctionRegistry {
    functions: HashMap<String, ExpressionFunction>,
}

impl std::fmt::Debug for FunctionRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FunctionRegistry")
            .field("functions", &format!("{} functions", self.functions.len()))
            .finish()
    }
}

impl FunctionRegistry {
    /// Create a new function registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a function
    pub fn register(&mut self, name: String, function: ExpressionFunction) {
        self.functions.insert(name, function);
    }

    /// Get a function
    pub fn get(&self, name: &str) -> Option<&ExpressionFunction> {
        self.functions.get(name)
    }

    /// Check if a function exists
    pub fn has(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    /// Remove a function
    pub fn remove(&mut self, name: &str) -> Option<ExpressionFunction> {
        self.functions.remove(name)
    }

    /// Get all function names
    pub fn function_names(&self) -> Vec<&String> {
        self.functions.keys().collect()
    }

    /// Clear all functions
    pub fn clear(&mut self) {
        self.functions.clear();
    }

    /// Create a registry with built-in functions
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        registry.register_builtins();
        registry
    }

    /// Register built-in functions
    pub fn register_builtins(&mut self) {
        // String functions
        self.register(
            "size".to_string(),
            Arc::new(|args| {
                if args.len() != 1 {
                    return Err(ExpressionError::invalid_function_call(
                        "size",
                        format!("Expected 1 argument, got {}", args.len()),
                    ));
                }
                match &args[0] {
                    ExpressionValue::String(s) => Ok(ExpressionValue::int(s.len() as i64)),
                    ExpressionValue::Array(a) => Ok(ExpressionValue::int(a.len() as i64)),
                    ExpressionValue::Object(o) => Ok(ExpressionValue::int(o.len() as i64)),
                    _ => Err(ExpressionError::invalid_function_call(
                        "size",
                        "Argument must be string, array, or object",
                    )),
                }
            }),
        );

        self.register(
            "contains".to_string(),
            Arc::new(|args| {
                if args.len() != 2 {
                    return Err(ExpressionError::invalid_function_call(
                        "contains",
                        format!("Expected 2 arguments, got {}", args.len()),
                    ));
                }
                match (&args[0], &args[1]) {
                    (ExpressionValue::String(haystack), ExpressionValue::String(needle)) => {
                        Ok(ExpressionValue::bool(haystack.contains(needle)))
                    }
                    (ExpressionValue::Array(haystack), needle) => Ok(ExpressionValue::bool(
                        haystack.iter().any(|v| v.equals(needle)),
                    )),
                    _ => Err(ExpressionError::invalid_function_call(
                        "contains",
                        "First argument must be string or array",
                    )),
                }
            }),
        );

        self.register(
            "startsWith".to_string(),
            Arc::new(|args| {
                if args.len() != 2 {
                    return Err(ExpressionError::invalid_function_call(
                        "startsWith",
                        format!("Expected 2 arguments, got {}", args.len()),
                    ));
                }
                match (&args[0], &args[1]) {
                    (ExpressionValue::String(s), ExpressionValue::String(prefix)) => {
                        Ok(ExpressionValue::bool(s.starts_with(prefix)))
                    }
                    _ => Err(ExpressionError::invalid_function_call(
                        "startsWith",
                        "Both arguments must be strings",
                    )),
                }
            }),
        );

        self.register(
            "endsWith".to_string(),
            Arc::new(|args| {
                if args.len() != 2 {
                    return Err(ExpressionError::invalid_function_call(
                        "endsWith",
                        format!("Expected 2 arguments, got {}", args.len()),
                    ));
                }
                match (&args[0], &args[1]) {
                    (ExpressionValue::String(s), ExpressionValue::String(suffix)) => {
                        Ok(ExpressionValue::bool(s.ends_with(suffix)))
                    }
                    _ => Err(ExpressionError::invalid_function_call(
                        "endsWith",
                        "Both arguments must be strings",
                    )),
                }
            }),
        );

        // Math functions
        self.register(
            "abs".to_string(),
            Arc::new(|args| {
                if args.len() != 1 {
                    return Err(ExpressionError::invalid_function_call(
                        "abs",
                        format!("Expected 1 argument, got {}", args.len()),
                    ));
                }
                match &args[0] {
                    ExpressionValue::Int(i) => Ok(ExpressionValue::int(i.abs())),
                    ExpressionValue::Float(f) => Ok(ExpressionValue::float(f.abs())),
                    _ => Err(ExpressionError::invalid_function_call(
                        "abs",
                        "Argument must be a number",
                    )),
                }
            }),
        );

        self.register(
            "max".to_string(),
            Arc::new(|args| {
                if args.len() < 2 {
                    return Err(ExpressionError::invalid_function_call(
                        "max",
                        "Expected at least 2 arguments",
                    ));
                }
                let mut max_val = &args[0];
                for arg in &args[1..] {
                    if arg.greater_than(max_val)? {
                        max_val = arg;
                    }
                }
                Ok(max_val.clone())
            }),
        );

        self.register(
            "min".to_string(),
            Arc::new(|args| {
                if args.len() < 2 {
                    return Err(ExpressionError::invalid_function_call(
                        "min",
                        "Expected at least 2 arguments",
                    ));
                }
                let mut min_val = &args[0];
                for arg in &args[1..] {
                    if arg.less_than(min_val)? {
                        min_val = arg;
                    }
                }
                Ok(min_val.clone())
            }),
        );

        // Type checking functions
        self.register(
            "type".to_string(),
            Arc::new(|args| {
                if args.len() != 1 {
                    return Err(ExpressionError::invalid_function_call(
                        "type",
                        format!("Expected 1 argument, got {}", args.len()),
                    ));
                }
                Ok(ExpressionValue::string(args[0].get_type().to_string()))
            }),
        );

        self.register(
            "isNull".to_string(),
            Arc::new(|args| {
                if args.len() != 1 {
                    return Err(ExpressionError::invalid_function_call(
                        "isNull",
                        format!("Expected 1 argument, got {}", args.len()),
                    ));
                }
                Ok(ExpressionValue::bool(args[0].is_null()))
            }),
        );
    }
}

/// Evaluation context that provides variables and functions for expression evaluation
#[derive(Debug, Clone)]
pub struct EvaluationContext {
    variables: VariableRegistry,
    functions: FunctionRegistry,
    /// Maximum recursion depth for expression evaluation
    max_recursion_depth: usize,
    /// Current recursion depth
    current_depth: usize,
}

impl EvaluationContext {
    /// Create a new evaluation context
    pub fn new() -> Self {
        Self {
            variables: VariableRegistry::new(),
            functions: FunctionRegistry::with_builtins(),
            max_recursion_depth: 100,
            current_depth: 0,
        }
    }

    /// Create a new evaluation context with custom settings
    pub fn with_settings(max_recursion_depth: usize) -> Self {
        Self {
            variables: VariableRegistry::new(),
            functions: FunctionRegistry::with_builtins(),
            max_recursion_depth,
            current_depth: 0,
        }
    }

    /// Add a variable to the context
    pub fn with_variable(mut self, name: impl Into<String>, value: Value) -> Self {
        self.variables
            .set(name.into(), ExpressionValue::from(value));
        self
    }

    /// Add multiple variables to the context
    pub fn with_variables(mut self, variables: HashMap<String, Value>) -> Self {
        for (name, value) in variables {
            self.variables.set(name, ExpressionValue::from(value));
        }
        self
    }

    /// Add a function to the context
    pub fn with_function(mut self, name: impl Into<String>, function: ExpressionFunction) -> Self {
        self.functions.register(name.into(), function);
        self
    }

    /// Set maximum recursion depth
    pub fn with_max_recursion_depth(mut self, depth: usize) -> Self {
        self.max_recursion_depth = depth;
        self
    }

    /// Get a variable value
    pub fn get_variable(&self, name: &str) -> Result<ExpressionValue> {
        self.variables
            .get(name)
            .cloned()
            .ok_or_else(|| ExpressionError::variable_not_found(name))
    }

    /// Set a variable value
    pub fn set_variable(&mut self, name: String, value: ExpressionValue) {
        self.variables.set(name, value);
    }

    /// Call a function
    pub fn call_function(&self, name: &str, args: &[ExpressionValue]) -> Result<ExpressionValue> {
        let function = self
            .functions
            .get(name)
            .ok_or_else(|| ExpressionError::function_not_found(name))?;
        function(args)
    }

    /// Check if we can recurse deeper
    pub fn can_recurse(&self) -> bool {
        self.current_depth < self.max_recursion_depth
    }

    /// Enter a new recursion level
    pub fn enter_recursion(&self) -> Result<Self> {
        if !self.can_recurse() {
            return Err(ExpressionError::stack_overflow(self.max_recursion_depth));
        }
        let mut new_context = self.clone();
        new_context.current_depth += 1;
        Ok(new_context)
    }

    /// Create a scoped context with additional variables
    pub fn with_scope(&self, scope_vars: HashMap<String, ExpressionValue>) -> Self {
        let mut new_context = self.clone();
        new_context.variables = self.variables.with_scope(scope_vars);
        new_context
    }

    /// Get all variable names
    pub fn variable_names(&self) -> Vec<&String> {
        self.variables.variable_names()
    }

    /// Get all function names
    pub fn function_names(&self) -> Vec<&String> {
        self.functions.function_names()
    }

    /// Check if a variable exists
    pub fn has_variable(&self, name: &str) -> bool {
        self.variables.has(name)
    }

    /// Check if a function exists
    pub fn has_function(&self, name: &str) -> bool {
        self.functions.has(name)
    }
}

impl Default for EvaluationContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper functions for creating contexts from common data sources
impl EvaluationContext {
    /// Create a context from an event
    pub fn from_event(event: &Event) -> Self {
        let mut context = Self::new();

        // Add event properties
        context.set_variable(
            "event".to_string(),
            ExpressionValue::from(serde_json::to_value(event).unwrap_or_default()),
        );

        // Add convenience variables
        context.set_variable(
            "event_name".to_string(),
            ExpressionValue::string(&event.name),
        );
        context.set_variable(
            "event_id".to_string(),
            ExpressionValue::string(event.id.to_string()),
        );
        context.set_variable(
            "event_timestamp".to_string(),
            ExpressionValue::int(event.timestamp.timestamp()),
        );

        // Add event data
        context.set_variable(
            "data".to_string(),
            ExpressionValue::from(serde_json::to_value(&event.data).unwrap_or_default()),
        );

        context
    }

    /// Create a context from JSON data
    pub fn from_json(data: Value) -> Self {
        let mut context = Self::new();

        if let Value::Object(obj) = data {
            for (key, value) in obj {
                context.set_variable(key, ExpressionValue::from(value));
            }
        }

        context
    }

    /// Create a context with environment variables
    pub fn with_env_vars(mut self) -> Self {
        let mut env_vars = HashMap::new();

        // Add common environment variables
        if let Ok(env) = std::env::var("ENVIRONMENT") {
            env_vars.insert("ENVIRONMENT".to_string(), ExpressionValue::string(env));
        }
        if let Ok(env) = std::env::var("NODE_ENV") {
            env_vars.insert("NODE_ENV".to_string(), ExpressionValue::string(env));
        }

        // Add all environment variables under "env" namespace
        let all_env: HashMap<String, ExpressionValue> = std::env::vars()
            .map(|(k, v)| (k, ExpressionValue::string(v)))
            .collect();

        self.set_variable("env".to_string(), ExpressionValue::object(all_env));

        for (key, value) in env_vars {
            self.set_variable(key, value);
        }

        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn test_variable_registry() {
        let mut fixture = VariableRegistry::new();
        fixture.set("x".to_string(), ExpressionValue::int(42));
        let actual = fixture.get("x").unwrap().clone();
        let expected = ExpressionValue::int(42);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_function_registry() {
        let mut fixture = FunctionRegistry::new();
        fixture.register(
            "double".to_string(),
            Arc::new(|args| {
                if let Some(ExpressionValue::Int(i)) = args.first() {
                    Ok(ExpressionValue::int(i * 2))
                } else {
                    Err(ExpressionError::invalid_function_call(
                        "double",
                        "Expected integer",
                    ))
                }
            }),
        );

        let function = fixture.get("double").unwrap();
        let actual = function(&[ExpressionValue::int(21)]).unwrap();
        let expected = ExpressionValue::int(42);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_evaluation_context_creation() {
        let fixture = EvaluationContext::new()
            .with_variable("x", json!(10))
            .with_variable("y", json!(20));

        let actual = fixture.get_variable("x").unwrap();
        let expected = ExpressionValue::int(10);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_evaluation_context_function_call() {
        let fixture = EvaluationContext::new();
        let actual = fixture
            .call_function("size", &[ExpressionValue::string("hello")])
            .unwrap();
        let expected = ExpressionValue::int(5);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_evaluation_context_recursion_limit() {
        let fixture = EvaluationContext::with_settings(2);
        let level1 = fixture.enter_recursion().unwrap();
        let level2 = level1.enter_recursion().unwrap();
        let level3_result = level2.enter_recursion();
        assert!(level3_result.is_err());
    }

    #[test]
    fn test_evaluation_context_from_event() {
        let event = Event::new("user.created", json!({"user_id": "123"}));
        let fixture = EvaluationContext::from_event(&event);

        let actual = fixture.get_variable("event_name").unwrap();
        let expected = ExpressionValue::string("user.created");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_evaluation_context_scoping() {
        let fixture = EvaluationContext::new().with_variable("x", json!(10));

        let mut scope_vars = HashMap::new();
        scope_vars.insert("y".to_string(), ExpressionValue::int(20));

        let scoped = fixture.with_scope(scope_vars);

        let actual_x = scoped.get_variable("x").unwrap();
        let actual_y = scoped.get_variable("y").unwrap();
        let expected_x = ExpressionValue::int(10);
        let expected_y = ExpressionValue::int(20);

        assert_eq!(actual_x, expected_x);
        assert_eq!(actual_y, expected_y);
    }

    #[test]
    fn test_builtin_functions() {
        let fixture = EvaluationContext::new();

        // Test size function
        let actual = fixture
            .call_function("size", &[ExpressionValue::string("hello")])
            .unwrap();
        let expected = ExpressionValue::int(5);
        assert_eq!(actual, expected);

        // Test contains function
        let actual = fixture
            .call_function(
                "contains",
                &[
                    ExpressionValue::string("hello world"),
                    ExpressionValue::string("world"),
                ],
            )
            .unwrap();
        let expected = ExpressionValue::bool(true);
        assert_eq!(actual, expected);

        // Test abs function
        let actual = fixture
            .call_function("abs", &[ExpressionValue::int(-42)])
            .unwrap();
        let expected = ExpressionValue::int(42);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_context_from_json() {
        let fixture = EvaluationContext::from_json(json!({
            "user_id": "123",
            "plan": "premium"
        }));

        let actual = fixture.get_variable("user_id").unwrap();
        let expected = ExpressionValue::string("123");
        assert_eq!(actual, expected);
    }
}
