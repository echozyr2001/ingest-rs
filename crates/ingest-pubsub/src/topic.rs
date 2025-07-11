use crate::{PubSubError, Result, TopicConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// A topic in the pub-sub system
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Topic {
    /// Topic name
    pub name: String,
    /// Topic configuration
    pub config: TopicConfig,
}

impl Topic {
    /// Create a new topic
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            config: TopicConfig::new(name.clone()),
            name,
        }
    }

    /// Create a topic with configuration
    pub fn with_config(name: impl Into<String>, config: TopicConfig) -> Self {
        Self {
            name: name.into(),
            config,
        }
    }

    /// Get the topic name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the topic configuration
    pub fn config(&self) -> &TopicConfig {
        &self.config
    }

    /// Update the topic configuration
    pub fn update_config(&mut self, config: TopicConfig) {
        self.config = config;
    }

    /// Check if the topic is persistent
    pub fn is_persistent(&self) -> bool {
        self.config.persistent
    }

    /// Get the maximum number of messages
    pub fn max_messages(&self) -> Option<u64> {
        self.config.max_messages
    }

    /// Get the retention duration in seconds
    pub fn retention_seconds(&self) -> Option<u64> {
        self.config.retention_seconds
    }

    /// Get the default TTL in seconds
    pub fn default_ttl_seconds(&self) -> Option<u64> {
        self.config.default_ttl_seconds
    }
}

/// Topic pattern for subscription matching
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TopicPattern {
    /// Pattern string
    pub pattern: String,
}

impl TopicPattern {
    /// Create a new topic pattern
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
        }
    }

    /// Create an exact match pattern
    pub fn exact(topic: impl Into<String>) -> Self {
        Self::new(topic)
    }

    /// Create a wildcard pattern
    pub fn wildcard(prefix: impl Into<String>) -> Self {
        Self::new(format!("{}.*", prefix.into()))
    }

    /// Check if the pattern matches a topic
    pub fn matches(&self, topic: &str) -> bool {
        if self.pattern == "*" {
            return true;
        }

        if self.pattern.ends_with(".*") {
            let prefix = &self.pattern[..self.pattern.len() - 2];
            return topic.starts_with(prefix)
                && topic.len() > prefix.len()
                && topic.chars().nth(prefix.len()) == Some('.');
        }

        if self.pattern.contains('*') {
            return self.glob_match(topic);
        }

        self.pattern == topic
    }

    /// Perform glob-style matching
    fn glob_match(&self, topic: &str) -> bool {
        let pattern_parts: Vec<&str> = self.pattern.split('*').collect();
        if pattern_parts.is_empty() {
            return false;
        }

        let mut topic_pos = 0;

        // Check first part (before first *)
        if !pattern_parts[0].is_empty() {
            if !topic.starts_with(pattern_parts[0]) {
                return false;
            }
            topic_pos += pattern_parts[0].len();
        }

        // Check middle parts
        for part in &pattern_parts[1..pattern_parts.len() - 1] {
            if part.is_empty() {
                continue;
            }

            if let Some(pos) = topic[topic_pos..].find(part) {
                topic_pos += pos + part.len();
            } else {
                return false;
            }
        }

        // Check last part (after last *)
        if let Some(last_part) = pattern_parts.last() {
            if !last_part.is_empty() {
                return topic[topic_pos..].ends_with(last_part);
            }
        }

        true
    }

    /// Get the pattern string
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Check if the pattern is exact (no wildcards)
    pub fn is_exact(&self) -> bool {
        !self.pattern.contains('*')
    }

    /// Check if the pattern is a wildcard pattern
    pub fn is_wildcard(&self) -> bool {
        self.pattern.contains('*')
    }
}

impl From<&str> for TopicPattern {
    fn from(pattern: &str) -> Self {
        Self::new(pattern)
    }
}

impl From<String> for TopicPattern {
    fn from(pattern: String) -> Self {
        Self::new(pattern)
    }
}

impl std::fmt::Display for TopicPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.pattern)
    }
}

/// Topic manager for handling topic lifecycle
#[derive(Debug)]
pub struct TopicManager {
    /// Topics storage
    topics: Arc<RwLock<HashMap<String, Topic>>>,
}

