//! Event routing rules engine

use crate::{
    Result,
    routing::patterns::{EventPattern, PatternMatcher},
};
use ingest_core::Event;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Route destination
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteDestination {
    /// Destination identifier
    pub id: String,
    /// Destination type (queue, handler, webhook, etc.)
    pub destination_type: String,
    /// Destination configuration
    pub config: HashMap<String, String>,
    /// Priority (higher = more priority)
    pub priority: i32,
    /// Whether this destination is enabled
    pub enabled: bool,
}

impl RouteDestination {
    /// Create new route destination
    pub fn new(id: impl Into<String>, destination_type: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            destination_type: destination_type.into(),
            config: HashMap::new(),
            priority: 0,
            enabled: true,
        }
    }

    /// Set configuration value
    pub fn with_config(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config.insert(key.into(), value.into());
        self
    }

    /// Set priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Enable or disable destination
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Get configuration value
    pub fn get_config(&self, key: &str) -> Option<&String> {
        self.config.get(key)
    }
}

/// Routing rule condition
#[derive(Debug, Clone)]
pub enum RuleCondition {
    /// Event name matches pattern
    EventName(EventPattern),
    /// Event source matches pattern
    EventSource(EventPattern),
    /// Event user matches pattern
    EventUser(EventPattern),
    /// Event data contains field
    DataField(String),
    /// Event data field matches pattern
    DataFieldPattern(String, EventPattern),
    /// Event metadata contains key
    MetadataKey(String),
    /// Event metadata key matches pattern
    MetadataKeyPattern(String, EventPattern),
    /// Logical AND of conditions
    And(Vec<RuleCondition>),
    /// Logical OR of conditions
    Or(Vec<RuleCondition>),
    /// Logical NOT of condition
    Not(Box<RuleCondition>),
}

impl RuleCondition {
    /// Check if event matches this condition
    pub fn matches(&self, event: &Event) -> bool {
        match self {
            Self::EventName(pattern) => pattern.matches(event),
            Self::EventSource(pattern) => event
                .source
                .as_ref()
                .map(|s| pattern.matches_string(s))
                .unwrap_or(false),
            Self::EventUser(pattern) => event
                .user
                .as_ref()
                .map(|u| pattern.matches_string(u))
                .unwrap_or(false),
            Self::DataField(field) => event
                .data()
                .data
                .as_object()
                .map(|obj| obj.contains_key(field))
                .unwrap_or(false),
            Self::DataFieldPattern(field, pattern) => event
                .data()
                .data
                .as_object()
                .and_then(|obj| obj.get(field))
                .and_then(|v| v.as_str())
                .map(|s| pattern.matches_string(s))
                .unwrap_or(false),
            Self::MetadataKey(key) => event.data().metadata.contains_key(key),
            Self::MetadataKeyPattern(key, pattern) => event
                .data()
                .metadata
                .get(key)
                .map(|v| pattern.matches_string(v))
                .unwrap_or(false),
            Self::And(conditions) => conditions.iter().all(|c| c.matches(event)),
            Self::Or(conditions) => conditions.iter().any(|c| c.matches(event)),
            Self::Not(condition) => !condition.matches(event),
        }
    }
}

/// Routing rule
#[derive(Debug, Clone)]
pub struct RoutingRule {
    /// Rule identifier
    pub id: String,
    /// Rule name
    pub name: String,
    /// Rule description
    pub description: Option<String>,
    /// Rule condition
    pub condition: RuleCondition,
    /// Destinations for matching events
    pub destinations: Vec<RouteDestination>,
    /// Rule priority (higher = evaluated first)
    pub priority: i32,
    /// Whether this rule is enabled
    pub enabled: bool,
    /// Whether to stop processing after this rule matches
    pub stop_on_match: bool,
}

impl RoutingRule {
    /// Create new routing rule
    pub fn new(id: impl Into<String>, name: impl Into<String>, condition: RuleCondition) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            condition,
            destinations: Vec::new(),
            priority: 0,
            enabled: true,
            stop_on_match: false,
        }
    }

    /// Set description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add destination
    pub fn with_destination(mut self, destination: RouteDestination) -> Self {
        self.destinations.push(destination);
        self
    }

    /// Set priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Enable or disable rule
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set stop on match behavior
    pub fn stop_on_match(mut self, stop: bool) -> Self {
        self.stop_on_match = stop;
        self
    }

    /// Check if event matches this rule
    pub fn matches(&self, event: &Event) -> bool {
        self.enabled && self.condition.matches(event)
    }

    /// Get enabled destinations
    pub fn enabled_destinations(&self) -> Vec<&RouteDestination> {
        self.destinations.iter().filter(|d| d.enabled).collect()
    }
}

