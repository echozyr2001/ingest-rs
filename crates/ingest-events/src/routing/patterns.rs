//! Event pattern matching for routing

use crate::{Result, error::EventError};
use ingest_core::Event;
use regex::Regex;
use std::collections::HashMap;

/// Pattern matching strategy
#[derive(Debug, Clone, PartialEq)]
pub enum PatternType {
    /// Exact string match
    Exact,
    /// Glob pattern matching (*, ?)
    Glob,
    /// Regular expression matching
    Regex,
    /// Prefix matching
    Prefix,
    /// Suffix matching
    Suffix,
    /// Contains matching
    Contains,
}

/// Event pattern for matching events
#[derive(Debug, Clone)]
pub struct EventPattern {
    /// Pattern type
    pub pattern_type: PatternType,
    /// Pattern string
    pub pattern: String,
    /// Compiled regex (if applicable)
    regex: Option<Regex>,
    /// Case sensitive matching
    pub case_sensitive: bool,
}

impl EventPattern {
    /// Create new exact pattern
    pub fn exact(pattern: impl Into<String>) -> Self {
        Self {
            pattern_type: PatternType::Exact,
            pattern: pattern.into(),
            regex: None,
            case_sensitive: true,
        }
    }

    /// Create new glob pattern
    pub fn glob(pattern: impl Into<String>) -> Self {
        Self {
            pattern_type: PatternType::Glob,
            pattern: pattern.into(),
            regex: None,
            case_sensitive: true,
        }
    }

    /// Create new regex pattern
    pub fn regex(pattern: impl Into<String>) -> Result<Self> {
        let pattern_str = pattern.into();
        let regex = Regex::new(&pattern_str)
            .map_err(|e| EventError::pattern_match(format!("Invalid regex: {e}")))?;

        Ok(Self {
            pattern_type: PatternType::Regex,
            pattern: pattern_str,
            regex: Some(regex),
            case_sensitive: true,
        })
    }

    /// Create new prefix pattern
    pub fn prefix(pattern: impl Into<String>) -> Self {
        Self {
            pattern_type: PatternType::Prefix,
            pattern: pattern.into(),
            regex: None,
            case_sensitive: true,
        }
    }

    /// Create new suffix pattern
    pub fn suffix(pattern: impl Into<String>) -> Self {
        Self {
            pattern_type: PatternType::Suffix,
            pattern: pattern.into(),
            regex: None,
            case_sensitive: true,
        }
    }

    /// Create new contains pattern
    pub fn contains(pattern: impl Into<String>) -> Self {
        Self {
            pattern_type: PatternType::Contains,
            pattern: pattern.into(),
            regex: None,
            case_sensitive: true,
        }
    }

    /// Set case sensitivity
    pub fn case_sensitive(mut self, sensitive: bool) -> Self {
        self.case_sensitive = sensitive;
        self
    }

    /// Check if event matches this pattern
    pub fn matches(&self, event: &Event) -> bool {
        self.matches_string(event.name())
    }

    /// Check if string matches this pattern
    pub fn matches_string(&self, text: &str) -> bool {
        let (pattern, text) = if self.case_sensitive {
            (self.pattern.as_str(), text)
        } else {
            // For case insensitive, we'll need to convert both to lowercase
            return self.matches_string_case_insensitive(text);
        };

        match self.pattern_type {
            PatternType::Exact => pattern == text,
            PatternType::Glob => self.matches_glob(pattern, text),
            PatternType::Regex => {
                if let Some(ref regex) = self.regex {
                    regex.is_match(text)
                } else {
                    false
                }
            }
            PatternType::Prefix => text.starts_with(pattern),
            PatternType::Suffix => text.ends_with(pattern),
            PatternType::Contains => text.contains(pattern),
        }
    }

    /// Case insensitive matching
    fn matches_string_case_insensitive(&self, text: &str) -> bool {
        let pattern_lower = self.pattern.to_lowercase();
        let text_lower = text.to_lowercase();

        match self.pattern_type {
            PatternType::Exact => pattern_lower == text_lower,
            PatternType::Glob => self.matches_glob(&pattern_lower, &text_lower),
            PatternType::Regex => {
                // For regex, we need to create a case-insensitive regex
                if let Ok(regex) = Regex::new(&format!("(?i){}", self.pattern)) {
                    regex.is_match(text)
                } else {
                    false
                }
            }
            PatternType::Prefix => text_lower.starts_with(&pattern_lower),
            PatternType::Suffix => text_lower.ends_with(&pattern_lower),
            PatternType::Contains => text_lower.contains(&pattern_lower),
        }
    }