impl TopicManager {
    /// Create a new topic manager
    pub fn new() -> Self {
        Self {
            topics: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a topic
    pub async fn create_topic(&self, name: impl Into<String>) -> Result<Topic> {
        let name = name.into();
        let topic = Topic::new(name.clone());

        let mut topics = self.topics.write().await;
        if topics.contains_key(&name) {
            return Err(PubSubError::topic(format!("Topic '{name}' already exists")));
        }

        topics.insert(name.clone(), topic.clone());
        info!("Created topic: {}", name);
        Ok(topic)
    }

    /// Create a topic with configuration
    pub async fn create_topic_with_config(
        &self,
        name: impl Into<String>,
        config: TopicConfig,
    ) -> Result<Topic> {
        let name = name.into();
        let topic = Topic::with_config(name.clone(), config);

        let mut topics = self.topics.write().await;
        if topics.contains_key(&name) {
            return Err(PubSubError::topic(format!("Topic '{name}' already exists")));
        }

        topics.insert(name.clone(), topic.clone());
        info!("Created topic with config: {}", name);
        Ok(topic)
    }

    /// Get a topic by name
    pub async fn get_topic(&self, name: &str) -> Result<Option<Topic>> {
        let topics = self.topics.read().await;
        Ok(topics.get(name).cloned())
    }

    /// Get or create a topic
    pub async fn get_or_create_topic(&self, name: impl Into<String>) -> Result<Topic> {
        let name = name.into();

        // Try to get existing topic first
        if let Some(topic) = self.get_topic(&name).await? {
            return Ok(topic);
        }

        // Create new topic
        self.create_topic(name).await
    }

    /// Update a topic configuration
    pub async fn update_topic(&self, name: &str, config: TopicConfig) -> Result<Topic> {
        let mut topics = self.topics.write().await;

        match topics.get_mut(name) {
            Some(topic) => {
                topic.update_config(config);
                info!("Updated topic: {}", name);
                Ok(topic.clone())
            }
            None => Err(PubSubError::topic(format!("Topic '{name}' not found"))),
        }
    }

    /// Delete a topic
    pub async fn delete_topic(&self, name: &str) -> Result<bool> {
        let mut topics = self.topics.write().await;

        match topics.remove(name) {
            Some(_) => {
                info!("Deleted topic: {}", name);
                Ok(true)
            }
            None => {
                warn!("Attempted to delete non-existent topic: {}", name);
                Ok(false)
            }
        }
    }

    /// List all topics
    pub async fn list_topics(&self) -> Result<Vec<Topic>> {
        let topics = self.topics.read().await;
        Ok(topics.values().cloned().collect())
    }

    /// List topic names
    pub async fn list_topic_names(&self) -> Result<Vec<String>> {
        let topics = self.topics.read().await;
        Ok(topics.keys().cloned().collect())
    }

    /// Check if a topic exists
    pub async fn topic_exists(&self, name: &str) -> Result<bool> {
        let topics = self.topics.read().await;
        Ok(topics.contains_key(name))
    }

    /// Get topics matching a pattern
    pub async fn get_topics_matching_pattern(&self, pattern: &TopicPattern) -> Result<Vec<Topic>> {
        let topics = self.topics.read().await;
        let matching_topics: Vec<Topic> = topics
            .values()
            .filter(|topic| pattern.matches(&topic.name))
            .cloned()
            .collect();

        debug!(
            "Found {} topics matching pattern '{}'",
            matching_topics.len(),
            pattern
        );
        Ok(matching_topics)
    }

    /// Get the number of topics
    pub async fn topic_count(&self) -> Result<usize> {
        let topics = self.topics.read().await;
        Ok(topics.len())
    }

    /// Clear all topics (use with caution)
    pub async fn clear_all_topics(&self) -> Result<usize> {
        let mut topics = self.topics.write().await;
        let count = topics.len();
        topics.clear();
        warn!("Cleared all {} topics", count);
        Ok(count)
    }
}

impl Default for TopicManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_topic_creation() {
        let name_fixture = "test.topic";
        let actual = Topic::new(name_fixture);

        assert_eq!(actual.name(), name_fixture);
        assert_eq!(actual.config().name, name_fixture);
        assert!(actual.is_persistent());
    }

    #[test]
    fn test_topic_with_config() {
        let name_fixture = "test.topic";
        let mut config_fixture = TopicConfig::new(name_fixture);
        config_fixture.persistent = false;
        config_fixture.max_messages = Some(1000);

        let actual = Topic::with_config(name_fixture, config_fixture.clone());

        assert_eq!(actual.name(), name_fixture);
        assert_eq!(actual.config(), &config_fixture);
        assert!(!actual.is_persistent());
        assert_eq!(actual.max_messages(), Some(1000));
    }

    #[test]
    fn test_topic_pattern_exact_match() {
        let pattern_fixture = TopicPattern::exact("test.topic");

        assert!(pattern_fixture.matches("test.topic"));
        assert!(!pattern_fixture.matches("test.other"));
        assert!(!pattern_fixture.matches("test.topic.sub"));
        assert!(pattern_fixture.is_exact());
        assert!(!pattern_fixture.is_wildcard());
    }

    #[test]
    fn test_topic_pattern_wildcard() {
        let pattern_fixture = TopicPattern::wildcard("test");

        assert!(pattern_fixture.matches("test.topic"));
        assert!(pattern_fixture.matches("test.other"));
        assert!(!pattern_fixture.matches("test"));
        assert!(!pattern_fixture.matches("other.topic"));
        assert!(!pattern_fixture.is_exact());
        assert!(pattern_fixture.is_wildcard());
    }

    #[test]
    fn test_topic_pattern_glob_matching() {
        let pattern_fixture = TopicPattern::new("test.*.completed");

        assert!(pattern_fixture.matches("test.function.completed"));
        assert!(pattern_fixture.matches("test.workflow.completed"));
        assert!(!pattern_fixture.matches("test.function.failed"));
        assert!(!pattern_fixture.matches("other.function.completed"));
    }

    #[test]
    fn test_topic_pattern_universal_wildcard() {
        let pattern_fixture = TopicPattern::new("*");

        assert!(pattern_fixture.matches("test.topic"));
        assert!(pattern_fixture.matches("any.topic"));
        assert!(pattern_fixture.matches("single"));
        assert!(pattern_fixture.is_wildcard());
    }

    #[test]
    fn test_topic_pattern_from_string() {
        let pattern_fixture = "test.*";
        let actual = TopicPattern::from(pattern_fixture);

        assert_eq!(actual.pattern(), pattern_fixture);
        assert!(actual.matches("test.topic"));
    }

    #[test]
    fn test_topic_pattern_display() {
        let pattern_fixture = TopicPattern::new("test.*");
        let actual = format!("{}", pattern_fixture);
        let expected = "test.*";
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn test_topic_manager_create_topic() {
        let fixture = TopicManager::new();
        let topic_name = "test.topic";

        let actual = fixture.create_topic(topic_name).await.unwrap();
        assert_eq!(actual.name(), topic_name);

        // Should fail to create duplicate
        let result = fixture.create_topic(topic_name).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_topic_manager_get_topic() {
        let fixture = TopicManager::new();
        let topic_name = "test.topic";

        // Should return None for non-existent topic
        let actual = fixture.get_topic(topic_name).await.unwrap();
        assert!(actual.is_none());

        // Create topic and get it
        fixture.create_topic(topic_name).await.unwrap();
        let actual = fixture.get_topic(topic_name).await.unwrap();
        assert!(actual.is_some());
        assert_eq!(actual.unwrap().name(), topic_name);
    }

    #[tokio::test]
    async fn test_topic_manager_get_or_create_topic() {
        let fixture = TopicManager::new();
        let topic_name = "test.topic";

        // Should create new topic
        let actual = fixture.get_or_create_topic(topic_name).await.unwrap();
        assert_eq!(actual.name(), topic_name);

        // Should return existing topic
        let actual = fixture.get_or_create_topic(topic_name).await.unwrap();
        assert_eq!(actual.name(), topic_name);
    }

    #[tokio::test]
    async fn test_topic_manager_update_topic() {
        let fixture = TopicManager::new();
        let topic_name = "test.topic";

        fixture.create_topic(topic_name).await.unwrap();

        let mut new_config = TopicConfig::new(topic_name);
        new_config.persistent = false;
        new_config.max_messages = Some(500);

        let actual = fixture.update_topic(topic_name, new_config).await.unwrap();
        assert!(!actual.is_persistent());
        assert_eq!(actual.max_messages(), Some(500));

        // Should fail for non-existent topic
        let result = fixture
            .update_topic("non.existent", TopicConfig::new("test"))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_topic_manager_delete_topic() {
        let fixture = TopicManager::new();
        let topic_name = "test.topic";

        // Should return false for non-existent topic
        let actual = fixture.delete_topic(topic_name).await.unwrap();
        assert!(!actual);

        // Create and delete topic
        fixture.create_topic(topic_name).await.unwrap();
        let actual = fixture.delete_topic(topic_name).await.unwrap();
        assert!(actual);

        // Should be gone now
        let actual = fixture.get_topic(topic_name).await.unwrap();
        assert!(actual.is_none());
    }

    #[tokio::test]
    async fn test_topic_manager_list_topics() {
        let fixture = TopicManager::new();

        // Empty initially
        let actual = fixture.list_topics().await.unwrap();
        assert!(actual.is_empty());

        // Create some topics
        fixture.create_topic("topic1").await.unwrap();
        fixture.create_topic("topic2").await.unwrap();
        fixture.create_topic("topic3").await.unwrap();

        let actual = fixture.list_topics().await.unwrap();
        assert_eq!(actual.len(), 3);

        let names: Vec<String> = actual.iter().map(|t| t.name().to_string()).collect();
        assert!(names.contains(&"topic1".to_string()));
        assert!(names.contains(&"topic2".to_string()));
        assert!(names.contains(&"topic3".to_string()));
    }

    #[tokio::test]
    async fn test_topic_manager_topic_exists() {
        let fixture = TopicManager::new();
        let topic_name = "test.topic";

        let actual = fixture.topic_exists(topic_name).await.unwrap();
        assert!(!actual);

        fixture.create_topic(topic_name).await.unwrap();
        let actual = fixture.topic_exists(topic_name).await.unwrap();
        assert!(actual);
    }

    #[tokio::test]
    async fn test_topic_manager_pattern_matching() {
        let fixture = TopicManager::new();

        fixture
            .create_topic("test.function.completed")
            .await
            .unwrap();
        fixture
            .create_topic("test.workflow.completed")
            .await
            .unwrap();
        fixture.create_topic("test.function.failed").await.unwrap();
        fixture
            .create_topic("other.function.completed")
            .await
            .unwrap();

        let pattern = TopicPattern::new("test.*.completed");
        let actual = fixture.get_topics_matching_pattern(&pattern).await.unwrap();

        assert_eq!(actual.len(), 2);
        let names: Vec<String> = actual.iter().map(|t| t.name().to_string()).collect();
        assert!(names.contains(&"test.function.completed".to_string()));
        assert!(names.contains(&"test.workflow.completed".to_string()));
    }

    #[tokio::test]
    async fn test_topic_manager_topic_count() {
        let fixture = TopicManager::new();

        let actual = fixture.topic_count().await.unwrap();
        assert_eq!(actual, 0);

        fixture.create_topic("topic1").await.unwrap();
        fixture.create_topic("topic2").await.unwrap();

        let actual = fixture.topic_count().await.unwrap();
        assert_eq!(actual, 2);
    }

    #[tokio::test]
    async fn test_topic_manager_clear_all_topics() {
        let fixture = TopicManager::new();

        fixture.create_topic("topic1").await.unwrap();
        fixture.create_topic("topic2").await.unwrap();
        fixture.create_topic("topic3").await.unwrap();

        let actual = fixture.clear_all_topics().await.unwrap();
        assert_eq!(actual, 3);

        let actual = fixture.topic_count().await.unwrap();
        assert_eq!(actual, 0);
    }
}
