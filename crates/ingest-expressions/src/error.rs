//! Error types for the expression engine

use thiserror::Error;

/// Result type for expression operations
pub type Result<T> = std::result::Result<T, ExpressionError>;

/// Comprehensive error types for expression engine operations
#[derive(Debug, Error, Clone)]
pub enum ExpressionError {
    /// Parse error when parsing expressions
    #[error("Parse error: {message}")]
    ParseError { message: String },

    /// Evaluation error during expression execution
    #[error("Evaluation error: {message}")]
    EvaluationError { message: String },

    /// Type error when working with expression values
    #[error("Type error: {message}")]
    TypeError { message: String },

    /// Security violation during expression evaluation
    #[error("Security violation: {message}")]
    SecurityError { message: String },

    /// Variable not found in evaluation context
    #[error("Variable not found: {variable_name}")]
    VariableNotFound { variable_name: String },

    /// Function not found in evaluation context
    #[error("Function not found: {function_name}")]
    FunctionNotFound { function_name: String },

    /// Invalid function call with wrong arguments
    #[error("Invalid function call: {function_name} - {message}")]
    InvalidFunctionCall {
        function_name: String,
        message: String,
    },

    /// Timeout error when expression takes too long to evaluate
    #[error("Expression evaluation timeout after {timeout_ms}ms")]
    TimeoutError { timeout_ms: u64 },

    /// Memory limit exceeded during evaluation
    #[error("Memory limit exceeded: used {used_bytes} bytes, limit {limit_bytes} bytes")]
    MemoryLimitExceeded {
        used_bytes: usize,
        limit_bytes: usize,
    },

    /// Stack overflow during recursive evaluation
    #[error("Stack overflow: maximum recursion depth {max_depth} exceeded")]
    StackOverflow { max_depth: usize },

    /// Invalid expression syntax
    #[error("Invalid syntax: {message} at position {position}")]
    SyntaxError { message: String, position: usize },

    /// Unsupported operation for given types
    #[error("Unsupported operation: {operation} for types {left_type} and {right_type}")]
    UnsupportedOperation {
        operation: String,
        left_type: String,
        right_type: String,
    },

    /// Division by zero error
    #[error("Division by zero")]
    DivisionByZero,

    /// Index out of bounds for array/list access
    #[error("Index out of bounds: index {index} for array of length {length}")]
    IndexOutOfBounds { index: usize, length: usize },

    /// Property not found on object
    #[error("Property not found: {property} on object of type {object_type}")]
    PropertyNotFound {
        property: String,
        object_type: String,
    },

    /// Compilation error when compiling expressions
    #[error("Compilation error: {message}")]
    CompilationError { message: String },

    /// Cache error when working with expression cache
    #[error("Cache error: {message}")]
    CacheError { message: String },

    /// Internal error for unexpected conditions
    #[error("Internal error: {message}")]
    InternalError { message: String },
}

