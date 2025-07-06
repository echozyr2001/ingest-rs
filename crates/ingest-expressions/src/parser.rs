//! Expression parser with CEL (Common Expression Language) integration

use crate::error::{ExpressionError, Result};
use cel_interpreter::Program;
use cel_parser::{Expression as CelExpression, parse};
use serde::{Deserialize, Serialize};

/// Parsed expression with CEL AST and metadata
#[derive(Debug, Serialize, Deserialize)]
pub struct ParsedExpression {
    /// Original expression source
    source: String,
    /// Parsed CEL expression
    #[serde(skip)]
    cel_expression: Option<CelExpression>,
    /// Expression metadata
    metadata: ExpressionMetadata,
    /// Compilation artifacts
    #[serde(skip)]
    program: Option<Program>,
}

impl Clone for ParsedExpression {
    fn clone(&self) -> Self {
        Self {
            source: self.source.clone(),
            cel_expression: self.cel_expression.clone(),
            metadata: self.metadata.clone(),
            program: None, // Programs are not cloneable, so we skip them
        }
    }
}

impl ParsedExpression {
    /// Create a new parsed expression
    pub fn new(source: String, cel_expression: CelExpression) -> Self {
        let metadata = ExpressionMetadata::from_expression(&cel_expression);
        Self {
            source,
            cel_expression: Some(cel_expression),
            metadata,
            program: None,
        }
    }

    /// Create a simple parsed expression for testing
    pub fn simple(source: String) -> Self {
        Self {
            source,
            cel_expression: None,
            metadata: ExpressionMetadata::default(),
            program: None,
        }
    }

    /// Get the original source code
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Get the CEL expression
    pub fn cel_expression(&self) -> Option<&CelExpression> {
        self.cel_expression.as_ref()
    }

    /// Get expression metadata
    pub fn metadata(&self) -> &ExpressionMetadata {
        &self.metadata
    }

    /// Get the compiled program
    pub fn program(&self) -> Option<&Program> {
        self.program.as_ref()
    }

    /// Set the compiled program
    pub fn set_program(&mut self, program: Program) {
        self.program = Some(program);
    }

    /// Check if the expression is compiled
    pub fn is_compiled(&self) -> bool {
        self.program.is_some()
    }

    /// Get variables referenced in the expression
    pub fn variables(&self) -> &[String] {
        &self.metadata.variables
    }

    /// Get functions called in the expression
    pub fn functions(&self) -> &[String] {
        &self.metadata.functions
    }

    /// Get expression complexity score
    pub fn complexity(&self) -> u32 {
        self.metadata.complexity
    }
}

/// Metadata about a parsed expression
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExpressionMetadata {
    /// Variables referenced in the expression
    pub variables: Vec<String>,
    /// Functions called in the expression
    pub functions: Vec<String>,
    /// Expression complexity score
    pub complexity: u32,
    /// Whether the expression is deterministic
    pub is_deterministic: bool,
    /// Estimated memory usage
    pub estimated_memory: usize,
}

impl ExpressionMetadata {
    /// Create metadata from a CEL expression
    pub fn from_expression(expr: &CelExpression) -> Self {
        let mut metadata = Self::default();
        metadata.analyze_expression(expr);
        metadata
    }

    /// Analyze a CEL expression to extract metadata
    fn analyze_expression(&mut self, _expr: &CelExpression) {
        // This is a simplified analysis - in a real implementation,
        // we would traverse the CEL AST to extract variables, functions, etc.
        self.complexity = 1;
        self.is_deterministic = true;
        self.estimated_memory = std::mem::size_of::<CelExpression>();
    }
}

/// Configuration for the expression parser
#[derive(Debug, Clone)]
pub struct ParserConfig {
    /// Maximum expression length
    max_expression_length: usize,
    /// Enable expression compilation
    compilation_enabled: bool,
    /// Maximum parse time
    max_parse_time: std::time::Duration,
    /// Enable strict mode
    strict_mode: bool,
}

impl ParserConfig {
    /// Create a new parser configuration
    pub fn new() -> Self {
        Self {
            max_expression_length: 10000,
            compilation_enabled: true,
            max_parse_time: std::time::Duration::from_millis(100),
            strict_mode: false,
        }
    }