    /// Simple glob matching implementation
    fn matches_glob(&self, pattern: &str, text: &str) -> bool {
        // Convert glob pattern to regex
        let regex_pattern = self.glob_to_regex(pattern);
        if let Ok(regex) = Regex::new(&regex_pattern) {
            regex.is_match(text)
        } else {
            false
        }
    }

    /// Convert glob pattern to regex
    fn glob_to_regex(&self, pattern: &str) -> String {
        let mut regex = String::new();
        regex.push('^');

        for ch in pattern.chars() {
            match ch {
                '*' => regex.push_str(".*"),
                '?' => regex.push('.'),
                '.' | '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '+' | '\\' => {
                    regex.push('\\');
                    regex.push(ch);
                }
                _ => regex.push(ch),
            }
        }

        regex.push('$');
        regex
    }
}

/// Pattern matcher for multiple patterns
#[derive(Debug)]
pub struct PatternMatcher {
    /// Patterns with their associated data
    patterns: Vec<(EventPattern, String)>,
    /// Cache for compiled patterns
    cache: HashMap<String, bool>,
}

impl PatternMatcher {
    /// Create new pattern matcher
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
            cache: HashMap::new(),
        }
    }

    /// Add pattern with associated data
    pub fn add_pattern(mut self, pattern: EventPattern, data: impl Into<String>) -> Self {
        self.patterns.push((pattern, data.into()));
        self
    }

    /// Find all matching patterns for an event
    pub fn find_matches(&mut self, event: &Event) -> Vec<String> {
        let mut matches = Vec::new();

        for (pattern, data) in &self.patterns {
            if pattern.matches(event) {
                matches.push(data.clone());
            }
        }

        matches
    }

    /// Find first matching pattern for an event
    pub fn find_first_match(&mut self, event: &Event) -> Option<String> {
        for (pattern, data) in &self.patterns {
            if pattern.matches(event) {
                return Some(data.clone());
            }
        }
        None
    }

    /// Check if any pattern matches
    pub fn has_match(&mut self, event: &Event) -> bool {
        self.patterns
            .iter()
            .any(|(pattern, _)| pattern.matches(event))
    }

    /// Get number of patterns
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    /// Clear all patterns
    pub fn clear(&mut self) {
        self.patterns.clear();
        self.cache.clear();
    }
}