/// Routing engine
#[derive(Debug)]
pub struct RoutingEngine {
    /// Routing rules
    rules: Vec<RoutingRule>,
    /// Pattern matcher for quick lookups
    pattern_matcher: PatternMatcher,
    /// Statistics
    stats: RoutingStats,
}

/// Routing statistics
#[derive(Debug, Clone, Default)]
pub struct RoutingStats {
    /// Total events processed
    pub events_processed: u64,
    /// Events routed successfully
    pub events_routed: u64,
    /// Events with no matching rules
    pub events_unrouted: u64,
    /// Rule match counts
    pub rule_matches: HashMap<String, u64>,
}

/// Routing result
#[derive(Debug, Clone)]
pub struct RoutingResult {
    /// Matched destinations
    pub destinations: Vec<RouteDestination>,
    /// Matched rule IDs
    pub matched_rules: Vec<String>,
    /// Whether routing should stop
    pub stop_processing: bool,
}

impl RoutingEngine {
    /// Create new routing engine
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            pattern_matcher: PatternMatcher::new(),
            stats: RoutingStats::default(),
        }
    }

    /// Add routing rule
    pub fn add_rule(mut self, rule: RoutingRule) -> Self {
        // Add rule patterns to pattern matcher for quick lookups
        self.add_patterns_from_condition(&rule.condition, &rule.id);

        self.rules.push(rule);
        self.sort_rules();
        self
    }

    /// Extract patterns from rule condition and add to pattern matcher
    fn add_patterns_from_condition(&mut self, condition: &RuleCondition, rule_id: &str) {
        match condition {
            RuleCondition::EventName(pattern) => {
                self.pattern_matcher =
                    std::mem::take(&mut self.pattern_matcher).add_pattern(pattern.clone(), rule_id);
            }
            RuleCondition::EventSource(pattern) => {
                self.pattern_matcher =
                    std::mem::take(&mut self.pattern_matcher).add_pattern(pattern.clone(), rule_id);
            }
            RuleCondition::EventUser(pattern) => {
                self.pattern_matcher =
                    std::mem::take(&mut self.pattern_matcher).add_pattern(pattern.clone(), rule_id);
            }
            RuleCondition::DataFieldPattern(_, pattern) => {
                self.pattern_matcher =
                    std::mem::take(&mut self.pattern_matcher).add_pattern(pattern.clone(), rule_id);
            }
            RuleCondition::MetadataKeyPattern(_, pattern) => {
                self.pattern_matcher =
                    std::mem::take(&mut self.pattern_matcher).add_pattern(pattern.clone(), rule_id);
            }
            RuleCondition::And(conditions) | RuleCondition::Or(conditions) => {
                for condition in conditions {
                    self.add_patterns_from_condition(condition, rule_id);
                }
            }
            RuleCondition::Not(condition) => {
                self.add_patterns_from_condition(condition, rule_id);
            }
            // Other conditions don't have patterns to extract
            _ => {}
        }
    }

    /// Route an event to destinations
    pub fn route(&mut self, event: &Event) -> Result<RoutingResult> {
        self.stats.events_processed += 1;

        let mut result = RoutingResult {
            destinations: Vec::new(),
            matched_rules: Vec::new(),
            stop_processing: false,
        };

        // Quick check if any patterns match before processing rules
        if !self.pattern_matcher.has_match(event) {
            self.stats.events_unrouted += 1;
            return Ok(result);
        }

        for rule in &self.rules {
            if rule.matches(event) {
                // Update statistics
                *self.stats.rule_matches.entry(rule.id.clone()).or_insert(0) += 1;

                result.matched_rules.push(rule.id.clone());

                // Add enabled destinations
                for destination in rule.enabled_destinations() {
                    result.destinations.push(destination.clone());
                }

                // Check if we should stop processing
                if rule.stop_on_match {
                    result.stop_processing = true;
                    break;
                }
            }
        }

        if result.destinations.is_empty() {
            self.stats.events_unrouted += 1;
        } else {
            self.stats.events_routed += 1;
        }

        Ok(result)
    }

    /// Get routing statistics
    pub fn stats(&self) -> &RoutingStats {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = RoutingStats::default();
    }

    /// Get rule by ID
    pub fn get_rule(&self, id: &str) -> Option<&RoutingRule> {
        self.rules.iter().find(|r| r.id == id)
    }

    /// Remove rule by ID
    pub fn remove_rule(&mut self, id: &str) -> bool {
        let original_len = self.rules.len();
        self.rules.retain(|r| r.id != id);
        self.rules.len() != original_len
    }

    /// Get all rules
    pub fn rules(&self) -> &[RoutingRule] {
        &self.rules
    }

    /// Sort rules by priority (highest first)
    fn sort_rules(&mut self) {
        self.rules.sort_by(|a, b| b.priority.cmp(&a.priority));
    }
}