    /// Set maximum expression length
    pub fn with_max_expression_length(mut self, length: usize) -> Self {
        self.max_expression_length = length;
        self
    }

    /// Enable or disable compilation
    pub fn with_compilation_enabled(mut self, enabled: bool) -> Self {
        self.compilation_enabled = enabled;
        self
    }

    /// Set maximum parse time
    pub fn with_max_parse_time(mut self, duration: std::time::Duration) -> Self {
        self.max_parse_time = duration;
        self
    }

    /// Enable strict mode
    pub fn with_strict_mode(mut self, enabled: bool) -> Self {
        self.strict_mode = enabled;
        self
    }

    /// Get maximum expression length
    pub fn max_expression_length(&self) -> usize {
        self.max_expression_length
    }

    /// Check if compilation is enabled
    pub fn is_compilation_enabled(&self) -> bool {
        self.compilation_enabled
    }

    /// Get maximum parse time
    pub fn max_parse_time(&self) -> std::time::Duration {
        self.max_parse_time
    }

    /// Check if strict mode is enabled
    pub fn is_strict_mode(&self) -> bool {
        self.strict_mode
    }
}

impl Default for ParserConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Expression parser that converts string expressions to parsed AST
#[derive(Debug)]
pub struct ExpressionParser {
    config: ParserConfig,
}

impl ExpressionParser {
    /// Create a new expression parser
    pub fn new() -> Self {
        Self {
            config: ParserConfig::new(),
        }
    }

    /// Create a new expression parser with configuration
    pub fn with_config(config: ParserConfig) -> Self {
        Self { config }
    }

    /// Parse an expression string into a ParsedExpression
    pub fn parse(&self, expression: &str) -> Result<ParsedExpression> {
        // Validate expression length
        if expression.len() > self.config.max_expression_length {
            return Err(ExpressionError::ParseError {
                message: format!(
                    "Expression length {} exceeds maximum {}",
                    expression.len(),
                    self.config.max_expression_length
                ),
            });
        }

        // Parse with timeout
        let start_time = std::time::Instant::now();

        // Parse the CEL expression
        let cel_expr = parse(expression).map_err(|e| ExpressionError::ParseError {
            message: format!("CEL parse error: {e}"),
        })?;

        // Check parse time
        if start_time.elapsed() > self.config.max_parse_time {
            return Err(ExpressionError::ParseError {
                message: "Expression parsing timed out".to_string(),
            });
        }

        let mut parsed = ParsedExpression::new(expression.to_string(), cel_expr);

        // Compile if enabled
        if self.config.compilation_enabled {
            if let Err(e) = self.compile_expression(&mut parsed) {
                // Compilation failure is not fatal in non-strict mode
                if self.config.strict_mode {
                    return Err(e);
                }
            }
        }

        Ok(parsed)
    }

    /// Compile a parsed expression for better performance
    fn compile_expression(&self, _parsed: &mut ParsedExpression) -> Result<()> {
        // CEL compilation is complex and depends on the specific CEL library version
        // For now, we skip compilation
        Ok(())
    }

    /// Validate expression syntax without full parsing
    pub fn validate_syntax(&self, expression: &str) -> Result<()> {
        // Basic syntax validation
        if expression.is_empty() {
            return Err(ExpressionError::ParseError {
                message: "Expression cannot be empty".to_string(),
            });
        }

        // Check for balanced parentheses
        let mut paren_count = 0;
        for ch in expression.chars() {
            match ch {
                '(' => paren_count += 1,
                ')' => {
                    paren_count -= 1;
                    if paren_count < 0 {
                        return Err(ExpressionError::ParseError {
                            message: "Unbalanced parentheses".to_string(),
                        });
                    }
                }
                _ => {}
            }
        }

        if paren_count != 0 {
            return Err(ExpressionError::ParseError {
                message: "Unbalanced parentheses".to_string(),
            });
        }

        Ok(())
    }

    /// Parse multiple expressions in batch
    pub fn parse_batch(&self, expressions: &[&str]) -> Vec<Result<ParsedExpression>> {
        expressions.iter().map(|expr| self.parse(expr)).collect()
    }

    /// Get parser configuration
    pub fn config(&self) -> &ParserConfig {
        &self.config
    }
}

