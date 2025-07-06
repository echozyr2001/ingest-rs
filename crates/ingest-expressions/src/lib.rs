//! # ingest-expressions
//!
//! Expression engine with CEL (Common Expression Language) support for the Inngest durable functions platform.
//!
//! This crate provides comprehensive expression evaluation capabilities including:
//! - CEL expression parsing and evaluation
//! - Type-safe expression evaluation with comprehensive error handling
//! - Security sandboxing and resource limits
//! - Performance optimization with caching and compilation
//! - Integration with event system and other components
//!
//! ## Quick Start
//!
//! ```rust
//! use ingest_expressions::{ExpressionEngine, EvaluationContext};
//! use serde_json::json;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create an expression engine
//!     let engine = ExpressionEngine::new();
//!     
//!     // Create evaluation context with variables
//!     let context = EvaluationContext::new()
//!         .with_variable("user_id", json!("123"))
//!         .with_variable("plan", json!("premium"));
//!     
//!     // Evaluate an expression
//!     let result = engine.evaluate_bool("user_id == '123' && plan == 'premium'", &context).await?;
//!     println!("Expression result: {}", result);
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Features
//!
//! ### Expression Evaluation
//!
//! ```rust
//! use ingest_expressions::{ExpressionEngine, EvaluationContext};
//! use serde_json::json;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let engine = ExpressionEngine::new();
//! let context = EvaluationContext::new()
//!     .with_variable("event", json!({
//!         "type": "user.created",
//!         "data": {"user_id": "123", "plan": "premium"}
//!     }));
//!
//! // Evaluate different types
//! let is_premium: bool = engine.evaluate_bool("event.data.plan == 'premium'", &context).await?;
//! let user_id: String = engine.evaluate_string("event.data.user_id", &context).await?;
//! let value = engine.evaluate("event.data", &context).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Security and Performance
//!
//! ```rust
//! use ingest_expressions::{ExpressionEngine, SecurityConfig, PerformanceConfig};
//! use std::time::Duration;
//!
//! let engine = ExpressionEngine::builder()
//!     .with_security(SecurityConfig::new()
//!         .with_max_execution_time(Duration::from_millis(100))
//!         .with_max_memory_usage(10 * 1024 * 1024) // 10MB
//!         .with_allowed_functions(vec!["size".to_string(), "contains".to_string()]))
//!     .with_performance(PerformanceConfig::new()
//!         .with_cache_size(1000)
//!         .with_compilation_enabled(true))
//!     .build();
//! ```

pub mod context;
pub mod error;
pub mod evaluator;
pub mod parser;
pub mod performance;
pub mod security;
pub mod types;

// Re-export commonly used types
pub use context::{EvaluationContext, FunctionRegistry, VariableRegistry};
pub use error::{ExpressionError, Result};
pub use evaluator::{EvaluatorConfig, ExpressionEvaluator};
pub use parser::{ExpressionParser, ParsedExpression};
pub use performance::{ExpressionCache, PerformanceConfig, PerformanceStats};
pub use security::{SecurityConfig, SecurityManager};
pub use types::{ExpressionType, ExpressionValue, TypeRegistry};

/// Main expression engine that combines all components
#[derive(Debug)]
pub struct ExpressionEngine {
    parser: ExpressionParser,
    evaluator: ExpressionEvaluator,
    security: SecurityManager,
    cache: ExpressionCache,
}

impl ExpressionEngine {
    /// Create a new expression engine with default configuration
    pub fn new() -> Self {
        Self::builder().build()
    }

    /// Create a builder for configuring the expression engine
    pub fn builder() -> ExpressionEngineBuilder {
        ExpressionEngineBuilder::new()
    }

    /// Evaluate an expression and return the result as a Value
    pub async fn evaluate(
        &self,
        expression: &str,
        context: &EvaluationContext,
    ) -> Result<ExpressionValue> {
        // Check cache first
        if let Some(cached) = self.cache.get(expression) {
            return self.evaluator.evaluate_parsed(&cached, context).await;
        }

        // Parse and evaluate
        let parsed = self.parser.parse(expression)?;
        let result = self.evaluator.evaluate_parsed(&parsed, context).await?;

        // Cache the parsed expression
        self.cache.insert(expression.to_string(), parsed);

        Ok(result)
    }

    /// Evaluate an expression and return the result as a boolean
    pub async fn evaluate_bool(
        &self,
        expression: &str,
        context: &EvaluationContext,
    ) -> Result<bool> {
        let value = self.evaluate(expression, context).await?;
        value.as_bool().ok_or_else(|| ExpressionError::TypeError {
            message: format!("Expression result is not a boolean: {value:?}"),
        })
    }

