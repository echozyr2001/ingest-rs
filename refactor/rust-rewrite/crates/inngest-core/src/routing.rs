use crate::{Event, Function, Result};
use std::collections::HashMap;
use uuid::Uuid;

/// Event routing engine that matches events to functions
#[derive(Debug, Clone)]
pub struct EventRouter {
    /// Registered functions indexed by their triggers
    function_registry: HashMap<String, Vec<Function>>,
    /// Event pattern matchers
    pattern_matchers: HashMap<String, EventPattern>,
}

/// Represents different event matching patterns
#[derive(Debug, Clone)]
pub enum EventPattern {
    /// Exact event name match
    Exact(String),
    /// Pattern with wildcards
    Pattern(String),
    /// Expression-based matching
    Expression(String),
}

/// Result of event routing
#[derive(Debug, Clone)]
pub struct RoutingResult {
    pub event: Event,
    pub matched_functions: Vec<Function>,
    pub routing_metadata: HashMap<String, serde_json::Value>,
}

impl EventRouter {
    /// Create a new event router
    pub fn new() -> Self {
        Self {
            function_registry: HashMap::new(),
            pattern_matchers: HashMap::new(),
        }
    }

    /// Register a function with its triggers
    pub fn register_function(&mut self, function: Function) -> Result<()> {
        for trigger in function.get_triggers() {
            match trigger {
                crate::function::Trigger::Event { event, .. } => {
                    // Register function for specific event
                    self.function_registry
                        .entry(event.clone())
                        .or_insert_with(Vec::new)
                        .push(function.clone());

                    // Create pattern matcher
                    let pattern = if event.contains('*') {
                        EventPattern::Pattern(event.clone())
                    } else {
                        EventPattern::Exact(event.clone())
                    };
                    self.pattern_matchers.insert(event.clone(), pattern);
                }
                crate::function::Trigger::Cron { .. } => {
                    // Register function for all events (cron or other triggers)
                    self.function_registry
                        .entry("*".to_string())
                        .or_insert_with(Vec::new)
                        .push(function.clone());
                }
            }
        }
        Ok(())
    }

    /// Route an event to matching functions
    pub fn route_event(&self, event: &Event) -> Result<RoutingResult> {
        let mut matched_functions = Vec::new();
        let mut routing_metadata = HashMap::new();

        // Debug info
        routing_metadata.insert(
            "pattern_matchers_count".to_string(),
            serde_json::Value::Number(self.pattern_matchers.len().into()),
        );

        // Direct match
        if let Some(functions) = self.function_registry.get(&event.name) {
            matched_functions.extend(functions.clone());
            routing_metadata.insert("direct_match".to_string(), serde_json::Value::Bool(true));
        }

        // Pattern matching
        let mut pattern_matches = Vec::new();
        for (pattern_name, pattern) in &self.pattern_matchers {
            if self.matches_pattern(pattern, &event.name) {
                pattern_matches.push(pattern_name.clone());
                if let Some(functions) = self.function_registry.get(pattern_name) {
                    for func in functions {
                        if !matched_functions.iter().any(|f| f.id == func.id) {
                            matched_functions.push(func.clone());
                        }
                    }
                }
            }
        }

        routing_metadata.insert(
            "pattern_matches".to_string(),
            serde_json::Value::Array(
                pattern_matches
                    .into_iter()
                    .map(|p| serde_json::Value::String(p))
                    .collect(),
            ),
        );

        // Add catch-all functions (those triggered by any event)
        if let Some(catch_all_functions) = self.function_registry.get("*") {
            for func in catch_all_functions {
                if !matched_functions.iter().any(|f| f.id == func.id) {
                    matched_functions.push(func.clone());
                }
            }
        }

        routing_metadata.insert(
            "matched_count".to_string(),
            serde_json::Value::Number(matched_functions.len().into()),
        );

        routing_metadata.insert(
            "event_id".to_string(),
            serde_json::Value::String(
                event
                    .id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
            ),
        );

        Ok(RoutingResult {
            event: event.clone(),
            matched_functions,
            routing_metadata,
        })
    }

    /// Check if an event name matches a pattern
    fn matches_pattern(&self, pattern: &EventPattern, event_name: &str) -> bool {
        match pattern {
            EventPattern::Exact(exact) => exact == event_name,
            EventPattern::Pattern(pattern_str) => {
                // Simple wildcard matching
                if pattern_str.ends_with("*") {
                    let prefix = &pattern_str[..pattern_str.len() - 1];
                    event_name.starts_with(prefix)
                } else if pattern_str.starts_with("*") {
                    let suffix = &pattern_str[1..];
                    event_name.ends_with(suffix)
                } else if pattern_str.contains("*") {
                    // More complex pattern matching could be implemented here
                    false
                } else {
                    pattern_str == event_name
                }
            }
            EventPattern::Expression(_expr) => {
                // TODO: Implement expression-based matching using a proper expression engine
                false
            }
        }
    }

    /// Get all registered functions
    pub fn get_all_functions(&self) -> Vec<Function> {
        let mut all_functions = Vec::new();
        for functions in self.function_registry.values() {
            for func in functions {
                if !all_functions.iter().any(|f: &Function| f.id == func.id) {
                    all_functions.push(func.clone());
                }
            }
        }
        all_functions
    }

    /// Get functions for a specific event name
    pub fn get_functions_for_event(&self, event_name: &str) -> Vec<Function> {
        self.function_registry
            .get(event_name)
            .cloned()
            .unwrap_or_default()
    }

    /// Remove a function from the registry
    pub fn unregister_function(&mut self, function_id: Uuid) -> Result<()> {
        for (_, functions) in &mut self.function_registry {
            functions.retain(|f| f.id != function_id);
        }
        Ok(())
    }

    /// Clear all registered functions
    pub fn clear(&mut self) {
        self.function_registry.clear();
        self.pattern_matchers.clear();
    }

    /// Get registry statistics
    pub fn get_stats(&self) -> HashMap<String, usize> {
        let mut stats = HashMap::new();
        stats.insert("total_patterns".to_string(), self.pattern_matchers.len());
        stats.insert(
            "total_functions".to_string(),
            self.get_all_functions().len(),
        );

        let total_registrations: usize = self
            .function_registry
            .values()
            .map(|funcs| funcs.len())
            .sum();
        stats.insert("total_registrations".to_string(), total_registrations);

        stats
    }
}

impl Default for EventRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_router_creation() {
        let router = EventRouter::new();
        assert_eq!(router.get_all_functions().len(), 0);
    }

    #[test]
    fn test_pattern_matching() {
        let router = EventRouter::new();

        // Test exact pattern matching
        let exact_pattern = EventPattern::Exact("user.created".to_string());
        assert!(router.matches_pattern(&exact_pattern, "user.created"));
        assert!(!router.matches_pattern(&exact_pattern, "user.updated"));

        // Test wildcard pattern matching
        let wildcard_pattern = EventPattern::Pattern("user.*".to_string());
        assert!(router.matches_pattern(&wildcard_pattern, "user.created"));
        assert!(router.matches_pattern(&wildcard_pattern, "user.updated"));
        assert!(!router.matches_pattern(&wildcard_pattern, "order.created"));
    }
}