impl Default for PatternMatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Create common event patterns
pub fn create_common_patterns() -> Vec<EventPattern> {
    vec![
        EventPattern::glob("user.*"),
        EventPattern::glob("admin.*"),
        EventPattern::glob("system.*"),
        EventPattern::prefix("api."),
        EventPattern::suffix(".error"),
        EventPattern::contains("login"),
        EventPattern::regex(r"^[a-z]+\.[a-z]+$").unwrap(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ingest_core::Event;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn create_test_event(name: &str) -> Event {
        Event::new(name, json!({}))
    }

    #[test]
    fn test_exact_pattern() {
        let fixture = EventPattern::exact("user.login");
        let event = create_test_event("user.login");
        assert!(fixture.matches(&event));

        let event2 = create_test_event("user.logout");
        assert!(!fixture.matches(&event2));
    }

    #[test]
    fn test_glob_pattern() {
        let fixture = EventPattern::glob("user.*");
        let event1 = create_test_event("user.login");
        let event2 = create_test_event("user.logout");
        let event3 = create_test_event("admin.login");

        assert!(fixture.matches(&event1));
        assert!(fixture.matches(&event2));
        assert!(!fixture.matches(&event3));
    }

    #[test]
    fn test_regex_pattern() {
        let fixture = EventPattern::regex(r"^user\.(login|logout)$").unwrap();
        let event1 = create_test_event("user.login");
        let event2 = create_test_event("user.logout");
        let event3 = create_test_event("user.register");

        assert!(fixture.matches(&event1));
        assert!(fixture.matches(&event2));
        assert!(!fixture.matches(&event3));
    }

    #[test]
    fn test_prefix_pattern() {
        let fixture = EventPattern::prefix("user.");
        let event1 = create_test_event("user.login");
        let event2 = create_test_event("user.logout");
        let event3 = create_test_event("admin.login");

        assert!(fixture.matches(&event1));
        assert!(fixture.matches(&event2));
        assert!(!fixture.matches(&event3));
    }

    #[test]
    fn test_suffix_pattern() {
        let fixture = EventPattern::suffix(".error");
        let event1 = create_test_event("user.login.error");
        let event2 = create_test_event("system.error");
        let event3 = create_test_event("user.login");

        assert!(fixture.matches(&event1));
        assert!(fixture.matches(&event2));
        assert!(!fixture.matches(&event3));
    }

    #[test]
    fn test_contains_pattern() {
        let fixture = EventPattern::contains("login");
        let event1 = create_test_event("user.login");
        let event2 = create_test_event("admin.login.success");
        let event3 = create_test_event("user.logout");

        assert!(fixture.matches(&event1));
        assert!(fixture.matches(&event2));
        assert!(!fixture.matches(&event3));
    }

    #[test]
    fn test_case_insensitive_pattern() {
        let fixture = EventPattern::exact("USER.LOGIN").case_sensitive(false);
        let event = create_test_event("user.login");
        assert!(fixture.matches(&event));
    }

    #[test]
    fn test_invalid_regex_pattern() {
        let fixture = EventPattern::regex("[invalid");
        assert!(fixture.is_err());
    }

    #[test]
    fn test_pattern_matcher_creation() {
        let fixture = PatternMatcher::new();
        assert_eq!(fixture.pattern_count(), 0);
    }

    #[test]
    fn test_pattern_matcher_add_pattern() {
        let pattern = EventPattern::exact("user.login");
        let fixture = PatternMatcher::new().add_pattern(pattern, "user-handler");
        assert_eq!(fixture.pattern_count(), 1);
    }

    #[test]
    fn test_pattern_matcher_find_matches() {
        let mut fixture = PatternMatcher::new()
            .add_pattern(EventPattern::glob("user.*"), "user-handler")
            .add_pattern(EventPattern::suffix(".login"), "login-handler");

        let event = create_test_event("user.login");
        let actual = fixture.find_matches(&event);
        assert_eq!(actual.len(), 2);
        assert!(actual.contains(&"user-handler".to_string()));
        assert!(actual.contains(&"login-handler".to_string()));
    }

    #[test]
    fn test_pattern_matcher_find_first_match() {
        let mut fixture = PatternMatcher::new()
            .add_pattern(EventPattern::glob("user.*"), "user-handler")
            .add_pattern(EventPattern::suffix(".login"), "login-handler");

        let event = create_test_event("user.login");
        let actual = fixture.find_first_match(&event);
        assert_eq!(actual, Some("user-handler".to_string()));
    }

    #[test]
    fn test_pattern_matcher_has_match() {
        let mut fixture =
            PatternMatcher::new().add_pattern(EventPattern::exact("user.login"), "handler");

        let event1 = create_test_event("user.login");
        let event2 = create_test_event("user.logout");

        assert!(fixture.has_match(&event1));
        assert!(!fixture.has_match(&event2));
    }

    #[test]
    fn test_pattern_matcher_clear() {
        let mut fixture =
            PatternMatcher::new().add_pattern(EventPattern::exact("user.login"), "handler");

        assert_eq!(fixture.pattern_count(), 1);
        fixture.clear();
        assert_eq!(fixture.pattern_count(), 0);
    }

    #[test]
    fn test_glob_to_regex() {
        let pattern = EventPattern::glob("test");
        assert_eq!(pattern.glob_to_regex("user.*"), "^user\\..*$");
        assert_eq!(pattern.glob_to_regex("test?"), "^test.$");
        assert_eq!(pattern.glob_to_regex("a.b*c"), "^a\\.b.*c$");
    }

    #[test]
    fn test_create_common_patterns() {
        let fixture = create_common_patterns();
        assert!(!fixture.is_empty());

        // Test that the patterns work
        let event = create_test_event("user.login");
        let user_pattern = &fixture[0]; // user.*
        assert!(user_pattern.matches(&event));
    }

    #[test]
    fn test_pattern_serialization() {
        let pattern = EventPattern::exact("user.login");
        // Test that patterns can be cloned (required for serialization)
        let _cloned = pattern.clone();
    }
}