impl Default for RoutingEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a basic routing engine with common rules
pub fn create_basic_routing_engine() -> RoutingEngine {
    let user_queue =
        RouteDestination::new("user-queue", "queue").with_config("queue_name", "user_events");

    let admin_queue =
        RouteDestination::new("admin-queue", "queue").with_config("queue_name", "admin_events");

    let error_webhook = RouteDestination::new("error-webhook", "webhook")
        .with_config("url", "https://api.example.com/errors");

    let user_rule = RoutingRule::new(
        "user-events",
        "User Events",
        RuleCondition::EventName(EventPattern::glob("user.*")),
    )
    .with_destination(user_queue);

    let admin_rule = RoutingRule::new(
        "admin-events",
        "Admin Events",
        RuleCondition::EventName(EventPattern::glob("admin.*")),
    )
    .with_destination(admin_queue);

    let error_rule = RoutingRule::new(
        "error-events",
        "Error Events",
        RuleCondition::EventName(EventPattern::suffix(".error")),
    )
    .with_destination(error_webhook)
    .with_priority(100); // High priority for errors

    RoutingEngine::new()
        .add_rule(user_rule)
        .add_rule(admin_rule)
        .add_rule(error_rule)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::patterns::EventPattern;
    use ingest_core::Event;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn create_test_event(name: &str) -> Event {
        Event::new(name, json!({"user_id": "123"}))
    }

    #[test]
    fn test_route_destination_creation() {
        let fixture = RouteDestination::new("test-queue", "queue");
        assert_eq!(fixture.id, "test-queue");
        assert_eq!(fixture.destination_type, "queue");
        assert!(fixture.enabled);
        assert_eq!(fixture.priority, 0);
    }

    #[test]
    fn test_route_destination_with_config() {
        let fixture = RouteDestination::new("test-queue", "queue")
            .with_config("queue_name", "events")
            .with_priority(10)
            .enabled(false);

        assert_eq!(
            fixture.get_config("queue_name"),
            Some(&"events".to_string())
        );
        assert_eq!(fixture.priority, 10);
        assert!(!fixture.enabled);
    }

    #[test]
    fn test_rule_condition_event_name() {
        let condition = RuleCondition::EventName(EventPattern::exact("user.login"));
        let event = create_test_event("user.login");
        assert!(condition.matches(&event));

        let event2 = create_test_event("user.logout");
        assert!(!condition.matches(&event2));
    }

    #[test]
    fn test_rule_condition_data_field() {
        let condition = RuleCondition::DataField("user_id".to_string());
        let event = create_test_event("test.event");
        assert!(condition.matches(&event));

        let event2 = Event::new("test.event", json!({"other_field": "value"}));
        assert!(!condition.matches(&event2));
    }

    #[test]
    fn test_rule_condition_and() {
        let condition = RuleCondition::And(vec![
            RuleCondition::EventName(EventPattern::prefix("user.")),
            RuleCondition::DataField("user_id".to_string()),
        ]);

        let event = create_test_event("user.login");
        assert!(condition.matches(&event));

        let event2 = Event::new("admin.login", json!({"user_id": "123"}));
        assert!(!condition.matches(&event2));
    }

    #[test]
    fn test_rule_condition_or() {
        let condition = RuleCondition::Or(vec![
            RuleCondition::EventName(EventPattern::exact("user.login")),
            RuleCondition::EventName(EventPattern::exact("user.logout")),
        ]);

        let event1 = create_test_event("user.login");
        let event2 = create_test_event("user.logout");
        let event3 = create_test_event("user.register");

        assert!(condition.matches(&event1));
        assert!(condition.matches(&event2));
        assert!(!condition.matches(&event3));
    }

    #[test]
    fn test_rule_condition_not() {
        let condition = RuleCondition::Not(Box::new(RuleCondition::EventName(
            EventPattern::prefix("admin."),
        )));

        let event1 = create_test_event("user.login");
        let event2 = create_test_event("admin.login");

        assert!(condition.matches(&event1));
        assert!(!condition.matches(&event2));
    }

    #[test]
    fn test_routing_rule_creation() {
        let condition = RuleCondition::EventName(EventPattern::exact("user.login"));
        let fixture = RoutingRule::new("test-rule", "Test Rule", condition);

        assert_eq!(fixture.id, "test-rule");
        assert_eq!(fixture.name, "Test Rule");
        assert!(fixture.enabled);
        assert!(!fixture.stop_on_match);
    }

    #[test]
    fn test_routing_rule_with_destination() {
        let condition = RuleCondition::EventName(EventPattern::exact("user.login"));
        let destination = RouteDestination::new("test-queue", "queue");
        let fixture =
            RoutingRule::new("test-rule", "Test Rule", condition).with_destination(destination);

        assert_eq!(fixture.destinations.len(), 1);
    }

    #[test]
    fn test_routing_rule_matches() {
        let condition = RuleCondition::EventName(EventPattern::exact("user.login"));
        let fixture = RoutingRule::new("test-rule", "Test Rule", condition);

        let event1 = create_test_event("user.login");
        let event2 = create_test_event("user.logout");

        assert!(fixture.matches(&event1));
        assert!(!fixture.matches(&event2));
    }

    #[test]
    fn test_routing_rule_disabled() {
        let condition = RuleCondition::EventName(EventPattern::exact("user.login"));
        let fixture = RoutingRule::new("test-rule", "Test Rule", condition).enabled(false);

        let event = create_test_event("user.login");
        assert!(!fixture.matches(&event));
    }

    #[test]
    fn test_routing_engine_creation() {
        let fixture = RoutingEngine::new();
        assert_eq!(fixture.rules().len(), 0);
        assert_eq!(fixture.stats().events_processed, 0);
    }

    #[test]
    fn test_routing_engine_add_rule() {
        let condition = RuleCondition::EventName(EventPattern::exact("user.login"));
        let rule = RoutingRule::new("test-rule", "Test Rule", condition);
        let fixture = RoutingEngine::new().add_rule(rule);

        assert_eq!(fixture.rules().len(), 1);
    }

    #[test]
    fn test_routing_engine_route() {
        let condition = RuleCondition::EventName(EventPattern::exact("user.login"));
        let destination = RouteDestination::new("test-queue", "queue");
        let rule =
            RoutingRule::new("test-rule", "Test Rule", condition).with_destination(destination);

        let mut fixture = RoutingEngine::new().add_rule(rule);
        let event = create_test_event("user.login");
        let actual = fixture.route(&event).unwrap();

        assert_eq!(actual.destinations.len(), 1);
        assert_eq!(actual.matched_rules.len(), 1);
        assert!(!actual.stop_processing);
    }

    #[test]
    fn test_routing_engine_no_match() {
        let condition = RuleCondition::EventName(EventPattern::exact("admin.login"));
        let destination = RouteDestination::new("admin-queue", "queue");
        let rule =
            RoutingRule::new("admin-rule", "Admin Rule", condition).with_destination(destination);

        let mut fixture = RoutingEngine::new().add_rule(rule);
        let event = create_test_event("user.login");
        let actual = fixture.route(&event).unwrap();

        assert_eq!(actual.destinations.len(), 0);
        assert_eq!(actual.matched_rules.len(), 0);
        assert_eq!(fixture.stats().events_unrouted, 1);
    }

    #[test]
    fn test_routing_engine_stop_on_match() {
        let condition1 = RuleCondition::EventName(EventPattern::glob("user.*"));
        let condition2 = RuleCondition::EventName(EventPattern::exact("user.login"));

        let rule1 = RoutingRule::new("rule1", "Rule 1", condition1)
            .with_destination(RouteDestination::new("queue1", "queue"))
            .stop_on_match(true);

        let rule2 = RoutingRule::new("rule2", "Rule 2", condition2)
            .with_destination(RouteDestination::new("queue2", "queue"));

        let mut fixture = RoutingEngine::new().add_rule(rule1).add_rule(rule2);

        let event = create_test_event("user.login");
        let actual = fixture.route(&event).unwrap();

        assert_eq!(actual.matched_rules.len(), 1);
        assert!(actual.stop_processing);
    }

    #[test]
    fn test_routing_engine_priority() {
        let condition = RuleCondition::EventName(EventPattern::exact("user.login"));

        let rule1 = RoutingRule::new("rule1", "Rule 1", condition.clone()).with_priority(1);

        let rule2 = RoutingRule::new("rule2", "Rule 2", condition).with_priority(10);

        let fixture = RoutingEngine::new().add_rule(rule1).add_rule(rule2);

        // Rules should be sorted by priority (highest first)
        assert_eq!(fixture.rules()[0].id, "rule2");
        assert_eq!(fixture.rules()[1].id, "rule1");
    }

    #[test]
    fn test_create_basic_routing_engine() {
        let fixture = create_basic_routing_engine();
        assert!(!fixture.rules().is_empty());

        // Test that it can route user events
        let mut engine = fixture;
        let event = create_test_event("user.login");
        let result = engine.route(&event).unwrap();
        assert!(!result.destinations.is_empty());
    }
}