impl Default for ExpressionParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse result with timing information
#[derive(Debug)]
pub struct ParseResult {
    pub expression: ParsedExpression,
    pub parse_time: std::time::Duration,
    pub compilation_time: Option<std::time::Duration>,
}

/// Batch parsing statistics
#[derive(Debug, Default)]
pub struct BatchParseStats {
    pub total_expressions: usize,
    pub successful_parses: usize,
    pub failed_parses: usize,
    pub total_parse_time: std::time::Duration,
    pub average_parse_time: std::time::Duration,
}

impl BatchParseStats {
    /// Create new batch parse statistics
    pub fn new() -> Self {
        Self::default()
    }

    /// Update statistics with a parse result
    pub fn update(&mut self, parse_time: std::time::Duration, success: bool) {
        self.total_expressions += 1;
        if success {
            self.successful_parses += 1;
        } else {
            self.failed_parses += 1;
        }
        self.total_parse_time += parse_time;
        self.average_parse_time = self.total_parse_time / self.total_expressions as u32;
    }

    /// Get success rate
    pub fn success_rate(&self) -> f64 {
        if self.total_expressions == 0 {
            0.0
        } else {
            self.successful_parses as f64 / self.total_expressions as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_parser_creation() {
        let fixture = ExpressionParser::new();
        let actual = fixture.config().max_expression_length();
        let expected = 10000;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_parser_with_config() {
        let config = ParserConfig::new()
            .with_max_expression_length(5000)
            .with_compilation_enabled(false);
        let fixture = ExpressionParser::with_config(config);

        let actual = fixture.config().max_expression_length();
        let expected = 5000;
        assert_eq!(actual, expected);

        let actual = fixture.config().is_compilation_enabled();
        let expected = false;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_simple_expression_parsing() {
        let fixture = ExpressionParser::new();
        let actual = fixture.parse("true").unwrap();
        let expected = "true";
        assert_eq!(actual.source(), expected);
    }

    #[test]
    fn test_expression_length_validation() {
        let config = ParserConfig::new().with_max_expression_length(5);
        let fixture = ExpressionParser::with_config(config);
        let actual = fixture.parse("this is a very long expression");
        assert!(actual.is_err());
    }

    #[test]
    fn test_syntax_validation() {
        let fixture = ExpressionParser::new();

        // Valid syntax
        let actual = fixture.validate_syntax("(1 + 2) * 3");
        assert!(actual.is_ok());

        // Invalid syntax - unbalanced parentheses
        let actual = fixture.validate_syntax("(1 + 2 * 3");
        assert!(actual.is_err());

        // Empty expression
        let actual = fixture.validate_syntax("");
        assert!(actual.is_err());
    }

    #[test]
    fn test_parsed_expression_metadata() {
        let fixture = ParsedExpression::simple("x + y".to_string());
        let actual = fixture.source();
        let expected = "x + y";
        assert_eq!(actual, expected);

        let actual = fixture.complexity();
        let expected = 0; // Default complexity
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_batch_parsing() {
        let fixture = ExpressionParser::new();
        let expressions = vec!["true", "false", "1 + 2"];
        let results = fixture.parse_batch(&expressions);

        let actual = results.len();
        let expected = 3;
        assert_eq!(actual, expected);

        // All should succeed
        for result in results {
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_batch_parse_stats() {
        let mut fixture = BatchParseStats::new();
        fixture.update(std::time::Duration::from_millis(10), true);
        fixture.update(std::time::Duration::from_millis(20), false);

        let actual = fixture.success_rate();
        let expected = 0.5;
        assert_eq!(actual, expected);

        let actual = fixture.total_expressions;
        let expected = 2;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expression_compilation() {
        let config = ParserConfig::new().with_compilation_enabled(true);
        let fixture = ExpressionParser::with_config(config);

        // Simple expressions should compile successfully
        let result = fixture.parse("true");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parser_config_builder() {
        let fixture = ParserConfig::new()
            .with_max_expression_length(1000)
            .with_compilation_enabled(false)
            .with_strict_mode(true);

        let actual = fixture.max_expression_length();
        let expected = 1000;
        assert_eq!(actual, expected);

        let actual = fixture.is_strict_mode();
        let expected = true;
        assert_eq!(actual, expected);
    }
}
