//! Security management for expression evaluation

use crate::error::{ExpressionError, Result};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Security configuration for expression evaluation
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Maximum execution time for expressions
    max_execution_time: Duration,
    /// Maximum memory usage in bytes
    max_memory_usage: usize,
    /// Maximum recursion depth
    max_recursion_depth: usize,
    /// Allowed function names (whitelist)
    allowed_functions: Option<HashSet<String>>,
    /// Denied function names (blacklist)
    denied_functions: HashSet<String>,
    /// Maximum string length
    max_string_length: usize,
    /// Maximum array length
    max_array_length: usize,
    /// Maximum object properties
    max_object_properties: usize,
    /// Enable strict mode (additional security checks)
    strict_mode: bool,
}

impl SecurityConfig {
    /// Create a new security configuration with default values
    pub fn new() -> Self {
        Self {
            max_execution_time: Duration::from_millis(100),
            max_memory_usage: 10 * 1024 * 1024, // 10MB
            max_recursion_depth: 100,
            allowed_functions: None,
            denied_functions: HashSet::new(),
            max_string_length: 1024 * 1024, // 1MB
            max_array_length: 10000,
            max_object_properties: 1000,
            strict_mode: false,
        }
    }

    /// Set maximum execution time
    pub fn with_max_execution_time(mut self, duration: Duration) -> Self {
        self.max_execution_time = duration;
        self
    }

    /// Set maximum memory usage
    pub fn with_max_memory_usage(mut self, bytes: usize) -> Self {
        self.max_memory_usage = bytes;
        self
    }

    /// Set maximum recursion depth
    pub fn with_max_recursion_depth(mut self, depth: usize) -> Self {
        self.max_recursion_depth = depth;
        self
    }

    /// Set allowed functions (whitelist mode)
    pub fn with_allowed_functions(mut self, functions: Vec<String>) -> Self {
        self.allowed_functions = Some(functions.into_iter().collect());
        self
    }

    /// Add denied functions (blacklist mode)
    pub fn with_denied_functions(mut self, functions: Vec<String>) -> Self {
        self.denied_functions = functions.into_iter().collect();
        self
    }

    /// Set maximum string length
    pub fn with_max_string_length(mut self, length: usize) -> Self {
        self.max_string_length = length;
        self
    }

    /// Set maximum array length
    pub fn with_max_array_length(mut self, length: usize) -> Self {
        self.max_array_length = length;
        self
    }

    /// Set maximum object properties
    pub fn with_max_object_properties(mut self, count: usize) -> Self {
        self.max_object_properties = count;
        self
    }

    /// Enable strict mode
    pub fn with_strict_mode(mut self, enabled: bool) -> Self {
        self.strict_mode = enabled;
        self
    }

    /// Get maximum execution time
    pub fn max_execution_time(&self) -> Duration {
        self.max_execution_time
    }

    /// Get maximum memory usage
    pub fn max_memory_usage(&self) -> usize {
        self.max_memory_usage
    }

    /// Get maximum recursion depth
    pub fn max_recursion_depth(&self) -> usize {
        self.max_recursion_depth
    }

    /// Get maximum string length
    pub fn max_string_length(&self) -> usize {
        self.max_string_length
    }

    /// Get maximum array length
    pub fn max_array_length(&self) -> usize {
        self.max_array_length
    }

    /// Get maximum object properties
    pub fn max_object_properties(&self) -> usize {
        self.max_object_properties
    }

    /// Check if strict mode is enabled
    pub fn is_strict_mode(&self) -> bool {
        self.strict_mode
    }