    /// Evaluate an expression and return the result as a string
    pub async fn evaluate_string(
        &self,
        expression: &str,
        context: &EvaluationContext,
    ) -> Result<String> {
        let value = self.evaluate(expression, context).await?;
        value.as_string().ok_or_else(|| ExpressionError::TypeError {
            message: format!("Expression result is not a string: {value:?}"),
        })
    }

    /// Evaluate an expression and return the result as a number
    pub async fn evaluate_number(
        &self,
        expression: &str,
        context: &EvaluationContext,
    ) -> Result<f64> {
        let value = self.evaluate(expression, context).await?;
        value.as_number().ok_or_else(|| ExpressionError::TypeError {
            message: format!("Expression result is not a number: {value:?}"),
        })
    }
    /// Get access to the security manager
    pub fn security(&self) -> &SecurityManager {
        &self.security
    }

    /// Get access to the parser
    pub fn parser(&self) -> &ExpressionParser {
        &self.parser
    }

    /// Get access to the evaluator
    pub fn evaluator(&self) -> &ExpressionEvaluator {
        &self.evaluator
    }

    /// Get access to the cache
    pub fn cache(&self) -> &ExpressionCache {
        &self.cache
    }
}

impl Default for ExpressionEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for configuring an ExpressionEngine
#[derive(Debug, Default)]
pub struct ExpressionEngineBuilder {
    security_config: Option<SecurityConfig>,
    performance_config: Option<PerformanceConfig>,
    evaluator_config: Option<EvaluatorConfig>,
}

impl ExpressionEngineBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure security settings
    pub fn with_security(mut self, config: SecurityConfig) -> Self {
        self.security_config = Some(config);
        self
    }

    /// Configure performance settings
    pub fn with_performance(mut self, config: PerformanceConfig) -> Self {
        self.performance_config = Some(config);
        self
    }

    /// Configure evaluator settings
    pub fn with_evaluator(mut self, config: EvaluatorConfig) -> Self {
        self.evaluator_config = Some(config);
        self
    }

    /// Build the expression engine
    pub fn build(self) -> ExpressionEngine {
        let security_config = self.security_config.unwrap_or_default();
        let performance_config = self.performance_config.unwrap_or_default();
        let evaluator_config = self.evaluator_config.unwrap_or_default();

        let parser = ExpressionParser::new();
        let security = SecurityManager::new(security_config);
        let cache = ExpressionCache::new(performance_config.cache_size());
        let evaluator = ExpressionEvaluator::new(evaluator_config, security.clone());

        ExpressionEngine {
            parser,
            evaluator,
            security,
            cache,
        }
    }
}

/// Expression processing prelude for common imports
pub mod prelude {
    pub use crate::{
        EvaluationContext, EvaluatorConfig, ExpressionEngine, ExpressionError, ExpressionValue,
        PerformanceConfig, Result, SecurityConfig,
    };
    pub use ingest_core::{Event, EventData};
    pub use serde_json::{Value, json};
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[tokio::test]
    async fn test_basic_expression_evaluation() {
        let fixture = ExpressionEngine::new();
        let context = EvaluationContext::new()
            .with_variable("x", json!(10))
            .with_variable("y", json!(20));

        let actual = fixture.evaluate_number("x + y", &context).await.unwrap();
        let expected = 30.0;

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn test_boolean_expression_evaluation() {
        let fixture = ExpressionEngine::new();
        let context = EvaluationContext::new()
            .with_variable("user_id", json!("123"))
            .with_variable("plan", json!("premium"));

        let actual = fixture
            .evaluate_bool("user_id == '123' && plan == 'premium'", &context)
            .await
            .unwrap();
        let expected = true;

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn test_string_expression_evaluation() {
        let fixture = ExpressionEngine::new();
        let context = EvaluationContext::new()
            .with_variable("first_name", json!("John"))
            .with_variable("last_name", json!("Doe"));

        let actual = fixture
            .evaluate_string("first_name + ' ' + last_name", &context)
            .await
            .unwrap();
        let expected = "John Doe".to_string();

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn test_expression_engine_builder() {
        let fixture = ExpressionEngine::builder()
            .with_security(
                SecurityConfig::new()
                    .with_max_execution_time(std::time::Duration::from_millis(100)),
            )
            .with_performance(PerformanceConfig::new().with_cache_size(500))
            .build();

        let context = EvaluationContext::new().with_variable("value", json!(42));
        let actual = fixture
            .evaluate_number("value * 2", &context)
            .await
            .unwrap();
        let expected = 84.0;

        assert_eq!(actual, expected);
    }
}