impl ExpressionError {
    /// Create a parse error
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::ParseError {
            message: message.into(),
        }
    }

    /// Create an evaluation error
    pub fn evaluation_error(message: impl Into<String>) -> Self {
        Self::EvaluationError {
            message: message.into(),
        }
    }

    /// Create a type error
    pub fn type_error(message: impl Into<String>) -> Self {
        Self::TypeError {
            message: message.into(),
        }
    }

    /// Create a security error
    pub fn security_error(message: impl Into<String>) -> Self {
        Self::SecurityError {
            message: message.into(),
        }
    }

    /// Create a variable not found error
    pub fn variable_not_found(variable_name: impl Into<String>) -> Self {
        Self::VariableNotFound {
            variable_name: variable_name.into(),
        }
    }

    /// Create a function not found error
    pub fn function_not_found(function_name: impl Into<String>) -> Self {
        Self::FunctionNotFound {
            function_name: function_name.into(),
        }
    }

    /// Create an invalid function call error
    pub fn invalid_function_call(
        function_name: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::InvalidFunctionCall {
            function_name: function_name.into(),
            message: message.into(),
        }
    }

    /// Create a timeout error
    pub fn timeout_error(timeout_ms: u64) -> Self {
        Self::TimeoutError { timeout_ms }
    }

    /// Create a memory limit exceeded error
    pub fn memory_limit_exceeded(used_bytes: usize, limit_bytes: usize) -> Self {
        Self::MemoryLimitExceeded {
            used_bytes,
            limit_bytes,
        }
    }

    /// Create a stack overflow error
    pub fn stack_overflow(max_depth: usize) -> Self {
        Self::StackOverflow { max_depth }
    }

    /// Create a syntax error
    pub fn syntax_error(message: impl Into<String>, position: usize) -> Self {
        Self::SyntaxError {
            message: message.into(),
            position,
        }
    }

    /// Create an unsupported operation error
    pub fn unsupported_operation(
        operation: impl Into<String>,
        left_type: impl Into<String>,
        right_type: impl Into<String>,
    ) -> Self {
        Self::UnsupportedOperation {
            operation: operation.into(),
            left_type: left_type.into(),
            right_type: right_type.into(),
        }
    }

    /// Create a division by zero error
    pub fn division_by_zero() -> Self {
        Self::DivisionByZero
    }

    /// Create an index out of bounds error
    pub fn index_out_of_bounds(index: usize, length: usize) -> Self {
        Self::IndexOutOfBounds { index, length }
    }

    /// Create a property not found error
    pub fn property_not_found(property: impl Into<String>, object_type: impl Into<String>) -> Self {
        Self::PropertyNotFound {
            property: property.into(),
            object_type: object_type.into(),
        }
    }

    /// Create a compilation error
    pub fn compilation_error(message: impl Into<String>) -> Self {
        Self::CompilationError {
            message: message.into(),
        }
    }

    /// Create a cache error
    pub fn cache_error(message: impl Into<String>) -> Self {
        Self::CacheError {
            message: message.into(),
        }
    }

    /// Create an internal error
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::InternalError {
            message: message.into(),
        }
    }

    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            // Parse and syntax errors are not recoverable
            Self::ParseError { .. } | Self::SyntaxError { .. } => false,
            // Security violations are not recoverable
            Self::SecurityError { .. } => false,
            // Timeouts and resource limits might be recoverable with different settings
            Self::TimeoutError { .. }
            | Self::MemoryLimitExceeded { .. }
            | Self::StackOverflow { .. } => true,
            // Type errors and evaluation errors might be recoverable with different input
            Self::TypeError { .. } | Self::EvaluationError { .. } => true,
            // Variable and function not found errors are recoverable with proper context
            Self::VariableNotFound { .. } | Self::FunctionNotFound { .. } => true,
            // Function call errors might be recoverable
            Self::InvalidFunctionCall { .. } => true,
            // Runtime errors are generally recoverable
            Self::UnsupportedOperation { .. }
            | Self::DivisionByZero
            | Self::IndexOutOfBounds { .. }
            | Self::PropertyNotFound { .. } => true,
            // System errors are generally not recoverable
            Self::CompilationError { .. }
            | Self::CacheError { .. }
            | Self::InternalError { .. } => false,
        }
    }

    /// Get the error category
    pub fn category(&self) -> ErrorCategory {
        match self {
            Self::ParseError { .. } | Self::SyntaxError { .. } => ErrorCategory::Parse,
            Self::EvaluationError { .. }
            | Self::UnsupportedOperation { .. }
            | Self::DivisionByZero
            | Self::IndexOutOfBounds { .. }
            | Self::PropertyNotFound { .. } => ErrorCategory::Runtime,
            Self::TypeError { .. } => ErrorCategory::Type,
            Self::SecurityError { .. }
            | Self::TimeoutError { .. }
            | Self::MemoryLimitExceeded { .. }
            | Self::StackOverflow { .. } => ErrorCategory::Security,
            Self::VariableNotFound { .. } | Self::FunctionNotFound { .. } => ErrorCategory::Context,
            Self::InvalidFunctionCall { .. } => ErrorCategory::Function,
            Self::CompilationError { .. } => ErrorCategory::Compilation,
            Self::CacheError { .. } => ErrorCategory::Cache,
            Self::InternalError { .. } => ErrorCategory::Internal,
        }
    }
}

/// Error categories for classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Parse and syntax errors
    Parse,
    /// Runtime evaluation errors
    Runtime,
    /// Type-related errors
    Type,
    /// Security and resource limit errors
    Security,
    /// Context-related errors (variables, functions)
    Context,
    /// Function call errors
    Function,
    /// Compilation errors
    Compilation,
    /// Cache-related errors
    Cache,
    /// Internal system errors
    Internal,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_error_creation() {
        let fixture = ExpressionError::parse_error("Invalid syntax");
        let actual = format!("{fixture}");
        let expected = "Parse error: Invalid syntax";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_error_recoverable() {
        let fixture = ExpressionError::variable_not_found("x");
        let actual = fixture.is_recoverable();
        let expected = true;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_error_category() {
        let fixture = ExpressionError::type_error("Cannot convert string to number");
        let actual = fixture.category();
        let expected = ErrorCategory::Type;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_timeout_error() {
        let fixture = ExpressionError::timeout_error(1000);
        let actual = format!("{fixture}");
        let expected = "Expression evaluation timeout after 1000ms";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_memory_limit_error() {
        let fixture = ExpressionError::memory_limit_exceeded(1024, 512);
        let actual = format!("{fixture}");
        let expected = "Memory limit exceeded: used 1024 bytes, limit 512 bytes";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_function_call_error() {
        let fixture = ExpressionError::invalid_function_call("size", "Expected 1 argument, got 2");
        let actual = format!("{fixture}");
        let expected = "Invalid function call: size - Expected 1 argument, got 2";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_unsupported_operation_error() {
        let fixture = ExpressionError::unsupported_operation("+", "string", "number");
        let actual = format!("{fixture}");
        let expected = "Unsupported operation: + for types string and number";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_index_out_of_bounds_error() {
        let fixture = ExpressionError::index_out_of_bounds(5, 3);
        let actual = format!("{fixture}");
        let expected = "Index out of bounds: index 5 for array of length 3";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_property_not_found_error() {
        let fixture = ExpressionError::property_not_found("name", "User");
        let actual = format!("{fixture}");
        let expected = "Property not found: name on object of type User";
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_syntax_error() {
        let fixture = ExpressionError::syntax_error("Unexpected token", 15);
        let actual = format!("{fixture}");
        let expected = "Invalid syntax: Unexpected token at position 15";
        assert_eq!(actual, expected);
    }
}