    /// Check if a function is allowed
    pub fn is_function_allowed(&self, function_name: &str) -> bool {
        // Check blacklist first
        if self.denied_functions.contains(function_name) {
            return false;
        }

        // Check whitelist if configured
        if let Some(ref allowed) = self.allowed_functions {
            allowed.contains(function_name)
        } else {
            true
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Resource usage tracking for security monitoring
#[derive(Debug, Clone, Default)]
pub struct ResourceUsage {
    /// Current memory usage in bytes
    memory_usage: usize,
    /// Peak memory usage in bytes
    peak_memory_usage: usize,
    /// Current recursion depth
    recursion_depth: usize,
    /// Peak recursion depth
    peak_recursion_depth: usize,
    /// Number of operations performed
    operation_count: usize,
    /// Start time of evaluation
    start_time: Option<Instant>,
}

impl ResourceUsage {
    /// Create a new resource usage tracker
    pub fn new() -> Self {
        Self {
            start_time: Some(Instant::now()),
            ..Default::default()
        }
    }

    /// Update memory usage
    pub fn update_memory_usage(&mut self, bytes: usize) {
        self.memory_usage = bytes;
        if bytes > self.peak_memory_usage {
            self.peak_memory_usage = bytes;
        }
    }

    /// Increment memory usage
    pub fn increment_memory_usage(&mut self, bytes: usize) {
        self.memory_usage += bytes;
        if self.memory_usage > self.peak_memory_usage {
            self.peak_memory_usage = self.memory_usage;
        }
    }

    /// Decrement memory usage
    pub fn decrement_memory_usage(&mut self, bytes: usize) {
        self.memory_usage = self.memory_usage.saturating_sub(bytes);
    }

    /// Update recursion depth
    pub fn update_recursion_depth(&mut self, depth: usize) {
        self.recursion_depth = depth;
        if depth > self.peak_recursion_depth {
            self.peak_recursion_depth = depth;
        }
    }

    /// Increment operation count
    pub fn increment_operations(&mut self) {
        self.operation_count += 1;
    }

    /// Get current memory usage
    pub fn memory_usage(&self) -> usize {
        self.memory_usage
    }

    /// Get peak memory usage
    pub fn peak_memory_usage(&self) -> usize {
        self.peak_memory_usage
    }

    /// Get current recursion depth
    pub fn recursion_depth(&self) -> usize {
        self.recursion_depth
    }

    /// Get peak recursion depth
    pub fn peak_recursion_depth(&self) -> usize {
        self.peak_recursion_depth
    }

    /// Get operation count
    pub fn operation_count(&self) -> usize {
        self.operation_count
    }

    /// Get elapsed time since start
    pub fn elapsed_time(&self) -> Option<Duration> {
        self.start_time.map(|start| start.elapsed())
    }
}

/// Security manager that enforces security policies during expression evaluation
#[derive(Debug, Clone)]
pub struct SecurityManager {
    config: SecurityConfig,
    resource_usage: Arc<Mutex<ResourceUsage>>,
}

impl SecurityManager {
    /// Create a new security manager
    pub fn new(config: SecurityConfig) -> Self {
        Self {
            config,
            resource_usage: Arc::new(Mutex::new(ResourceUsage::new())),
        }
    }

    /// Check if execution time limit has been exceeded
    pub fn check_execution_time(&self) -> Result<()> {
        let usage = self.resource_usage.lock().unwrap();
        if let Some(elapsed) = usage.elapsed_time() {
            if elapsed > self.config.max_execution_time {
                return Err(ExpressionError::timeout_error(
                    self.config.max_execution_time.as_millis() as u64,
                ));
            }
        }
        Ok(())
    }

    /// Check if memory limit has been exceeded
    pub fn check_memory_usage(&self) -> Result<()> {
        let usage = self.resource_usage.lock().unwrap();
        if usage.memory_usage() > self.config.max_memory_usage {
            return Err(ExpressionError::memory_limit_exceeded(
                usage.memory_usage(),
                self.config.max_memory_usage,
            ));
        }
        Ok(())
    }

    /// Check if recursion depth limit has been exceeded
    pub fn check_recursion_depth(&self) -> Result<()> {
        let usage = self.resource_usage.lock().unwrap();
        if usage.recursion_depth() > self.config.max_recursion_depth {
            return Err(ExpressionError::stack_overflow(
                self.config.max_recursion_depth,
            ));
        }
        Ok(())
    }

    /// Check if a function call is allowed
    pub fn check_function_call(&self, function_name: &str) -> Result<()> {
        if !self.config.is_function_allowed(function_name) {
            return Err(ExpressionError::security_error(format!(
                "Function '{function_name}' is not allowed"
            )));
        }
        Ok(())
    }

    /// Check if string length is within limits
    pub fn check_string_length(&self, length: usize) -> Result<()> {
        if length > self.config.max_string_length {
            return Err(ExpressionError::security_error(format!(
                "String length {} exceeds maximum allowed length {}",
                length, self.config.max_string_length
            )));
        }
        Ok(())
    }

    /// Check if array length is within limits
    pub fn check_array_length(&self, length: usize) -> Result<()> {
        if length > self.config.max_array_length {
            return Err(ExpressionError::security_error(format!(
                "Array length {} exceeds maximum allowed length {}",
                length, self.config.max_array_length
            )));
        }
        Ok(())
    }

    /// Check if object property count is within limits
    pub fn check_object_properties(&self, count: usize) -> Result<()> {
        if count > self.config.max_object_properties {
            return Err(ExpressionError::security_error(format!(
                "Object property count {} exceeds maximum allowed count {}",
                count, self.config.max_object_properties
            )));
        }
        Ok(())
    }

    /// Update memory usage
    pub fn update_memory_usage(&self, bytes: usize) {
        if let Ok(mut usage) = self.resource_usage.lock() {
            usage.update_memory_usage(bytes);
        }
    }

    /// Increment memory usage
    pub fn increment_memory_usage(&self, bytes: usize) {
        if let Ok(mut usage) = self.resource_usage.lock() {
            usage.increment_memory_usage(bytes);
        }
    }

    /// Update recursion depth
    pub fn update_recursion_depth(&self, depth: usize) {
        if let Ok(mut usage) = self.resource_usage.lock() {
            usage.update_recursion_depth(depth);
        }
    }

    /// Increment operation count
    pub fn increment_operations(&self) {
        if let Ok(mut usage) = self.resource_usage.lock() {
            usage.increment_operations();
        }
    }

    /// Perform comprehensive security check
    pub fn check_all(&self) -> Result<()> {
        self.check_execution_time()?;
        self.check_memory_usage()?;
        self.check_recursion_depth()?;
        Ok(())
    }

    /// Get current resource usage
    pub fn get_resource_usage(&self) -> ResourceUsage {
        self.resource_usage.lock().unwrap().clone()
    }

    /// Get security configuration
    pub fn config(&self) -> &SecurityConfig {
        &self.config
    }

    /// Reset resource usage tracking
    pub fn reset_resource_usage(&self) {
        if let Ok(mut usage) = self.resource_usage.lock() {
            *usage = ResourceUsage::new();
        }
    }

    /// Create a security manager with strict configuration
    pub fn strict() -> Self {
        let config = SecurityConfig::new()
            .with_max_execution_time(Duration::from_millis(50))
            .with_max_memory_usage(1024 * 1024) // 1MB
            .with_max_recursion_depth(50)
            .with_max_string_length(1024)
            .with_max_array_length(100)
            .with_max_object_properties(50)
            .with_strict_mode(true)
            .with_allowed_functions(vec![
                "size".to_string(),
                "contains".to_string(),
                "startsWith".to_string(),
                "endsWith".to_string(),
                "type".to_string(),
                "isNull".to_string(),
            ]);
        Self::new(config)
    }

    /// Create a security manager with permissive configuration
    pub fn permissive() -> Self {
        let config = SecurityConfig::new()
            .with_max_execution_time(Duration::from_secs(5))
            .with_max_memory_usage(100 * 1024 * 1024) // 100MB
            .with_max_recursion_depth(1000)
            .with_max_string_length(10 * 1024 * 1024) // 10MB
            .with_max_array_length(100000)
            .with_max_object_properties(10000)
            .with_strict_mode(false);
        Self::new(config)
    }
}

impl Default for SecurityManager {
    fn default() -> Self {
        Self::new(SecurityConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::thread;

    #[test]
    fn test_security_config_creation() {
        let fixture = SecurityConfig::new()
            .with_max_execution_time(Duration::from_millis(200))
            .with_max_memory_usage(1024);

        let actual = fixture.max_execution_time();
        let expected = Duration::from_millis(200);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_security_config_function_whitelist() {
        let fixture = SecurityConfig::new()
            .with_allowed_functions(vec!["size".to_string(), "contains".to_string()]);

        let actual = fixture.is_function_allowed("size");
        let expected = true;
        assert_eq!(actual, expected);

        let actual = fixture.is_function_allowed("eval");
        let expected = false;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_security_config_function_blacklist() {
        let fixture = SecurityConfig::new()
            .with_denied_functions(vec!["eval".to_string(), "exec".to_string()]);

        let actual = fixture.is_function_allowed("eval");
        let expected = false;
        assert_eq!(actual, expected);

        let actual = fixture.is_function_allowed("size");
        let expected = true;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_resource_usage_tracking() {
        let mut fixture = ResourceUsage::new();
        fixture.increment_memory_usage(1024);
        fixture.increment_memory_usage(512);

        let actual = fixture.memory_usage();
        let expected = 1536;
        assert_eq!(actual, expected);

        let actual = fixture.peak_memory_usage();
        let expected = 1536;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_security_manager_memory_check() {
        let config = SecurityConfig::new().with_max_memory_usage(1024);
        let fixture = SecurityManager::new(config);

        fixture.increment_memory_usage(2048);
        let actual = fixture.check_memory_usage();
        assert!(actual.is_err());
    }

    #[test]
    fn test_security_manager_recursion_check() {
        let config = SecurityConfig::new().with_max_recursion_depth(5);
        let fixture = SecurityManager::new(config);

        fixture.update_recursion_depth(10);
        let actual = fixture.check_recursion_depth();
        assert!(actual.is_err());
    }

    #[test]
    fn test_security_manager_function_check() {
        let config = SecurityConfig::new().with_allowed_functions(vec!["size".to_string()]);
        let fixture = SecurityManager::new(config);

        let actual = fixture.check_function_call("eval");
        assert!(actual.is_err());

        let actual = fixture.check_function_call("size");
        assert!(actual.is_ok());
    }

    #[test]
    fn test_security_manager_timeout_check() {
        let config = SecurityConfig::new().with_max_execution_time(Duration::from_millis(10));
        let fixture = SecurityManager::new(config);

        // Wait longer than the timeout
        thread::sleep(Duration::from_millis(20));

        let actual = fixture.check_execution_time();
        assert!(actual.is_err());
    }

    #[test]
    fn test_security_manager_string_length_check() {
        let config = SecurityConfig::new().with_max_string_length(10);
        let fixture = SecurityManager::new(config);

        let actual = fixture.check_string_length(20);
        assert!(actual.is_err());

        let actual = fixture.check_string_length(5);
        assert!(actual.is_ok());
    }

    #[test]
    fn test_security_manager_array_length_check() {
        let config = SecurityConfig::new().with_max_array_length(10);
        let fixture = SecurityManager::new(config);

        let actual = fixture.check_array_length(20);
        assert!(actual.is_err());

        let actual = fixture.check_array_length(5);
        assert!(actual.is_ok());
    }

    #[test]
    fn test_security_manager_strict_preset() {
        let fixture = SecurityManager::strict();
        let actual = fixture.config().is_strict_mode();
        let expected = true;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_security_manager_permissive_preset() {
        let fixture = SecurityManager::permissive();
        let actual = fixture.config().max_execution_time();
        let expected = Duration::from_secs(5);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_resource_usage_elapsed_time() {
        let fixture = ResourceUsage::new();
        thread::sleep(Duration::from_millis(10));
        let actual = fixture.elapsed_time().unwrap();
        assert!(actual >= Duration::from_millis(10));
    }
}
